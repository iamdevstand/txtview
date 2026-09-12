mod ansi;
mod layout;
#[cfg(test)]
mod layout_tests;
mod navigation;
mod render;
mod run;
mod sgr;
mod wrap;

use crate::TxtViewConfig;

/// A terminal text viewer.
///
/// Wrap any text with [`TxtView::new`], optionally tweak it with
/// [`TxtView::with_config`], and show it with [`TxtView::run`]:
///
/// ```
/// use txtview::{TxtView, TxtViewConfig};
///
/// let mut viewer = TxtView::new("hello world")
///     .with_config(TxtViewConfig::default());
/// ```
///
/// ANSI styling is preserved: SGR color, style codes and OSC8 hyperlinks
/// pass through, and the active style is re-emitted compactly when a styled
/// line wraps. Control bytes and other escape sequences are shown as visible
/// caret notation. See [`TxtViewConfig`] for the available display options.
pub struct TxtView {
    lines: Vec<String>,
    display: Vec<String>,
    offset: usize,
    max_offset: usize,
    config: TxtViewConfig,
    dragging: bool,
    drag_grab_offset: usize,
    display_geometry: Option<(usize, usize)>,
    #[cfg(test)]
    rebuild_count: usize,
}

impl TxtView {
    /// Create a viewer for the given text.
    ///
    /// The text is split into lines. Blank lines are preserved and a single
    /// trailing newline is ignored. Both borrowed and owned text are accepted:
    ///
    /// ```
    /// use txtview::TxtView;
    ///
    /// let viewer = TxtView::new("line one\nline two");
    /// assert_eq!(viewer.line_count(), 2);
    ///
    /// let owned = String::from("line one\nline two");
    /// let viewer = TxtView::new(owned);
    /// assert_eq!(viewer.line_count(), 2);
    /// ```
    pub fn new(input: impl AsRef<str>) -> Self {
        let lines: Vec<String> = input.as_ref().lines().map(String::from).collect();
        let mut view = TxtView {
            lines,
            display: Vec::new(),
            offset: 0,
            max_offset: 0,
            config: TxtViewConfig::default(),
            dragging: false,
            drag_grab_offset: 0,
            display_geometry: None,
            #[cfg(test)]
            rebuild_count: 0,
        };
        view.refresh_bounds();
        view
    }

    /// Apply configuration and rebuild the display layout.
    ///
    /// Consumes the viewer and returns it so callers can chain:
    ///
    /// ```
    /// use txtview::{TxtView, TxtViewConfig};
    ///
    /// let viewer = TxtView::new("hello")
    ///     .with_config(TxtViewConfig {
    ///         show_line_numbers: true,
    ///         ..TxtViewConfig::default()
    ///     });
    /// ```
    #[must_use]
    pub fn with_config(mut self, config: TxtViewConfig) -> Self {
        self.config = config;
        self.refresh_bounds();
        self
    }

    /// The number of logical lines in the input.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
}
