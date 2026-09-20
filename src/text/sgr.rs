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
const SGR_EXTENDED_UNDERLINE_COLOR: u8 = 58;
const SGR_DEFAULT_UNDERLINE_COLOR: u8 = 59;
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

/// One SGR parameter slot. Parameters are decimal, but ISO 8613-6 colon
/// forms and doubled separators leave empty slots, for example the
/// color-space slot in `38:2::220:0:0`, which stay distinct here so the
/// extended-color cursors can skip them instead of folding them as zeroes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Param {
    Num(u8),
    Empty,
}

impl Param {
    /// Parse one raw separator-delimited field; malformed bytes stay dropped
    /// just as the old `u8` parse dropped them.
    fn parse(field: &str) -> Option<Param> {
        if field.is_empty() {
            Some(Param::Empty)
        } else {
            field.parse::<u8>().ok().map(Param::Num)
        }
    }

    /// The numeric value a parameter contributes to a color: `0` for an
    /// empty slot, per the ECMA-48 implicit-default rule.
    fn value(self) -> String {
        match self {
            Param::Num(n) => n.to_string(),
            Param::Empty => "0".to_string(),
        }
    }
}

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
    other: Vec<String>,
    fg: Option<String>,
    bg: Option<String>,
    ulcolor: Option<String>,
}

