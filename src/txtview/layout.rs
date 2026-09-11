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

    fn resolved_height(&self) -> u16 {
        match self.config.viewport_height {
            Some(h) => h,
            None => Self::term_size().1,
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
        let lines = Self::help_text()
            .chars()
            .count()
            .div_ceil(self.help_wrap_cols())
            .max(1);
        1 + lines
    }

    pub(super) fn visible_rows(&self) -> u16 {
        self.resolved_height()
            .saturating_sub(u16::try_from(self.help_height()).unwrap_or(u16::MAX))
    }

    fn layout_cols(&self) -> usize {
        self.resolved_width()
    }

    fn rebuild_display(&mut self) {
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
        self.rebuild_display();
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
        if bytes[i] == 0x1b {
            let seq_start = i;
            i += 1;
            if i < bytes.len() && bytes[i] == b'[' {
                i += 1;
                while i < bytes.len() && !bytes[i].is_ascii_alphabetic() {
                    i += 1;
                }
                if i < bytes.len() {
                    i += 1;
                }
            }
            let seq = &line[seq_start..i];
            current_chunk.push_str(seq);
            if seq.ends_with('m') {
                if seq == "\x1b[0m" {
                    ansi_state.clear();
                } else {
                    ansi_state.push_str(seq);
                }
            }
        } else {
            let ch_start = i;
            i += 1;
            while i < bytes.len() && (bytes[i] & 0xC0) == 0x80 {
                i += 1;
            }
            let ch = &line[ch_start..i];
            let ch_width = match ch.chars().next() {
                Some('\t') => TAB_WIDTH - (start_col + visible_width) % TAB_WIDTH,
                Some(c) => c.width().unwrap_or(1),
                None => 1,
            };
            if visible_width + ch_width > width && !current_chunk.is_empty() {
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
            current_chunk.push_str(ch);
            visible_width += ch_width;
        }
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
