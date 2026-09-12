use std::borrow::Cow;

use crossterm::terminal;
use unicode_width::UnicodeWidthChar;

use super::TxtView;

pub(super) struct ScrollGeometry {
    pub column: u16,
    pub top: usize,
    pub size: usize,
    pub visible: usize,
}

impl TxtView {
    pub(super) fn term_size() -> (u16, u16) {
        terminal::size().unwrap_or((80, 24))
    }

    fn resolved_width(&self) -> usize {
        match self.config.viewport_width {
            Some(w) => usize::from(w),
            None => usize::from(Self::term_size().0),
        }
    }

    pub(super) fn resolved_height(&self) -> u16 {
        let rows = Self::term_size().1;
        match self.config.viewport_height {
            Some(h) => h.min(rows),
            None => rows,
        }
    }

    pub(super) fn help_wrap_cols(&self) -> usize {
        self.resolved_width().max(1)
    }

    pub(super) fn help_text() -> String {
        "q: quit | ↑/↓, j/k, Mouse: scroll | PgUp/PgDn: page | Home/End, g/G: start/end".to_string()
    }

    fn help_height(&self) -> usize {
        if !self.config.show_help_bar {
            return 0;
        }
        let total = usize::from(self.resolved_height());
        if total < 2 {
            return 0;
        }
        let lines = Self::help_text()
            .chars()
            .count()
            .div_ceil(self.help_wrap_cols())
            .max(1)
            .min(total - 2);
        1 + lines
    }

    pub(super) fn visible_rows(&self) -> u16 {
        self.resolved_height()
            .saturating_sub(u16::try_from(self.help_height()).unwrap_or(u16::MAX))
            .max(1)
    }

    fn layout_cols(&self) -> usize {
        self.resolved_width()
    }

    fn layout_geometry(&self) -> (usize, usize) {
        let cols = self.layout_cols().max(1);
        let scrollbar_width = if self.config.show_scrollbar { 1 } else { 0 };
        let prefix_width = if self.config.show_line_numbers {
            self.lines.len().to_string().len() + 3
        } else {
            0
        };
        let avail = cols
            .saturating_sub(prefix_width)
            .saturating_sub(scrollbar_width)
            .max(1);
        (avail, prefix_width)
    }

    fn rebuild_display(&mut self) {
        #[cfg(test)]
        {
            self.rebuild_count += 1;
        }
        let (avail, prefix_width) = self.layout_geometry();
        self.display_geometry = Some((avail, prefix_width));

        let mut display = Vec::new();

        for (i, line) in self.lines.iter().enumerate() {
            if line.is_empty() {
                display.push(self.line_prefix(i, 0));
            } else {
                let chunks = wrap_line_ansi(line, avail, prefix_width);
                for (ci, chunk) in chunks.iter().enumerate() {
                    let mut row = self.line_prefix(i, ci);
                    row.push_str(chunk);
                    display.push(row);
                }
            }
        }

        self.display = display;
    }

    fn line_prefix(&self, line_index: usize, chunk_index: usize) -> String {
        if !self.config.show_line_numbers {
            return String::new();
        }
        let width = self.lines.len().to_string().len();
        let label = if chunk_index == 0 {
            (line_index + 1).to_string()
        } else {
            "↳".to_string()
        };
        format!("{:>width$} │ ", label, width = width)
    }

    pub(super) fn refresh_bounds(&mut self) {
        if self.display_geometry != Some(self.layout_geometry()) {
            self.rebuild_display();
        }
        let vr = usize::from(self.visible_rows());
        self.max_offset = self.display.len().saturating_sub(vr);
        self.clamp_offset();
    }

    fn clamp_offset(&mut self) {
        if self.offset > self.max_offset {
            self.offset = self.max_offset;
        }
    }

    pub(super) fn scroll_geometry(&self, visible: usize) -> Option<ScrollGeometry> {
        if !self.config.show_scrollbar || self.max_offset == 0 || visible == 0 {
            return None;
        }
        let total = self.display.len().max(1);
        let thumb = (visible * visible / total).max(1).min(visible);
        let top = (self.offset * (visible - thumb) / self.max_offset).min(visible - thumb);
        Some(ScrollGeometry {
            column: u16::try_from(self.help_wrap_cols().saturating_sub(1)).unwrap_or(u16::MAX),
            top,
            size: thumb,
            visible,
        })
    }

