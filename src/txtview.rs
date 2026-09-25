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
/// line wraps. Control bytes and other escape sequences (tabs are expanded
/// to spaces at their column stops) are shown as visible caret notation.
/// See [`TxtViewConfig`] for the available display options.
#[derive(Clone)]
pub struct TxtView {
    /// The original input text, kept so lines can be borrowed from one buffer.
    text: String,
    /// Byte offsets of each logical line's start, indexed per `str::lines()`.
    line_starts: Vec<usize>,
    /// The wrapped display rows currently laid out.
    display: Vec<String>,
    /// The first display row shown, the scroll offset.
    offset: usize,
    /// The largest usable `offset` for the current layout.
    max_offset: usize,
    /// The display options in effect.
    config: TxtViewConfig,
    /// Where a live scrollbar drag grabbed the thumb, `Some` while dragging.
    drag_grab_offset: Option<usize>,
    /// Whether the document qualifies for a scrollbar: set from
    /// `config.show_scrollbar` and kept only while the document overflows
    /// the viewport and the track can host a moving thumb. The column is
    /// reserved only when this is also affordable, so `scrollbar_active`
    /// can be true without a column being reserved.
    scrollbar_active: bool,
    /// The geometry the `display` rows were wrapped for, `None` before the
    /// first layout.
    display_geometry: Option<LayoutGeometry>,
}

impl fmt::Debug for TxtView {
    /// Describe the viewer without dumping its contents: the document can be
    /// huge, and the wrapped `display` rows mirror it. Counts, the scroll
    /// state, the scrollbar activity and the configuration are shown.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TxtView")
            .field("line_count", &self.line_starts.len())
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
        let text = input.as_ref().to_owned();
        let line_starts = index_lines(&text);
        let mut view = TxtView {
            text,
            line_starts,
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

    /// Apply configuration and reconcile the display layout (re-wrapping
    /// only when the geometry or overflow state changes).
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
        self.line_starts.len()
    }

    /// The `i`-th line as a borrowed slice of the single text buffer.
    ///
    /// The slice is the content between line terminators: without the final
    /// newline and without a `\r` that introduces it, so CRLF input renders
    /// identically to how `str::lines()` split it. There is no per-line
    /// allocation, callers that need one can copy the slice themselves.
    fn line(&self, i: usize) -> &str {
        let start = self.line_starts[i];
        let raw_end = self
            .line_starts
            .get(i + 1)
            .copied()
            .unwrap_or(self.text.len());
        let bytes = self.text.as_bytes();
        let mut end = raw_end;
        if end > start && bytes[end - 1] == b'\n' {
            end -= 1;
            if end > start && bytes[end - 1] == b'\r' {
                end -= 1;
            }
        }
        &self.text[start..end]
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

/// Byte offsets of each line's content, matching `str::lines()`.
///
/// Each `\n` ends the line that precedes it, a segment after the last newline
/// is a line only when the text does not end with one (the trailing
/// terminator is final, `""` has no lines at all). The `\n` and any `\r`
/// before it stay outside the line and are stripped by [`TxtView::line`].
fn index_lines(text: &str) -> Vec<usize> {
    let bytes = text.as_bytes();
    let mut starts = Vec::new();
    let mut start = 0;
    for (i, &byte) in bytes.iter().enumerate() {
        if byte == b'\n' {
            starts.push(start);
            start = i + 1;
        }
    }
    if !bytes.is_empty() && bytes[bytes.len() - 1] != b'\n' {
        starts.push(start);
    }
    starts
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

    #[test]
    fn line_index_matches_str_lines() {
        let cases = [
            ("", vec![] as Vec<&str>),
            ("a", vec!["a"]),
            ("a\nb", vec!["a", "b"]),
            ("a\n", vec!["a"]),
            ("a\n\n", vec!["a", ""]),
            ("\n", vec![""]),
            ("\n\n", vec!["", ""]),
            ("a\n\nb", vec!["a", "", "b"]),
            ("a\r\nb", vec!["a", "b"]),
            ("a\rb", vec!["a\rb"]),
            ("a\r", vec!["a\r"]),
            ("\r\n", vec![""]),
            ("\r\r\n", vec!["\r"]),
            ("a\r\n\r\nb", vec!["a", "", "b"]),
            ("a\r\nb\n", vec!["a", "b"]),
        ];
        for (input, expected) in cases {
            let v = TxtView::new(input);
            let got: Vec<&str> = (0..v.line_count()).map(|i| v.line(i)).collect();
            assert_eq!(got, expected, "indexed lines for {input:?}");
            let reference: Vec<&str> = input.lines().collect();
            assert_eq!(got, reference, "must equal str::lines for {input:?}");
        }
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
