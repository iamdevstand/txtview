#[derive(Debug, Clone)]
pub struct TxtViewConfig {
    pub show_line_numbers: bool,
    pub show_help_bar: bool,
    pub show_progress: bool,
    pub viewport_width: Option<u16>,
    pub viewport_height: Option<u16>,
}

impl Default for TxtViewConfig {
    fn default() -> Self {
        TxtViewConfig {
            show_line_numbers: false,
            show_help_bar: true,
            show_progress: true,
            viewport_width: None,
            viewport_height: None,
        }
    }
}
