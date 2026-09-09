/// Options for a [`TxtView`](crate::TxtView).
///
/// Set only the fields you care about and fill the rest from the defaults:
///
/// ```rust
/// use txtview::TxtViewConfig;
///
/// let config = TxtViewConfig {
///     show_line_numbers: true,
///     ..TxtViewConfig::default()
/// };
/// ```
///
/// Or chain the `with_*` setters, which stay compatible as new options are
/// added:
///
/// ```rust
/// use txtview::TxtViewConfig;
///
/// let config = TxtViewConfig::default()
///     .with_show_line_numbers(true)
///     .with_show_scrollbar(false);
/// ```
/// New options arrive in minor releases (`0.x.0`) and always default to the
/// previous behavior, so an existing viewer only changes look when you opt
/// in. Patches (`0.0.x`) ship only bug fixes and purely additive methods.
#[derive(Debug, Clone)]
pub struct TxtViewConfig {
    /// Show a line-number column on the left side of the viewport.
    pub show_line_numbers: bool,
    /// Show the keybinding help bar at the bottom of the viewport.
    pub show_help_bar: bool,
    /// Show an interactive vertical scrollbar on the right edge. It can be
    /// clicked to jump to a position or dragged to scroll.
    pub show_scrollbar: bool,
    /// Fixed viewport width in columns. `None` uses the terminal width.
    pub viewport_width: Option<u16>,
    /// Fixed viewport height in rows. `None` uses the terminal height.
    pub viewport_height: Option<u16>,
}

impl Default for TxtViewConfig {
    fn default() -> Self {
        TxtViewConfig {
            show_line_numbers: false,
            show_help_bar: true,
            show_scrollbar: true,
            viewport_width: None,
            viewport_height: None,
        }
    }
}

impl TxtViewConfig {
    /// Set whether line numbers are shown.
    pub fn with_show_line_numbers(mut self, show: bool) -> Self {
        self.show_line_numbers = show;
        self
    }

    /// Set whether the keybinding help bar is shown.
    pub fn with_show_help_bar(mut self, show: bool) -> Self {
        self.show_help_bar = show;
        self
    }

    /// Set whether the interactive scrollbar is shown.
    pub fn with_show_scrollbar(mut self, show: bool) -> Self {
        self.show_scrollbar = show;
        self
    }

    /// Set a fixed viewport width in columns. `None` uses the terminal width.
    pub fn with_viewport_width(mut self, width: Option<u16>) -> Self {
        self.viewport_width = width;
        self
    }

    /// Set a fixed viewport height in rows. `None` uses the terminal height.
    pub fn with_viewport_height(mut self, height: Option<u16>) -> Self {
        self.viewport_height = height;
        self
    }
}
