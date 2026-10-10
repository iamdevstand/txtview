//! Text utilities shared by the whole viewer.
//!
//! [`wrap_line_ansi`] turns a line into terminal-safe display rows of a given
//! width: it keeps ANSI styling and OSC8 hyperlinks intact, shows control
//! bytes and other escapes as visible caret notation, and never splits a
//! grapheme cluster. The content and the help bar both wrap through this one
//! implementation. [`write_visible_row`] appends the leading part of a display
//! row that fits a given width to a byte buffer the same way, skipping a
//! trailing cluster that would cross the width edge, so a piece never paints
//! past the area it owns.

pub(crate) mod ansi;
pub(crate) mod sgr;
pub(crate) mod wrap;

use unicode_segmentation::UnicodeSegmentation;

use self::ansi::{Esc, display_width, escape_display, parse_escape};
use self::wrap::cluster_width_at;

pub(crate) use self::wrap::{WrappedLine, wrap_line_ansi, wrap_line_ansi_count};

/// Append the leading part of a display row that fits in `width` terminal
/// columns to `buf` and return the columns written. SGR and OSC8 escapes pass
/// through raw and take no width, every other escape is shown as caret
/// notation, tabs are written as the space run to their stop and grapheme
/// clusters are never split: a cluster wide enough to cross the `width` edge
/// (possible on a one-column viewport) is skipped whole so the row never
/// paints past the area it owns.
///
/// The caller commits the whole row with a single write, so this appends raw
/// bytes instead of formatting each piece through `core::fmt`.
///
/// Tabs are never emitted as the literal byte: the terminal would advance
/// without touching the gap cells, leaving stale glyphs from the previous
/// frame there when the row scrolls.
pub(crate) fn write_visible_row(text: &str, width: usize, buf: &mut Vec<u8>) -> usize {
    let bytes = text.as_bytes();
    let mut used = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b {
            let (end, kind) = parse_escape(bytes, i);
            let seq = &text[i..end];
            if matches!(kind, Esc::Sgr | Esc::Osc8) {
                buf.extend_from_slice(seq.as_bytes());
            } else {
                let display = escape_display(seq);
                let w = display_width(&display);
                if used + w > width {
                    return used;
                }
                buf.extend_from_slice(display.as_bytes());
                used += w;
            }
            i = end;
            continue;
        }
        let cluster = &text[i..].graphemes(true).next().unwrap_or_default();
        let w = cluster_width_at(cluster, used);
        if used + w > width {
            return used;
        }
        if *cluster == "\t" {
            buf.resize(buf.len() + w, b' ');
        } else {
            buf.extend_from_slice(cluster.as_bytes());
        }
        used += w;
        i += cluster.len();
    }
    used
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(text: &str, width: usize) -> (String, usize) {
        let mut buf = Vec::new();
        let used = write_visible_row(text, width, &mut buf);
        (String::from_utf8(buf).unwrap(), used)
    }

    #[test]
    fn writes_a_plain_row_verbatim() {
        assert_eq!(row("hello", 10), ("hello".to_string(), 5));
    }

    #[test]
    fn expands_a_tab_to_its_stop() {
        assert_eq!(row("a\t", 8), ("a       ".to_string(), 8));
    }

    #[test]
    fn lone_tab_fills_to_the_stop() {
        assert_eq!(row("\t", 8), ("        ".to_string(), 8));
    }

    #[test]
    fn neutralizes_a_visible_escape() {
        assert_eq!(row("\x1b[", 5), ("^[[".to_string(), 3));
    }

    #[test]
    fn passes_sgr_through_raw() {
        assert_eq!(row("\x1b[31mX", 10), ("\x1b[31mX".to_string(), 1));
    }

    #[test]
    fn clips_a_cluster_wider_than_the_width_whole() {
        assert_eq!(row("🎉", 1), (String::new(), 0));
        assert_eq!(row("ab🎉", 3), ("ab".to_string(), 2));
    }

    #[test]
    fn skips_an_escape_that_would_cross_the_edge() {
        assert_eq!(row("\x1b[", 1), (String::new(), 0));
    }
}