impl SgrState {
    /// Fold an SGR sequence (an `ESC [...m` string) into the live state.
    pub(super) fn apply(&mut self, seq: &str) {
        let inner = seq.strip_prefix("\x1b[").and_then(|s| s.strip_suffix('m'));
        let Some(inner) = inner else {
            return;
        };
        // Split parameters on `;` or `:` (ISO 8613-6 colon forms separate
        // parameters with colons too), keeping empty slots so a colon-form
        // color like `38:2::220:0:0` keeps its color-space position. The
        // separator that precedes each slot is remembered because a colon
        // form paints a different meaning onto the extended-color fields
        let mut params: Vec<Param> = Vec::new();
        let mut sep_colon: Vec<bool> = Vec::new();
        let mut start = 0;
        let mut pending_colon = false;
        for (idx, ch) in inner.char_indices() {
            if ch == ';' || ch == ':' {
                if let Some(p) = Param::parse(&inner[start..idx]) {
                    params.push(p);
                    sep_colon.push(pending_colon);
                }
                start = idx + 1;
                pending_colon = ch == ':';
            }
        }
        if let Some(p) = Param::parse(&inner[start..]) {
            params.push(p);
            sep_colon.push(pending_colon);
        }
        if params.is_empty() || params.iter().all(|&p| p == Param::Empty) {
            self.clear();
            return;
        }
        let mut i = 0;
        while i < params.len() {
            let code = match params[i] {
                Param::Num(code) => code,
                Param::Empty => {
                    // A doubled separator is an implicit default parameter,
                    // which ECMA-48 defines as 0 (full reset).
                    self.clear();
                    i += 1;
                    continue;
                }
            };
            match code {
                SGR_RESET => self.clear(),
                SGR_BOLD | SGR_DIM | SGR_ITALIC | SGR_UNDERLINE | SGR_BLINK | SGR_RAPID_BLINK
                | SGR_REVERSE | SGR_CONCEAL | SGR_STRIKE | SGR_DOUBLE_UNDERLINE | SGR_FRAME
                | SGR_CIRCLE | SGR_OVERLINE => {
                    self.attrs.insert(code);
                }
                SGR_DEFAULT_FONT => self.font = None,
                SGR_ALT_FONT_MIN..=SGR_ALT_FONT_MAX => self.font = Some(code),
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
                    self.fg = Some(code.to_string());
                }
                SGR_DEFAULT_FG => self.fg = None,
                SGR_BG_MIN..=SGR_BG_MAX | SGR_BRIGHT_BG_MIN..=SGR_BRIGHT_BG_MAX => {
                    self.bg = Some(code.to_string());
                }
                SGR_DEFAULT_BG => self.bg = None,
                SGR_DEFAULT_UNDERLINE_COLOR => self.ulcolor = None,
                SGR_EXTENDED_FG | SGR_EXTENDED_BG | SGR_EXTENDED_UNDERLINE_COLOR => {
                    let paramlen = params.len();
                    if paramlen > i + 2 && params[i + 1] == Param::Num(SGR_COLOR_INDEXED) {
                        if let Param::Num(n) = params[i + 2] {
                            let slot = format!("{code};{};{}", SGR_COLOR_INDEXED, n);
                            self.set_color(code, slot);
                            i += 2;
                        }
                    } else if paramlen > i + 4 && params[i + 1] == Param::Num(SGR_COLOR_RGB) {
                        // A colon form may carry an optional color-space slot
                        // between the mode and the RGB triplet
                        // (`38:2:cs:r:g:b`, `38:2::r:g:b`); when the color was
                        // entered with colors the list is long enough that the
                        // extra slot exists and is dropped. Semicolon forms
                        // never have it, so a trailing `;31`-style parameter
                        // keeps folding as its own SGR code below
                        let colon_form = sep_colon[i + 1];
                        let has_cs = colon_form && paramlen > i + 5;
                        let r = i + if has_cs { 3 } else { 2 };
                        let slot = format!(
                            "{code};{};{};{};{}",
                            SGR_COLOR_RGB,
                            params[r].value(),
                            params[r + 1].value(),
                            params[r + 2].value()
                        );
                        self.set_color(code, slot);
                        i += if has_cs { 5 } else { 4 };
                    }
                    // A malformed extended color (a bare 38, 48, 58 or
                    // sub-params that are neither 5 nor 2) is ignored
                    // rather than replayed, so a lone `38` can never
                    // corrupt the color that follows. Its leftover
                    // parameters are still folded below
                }
                _ => {
                    self.push_other(code.to_string());
                }
            }
            i += 1;
        }
    }

    /// Store a folded extended color slot in the field owned by `code`.
    fn set_color(&mut self, code: u8, slot: String) {
        if code == SGR_EXTENDED_FG {
            self.fg = Some(slot);
        } else if code == SGR_EXTENDED_BG {
            self.bg = Some(slot);
        } else {
            self.ulcolor = Some(slot);
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
        if let Some(u) = &self.ulcolor {
            codes.push(u.clone());
        }
        if codes.is_empty() {
            return None;
        }
        Some(format!("\x1b[{}m", codes.join(";")))
    }

    fn clear(&mut self) {
        *self = Self::default();
    }

    /// Record an unknown code once, in the order it first appeared, so the
    /// replayed sequence matches the original for terminals where the order
    /// of these codes matters.
    fn push_other(&mut self, code: String) {
        if !self.other.contains(&code) {
            self.other.push(code);
        }
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
    fn unknown_codes_keep_their_sequence_order_and_dedup() {
        assert_eq!(
            state(&["\x1b[62m", "\x1b[60m"]),
            Some("\x1b[62;60m".to_string())
        );
        assert_eq!(
            state(&["\x1b[60m", "\x1b[62m", "\x1b[60m"]),
            Some("\x1b[60;62m".to_string())
        );
    }

    #[test]
    fn malformed_extended_color_is_ignored_not_replayed() {
        assert_eq!(state(&["\x1b[38m"]), None);
        assert_eq!(state(&["\x1b[48m"]), None);
        assert_eq!(
            state(&["\x1b[38;5m"]),
            Some("\x1b[5m".to_string()),
            "a truncated extended color must not replay a bare 38"
        );
        assert_eq!(
            state(&["\x1b[38;4;5m"]),
            Some("\x1b[4;5m".to_string()),
            "leftover parameters of a malformed extended color still fold"
        );
    }

    #[test]
    fn colon_form_rgb_sets_the_same_color() {
        assert_eq!(
            state(&["\x1b[38:2::220:0:0m"]),
            Some("\x1b[38;2;220;0;0m".to_string())
        );
        assert_eq!(
            state(&["\x1b[38:2:0:220:0:0m"]),
            Some("\x1b[38;2;220;0;0m".to_string())
        );
        assert_eq!(
            state(&["\x1b[48:2::0:255:0m"]),
            Some("\x1b[48;2;0;255;0m".to_string())
        );
    }

    #[test]
    fn colon_form_indexed_color_folds() {
        assert_eq!(
            state(&["\x1b[38:5:123m"]),
            Some("\x1b[38;5;123m".to_string())
        );
    }

    #[test]
    fn underline_color_folds_like_fg_bg() {
        assert_eq!(
            state(&["\x1b[58;5;10m"]),
            Some("\x1b[58;5;10m".to_string()),
            "a naked underline-color code must not leak a bare blink parameter"
        );
        assert_eq!(
            state(&["\x1b[58:2::10:20:30m"]),
            Some("\x1b[58;2;10;20;30m".to_string())
        );
    }

    #[test]
    fn trailing_param_after_semicolon_rgb_stays_its_own_code() {
        assert_eq!(
            state(&["\x1b[38;2;10;20;30;31m"]),
            Some("\x1b[31m".to_string()),
            "a semicolon form never swallows a trailing code into a color space"
        );
    }

    #[test]
    fn default_underline_color_clears_underline_color() {
        assert_eq!(state(&["\x1b[58;5;10m", "\x1b[59m"]), None);
    }
}
