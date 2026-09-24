use txtview::{TxtView, TxtViewConfig};

mod common;

#[test]
fn config_has_documented_defaults() {
    common::assert_default_config(&TxtViewConfig::default());
}

#[test]
fn config_setters_round_trip_through_config() {
    let config = TxtViewConfig::default()
        .with_show_line_numbers(true)
        .with_show_help_bar(false)
        .with_show_scrollbar(false)
        .with_viewport_width(Some(40))
        .with_viewport_height(Some(12));

    assert!(config.show_line_numbers);
    assert!(!config.show_help_bar);
    assert!(!config.show_scrollbar);
    assert_eq!(config.viewport_width, Some(40));
    assert_eq!(config.viewport_height, Some(12));

    let viewer = TxtView::new("x").with_config(config);
    let c = viewer.config();
    assert!(c.show_line_numbers);
    assert!(!c.show_help_bar);
    assert!(!c.show_scrollbar);
    assert_eq!(c.viewport_width, Some(40));
    assert_eq!(c.viewport_height, Some(12));
}

#[test]
fn setters_only_touch_their_own_field() {
    let config = TxtViewConfig::default().with_show_help_bar(false);
    assert!(!config.show_line_numbers);
    assert!(!config.show_help_bar);
    assert!(config.show_scrollbar);
    assert_eq!(config.viewport_width, None);
    assert_eq!(config.viewport_height, None);
}

#[test]
fn with_config_does_not_change_line_count() {
    let viewer =
        TxtView::new("a\nb\nc").with_config(TxtViewConfig::default().with_show_line_numbers(true));
    assert_eq!(viewer.line_count(), 3);
}

#[test]
fn config_can_be_built_with_struct_update_syntax() {
    let config = TxtViewConfig {
        show_line_numbers: true,
        ..TxtViewConfig::default()
    };
    assert!(config.show_line_numbers);
    assert!(config.show_help_bar);
    assert!(config.show_scrollbar);
    assert_eq!(config.viewport_width, None);
    assert_eq!(config.viewport_height, None);
}

#[test]
fn config_clone_copies_every_field() {
    let original = TxtViewConfig::default()
        .with_show_line_numbers(true)
        .with_show_help_bar(false)
        .with_show_scrollbar(false)
        .with_viewport_width(Some(40))
        .with_viewport_height(Some(12));
    let cloned = original.clone();
    assert!(cloned.show_line_numbers);
    assert!(!cloned.show_help_bar);
    assert!(!cloned.show_scrollbar);
    assert_eq!(cloned.viewport_width, Some(40));
    assert_eq!(cloned.viewport_height, Some(12));
    assert!(original.show_line_numbers);
}

#[test]
fn config_debug_lists_the_configuration_fields() {
    let config = TxtViewConfig::default()
        .with_show_line_numbers(true)
        .with_viewport_width(Some(40));
    let out = format!("{config:?}");
    assert!(out.contains("show_line_numbers"), "missing field: {out}");
    assert!(out.contains("show_help_bar"), "missing field: {out}");
    assert!(out.contains("show_scrollbar"), "missing field: {out}");
    assert!(out.contains("viewport_width"), "missing field: {out}");
    assert!(out.contains("viewport_height"), "missing field: {out}");
}
