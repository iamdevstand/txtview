//! Line wrapping that keeps ANSI styling intact while neutralizing control
//! bytes and non-SGR escape sequences.

use std::borrow::Cow;

use unicode_segmentation::UnicodeSegmentation;

use super::ansi::{Esc, caret_notation, display_width, escape_display, parse_escape};
use super::sgr::SgrState;

/// Columns between tab stops.
const TAB_WIDTH: usize = 8;

/// The terminal columns a grapheme cluster starting at column `col` occupies,
/// measured the way the wrapper lays rows out: a tab advances to its next
/// 8-column stop, a control character takes its caret notation width, and
/// anything else keeps its [`cluster_visual_width`]. Both the wrapper and
/// [`write_visible_row`](super::write_visible_row) measure rows through this
/// one function.
pub(super) fn cluster_width_at(cluster: &str, col: usize) -> usize {
    let c = cluster.chars().next().unwrap_or('\u{fffd}');
    match c {
        '\t' => TAB_WIDTH - col % TAB_WIDTH,
        c if c.is_control() => 2,
        _ => cluster_visual_width(cluster),
    }
}

/// Measure the next wrapped piece of `line` starting at byte `i`, which is
/// the first piece at terminal column `col` of its (possibly fresh) row.
///
/// Returns the text to place (SGR and OSC8 sequences raw, every other escape
/// and control char as visible caret notation), the terminal columns it
/// occupies, whether it is a tab, the byte offset of the next piece and the
/// SGR sequence itself when the piece is one, so callers that track
/// replayable style can fold it into their live state.
fn measure_piece<'a>(
    line: &'a str,
    bytes: &'a [u8],
    i: usize,
    col: usize,
) -> (Cow<'a, str>, usize, bool, usize, Option<&'a str>) {
    if bytes[i] == 0x1b {
        let (end, kind) = parse_escape(bytes, i);
        let seq = &line[i..end];
        match kind {
            Esc::Sgr => (Cow::Borrowed(seq), 0, false, end, Some(seq)),
            Esc::Osc8 => {
                // OSC8 hyperlinks pass through raw so they render as real
                // links, emitted as one atomic unit (opener, payload and
                // terminator together) so a wrap can never split them. OSC
                // bytes take zero terminal columns, so no width
                (Cow::Borrowed(seq), 0, false, end, None)
            }
            Esc::Visible => {
                // Any other escape (OSC titles, cursor moves, clears, CSI,
                // 2-byte) is shown as visible caret notation, like `less`.
                // It is emitted as one atomic unit: never executed, never
                // split across a wrap boundary
                let display = escape_display(seq);
                let width = display_width(&display);
                (Cow::Owned(display), width, false, end, None)
            }
        }
    } else {
        // Process a full extended grapheme cluster together so a wrap can
        // never split a base char from its combining marks, skin-tone
        // modifiers, ZWJ sequences or flag pairs
        let cluster = &line[i..].graphemes(true).next().unwrap_or_default();
        let is_tab = *cluster == "\t";
        let end = i + cluster.len();
        let c = cluster.chars().next().unwrap_or('\u{fffd}');
        // Map the character to the text placed in the chunk, neutralizing
        // control bytes so they cannot corrupt the terminal:
        // - tab keeps its literal byte in the display row (wrapped at
        //   its 8-column stop, the write layer expands it to spaces),
        // - C0/C1 controls and DEL become visible caret notation.
        let replacement: Cow<'_, str> = match c {
            '\t' => Cow::Borrowed(cluster),
            c if c.is_control() => Cow::Owned(caret_notation(c)),
            _ => Cow::Borrowed(cluster),
        };
        let ch_width = cluster_width_at(cluster, col);
        (replacement, ch_width, is_tab, end, None)
    }
}

