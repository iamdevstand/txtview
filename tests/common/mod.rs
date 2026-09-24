use txtview::TxtViewConfig;

pub fn assert_default_config(config: &TxtViewConfig) {
    assert!(!config.show_line_numbers, "line numbers must default off");
    assert!(config.show_help_bar, "help bar must default on");
    assert!(config.show_scrollbar, "scrollbar must default on");
    assert_eq!(
        config.viewport_width, None,
        "viewport width must default to auto"
    );
    assert_eq!(
        config.viewport_height, None,
        "viewport height must default to auto"
    );
}
