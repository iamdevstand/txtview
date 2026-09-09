mod layout;
mod navigation;
mod render;
mod run;

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
/// The content is written verbatim, so ANSI-styled text (for example built
/// with crossterm's `style`) is rendered as-is. See [`TxtViewConfig`] for
/// the available display options.
pub struct TxtView {
    lines: Vec<String>,
    display: Vec<String>,
    rows_per_line: Vec<usize>,
    offset: usize,
    max_offset: usize,
    config: TxtViewConfig,
}

impl TxtView {
    /// Create a viewer for the given text.
    ///
    /// The text is split into lines. Blank lines are preserved and a single
    /// trailing newline is ignored.
    ///
    /// ```
    /// use txtview::TxtView;
    ///
    /// let viewer = TxtView::new("line one\nline two");
    /// assert_eq!(viewer.line_count(), 2);
    /// ```
    pub fn new(input: &str) -> Self {
        let lines: Vec<String> = input.lines().map(String::from).collect();
        let mut view = TxtView {
            rows_per_line: vec![1; lines.len()],
            lines,
            display: Vec::new(),
            offset: 0,
            max_offset: 0,
            config: TxtViewConfig::default(),
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
