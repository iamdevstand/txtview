//! Line wrapping that keeps ANSI styling intact while neutralizing control
//! bytes and non-SGR escape sequences.

use std::borrow::Cow;

use unicode_width::UnicodeWidthChar;

use super::ansi::{Esc, caret_notation, caret_width, escape_display, parse_escape};

/// Wrap `line` into visual rows of at most `width` terminal columns.
///
/// `start_col` is the width of any prefix the caller renders before the
/// line, used so tabs land on their column stops and wraps line up under
/// the prefix.
///
/// The wrapping preserves safe content verbatim and neutralizes the rest:
/// - SGR (`ESC [...]m`) and OSC8 hyperlinks pass through raw (OSC8 emits as
///   one atomic unit so a wrap can never split it),
/// - C0 controls, DEL, and every other escape become visible caret
///   notation,
/// - tabs keep their literal byte and are wrapped at their 8-column stop.
pub(super) fn wrap_line_ansi(line: &str, width: usize, start_col: usize) -> Vec<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
