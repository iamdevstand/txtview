//! SGR (Select Graphic Rendition) styling state.
//!
//! Folds the SGR codes seen in the wrapped line into a live model of the
//! terminal style, so [`SgrState::to_ansi`] can re-emit a compact and
//! correct prefix at every wrap boundary.

use std::collections::BTreeSet;

// SGR parameter codes for the style state machine (ECMA-48 / ITU-T T.416).
const SGR_RESET: u8 = 0;
const SGR_BOLD: u8 = 1;
const SGR_DIM: u8 = 2;
const SGR_ITALIC: u8 = 3;
const SGR_UNDERLINE: u8 = 4;
const SGR_BLINK: u8 = 5;
const SGR_RAPID_BLINK: u8 = 6;
const SGR_REVERSE: u8 = 7;
const SGR_CONCEAL: u8 = 8;
const SGR_STRIKE: u8 = 9;
const SGR_DEFAULT_FONT: u8 = 10;
const SGR_ALT_FONT_MIN: u8 = 11;
const SGR_ALT_FONT_MAX: u8 = 19;
const SGR_DOUBLE_UNDERLINE: u8 = 21;
const SGR_OFF_BOLD_DIM: u8 = 22;
const SGR_OFF_ITALIC: u8 = 23;
const SGR_OFF_UNDERLINE: u8 = 24;
const SGR_OFF_BLINK: u8 = 25;
const SGR_OFF_REVERSE: u8 = 27;
const SGR_OFF_CONCEAL: u8 = 28;
const SGR_OFF_STRIKE: u8 = 29;
const SGR_FG_MIN: u8 = 30;
const SGR_FG_MAX: u8 = 37;
const SGR_EXTENDED_FG: u8 = 38;
const SGR_DEFAULT_FG: u8 = 39;
const SGR_BG_MIN: u8 = 40;
const SGR_BG_MAX: u8 = 47;
const SGR_EXTENDED_BG: u8 = 48;
const SGR_DEFAULT_BG: u8 = 49;
const SGR_FRAME: u8 = 51;
const SGR_CIRCLE: u8 = 52;
const SGR_OVERLINE: u8 = 53;
const SGR_OFF_FRAME_CIRCLE: u8 = 54;
const SGR_OFF_OVERLINE: u8 = 55;
const SGR_BRIGHT_FG_MIN: u8 = 90;
const SGR_BRIGHT_FG_MAX: u8 = 97;
const SGR_BRIGHT_BG_MIN: u8 = 100;
const SGR_BRIGHT_BG_MAX: u8 = 107;
const SGR_COLOR_INDEXED: u8 = 5;
const SGR_COLOR_RGB: u8 = 2;

/// Live styling state folded from the SGR codes seen so far.
///
/// The wrapper re-emits this at each wrap boundary so a wrapped row opens
/// with the exact style that was active when the line broke. Unlike a raw
/// code log it understands attribute-off and reset semantics, so the
/// emitted prefix stays small and contradiction-free.
#[derive(Default)]
pub(super) struct SgrState {
    attrs: BTreeSet<u8>,
    font: Option<u8>,
    other: BTreeSet<String>,
    fg: Option<String>,
    bg: Option<String>,
}