/// A resumable scan of one line's wrapped display rows.
///
/// [`next`](WrappedLine::next) materializes one display row at a time into a
/// caller-provided buffer and [`skip_to`](WrappedLine::skip_to) advances past
/// rows without building them, so the layout can produce exactly the one row
/// it paints instead of the line's whole `Vec`. [`wrap_line_ansi`] and
/// [`wrap_line_ansi_count`] are thin loops over this same scan, so the row
/// boundaries, the tab-overflow substitution and the style replay live in one
/// place and the count can never drift from the build. A row this scan
/// produces is byte-identical to the matching entry [`wrap_line_ansi`]
/// builds.
pub(crate) struct WrappedLine<'a> {
    line: &'a str,
    bytes: &'a [u8],
    width: usize,
    start_col: usize,
    /// Byte offset of the next piece to measure.
    i: usize,
    /// The style folded through every SGR processed so far, replayed as the
    /// next row's opening prefix.
    style: SgrState,
    /// The piece that overflowed the previous row, placed at the top of the
    /// fresh row it started. A wrapped tab's width is already re-measured
    /// from the fresh row's first column.
    pending: Option<(Cow<'a, str>, usize, bool)>,
    /// Every piece is consumed and the last row emitted.
    exhausted: bool,
}

impl<'a> WrappedLine<'a> {
    pub(crate) fn new(line: &'a str, width: usize, start_col: usize) -> Self {
        WrappedLine {
            line,
            bytes: line.as_bytes(),
            width: width.max(1),
            start_col,
            i: 0,
            style: SgrState::default(),
            pending: None,
            exhausted: false,
        }
    }

    /// Replace `out`'s contents with the next display row and return `true`,
    /// `false` once the line is exhausted, after which the scan emits nothing
    /// more and `out` is left unchanged.
    pub(crate) fn next(&mut self, out: &mut String) -> bool {
        self.next_row(Some(out))
    }

    /// Advance past `target` display rows without building them, returning
    /// how many rows were skipped. Style keeps being folded and wrapped tabs
    /// stay re-measured while skipping, so a row read afterwards replays and
    /// measures exactly as if the skipped rows had been built.
    pub(crate) fn skip_to(&mut self, target: usize) -> usize {
        let mut skipped = 0;
        while skipped < target && self.skip_one() {
            skipped += 1;
        }
        skipped
    }

    /// Consume the remaining rows, counting them without building them.
    fn count_rows(&mut self) -> usize {
        let mut rows = 0;
        while self.skip_one() {
            rows += 1;
        }
        rows
    }

    /// Skip the next display row without producing it, `false` at the end.
    fn skip_one(&mut self) -> bool {
        self.next_row(None)
    }

    /// Produce exactly one display row: written to `out` when present,
    /// otherwise consumed without output. Both paths run the same scan with
    /// the same boundary decision, tab-overflow substitution, style folding
    /// and re-measurement, so a skipped row costs what measuring a row
    /// costs, never what building one does.
    fn next_row(&mut self, mut out: Option<&mut String>) -> bool {
        if self.exhausted {
            return false;
        }
        if let Some(out) = &mut out {
            out.clear();
        }
        let mut row_started = false;
        let mut visible_width = 0;
        // A fresh row opens with its folded style replayed, exactly as the
        // builder seeds a chunk at a wrap boundary. The first row gets no
        // prefix because the scan starts in the default style.
        if self.style.is_active() {
            row_started = true;
            if let Some(out) = &mut out {
                self.style.write_replay(out);
            }
        }

        loop {
            let (piece, piece_width, is_tab) = match self.pending.take() {
                // A piece that overflowed the previous row was measured and
                // folded there, on the row it started it is placed without a
                // second boundary decision, exactly like the builder
                Some(pending) => pending,
                None => {
                    if self.i >= self.bytes.len() {
                        // End of the line: a row that holds anything is
                        // flushed with its closing reset. One that holds
                        // nothing ends the scan
                        if !row_started {
                            self.exhausted = true;
                            return false;
                        }
                        if self.style.is_active() {
                            if let Some(out) = &mut out {
                                out.push_str("\x1b[0m");
                            }
                        }
                        self.exhausted = true;
                        return true;
                    }
                    let (piece, piece_width, is_tab, next, sgr) = measure_piece(
                        self.line,
                        self.bytes,
                        self.i,
                        self.start_col + visible_width,
                    );
                    self.i = next;
                    if let Some(seq) = sgr {
                        self.style.apply(seq);
                    }
                    if visible_width + piece_width > self.width && row_started {
                        // The piece overflows the row it landed in: close the
                        // row and start a fresh one with it, re-measuring a
                        // wrapped tab from the fresh row's first column
                        if self.style.is_active() {
                            if let Some(out) = &mut out {
                                out.push_str("\x1b[0m");
                            }
                        }
                        // `visible_width` is 0 on the fresh row, so the tab
                        // re-measure lands on the row's first column stop
                        let width = if is_tab {
                            TAB_WIDTH - self.start_col % TAB_WIDTH
                        } else {
                            piece_width
                        };
                        self.pending = Some((piece, width, is_tab));
                        return true;
                    }
                    (piece, piece_width, is_tab)
                }
            };

            if piece_width > self.width && is_tab {
                // A tab whose stop lies beyond the whole row cannot reach it,
                // so it renders as spaces filling the row and never pushes
                // the cursor past the wrap width. Grapheme clusters stay
                // whole even when wider than the width (they cannot be
                // split), so only tabs get this substitution
                if let Some(out) = &mut out {
                    out.push_str(&" ".repeat(self.width));
                }
                visible_width = self.width;
                row_started = true;
            } else {
                if let Some(out) = &mut out {
                    out.push_str(&piece);
                }
                visible_width += piece_width;
                row_started = true;
            }
        }
    }
}

