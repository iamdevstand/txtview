mod layout;
#[cfg(test)]
mod layout_tests;
mod navigation;
mod render;
mod run;

use std::fmt;

use crate::TxtViewConfig;

use layout::LayoutGeometry;

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
#[derive(Clone)]
pub struct TxtView {
    lines: Vec<String>,
    display: Vec<String>,
    offset: usize,
    max_offset: usize,
    config: TxtViewConfig,
    drag_grab_offset: Option<usize>,
    scrollbar_active: bool,
    display_geometry: Option<LayoutGeometry>,
}

impl fmt::Debug for TxtView {
    /// Describe the viewer without dumping its contents: the document can be
    /// huge, and the wrapped `display` rows mirror it. Only counts and the
    /// scroll state are shown.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TxtView")
            .field("line_count", &self.lines.len())
            .field("display_rows", &self.display.len())
            .field("offset", &self.offset)
            .field("max_offset", &self.max_offset)
            .field("scrollbar_active", &self.scrollbar_active)
            .field("config", &self.config)
            .finish()
    }
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
            drag_grab_offset: None,
            scrollbar_active: false,
            display_geometry: None,
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

    /// Returns a reference to the current configuration of this [`TxtView`].
    ///
    /// Use it to inspect the active settings or to build a modified config
    /// and apply it via [`TxtView::with_config`].
    ///
    /// ```
    /// use txtview::{TxtView, TxtViewConfig};
    ///
    /// let viewer = TxtView::new("hello");
    /// let config = viewer.config();
    /// assert!(!config.show_line_numbers);
    /// ```
    pub fn config(&self) -> &TxtViewConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_reports_counts_instead_of_the_document() {
        let viewer = TxtView::new("alpha\nbeta\ngamma");
        let out = format!("{viewer:?}");
        assert!(
            out.contains("line_count: 3"),
            "must report the line count: {out}"
        );
        assert!(
            out.contains("display_rows:"),
            "must report the wrapped rows: {out}"
        );
        assert!(!out.contains("alpha"), "must not dump the document: {out}");
    }
}

#[cfg(test)]
mod test_metrics {
    //! Counting of display rebuilds and wrap passes. Test-only, so the
    //! production struct carries no instrumentation. Each test gets its own
    //! thread, so a thread-local counter is per-test: exact equality
    //! assertions cannot observe other tests rebuilding.

    use std::cell::Cell;

    thread_local! {
        static REBUILDS: Cell<usize> = const { Cell::new(0) };
        static WRAPS: Cell<usize> = const { Cell::new(0) };
    }

    pub(crate) fn reset() {
        REBUILDS.with(|c| c.set(0));
        WRAPS.with(|c| c.set(0));
    }

    pub(crate) fn bump_rebuild() {
        REBUILDS.with(|c| c.set(c.get() + 1));
    }

    pub(crate) fn bump_wrap() {
        WRAPS.with(|c| c.set(c.get() + 1));
    }

    pub(crate) fn rebuilds() -> usize {
        REBUILDS.with(Cell::get)
    }

    pub(crate) fn wraps() -> usize {
        WRAPS.with(Cell::get)
    }
}