impl SgrState {
    /// Fold an SGR sequence (an `ESC [...m` string) into the live state.
    pub(super) fn apply(&mut self, seq: &str) {
        let inner = seq.strip_prefix("\x1b[").and_then(|s| s.strip_suffix('m'));
        let Some(inner) = inner else {
            return;
        };
        let params: Vec<u8> = inner
            .split(';')
            .filter_map(|p| p.parse::<u8>().ok())
            .collect();
        if params.is_empty() {
            self.clear();
            return;
        }
        let mut i = 0;
        while i < params.len() {
            match params[i] {
                SGR_RESET => self.clear(),
                SGR_BOLD | SGR_DIM | SGR_ITALIC | SGR_UNDERLINE | SGR_BLINK | SGR_RAPID_BLINK
                | SGR_REVERSE | SGR_CONCEAL | SGR_STRIKE | SGR_DOUBLE_UNDERLINE | SGR_FRAME
                | SGR_CIRCLE | SGR_OVERLINE => {
                    self.attrs.insert(params[i]);
                }
                SGR_DEFAULT_FONT => self.font = None,
                SGR_ALT_FONT_MIN..=SGR_ALT_FONT_MAX => self.font = Some(params[i]),
                SGR_OFF_BOLD_DIM => {
                    self.attrs.remove(&SGR_BOLD);
                    self.attrs.remove(&SGR_DIM);
                }
                SGR_OFF_ITALIC => {
                    self.attrs.remove(&SGR_ITALIC);
                }
                SGR_OFF_UNDERLINE => {
                    self.attrs.remove(&SGR_UNDERLINE);
                    self.attrs.remove(&SGR_DOUBLE_UNDERLINE);
                }
                SGR_OFF_BLINK => {
                    self.attrs.remove(&SGR_BLINK);
                    self.attrs.remove(&SGR_RAPID_BLINK);
                }
                SGR_OFF_REVERSE => {
                    self.attrs.remove(&SGR_REVERSE);
                }
                SGR_OFF_CONCEAL => {
                    self.attrs.remove(&SGR_CONCEAL);
                }
                SGR_OFF_STRIKE => {
                    self.attrs.remove(&SGR_STRIKE);
                }
                SGR_OFF_FRAME_CIRCLE => {
                    self.attrs.remove(&SGR_FRAME);
                    self.attrs.remove(&SGR_CIRCLE);
                }
                SGR_OFF_OVERLINE => {
                    self.attrs.remove(&SGR_OVERLINE);
                }
                SGR_FG_MIN..=SGR_FG_MAX | SGR_BRIGHT_FG_MIN..=SGR_BRIGHT_FG_MAX => {
                    self.fg = Some(params[i].to_string());
                }
                SGR_DEFAULT_FG => self.fg = None,
                SGR_BG_MIN..=SGR_BG_MAX | SGR_BRIGHT_BG_MIN..=SGR_BRIGHT_BG_MAX => {
                    self.bg = Some(params[i].to_string());
                }
                SGR_DEFAULT_BG => self.bg = None,
                SGR_EXTENDED_FG | SGR_EXTENDED_BG => {
                    let code = params[i];
                    if i + 2 < params.len() && params[i + 1] == SGR_COLOR_INDEXED {
                        let slot = format!("{code};{};{}", SGR_COLOR_INDEXED, params[i + 2]);
                        if code == SGR_EXTENDED_FG {
                            self.fg = Some(slot);
                        } else {
                            self.bg = Some(slot);
                        }
                        i += 2;
                    } else if i + 4 < params.len() && params[i + 1] == SGR_COLOR_RGB {
                        let slot = format!(
                            "{code};{};{};{};{}",
                            SGR_COLOR_RGB,
                            params[i + 2],
                            params[i + 3],
                            params[i + 4]
                        );
                        if code == SGR_EXTENDED_FG {
                            self.fg = Some(slot);
                        } else {
                            self.bg = Some(slot);
                        }
                        i += 4;
                    } else {
                        self.other.insert(code.to_string());
                    }
                }
                _ => {
                    self.other.insert(params[i].to_string());
                }
            }
            i += 1;
        }
    }

    /// The canonical SGR string re-opening this style, or `None` when
    /// nothing is styled.
    pub(super) fn to_ansi(&self) -> Option<String> {
        let mut codes = Vec::with_capacity(5);
        for a in &self.attrs {
            codes.push(a.to_string());
        }
        if let Some(f) = self.font {
            codes.push(f.to_string());
        }
        for o in &self.other {
            codes.push(o.clone());
        }
        if let Some(f) = &self.fg {
            codes.push(f.clone());
        }
        if let Some(b) = &self.bg {
            codes.push(b.clone());
        }
        if codes.is_empty() {
            return None;
        }
        Some(format!("\x1b[{}m", codes.join(";")))
    }

    fn clear(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(codes: &[&str]) -> Option<String> {
        let mut s = SgrState::default();
        for c in codes {
            s.apply(c);
        }
        s.to_ansi()
    }

    #[test]
    fn reset_forms_clear_everything() {
        assert_eq!(state(&["\x1b[31m", "\x1b[0m"]), None);
        assert_eq!(state(&["\x1b[31m", "\x1b[m"]), None);
        assert_eq!(state(&["\x1b[1;32m", "\x1b[0m"]), None);
        assert_eq!(state(&["\x1b[m"]), None);
    }

    #[test]
    fn attribute_off_removes_attribute() {
        assert_eq!(state(&["\x1b[1m", "\x1b[22m"]), None);
        assert_eq!(
            state(&["\x1b[1;31m", "\x1b[22m"]),
            Some("\x1b[31m".to_string())
        );
        assert_eq!(state(&["\x1b[4m", "\x1b[24m"]), None);
    }

    #[test]
    fn canonical_order_is_attrs_then_fg_then_bg() {
        assert_eq!(
            state(&["\x1b[42m", "\x1b[1m", "\x1b[31m"]),
            Some("\x1b[1;31;42m".to_string())
        );
    }

    #[test]
    fn extended_colors_stay_whole() {
        assert_eq!(
            state(&["\x1b[38;5;123m"]),
            Some("\x1b[38;5;123m".to_string())
        );
        assert_eq!(
            state(&["\x1b[38;2;10;20;30m"]),
            Some("\x1b[38;2;10;20;30m".to_string())
        );
        assert_eq!(state(&["\x1b[48;5;42m"]), Some("\x1b[48;5;42m".to_string()));
    }

    #[test]
    fn newer_fg_replaces_older_fg() {
        assert_eq!(
            state(&["\x1b[31m", "\x1b[32m"]),
            Some("\x1b[32m".to_string())
        );
    }

    #[test]
    fn unknown_codes_are_preserved_and_deduped() {
        assert_eq!(
            state(&["\x1b[60m", "\x1b[60m"]),
            Some("\x1b[60m".to_string())
        );
    }
}