    /// Map a thumb-top row (0..visible) to a scroll offset using the same
    /// proportion that [`TxtView::scroll_geometry`] uses in reverse.
    pub(super) fn offset_from_thumb_top(&self, top: i64, g: &ScrollGeometry) -> usize {
        let travel = g.visible.saturating_sub(g.size).max(1);
        let clamped = usize::try_from(top).unwrap_or(0).min(travel);
        clamped.saturating_mul(self.max_offset) / travel
    }
}

fn wrap_line_ansi(line: &str, width: usize, start_col: usize) -> Vec<String> {
    const TAB_WIDTH: usize = 8;
    let width = width.max(1);
    let bytes = line.as_bytes();
    let mut chunks = Vec::new();
    let mut current_chunk = String::new();
    let mut visible_width = 0;
    let mut ansi_state = String::new();

    let mut i = 0;
    while i < bytes.len() {
        let (piece, piece_width, next): (Cow<'_, str>, usize, usize) = if bytes[i] == 0x1b {
            let (end, kind) = parse_escape(bytes, i);
            let seq = &line[i..end];
            match kind {
                Esc::Sgr => {
                    // SGR passes through raw so styling works, and is tracked
                    // so it can be re-applied after a wrap.
                    if seq == "\x1b[0m" {
                        ansi_state.clear();
                    } else {
                        ansi_state.push_str(seq);
                    }
                    (Cow::Borrowed(seq), 0, end)
                }
                Esc::Osc8 => {
                    // OSC8 hyperlinks pass through raw so they render as real
                    // links, emitted as one atomic unit (opener, payload and
                    // terminator together) so a wrap can never split them.
                    // OSC bytes take zero terminal columns, so no width.
                    (Cow::Borrowed(seq), 0, end)
                }
                Esc::Visible => {
                    // Any other escape (OSC titles, cursor moves, clears, CSI,
                    // 2-byte) is shown as visible caret notation, like `less`.
                    // It is emitted as one atomic unit: never executed, never
                    // split across a wrap boundary.
                    let display = escape_display(seq);
                    let width = display
                        .chars()
                        .map(|c| c.width().unwrap_or(1))
                        .sum::<usize>();
                    (Cow::Owned(display), width, end)
                }
            }
        } else {
            let ch_start = i;
            i += 1;
            while i < bytes.len() && (bytes[i] & 0xC0) == 0x80 {
                i += 1;
            }
            let ch = &line[ch_start..i];
            let c = ch.chars().next().unwrap_or('\u{fffd}');
            // Map the character to the text placed in the chunk, neutralizing
            // control bytes so they cannot corrupt the terminal:
            // - tab keeps its literal byte (wrapped at its 8-column stop),
            // - C0 controls and DEL become visible caret notation.
            let replacement: Cow<'_, str> = match c {
                '\t' => Cow::Borrowed(ch),
                c if c.is_control() => Cow::Owned(caret_notation(c)),
                _ => Cow::Borrowed(ch),
            };
            let ch_width = match c {
                '\t' => TAB_WIDTH - (start_col + visible_width) % TAB_WIDTH,
                c if c.is_control() => {
                    if caret_width(c) {
                        2
                    } else {
                        1
                    }
                }
                _ => c.width().unwrap_or(1),
            };
            (replacement, ch_width, i)
        };
        i = next;
        if visible_width + piece_width > width && !current_chunk.is_empty() {
            if !ansi_state.is_empty() {
                current_chunk.push_str("\x1b[0m");
            }
            chunks.push(current_chunk);
            current_chunk = String::new();
            visible_width = 0;
            if !ansi_state.is_empty() {
                current_chunk.push_str(&ansi_state);
            }
        }
        current_chunk.push_str(&piece);
        visible_width += piece_width;
    }

    if !current_chunk.is_empty() {
        if !ansi_state.is_empty() {
            current_chunk.push_str("\x1b[0m");
        }
        chunks.push(current_chunk);
    }

    if chunks.is_empty() {
        chunks.push(String::new());
    }

    chunks
}

/// Whether `c` is rendered as 2-column caret notation (`^X` / `^?`).
///
/// C0 controls (except tab and `ESC`, which are handled elsewhere) and DEL are
/// escaped so they never reach the terminal as live control bytes.
fn caret_width(c: char) -> bool {
    let code = c as u32;
    code <= 0x1f || code == 0x7f
}

