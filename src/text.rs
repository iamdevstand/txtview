//! Text utilities shared by the whole viewer.
//!
//! [`wrap_line_ansi`] turns a line into terminal-safe display rows of a given
//! width: it keeps ANSI styling and OSC8 hyperlinks intact, shows control
//! bytes and other escapes as visible caret notation, and never splits a
//! grapheme cluster. The content and the help bar both wrap through this one
//! implementation. [`write_visible_row`] writes the leading part of a display
//! row that fits a given width the same way, skipping a trailing cluster that
//! would cross the width edge, so a piece never paints past the area it owns.

pub(crate) mod ansi;
pub(crate) mod sgr;
pub(crate) mod wrap;

use std::io;

use unicode_segmentation::UnicodeSegmentation;

use self::ansi::{Esc, display_width, escape_display, parse_escape};
use self::wrap::cluster_width_at;

pub(crate) use self::wrap::wrap_line_ansi;

/// Write the leading part of a display row that fits in `width` terminal
/// columns and return the columns written. SGR and OSC8 escapes pass through
/// raw and take no width, every other escape is shown as caret notation, and
/// grapheme clusters are never split: a cluster wide enough to cross the
/// `width` edge (possible on a one-column viewport) is skipped whole so the
/// row never paints past the area it owns (issue #12).
pub(crate) fn write_visible_row(
    text: &str,
    width: usize,
    out: &mut dyn io::Write,
) -> io::Result<usize> {
    let bytes = text.as_bytes();
    let mut used = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b {
            let (end, kind) = parse_escape(bytes, i);
            let seq = &text[i..end];
            if matches!(kind, Esc::Sgr | Esc::Osc8) {
                write!(out, "{seq}")?;
            } else {
                let display = escape_display(seq);
                let w = display_width(&display);
                if used + w > width {
                    return Ok(used);
                }
                write!(out, "{display}")?;
                used += w;
            }
            i = end;
            continue;
        }
        let cluster = &text[i..].graphemes(true).next().unwrap_or_default();
        let w = cluster_width_at(cluster, used);
        if used + w > width {
            return Ok(used);
        }
        write!(out, "{cluster}")?;
        used += w;
        i += cluster.len();
    }
    Ok(used)
}
