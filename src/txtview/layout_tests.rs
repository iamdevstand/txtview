//! Integration tests for the layout module: display rows, wrapping, geometry,
//! and scroll-bounds behavior exercised through the full [TxtView].
//!
//! These live in a sibling module under `txtview` so they can read the
//! viewer's private fields (`display`, `offset`, `max_offset`,
//! `rebuild_count`).

use crate::TxtViewConfig;

use super::TxtView;

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
fn tab_wrapped_line_renders_correctly() {
    let v = viewer("a\tb", 8);
    assert_eq!(v.display, vec!["a\t", "b"]);
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
fn offset_only_scroll_skips_display_rebuild() {
    let input = (0..100)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut v = viewer(&input, 10);
    let built = v.rebuild_count;
    v.scroll_down(50);
    v.refresh_bounds();
    assert_eq!(
        v.rebuild_count, built,
        "scroll re-wrapped the whole document"
    );
    assert_eq!(v.offset, 50);
}

#[test]
fn visible_rows_keeps_one_row_on_narrow_width() {
    let config = TxtViewConfig {
        show_help_bar: true,
        show_scrollbar: false,
        viewport_width: Some(1),
        viewport_height: Some(24),
        ..TxtViewConfig::default()
    };
    let v = TxtView::new("hello\nworld").with_config(config);
    assert_eq!(v.visible_rows(), 1);
}

#[test]
fn visible_rows_keeps_one_row_on_tiny_terminal() {
    let config = TxtViewConfig {
        show_help_bar: true,
        show_scrollbar: false,
        viewport_width: Some(80),
        viewport_height: Some(2),
        ..TxtViewConfig::default()
    };
    let v = TxtView::new("hello\nworld").with_config(config);
    assert_eq!(v.visible_rows(), 1);
}

#[test]
fn visible_rows_unchanged_when_help_fits() {
    let config = TxtViewConfig {
        show_help_bar: true,
        show_scrollbar: false,
        viewport_width: Some(80),
        viewport_height: Some(10),
        ..TxtViewConfig::default()
    };
    let v = TxtView::new("hello\nworld").with_config(config);
    let help = 1 + TxtView::help_text().chars().count().div_ceil(80).max(1);
    assert_eq!(usize::from(v.visible_rows()), 10 - help);
}

#[test]
fn width_change_rebuilds_display() {
    let config = |w| TxtViewConfig {
        show_help_bar: false,
        show_scrollbar: false,
        viewport_width: Some(w),
        viewport_height: Some(10),
        ..TxtViewConfig::default()
    };
    let mut v = TxtView::new("abcdefghij").with_config(config(10));
    let n = v.rebuild_count;
    v = v.with_config(config(5));
    assert!(v.rebuild_count > n, "width change must re-wrap");
}

#[test]
fn height_change_reuses_display() {
    let config = |h| TxtViewConfig {
        show_help_bar: false,
        viewport_width: Some(80),
        viewport_height: Some(h),
        ..TxtViewConfig::default()
    };
    let input = (0..100)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut v = TxtView::new(input).with_config(config(10));
    let n = v.rebuild_count;
    let m0 = v.max_offset;
    v = v.with_config(config(20));
    assert_eq!(v.rebuild_count, n, "height change must not re-wrap");
    assert!(v.max_offset < m0);
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
