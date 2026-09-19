//! Text utilities shared by the whole viewer.
//!
//! [`wrap_line_ansi`] turns a line into terminal-safe display rows of a given
//! width: it keeps ANSI styling and OSC8 hyperlinks intact, shows control
//! bytes and other escapes as visible caret notation, and never splits a
//! grapheme cluster. The content and the help bar both wrap through this one
//! implementation. [`visual_len`] measures a display row the same way, so a
//! piece can pad a row to its area's width without writing past the area it
//! owns.

pub(crate) mod ansi;
pub(crate) mod sgr;
pub(crate) mod wrap;

use unicode_segmentation::UnicodeSegmentation;

use self::ansi::{Esc, display_width, escape_display, parse_escape};
use self::wrap::cluster_width_at;

pub(crate) use self::wrap::wrap_line_ansi;

/// The terminal columns a display row occupies, measured the same way
/// [`wrap_line_ansi`] wraps: SGR and OSC8 escapes take no width, every other
/// escape is counted by its caret notation, tabs advance to their 8-column
/// stop, control characters become caret notation, and grapheme clusters keep
/// their visual width. A wrapped row never exceeds the width it was wrapped
/// to, so padding its remaining columns with spaces keeps it inside the area
/// it owns.
pub(crate) fn visual_len(s: &str) -> usize {
    let bytes = s.as_bytes();
    let mut width = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b {
            let (end, kind) = parse_escape(bytes, i);
            let seq = &s[i..end];
            if !matches!(kind, Esc::Sgr | Esc::Osc8) {
                width += display_width(&escape_display(seq));
            }
            i = end;
            continue;
        }
        let cluster = &s[i..].graphemes(true).next().unwrap_or_default();
        width += cluster_width_at(cluster, width);
        i += cluster.len();
    }
    width
}
