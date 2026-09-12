//! Escape-sequence parsing and neutralization.
//!
//! Control bytes and non-SGR escape sequences are turned into visible caret
//! notation so they can never execute on the terminal. SGR styling and OSC8
//! hyperlinks are classified for raw passthrough by [super::wrap].

/// Whether `c` is rendered as 2-column caret notation (`^X` / `^?`).
///
/// C0 controls (except tab and `ESC`, which are handled elsewhere) and DEL are
/// escaped so they never reach the terminal as live control bytes.
pub(super) fn caret_width(c: char) -> bool {
    let code = c as u32;
    code <= 0x1f || code == 0x7f
}

/// Render a control character as its visible caret-notation equivalent.
///
/// C0 controls map to `^@`..=`^_` (e.g. BEL → `^G`, backspace → `^H`,
/// CR → `^M`); DEL maps to `^?`. C1 controls are returned unchanged.
pub(super) fn caret_notation(c: char) -> String {
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
/// consumes `ESC [` plus parameter / intermediate bytes and one final byte.
/// OSC (`ESC ]`) consumes until a TERM (`BEL` or ST `ESC \`), everything else
/// is a two-byte escape `ESC <byte>`. A lone `ESC` at the end of input is left
/// unconsumed after itself.
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
