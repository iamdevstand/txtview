//! Integration tests for the layout module: display rows, wrapping, geometry,
//! and scroll-bounds behavior exercised through the full [TxtView].
//!
//! These live in a sibling module under `txtview` so they can read the
//! viewer's private fields (`display`, `offset`, `max_offset`) and via
//! `super::test_metrics`, the rebuild and wrap counters the production
//! struct itself does not carry.

use crate::TxtViewConfig;

use super::TxtView;
use super::test_metrics::{rebuilds, reset, wraps};

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

fn scrolling_viewer(n: usize) -> TxtView {
    let text = (0..n)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let config = TxtViewConfig {
        viewport_height: Some(10),
        viewport_width: Some(80),
        show_help_bar: false,
        ..TxtViewConfig::default()
    };
    TxtView::new(&text).with_config(config)
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
fn display_reserves_no_scrollbar_column_when_fits() {
    let config = TxtViewConfig {
        show_help_bar: false,
        show_scrollbar: true,
        viewport_width: Some(4),
        viewport_height: Some(10),
        ..TxtViewConfig::default()
    };
    let v = TxtView::new("abcdef").with_config(config);
    assert_eq!(v.display, vec!["abcd", "ef"]);
    assert_eq!(v.max_offset, 0);
}

#[test]
fn display_reserves_scrollbar_column_when_overflowing() {
    let config = TxtViewConfig {
        show_help_bar: false,
        show_scrollbar: true,
        viewport_width: Some(4),
        viewport_height: Some(10),
        ..TxtViewConfig::default()
    };
    let input = (0..11)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let v = TxtView::new(&input).with_config(config);
    assert!(v.max_offset > 0, "must actually overflow");
    assert!(
        v.display.iter().all(|row| row.chars().count() <= 3),
        "scrollbar column must be reserved on overflow: {:?}",
        v.display
    );
}

#[test]
fn scrollbar_column_released_when_config_turned_off() {
    let config = |show: bool| TxtViewConfig {
        show_help_bar: false,
        show_scrollbar: show,
        viewport_width: Some(4),
        viewport_height: Some(10),
        ..TxtViewConfig::default()
    };
    let input = (0..11)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let v = TxtView::new(&input).with_config(config(true));
    assert!(
        v.display.iter().all(|row| row.chars().count() <= 3),
        "scrollbar enabled + overflow must reserve: {:?}",
        v.display
    );
    let v = v.with_config(config(false));
    assert!(
        v.display.iter().all(|row| row.chars().count() <= 4),
        "scrollbar off must release the column: {:?}",
        v.display
    );
}

#[test]
fn offset_only_scroll_skips_display_rebuild() {
    reset();
    let input = (0..100)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut v = viewer(&input, 10);
    let built = rebuilds();
    v.scroll_down(50);
    v.refresh_bounds();
    assert_eq!(rebuilds(), built, "scroll re-wrapped the whole document");
    assert_eq!(v.offset, 50);
}

#[test]
fn visible_rows_keep_the_whole_viewport_when_help_cannot_fit() {
    let config = TxtViewConfig {
        show_help_bar: true,
        show_scrollbar: false,
        viewport_width: Some(1),
        viewport_height: Some(24),
        ..TxtViewConfig::default()
    };
    let v = TxtView::new("hello\nworld").with_config(config);
    assert_eq!(v.visible_rows(), 24);
}

#[test]
fn visible_rows_span_a_tiny_terminal_without_help() {
    let config = TxtViewConfig {
        show_help_bar: true,
        show_scrollbar: false,
        viewport_width: Some(80),
        viewport_height: Some(2),
        ..TxtViewConfig::default()
    };
    let v = TxtView::new("hello\nworld").with_config(config);
    assert_eq!(v.visible_rows(), 2);
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
    reset();
    let config = |w| TxtViewConfig {
        show_help_bar: false,
        show_scrollbar: false,
        viewport_width: Some(w),
        viewport_height: Some(10),
        ..TxtViewConfig::default()
    };
    let v = TxtView::new("abcdefghij").with_config(config(10));
    let n = rebuilds();
    let v = v.with_config(config(5));
    assert!(rebuilds() > n, "width change must re-wrap");
    assert_eq!(v.display, vec!["abcde", "fghij"]);
}

#[test]
fn height_change_reuses_display() {
    reset();
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
    let n = rebuilds();
    let m0 = v.max_offset;
    v = v.with_config(config(20));
    assert_eq!(rebuilds(), n, "height change must not re-wrap");
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

#[test]
fn scrollbar_skipped_when_everything_fits() {
    let v = scrolling_viewer(5);
    assert_eq!(v.max_offset, 0);
    assert!(
        v.scroll_geometry().is_none(),
        "no geometry must be produced when everything fits"
    );
}

#[test]
fn offset_from_thumb_top_is_monotonic_and_bounded() {
    let v = scrolling_viewer(100);
    let g = v.scroll_geometry().expect("scrollbar present");
    let travel = i64::try_from(g.visible - g.size).unwrap_or(i64::MAX);

    assert_eq!(g.offset_from_thumb_top(-5, v.max_offset), 0);
    assert_eq!(g.offset_from_thumb_top(0, v.max_offset), 0);
    assert_eq!(g.offset_from_thumb_top(travel, v.max_offset), v.max_offset);

    let mut prev = 0;
    for top in 0..=travel {
        let off = g.offset_from_thumb_top(top, v.max_offset);
        assert!(off >= prev, "not monotonic at top={top}");
        assert!(off <= v.max_offset, "exceeds max_offset at top={top}");
        prev = off;
    }
}

#[test]
fn dragging_keeps_thumb_on_mouse() {
    let mut v = scrolling_viewer(100);
    let visible = v.visible_rows() as usize;
    let g = v.scroll_geometry().unwrap();

    for mouse_y in 0..u16::try_from(visible).unwrap_or(0) {
        let off = g.offset_from_thumb_top(i64::from(mouse_y), v.max_offset);
        v.offset = off;
        let moved = v.scroll_geometry().unwrap();
        let travel = g.visible - g.size;
        let desired = usize::from(mouse_y).clamp(0, travel);
        assert_eq!(
            moved.top, desired,
            "thumb at {} dragged to {} for y={}",
            moved.top, desired, mouse_y
        );
    }
}

#[test]
fn non_divisible_geometry_keeps_thumb_bounded_and_near_mouse() {
    let mut v = scrolling_viewer(15);
    let visible = v.visible_rows() as usize;
    let g = v.scroll_geometry().expect("scrollbar present");
    let travel = g.visible - g.size;
    assert!(
        v.max_offset % travel.max(1) != 0,
        "the fixture must produce non-divisible geometry"
    );

    let mut prev = 0;
    for mouse_y in 0..u16::try_from(visible).unwrap_or(0) {
        let off = g.offset_from_thumb_top(i64::from(mouse_y), v.max_offset);
        v.offset = off;
        let moved = v.scroll_geometry().unwrap();
        let desired = usize::from(mouse_y).clamp(0, travel);
        assert!(
            moved.top.abs_diff(desired) <= 1,
            "page-flip drifts: thumb at {} dragged to {} for y={}",
            moved.top,
            desired,
            mouse_y
        );
        assert!(
            moved.top >= prev,
            "not monotonic at y={mouse_y}: {} then {}",
            prev,
            moved.top
        );
        assert!(
            moved.top <= travel,
            "thumb escaped the track at y={mouse_y}: {}",
            moved.top
        );
        prev = moved.top;
    }
}

#[test]
fn scrollbar_hidden_when_track_cannot_host_thumb_and_travel() {
    for height in [1, 2] {
        let config = TxtViewConfig {
            show_help_bar: false,
            show_scrollbar: true,
            viewport_width: Some(80),
            viewport_height: Some(height),
            ..TxtViewConfig::default()
        };
let v = TxtView::new(scratch_lines(100)).with_config(config);
        assert!(
            !v.scrollbar_active,
            "a {height}-row viewport cannot host a thumb plus travel"
        );
        assert!(
            v.scroll_geometry().is_none(),
            "no geometry for a {height}-row viewport"
        );
    }
}

#[test]
fn thumb_capped_so_a_bare_overflow_keeps_travel_room() {
    let config = TxtViewConfig {
        show_help_bar: false,
        show_scrollbar: true,
        viewport_width: Some(80),
        viewport_height: Some(10),
        ..TxtViewConfig::default()
    };
    let v = TxtView::new(scratch_lines(11)).with_config(config.clone());
    let geometry = v
        .scroll_geometry()
        .expect("11 rows over 10 must stay usable");
    assert_eq!(
        geometry.visible - geometry.size,
        2,
        "the 11-over-10 thumb must give up cells to keep MIN_TRAVEL of travel"
    );

    let v = TxtView::new(scratch_lines(100)).with_config(config);
    let geometry = v
        .scroll_geometry()
        .expect("a tall document must keep the smallest thumb");
    assert_eq!(
        geometry.size, 1,
        "the cap must not touch normal sized thumbs"
    );
}

#[test]
fn overflow_reserves_column_only_after_one_wrap() {
    reset();
    let v = scrolling_viewer(100);
    assert!(v.scrollbar_active, "fixture must overflow");
    assert!(rebuilds() >= 1, "construction must have rebuilt");
    assert_eq!(
        wraps(),
        rebuilds(),
        "an overflowing document must be wrapped once per rebuild: the \
         scrollbar column is reserved before wrapping, so no re-wrap is needed \
         to carve it out (wrapped {} times for {} rebuilds)",
        wraps(),
        rebuilds(),
    );
}

#[test]
fn fitting_document_releases_the_reserved_column() {
    reset();
    let v = scrolling_viewer(5);
    assert!(
        !v.scrollbar_active,
        "a fitting document must not keep a bar"
    );
    assert_eq!(
        wraps() - rebuilds(),
        rebuilds(),
        "the release re-wrap costs one extra pass per rebuild, on a document \
         small enough that the reserved-width wrap only just fit"
    );
}

fn scratch_lines(n: usize) -> String {
    (0..n)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn scrollbar_column_released_when_height_makes_content_fit() {
    let input = (0..11)
        .map(|i| format!("{i:04}"))
        .collect::<Vec<_>>()
        .join("\n");
    let tall = TxtViewConfig {
        show_help_bar: false,
        show_scrollbar: true,
        viewport_width: Some(4),
        viewport_height: Some(10),
        ..TxtViewConfig::default()
    };
    let mut v = TxtView::new(&input).with_config(tall.clone());
    assert!(v.scrollbar_active, "11 rows must overflow 10");
    assert!(
        v.display.iter().all(|row| row.chars().count() <= 3),
        "the scrollbar column must be reserved while it overflows: {:?}",
        v.display
    );

    let short = TxtViewConfig {
        viewport_height: Some(40),
        ..tall
    };
    v = v.with_config(short);
    v.refresh_bounds();
    assert!(
        !v.scrollbar_active,
        "once the content fits, the bar must go"
    );
    assert_eq!(v.max_offset, 0);
    assert_eq!(
        v.display.len(),
        11,
        "the freed column must stop the wrapping"
    );
    assert!(
        v.display.iter().all(|row| row.chars().count() <= 4),
        "the freed column must be released: {:?}",
        v.display
    );
}