/// Render a control character as its visible caret-notation equivalent.
///
/// C0 controls map to `^@`..=`^_` (e.g. BEL → `^G`, backspace → `^H`,
/// CR → `^M`); DEL maps to `^?`. C1 controls are returned unchanged.
fn caret_notation(c: char) -> String {
    let code = c as u32;
    if code <= 0x1f {
        let letter = char::from_u32(code + 0x40).unwrap_or('?');
        format!("^{letter}")
    } else if code == 0x7f {
        "^?".to_string()
    } else {
        c.to_string()
    }
}

/// Classification of an escape sequence for [`wrap_line_ansi`].
enum Esc {
    /// SGR (`ESC [ ... m`), passes through raw for styling.
    Sgr,
    /// OSC8 hyperlink with a terminator (`ESC ] 8 ; ... BEL`/ST), passes
    /// through raw so links stay live, kept as one atomic unit.
    Osc8,
    /// Anything else rendered as visible caret text.
    Visible,
}

/// Parse an escape sequence starting at the `ESC` byte `start`.
///
/// Returns the byte index just past the sequence and its classification. CSI
/// consumes `ESC [` plus parameter / intermediate bytes and one final byte.
/// OSC (`ESC ]`) consumes until a TERM (`BEL` or ST `ESC \`), everything else
/// is a two-byte escape `ESC <byte>`. A lone `ESC` at the end of input is left
/// unconsumed after itself.
fn parse_escape(bytes: &[u8], start: usize) -> (usize, Esc) {
    let mut i = start + 1;
    if i >= bytes.len() {
        return (i, Esc::Visible);
    }
    match bytes[i] {
        b'[' => {
            i += 1;
            while i < bytes.len() && (0x20..=0x3f).contains(&bytes[i]) {
                i += 1;
            }
            let is_sgr = i < bytes.len() && bytes[i] == b'm';
            if i < bytes.len() {
                i += 1;
            }
            (i, if is_sgr { Esc::Sgr } else { Esc::Visible })
        }
        b']' => {
            i += 1;
            let osc8 = i + 1 < bytes.len() && bytes[i] == b'8' && bytes[i + 1] == b';';
            let mut terminated = false;
            while i < bytes.len() && bytes[i] != 0x07 && bytes[i] != 0x1b {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == 0x1b {
                i += 1;
                if i < bytes.len() && bytes[i] == b'\\' {
                    i += 1;
                    terminated = true;
                }
            } else if i < bytes.len() && bytes[i] == 0x07 {
                i += 1;
                terminated = true;
            }
            let kind = if osc8 && terminated {
                Esc::Osc8
            } else {
                Esc::Visible
            };
            (i, kind)
        }
        b if (0x20..=0x7e).contains(&b) => (i + 1, Esc::Visible),
        _ => (i, Esc::Visible),
    }
}

/// Render a non-SGR escape sequence as visible text, like `less` does:
/// `ESC` → `^[`, other control bytes → caret notation, everything else as-is.
fn escape_display(seq: &str) -> String {
    let mut out = String::new();
    for c in seq.chars() {
        if c == '\u{1b}' {
            out.push_str("^[");
        } else if caret_width(c) {
            out.push_str(&caret_notation(c));
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TxtViewConfig;

    fn viewer(input: &str, width: u16) -> TxtView {
        let config = TxtViewConfig {
            show_help_bar: false,
            show_scrollbar: false,
            viewport_width: Some(width),
            viewport_height: Some(10),
            ..TxtViewConfig::default()
        };
        TxtView::new(input).with_config(config)
    }

    const LINE_NUMBERS: TxtViewConfig = TxtViewConfig {
        show_help_bar: false,
        show_scrollbar: false,
        show_line_numbers: true,
        viewport_width: Some(8),
        viewport_height: Some(10),
    };

    #[test]
    fn ansi_single_style_fits_one_row() {
        let line = "\x1b[31mhello\x1b[0m";
        let chunks = wrap_line_ansi(line, 10, 0);
        assert_eq!(chunks, vec!["\x1b[31mhello\x1b[0m"]);
    }

    #[test]
    fn ansi_wraps_without_splitting_codes() {
        let line = "\x1b[31m12345\x1b[0m";
        let chunks = wrap_line_ansi(line, 3, 0);
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].contains("\x1b[31m"));
        assert!(chunks[0].contains("123"));
        assert!(chunks[0].ends_with("\x1b[0m"));
        assert!(chunks[1].contains("\x1b[31m"));
        assert!(chunks[1].contains("45"));
        assert!(chunks[1].ends_with("\x1b[0m"));
    }

    #[test]
    fn ansi_state_cleared_by_reset() {
        let line = "\x1b[31mabc\x1b[0mdefghi";
        let chunks = wrap_line_ansi(line, 3, 0);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], "\x1b[31mabc\x1b[0m");
        assert_eq!(chunks[1], "def");
        assert_eq!(chunks[2], "ghi");
    }

    #[test]
    fn plain_text_unchanged() {
        let line = "hello world";
        let chunks = wrap_line_ansi(line, 5, 0);
        assert_eq!(chunks, vec!["hello", " worl", "d"]);
    }

    #[test]
    fn tab_advances_to_next_stop() {
        let chunks = wrap_line_ansi("a\tb", 8, 0);
        assert_eq!(chunks, vec!["a\t", "b"]);
    }

    #[test]
    fn tab_overshoots_own_row_when_it_does_not_fit() {
        let chunks = wrap_line_ansi("a\tb", 5, 0);
        assert_eq!(chunks, vec!["a", "\t", "b"]);
    }

    #[test]
    fn tab_at_column_zero_advances_full_width() {
        let chunks = wrap_line_ansi("\t\tx", 8, 0);
        assert_eq!(chunks, vec!["\t", "\t", "x"]);
    }

    #[test]
    fn tab_advance_accounts_for_prefix_column() {
        assert_eq!(wrap_line_ansi("\tX", 5, 0), vec!["\t", "X"]);
        assert_eq!(wrap_line_ansi("\tX", 5, 5), vec!["\tX"]);
    }

    #[test]
    fn tab_wrapped_line_renders_correctly() {
        let v = viewer("a\tb", 8);
        assert_eq!(v.display, vec!["a\t", "b"]);
    }

    #[test]
    fn mid_tab_stop_wraps_tab_followed_by_text() {
        let chunks = wrap_line_ansi("abcdefgh\tx", 12, 0);
        assert_eq!(chunks, vec!["abcdefgh", "\tx"]);
    }

    #[test]
    fn tab_mid_line_wraps_after_the_stop() {
        assert_eq!(wrap_line_ansi("abc\tdef", 8, 0), vec!["abc\t", "def"]);
        assert_eq!(wrap_line_ansi("ab\tcde", 8, 0), vec!["ab\t", "cde"]);
    }

    #[test]
    fn tab_at_line_end_and_following_wrap() {
        assert_eq!(wrap_line_ansi("ab\tcd", 4, 0), vec!["ab", "\t", "cd"]);
    }

    #[test]
    fn tab_survives_ansi_prefix_replay_on_next_row() {
        let chunks = wrap_line_ansi("\x1b[31mabcd\tef\x1b[0m", 8, 0);
        assert_eq!(chunks, vec!["\x1b[31mabcd\t\x1b[0m", "\x1b[31mef\x1b[0m"]);
    }

    #[test]
    fn tab_after_wide_char_counts_remaining_stop() {
        assert_eq!(wrap_line_ansi("中\tx", 10, 0), vec!["中\tx"]);
        assert_eq!(wrap_line_ansi("中\tx", 8, 0), vec!["中\t", "x"]);
    }

    #[test]
    fn control_chars_become_caret_notation() {
        let chunks = wrap_line_ansi("a\x07b\x08c\x7fd", 20, 0);
        assert_eq!(chunks, vec!["a^Gb^Hc^?d"]);
    }

    #[test]
    fn caret_notation_counts_two_columns() {
        assert_eq!(wrap_line_ansi("ab\x07cd", 4, 0), vec!["ab^G", "cd"]);
    }

    #[test]
    fn caret_notation_carries_ansi_state_across_wrap() {
        let chunks = wrap_line_ansi("\x1b[31mab\x07cd\x1b[0m", 4, 0);
        assert_eq!(chunks, vec!["\x1b[31mab^G\x1b[0m", "\x1b[31mcd\x1b[0m"]);
    }

    #[test]
    fn sgr_passes_through_lone_control_becomes_caret() {
        let chunks = wrap_line_ansi("\x1b[1mhi\x07\x1b[0m", 20, 0);
        assert_eq!(chunks, vec!["\x1b[1mhi^G\x1b[0m"]);
    }

    #[test]
    fn non_sgr_csi_is_visible_not_executed() {
        assert_eq!(wrap_line_ansi("\x1b[2Aab", 20, 0), vec!["^[[2Aab"]);
        assert_eq!(wrap_line_ansi("\x1b[2Jx", 20, 0), vec!["^[[2Jx"]);
    }

    #[test]
    fn csi_with_non_alpha_final_byte_is_visible() {
        assert_eq!(wrap_line_ansi("\x1b[2~ab", 20, 0), vec!["^[[2~ab"]);
    }

    #[test]
    fn osc8_hyperlink_passes_through_whole() {
        let chunks = wrap_line_ansi("\x1b]8;;https://x.dev\x07here", 8, 0);
        assert_eq!(chunks, vec!["\x1b]8;;https://x.dev\x07here"]);
    }

    #[test]
    fn osc8_never_split_across_wrap() {
        let chunks = wrap_line_ansi("abcde\x1b]8;;u\x07fghij", 5, 0);
        assert_eq!(chunks, vec!["abcde\x1b]8;;u\x07", "fghij"]);
    }

    #[test]
    fn osc8_kept_with_styling_across_wrap() {
        let chunks = wrap_line_ansi("\x1b[31mabc\x1b]8;;u\x07defgh", 5, 0);
        assert_eq!(
            chunks,
            vec!["\x1b[31mabc\x1b]8;;u\x07de\x1b[0m", "\x1b[31mfgh\x1b[0m"]
        );
    }

    #[test]
    fn osc8_hyperlink_reset_link_passes_through() {
        let chunks = wrap_line_ansi("\x1b]8;;u\x07go\x1b]8;;\x07", 40, 0);
        assert_eq!(chunks, vec!["\x1b]8;;u\x07go\x1b]8;;\x07"]);
    }

    #[test]
    fn unterminated_osc8_is_visible_not_raw() {
        let chunks = wrap_line_ansi("\x1b]8;;https://x.dev", 40, 0);
        assert_eq!(chunks, vec!["^[]8;;https://x.dev"]);
    }

    #[test]
    fn non_osc8_osc_stays_visible() {
        let chunks = wrap_line_ansi("\x1b]0;longtitle\x07body", 8, 0);
        assert_eq!(chunks, vec!["^[]0;longtitle^G", "body"]);
    }

    #[test]
    fn osc_terminated_by_st_is_visible() {
        assert_eq!(
            wrap_line_ansi("\x1b]0;hi\x1b\\x", 20, 0),
            vec!["^[]0;hi^[\\x"]
        );
    }

    #[test]
    fn two_byte_and_lone_escapes_are_visible() {
        assert_eq!(wrap_line_ansi("\x1b7abc", 20, 0), vec!["^[7abc"]);
        assert_eq!(wrap_line_ansi("\x1bXab", 20, 0), vec!["^[Xab"]);
        assert_eq!(wrap_line_ansi("\x1b", 20, 0), vec!["^["]);
    }

    #[test]
    fn empty_line_returns_empty_string() {
        let chunks = wrap_line_ansi("", 10, 0);
        assert_eq!(chunks, vec![""]);
    }

    #[test]
    fn cjk_chars_wrap_by_visual_width() {
        let chunks = wrap_line_ansi("一二三四五六七八九十", 10, 0);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], "一二三四五");
        assert_eq!(chunks[1], "六七八九十");
    }

    #[test]
    fn japanese_chars_wrap_by_visual_width() {
        let chunks = wrap_line_ansi("ひらがなカタカナ", 8, 0);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], "ひらがな");
        assert_eq!(chunks[1], "カタカナ");
    }

    #[test]
    fn korean_chars_wrap_by_visual_width() {
        let chunks = wrap_line_ansi("한국어테스트", 8, 0);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], "한국어테");
        assert_eq!(chunks[1], "스트");
    }

    #[test]
    fn mixed_ascii_cjk_wrap_correctly() {
        let chunks = wrap_line_ansi("A中B日C", 5, 0);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], "A中B");
        assert_eq!(chunks[1], "日C");
    }

    #[test]
    fn mixed_ascii_japanese_wrap_correctly() {
        let chunks = wrap_line_ansi("A日本語B", 4, 0);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], "A日");
        assert_eq!(chunks[1], "本語");
        assert_eq!(chunks[2], "B");
    }

    #[test]
    fn display_single_row_per_line_when_fits() {
        let v = viewer("abc\ndef\nghi", 80);
        assert_eq!(v.display, vec!["abc", "def", "ghi"]);
    }

    #[test]
    fn display_wraps_long_lines() {
        let v = viewer("abcdefghij", 4);
        assert_eq!(v.display, vec!["abcd", "efgh", "ij"]);
    }

    #[test]
    fn display_preserves_empty_lines() {
        let v = viewer("a\n\nb", 10);
        assert_eq!(v.display, vec!["a", "", "b"]);
    }

    #[test]
    fn display_adds_line_number_prefix() {
        let v = TxtView::new("abc\ndef").with_config(LINE_NUMBERS);
        assert_eq!(v.display, vec!["1 │ abc", "2 │ def"]);
    }

    #[test]
    fn display_carries_prefix_to_wrapped_rows() {
        let v = TxtView::new("abcdefghijk").with_config(LINE_NUMBERS);
        assert_eq!(v.display, vec!["1 │ abcd", "↳ │ efgh", "↳ │ ijk"]);
    }

    #[test]
    fn display_reserves_scrollbar_column() {
        let config = TxtViewConfig {
            show_help_bar: false,
            show_scrollbar: true,
            viewport_width: Some(4),
            viewport_height: Some(10),
            ..TxtViewConfig::default()
        };
        let v = TxtView::new("abcdef").with_config(config);
        assert_eq!(v.display, vec!["abc", "def"]);
    }

    #[test]
    fn offset_only_scroll_skips_display_rebuild() {
        let input = (0..100)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut v = viewer(&input, 10);
        let built = v.rebuild_count;
        v.scroll_down(50);
        v.refresh_bounds();
        assert_eq!(
            v.rebuild_count, built,
            "scroll re-wrapped the whole document"
        );
        assert_eq!(v.offset, 50);
    }

    #[test]
    fn visible_rows_keeps_one_row_on_narrow_width() {
        let config = TxtViewConfig {
            show_help_bar: true,
            show_scrollbar: false,
            viewport_width: Some(1),
            viewport_height: Some(24),
            ..TxtViewConfig::default()
        };
        let v = TxtView::new("hello\nworld").with_config(config);
        assert_eq!(v.visible_rows(), 1);
    }

    #[test]
    fn visible_rows_keeps_one_row_on_tiny_terminal() {
        let config = TxtViewConfig {
            show_help_bar: true,
            show_scrollbar: false,
            viewport_width: Some(80),
            viewport_height: Some(2),
            ..TxtViewConfig::default()
        };
        let v = TxtView::new("hello\nworld").with_config(config);
        assert_eq!(v.visible_rows(), 1);
    }

    #[test]
    fn visible_rows_unchanged_when_help_fits() {
        let config = TxtViewConfig {
            show_help_bar: true,
            show_scrollbar: false,
            viewport_width: Some(80),
            viewport_height: Some(10),
            ..TxtViewConfig::default()
        };
        let v = TxtView::new("hello\nworld").with_config(config);
        let help = 1 + TxtView::help_text().chars().count().div_ceil(80).max(1);
        assert_eq!(usize::from(v.visible_rows()), 10 - help);
    }

    #[test]
    fn width_change_rebuilds_display() {
        let config = |w| TxtViewConfig {
            show_help_bar: false,
            show_scrollbar: false,
            viewport_width: Some(w),
            viewport_height: Some(10),
            ..TxtViewConfig::default()
        };
        let mut v = TxtView::new("abcdefghij").with_config(config(10));
        let n = v.rebuild_count;
        v = v.with_config(config(5));
        assert!(v.rebuild_count > n, "width change must re-wrap");
    }

    #[test]
    fn height_change_reuses_display() {
        let config = |h| TxtViewConfig {
            show_help_bar: false,
            viewport_width: Some(80),
            viewport_height: Some(h),
            ..TxtViewConfig::default()
        };
        let input = (0..100)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut v = TxtView::new(input).with_config(config(10));
        let n = v.rebuild_count;
        let m0 = v.max_offset;
        v = v.with_config(config(20));
        assert_eq!(v.rebuild_count, n, "height change must not re-wrap");
        assert!(v.max_offset < m0);
    }

    #[test]
    fn display_counts_wide_chars_by_visual_width() {
        let v = viewer("ひらがなカタカナ", 8);
        assert_eq!(v.display, vec!["ひらがな", "カタカナ"]);
    }

    #[test]
    fn display_keeps_ansi_codes_intact() {
        let v = viewer("\x1b[31m12345\x1b[0m", 3);
        assert_eq!(v.display, vec!["\x1b[31m123\x1b[0m", "\x1b[31m45\x1b[0m"]);
    }
}
