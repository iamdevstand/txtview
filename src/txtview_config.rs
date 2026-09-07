#[derive(Debug, Clone)]
pub struct TxtViewConfig {
    pub show_line_numbers: bool,
    pub status_bar_visible: bool,
    pub viewport_width: Option<u16>,
    pub viewport_height: Option<u16>,
}

impl Default for TxtViewConfig {
    fn default() -> Self {
        TxtViewConfig {
            show_line_numbers: false,
            status_bar_visible: true,
            viewport_width: None,
            viewport_height: None,
        }
    }
}