/// Wrap `line` into visual rows of at most `width` terminal columns.
///
/// `start_col` is the width of any prefix the caller renders before the
/// line, used so tabs land on their column stops and wraps line up under
/// the prefix.
///
/// The wrapping preserves safe content verbatim and neutralizes the rest:
/// - text is measured per grapheme cluster, so combining marks, skin-tone
///   modifiers, ZWJ sequences and flag pairs never split across a wrap,
/// - SGR (`ESC [...]m`) and OSC8 hyperlinks pass through raw (OSC8 emits as
///   one atomic unit so a wrap can never split it),
/// - C0 and C1 controls, DEL, and every other escape become visible caret
///   notation,
/// - tabs stay as literal bytes in the wrapped row, measured at their
///   8-column stop, a tab whose stop lies beyond the wrap width renders as
///   spaces filling the row so the row never exceeds the width. The display
///   rows can therefore contain literal tabs, the write layer expands them
///   to spaces, so a tab byte itself never reaches the terminal
///   ([`write_visible_row`](super::write_visible_row)).
pub(crate) fn wrap_line_ansi(line: &str, width: usize, start_col: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut scan = WrappedLine::new(line, width, start_col);
    let mut row = String::new();
    while scan.next(&mut row) {
        chunks.push(std::mem::take(&mut row));
    }

    if chunks.is_empty() {
        chunks.push(String::new());
    }

    chunks
}

/// Count the wrapped display rows [`wrap_line_ansi`] would produce for `line`
/// at a given `width` and `start_col`, without building them.
///
/// The count is [`WrappedLine`]'s own scan run in build-less mode: the same
/// boundary and tab-overflow decisions as the building path, with the style
/// folded rather than emitted. A fresh-row style prefix is zero width, so
/// counting and building through one state machine can never disagree.
/// Skipping rows costs what measuring them costs, never what building does.
pub(crate) fn wrap_line_ansi_count(line: &str, width: usize, start_col: usize) -> usize {
    // An empty line still produces one blank display row, mirroring the
    // builder's `[String::new()]` fallback
    WrappedLine::new(line, width, start_col).count_rows().max(1)
}

