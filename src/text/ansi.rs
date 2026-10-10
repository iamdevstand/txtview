//! Escape-sequence parsing and neutralization.
//!
//! Control bytes and non-SGR escape sequences are turned into visible caret
//! notation so they can never execute on the terminal. SGR styling and OSC8
//! hyperlinks are classified for raw passthrough by [super::wrap].

use unicode_width::UnicodeWidthChar;

/// Whether `c` is rendered as 2-column caret notation (`^X` / `^?`).
///
/// C0 and C1 controls and DEL are escaped to caret notation so they never
/// reach the terminal as live control bytes. Callers must handle tab and
/// `ESC` before calling this, as the wrapper does.
pub(super) fn caret_width(c: char) -> bool {
    let code = c as u32;
    code <= 0x1f || (0x7f..=0x9f).contains(&code)
}

/// Render a control character as its visible caret-notation equivalent.
///
/// C0 controls map to `^@`..=`^_` (e.g. BEL → `^G`, backspace → `^H`,
/// CR → `^M`); DEL maps to `^?`; C1 controls (the 8-bit control set, the
/// high-bit equivalents of C0, e.g. 8-bit CSI → `^[`) map the same way so
/// they can never execute on the terminal.
pub(super) fn caret_notation(c: char) -> String {
    let code = c as u32;
    match code {
        0..=0x1f | 0x80..=0x9f => {
            let letter = char::from_u32(code % 0x40 + 0x40).unwrap_or('?');
            format!("^{letter}")
        }
        0x7f => "^?".to_string(),
        _ => c.to_string(),
    }
}

/// Classification of an escape sequence for [`super::wrap::wrap_line_ansi`].
pub(super) enum Esc {
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
/// consumes `ESC [` plus parameter / intermediate bytes and one final byte
/// (0x40-0x7e); when the byte after the parameters is not a valid final byte
/// the sequence stops before it, leaving that byte to be handled as text.
/// OSC (`ESC ]`) consumes until a TERM (`BEL` or ST `ESC \`), everything else
/// is a two-byte escape `ESC <byte>`. A lone `ESC` at the end of input is left
/// unconsumed after itself. The returned index always lands on a UTF-8 char
/// boundary.
pub(super) fn parse_escape(bytes: &[u8], start: usize) -> (usize, Esc) {
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
            if i < bytes.len() && (0x40..=0x7e).contains(&bytes[i]) {
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
pub(super) fn escape_display(seq: &str) -> String {
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

/// The terminal columns [`escape_display`] occupies, computed without building
/// the display string. `ESC` and every control byte render as a two-character
/// caret pair, everything else keeps its own width. The two helpers share the
/// same predicates, so they agree for any input.
/// `escape_display_width_matches_display` pins that across a corpus.
pub(super) fn escape_display_width(seq: &str) -> usize {
    let mut width = 0;
    for c in seq.chars() {
        width += if c == '\u{1b}' || caret_width(c) {
            2
        } else {
            c.width().unwrap_or(1)
        };
    }
    width
}

/// The terminal columns of a run of already-neutralized display text: each
/// character takes its Unicode width, with a guaranteed minimum of one.
pub(super) fn display_width(text: &str) -> usize {
    text.chars().map(|c| c.width().unwrap_or(1)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_display_width_matches_display() {
        // Every C0/C1 control and DEL, both bare and inside an escape prefix
        for c in 0u32..=0x9f {
            let ch = char::from_u32(c).unwrap();
            for text in [ch.to_string(), format!("\x1b{ch}"), format!("\x1b[{ch}")] {
                assert_eq!(
                    escape_display_width(&text),
                    display_width(&escape_display(&text)),
                    "width of {text:?}"
                );
            }
        }
        let corpus = [
            "\x1b[",
            "\x1b[2A",
            "\x1b[2J",
            "\x1b]0;hi\x07",
            "\x1b]0;hi\x1b\\",
            "\x1b7",
            "\x1bX",
            "\x1b",
            "\x1b[中",
            "\x1b[31中x",
            "a\x07\x7f中",
            "\x1b]0;中❤\u{fe0f}\x07",
        ];
        for seq in corpus {
            assert_eq!(
                escape_display_width(seq),
                display_width(&escape_display(seq)),
                "width of {seq:?}"
            );
        }
    }
}