/// Visual width of a grapheme cluster. Emoji-presentation sequences carry the
/// U+FE0F variation selector (e.g. ❤️, ©️) or a U+20E3 keycap (e.g. #️⃣, 1️⃣)
/// for rendering at double width in modern terminals; `unicode-width` keeps
/// the base char's text width, so bump those clusters to two columns.
pub(super) fn cluster_visual_width(cluster: &str) -> usize {
    if cluster.len() == 1 {
        let b = cluster.as_bytes()[0];
        if (0x20..=0x7e).contains(&b) {
            return 1;
        }
    }
    let width = display_width(cluster);
    if cluster.contains('\u{fe0f}') || cluster.contains('\u{20e3}') {
        width.max(2)
    } else {
        width
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csi_ending_in_multibyte_keeps_the_char() {
        assert_eq!(wrap_line_ansi("\x1b[中", 20, 0), vec!["^[[中"]);
        assert_eq!(wrap_line_ansi("\x1b[31中x", 20, 0), vec!["^[[31中x"]);
    }

    #[test]
    fn adversarial_inputs_do_not_panic() {
        let inputs = [
            "\x1b[🎉",
            "\x1b[中x",
            "a\x1b[🎉b",
            "🎉\x1b[🎉",
            "\x1b\x1b[中",
            "\x1b[3;🎉",
            "\x1b[;中",
            "\x1b[中\x1b[0m",
        ];
        for input in inputs {
            let _ = wrap_line_ansi(input, 4, 0);
        }
    }

    #[test]
    fn grapheme_clusters_are_not_split() {
        assert_eq!(
            wrap_line_ansi("🎉🏽👍🇺🇸abc", 4, 0),
            vec!["🎉🏽", "👍🇺🇸", "abc"]
        );
        assert_eq!(wrap_line_ansi("e\u{301}x", 1, 0), vec!["e\u{301}", "x"]);
        assert_eq!(
            wrap_line_ansi("x👨\u{200d}👩\u{200d}👧y", 4, 0),
            vec!["x", "👨\u{200d}👩\u{200d}👧", "y"]
        );
    }

    #[test]
    fn keycap_sequence_counts_two_columns() {
        let chunks = wrap_line_ansi("#\u{fe0f}\u{20e3}x", 2, 0);
        assert_eq!(chunks, vec!["#\u{fe0f}\u{20e3}", "x"]);
    }

    #[test]
    fn vs16_emoji_counts_two_columns() {
        let chunks = wrap_line_ansi("❤\u{fe0f}x", 2, 0);
        assert_eq!(chunks, vec!["❤\u{fe0f}", "x"]);
    }

    #[test]
    fn vs15_emoji_keeps_text_width() {
        let chunks = wrap_line_ansi("❤\u{fe0e}x", 2, 0);
        assert_eq!(chunks, vec!["❤\u{fe0e}x"]);
    }

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
    fn empty_sgr_reset_clears_tracked_state() {
        let chunks = wrap_line_ansi("\x1b[31mab\x1b[mcd\x1b[0m", 3, 0);
        assert_eq!(chunks, vec!["\x1b[31mab\x1b[mc", "d\x1b[0m"]);
    }

    #[test]
    fn attribute_off_removes_attribute_from_replay() {
        let chunks = wrap_line_ansi("\x1b[1mbold\x1b[22mnormal", 4, 0);
        assert_eq!(chunks, vec!["\x1b[1mbold\x1b[22m", "norm", "al"]);
    }

    #[test]
    fn replayed_state_is_canonical_and_compact() {
        let chunks = wrap_line_ansi("\x1b[1m\x1b[31mabcdef\x1b[0m", 3, 0);
        assert_eq!(
            chunks,
            vec!["\x1b[1m\x1b[31mabc\x1b[0m", "\x1b[1;31mdef\x1b[0m"]
        );
    }

    #[test]
    fn extended_color_survives_replay() {
        let chunks = wrap_line_ansi("\x1b[38;5;123mabcdef\x1b[0m", 3, 0);
        assert_eq!(
            chunks,
            vec!["\x1b[38;5;123mabc\x1b[0m", "\x1b[38;5;123mdef\x1b[0m"]
        );
    }

    #[test]
    fn newer_foreground_replaces_older_in_replay() {
        let chunks = wrap_line_ansi("\x1b[31m\x1b[32mabcdef\x1b[0m", 3, 0);
        assert_eq!(
            chunks,
            vec!["\x1b[31m\x1b[32mabc\x1b[0m", "\x1b[32mdef\x1b[0m"]
        );
    }

    #[test]
    fn exclusive_pair_replays_only_the_later_member_across_wrap() {
        assert_eq!(
            wrap_line_ansi("\x1b[21m\x1b[4mabcdef\x1b[0m", 3, 0),
            vec!["\x1b[21m\x1b[4mabc\x1b[0m", "\x1b[4mdef\x1b[0m"],
            "a wrapped row opens single underlined, not double"
        );
        assert_eq!(
            wrap_line_ansi("\x1b[4m\x1b[21mabcdef\x1b[0m", 3, 0),
            vec!["\x1b[4m\x1b[21mabc\x1b[0m", "\x1b[21mdef\x1b[0m"],
            "a wrapped row opens double underlined, not single"
        );
        assert_eq!(
            wrap_line_ansi("\x1b[6m\x1b[5mabcdef\x1b[0m", 3, 0),
            vec!["\x1b[6m\x1b[5mabc\x1b[0m", "\x1b[5mdef\x1b[0m"],
            "a wrapped row opens blinking, not rapid blinking"
        );
        assert_eq!(
            wrap_line_ansi("\x1b[5m\x1b[6mabcdef\x1b[0m", 3, 0),
            vec!["\x1b[5m\x1b[6mabc\x1b[0m", "\x1b[6mdef\x1b[0m"],
            "a wrapped row opens rapid blinking, not blinking"
        );
        assert_eq!(
            wrap_line_ansi("\x1b[52m\x1b[51mabcdef\x1b[0m", 3, 0),
            vec!["\x1b[52m\x1b[51mabc\x1b[0m", "\x1b[51mdef\x1b[0m"],
            "a wrapped row opens framed, not encircled"
        );
        assert_eq!(
            wrap_line_ansi("\x1b[51m\x1b[52mabcdef\x1b[0m", 3, 0),
            vec!["\x1b[51m\x1b[52mabc\x1b[0m", "\x1b[52mdef\x1b[0m"],
            "a wrapped row opens encircled, not framed"
        );
        assert_eq!(
            wrap_line_ansi("\x1b[1m\x1b[21m\x1b[4mabcdef\x1b[0m", 3, 0),
            vec!["\x1b[1m\x1b[21m\x1b[4mabc\x1b[0m", "\x1b[1;4mdef\x1b[0m"],
            "a wrapped row keeps bold alongside the later underline"
        );
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
        assert_eq!(chunks, vec!["a", "     ", "b"]);
    }

    #[test]
    fn tab_at_column_zero_advances_full_width() {
        let chunks = wrap_line_ansi("\t\tx", 8, 0);
        assert_eq!(chunks, vec!["\t", "\t", "x"]);
    }

    #[test]
    fn tab_advance_accounts_for_prefix_column() {
        assert_eq!(wrap_line_ansi("\tX", 5, 0), vec!["     ", "X"]);
        assert_eq!(wrap_line_ansi("\tX", 5, 5), vec!["\tX"]);
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
        assert_eq!(wrap_line_ansi("ab\tcd", 4, 0), vec!["ab", "    ", "cd"]);
    }

    #[test]
    fn styled_tab_on_narrow_viewport_never_exceeds_the_width() {
        let chunks = wrap_line_ansi("\x1b[31ma\tb\x1b[0m", 5, 0);
        assert_eq!(
            chunks,
            vec![
                "\x1b[31ma\x1b[0m",
                "\x1b[31m     \x1b[0m",
                "\x1b[31mb\x1b[0m"
            ]
        );
    }

    #[test]
    fn emoji_wider_than_the_viewport_stays_whole() {
        assert_eq!(
            wrap_line_ansi("🎉", 1, 0),
            vec!["🎉"],
            "a grapheme wider than the viewport is never split or dropped"
        );
        assert_eq!(wrap_line_ansi("🎉\t🎉", 1, 0), vec!["🎉", " ", "🎉"]);
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
    fn c1_controls_become_caret_notation() {
        // U+009B is the 8-bit CSI and U+0085 the 8-bit NEL; fed to a terminal
        // raw they execute, so they must become visible caret notation.
        assert_eq!(wrap_line_ansi("a\u{9b}2Jb\u{85}", 20, 0), vec!["a^[2Jb^E"]);
        assert_eq!(
            wrap_line_ansi("\u{9b}2J", 3, 0),
            vec!["^[2", "J"],
            "8-bit CSI caret notation must count two columns"
        );
        assert_eq!(
            wrap_line_ansi("\u{80}\u{9f}", 20, 0),
            vec!["^@^_"],
            "C1 maps to the same caret range as its C0 equivalent"
        );
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
    fn count_matches_wrap_line_ansi() {
        for line in scan_corpus() {
            for width in 1..=12 {
                for start_col in [0, 2, 4, 7] {
                    let counted = wrap_line_ansi_count(line, width, start_col);
                    let built = wrap_line_ansi(line, width, start_col).len();
                    assert_eq!(
                        counted, built,
                        "count({line:?}, width {width}, start {start_col}) \
                         must equal the built row count ({built})"
                    );
                }
            }
        }
    }

    #[test]
    fn count_treats_purely_styled_input_as_one_row() {
        assert_eq!(wrap_line_ansi_count("\x1b[31m\x1b[1m\x1b[0m", 5, 0), 1);
        assert_eq!(
            wrap_line_ansi_count("\x1b[31m\x1b[1m", 3, 0),
            1,
            "an unterminated style on an empty line still yields one row"
        );
    }

    #[test]
    fn count_anchors_pin_literal_rows() {
        assert_eq!(wrap_line_ansi_count("abc", 3, 0), 1);
        assert_eq!(wrap_line_ansi_count("abcdefghij", 3, 0), 4);
        assert_eq!(
            wrap_line_ansi_count("ab\tcdef", 4, 0),
            3,
            "a tab padding a row to the exact width still counts the wrap"
        );
        assert_eq!(
            wrap_line_ansi_count("\x1b[31m12345\x1b[0m", 3, 0),
            2,
            "a styled row replaying on the next row still counts it"
        );
        assert_eq!(
            wrap_line_ansi_count("\x1b[31mabcd\tef\x1b[0m", 8, 0),
            2,
            "matches the pinned builder output for the same input"
        );
    }

    /// Inputs for the scan checks and the count cross-check: styled and plain
    /// text, tabs, wide graphemes, control bytes and OSC8 hyperlinks, across
    /// a spread of widths and prefixes.
    fn scan_corpus() -> [&'static str; 25] {
        [
            "",
            "abc",
            "hello world",
            "a\tb",
            "\t",
            "\t\tx",
            "abcdefgh\tx",
            "ab\tcd",
            "abcdefghij",
            "\x1b[31mhello\x1b[0m",
            "\x1b[31m12345\x1b[0m",
            "\x1b[1m\x1b[31mabcdef\x1b[0m",
            "\x1b[1m\x1b[21m\x1b[4mabcdef\x1b[0m",
            "一二三四五六七八九十",
            "🎉🏽👍🇺🇸abc",
            "e\u{301}x",
            "a\x07b\x08c\x7fd",
            "\x1b]8;;https://x.dev\x07here",
            "abcde\x1b]8;;u\x07fghij",
            "\x1b[31mabc\x1b]8;;u\x07defgh",
            "\x1b[2Aab",
            "\x1b",
            "a中b",
            "\x1b[31ma\tb\x1b[0m",
            "🎉\t🎉",
        ]
    }

    #[test]
    fn scan_pins_literal_rows() {
        let mut row = String::new();
        let mut scan = WrappedLine::new("hello world", 5, 0);
        assert!(scan.next(&mut row));
        assert_eq!(row, "hello");
        assert!(scan.next(&mut row));
        assert_eq!(row, " worl");
        assert!(scan.next(&mut row));
        assert_eq!(row, "d");
        assert!(!scan.next(&mut row), "the last row exhausts the scan");

        let mut scan = WrappedLine::new("a中b", 3, 0);
        assert!(scan.next(&mut row));
        assert_eq!(row, "a中");
        assert!(scan.next(&mut row));
        assert_eq!(row, "b");

        let mut scan = WrappedLine::new("e\u{301}x", 4, 0);
        assert!(scan.next(&mut row));
        assert_eq!(
            row, "e\u{301}x",
            "combining marks travel with their base char"
        );

        let mut scan = WrappedLine::new("", 5, 0);
        assert!(!scan.next(&mut row), "an empty line produces no scan row");
    }

    #[test]
    fn skip_then_next_reaches_each_built_row() {
        for line in scan_corpus() {
            for width in 1..=12 {
                for start_col in [0, 2, 4, 7] {
                    let built = wrap_line_ansi(line, width, start_col);
                    for (k, expected) in built.iter().enumerate() {
                        let mut scan = WrappedLine::new(line, width, start_col);
                        assert_eq!(
                            scan.skip_to(k),
                            k,
                            "skipping {k} rows must land on row {k} of {line:?}"
                        );
                        let mut row = String::new();
                        let present = scan.next(&mut row);
                        if line.is_empty() {
                            assert!(
                                !present,
                                "an empty line has no scan row {k}; the blank row is \
                                 the builder's `[String::new()]` fallback"
                            );
                        } else {
                            assert!(
                                present,
                                "row {k} must still be materializable after the skip"
                            );
                            assert_eq!(
                                &row, expected,
                                "row {k} after skip_to must equal the built row ({expected:?})"
                            );
                        }
                    }
                }
            }
        }
    }
}
