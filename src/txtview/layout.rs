//! Display layout: terminal/viewport geometry, scrolling bounds and the
//! rebuild of the wrapped `display` rows.

use crossterm::terminal;

use super::TxtView;
use crate::components::HelpBar;
use crate::components::scrollbar::ScrollGeometry;
use crate::text::wrap_line_ansi;

/// The geometry the wrapped display is produced from: the columns available
/// to the text and the width of the line-number prefix. Carried as a named
/// struct so the two values cannot be swapped in a call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct LayoutGeometry {
    avail: usize,
    prefix_width: usize,
}

impl TxtView {
    pub(super) fn term_size() -> (u16, u16) {
        terminal::size().unwrap_or((80, 24))
    }

    /// The viewport width, clamped to the terminal the way the height is, so
    /// a configured width wider than the terminal cannot push writes past its
    /// edge. A fixed width of 0 is lifted to 1, so `Some(0)` shows one column
    /// instead of silently rendering nothing.
    fn resolved_width(&self) -> u16 {
        let cols = Self::term_size().0;
        match self.config.viewport_width {
            Some(w) => w.max(1).min(cols),
            None => cols,
        }
    }

    pub(super) fn resolved_height(&self) -> u16 {
        let rows = Self::term_size().1;
        match self.config.viewport_height {
            Some(h) => h.max(1).min(rows),
            None => rows,
        }
    }

    pub(super) fn help_text() -> String {
        "q: quit | ↑/↓, j/k, Mouse: scroll | PgUp/PgDn: page | Home/End, g/G: start/end".to_string()
    }

    /// The wrapped rows of the help text, produced the same way the content
    /// rows are: one `wrap_line_ansi` over the viewport width, so the help
    /// bar and the content share a single wrapping routine and geometry. The
    /// bar spans the full viewport width (its row runs under the scrollbar's
    /// column too), so it wraps at `content_cols`, not at the content's
    /// narrower `avail`.
    pub(super) fn help_rows(&self) -> Vec<String> {
        wrap_line_ansi(
            &Self::help_text(),
            usize::from(self.content_cols()).max(1),
            0,
        )
    }

    fn help_height(&self) -> u16 {
        if !self.config.show_help_bar {
            return 0;
        }
        HelpBar::new(self.help_rows()).height(self.resolved_height())
    }

    /// The columns the viewport spans: the resolved width, before the
    /// scrollbar column is carved out of the content's text.
    pub(super) fn content_cols(&self) -> u16 {
        self.resolved_width()
    }

    /// The rows the content owns once the help bar takes its footprint.
    pub(super) fn visible_rows(&self) -> u16 {
        // `resolved_height` and `help_height` are both u16, so the whole
        // geometry here stays in u16 and can never need a truncating
        // conversion
        self.resolved_height()
            .saturating_sub(self.help_height())
            .max(1)
    }

    /// The width of the line-number prefix column, zero when line numbers
    /// are off.
    fn prefix_width(&self) -> usize {
        if self.config.show_line_numbers {
            self.line_count().to_string().len() + 3
        } else {
            0
        }
    }

    /// Whether the scrollbar column is reserved in the current geometry.
    ///
    /// A scrollbar only appears when it leaves the text at least three
    /// columns: on a one- or two-column viewport (or under a wide
    /// line-number prefix) the track would claim nearly every cell and
    /// leave Content nothing to paint, contradicting the canvas guarantee
    /// that content can never be squeezed away entirely.
    /// When this is false both the wrap geometry and the scrollbar
    /// placement drop the column, so they always agree.
    fn scrollbar_reserved(&self) -> bool {
        self.config.show_scrollbar
            && self.scrollbar_active
            && usize::from(self.content_cols()).saturating_sub(self.prefix_width()) >= 3
    }

    fn layout_geometry(&self) -> LayoutGeometry {
        let cols = usize::from(self.content_cols().max(1));
        let scrollbar_width = usize::from(self.scrollbar_reserved());
        let prefix_width = self.prefix_width();
        let avail = cols
            .saturating_sub(prefix_width)
            .saturating_sub(scrollbar_width)
            .max(1);
        LayoutGeometry {
            avail,
            prefix_width,
        }
    }

    fn build_display(&self, geometry: LayoutGeometry) -> Vec<String> {
        let mut display = Vec::new();

        for i in 0..self.line_count() {
            let line = self.line(i);
            if line.is_empty() {
                display.push(self.line_prefix(i, 0));
            } else {
                let chunks = wrap_line_ansi(line, geometry.avail, geometry.prefix_width);
                for (ci, chunk) in chunks.iter().enumerate() {
                    let mut row = self.line_prefix(i, ci);
                    row.push_str(chunk);
                    display.push(row);
                }
            }
        }

        display
    }

    /// Rewrap the document into `display`, deciding scrollbar presence from
    /// the wrap itself.
    ///
    /// Reserves the scrollbar column before wrapping, so an overflowing
    /// document (the common large-file case) is wrapped exactly once. The
    /// column is released, and the text re-wrapped at the full width, only
    /// when the wrap shows no bar is needed, a cheap pass since a fitting
    /// document is small. The final `display` always matches the final
    /// `scrollbar_active`, so the presence check needs no second wrap.
    fn rebuild_display(&mut self) {
        #[cfg(test)]
        {
            super::test_metrics::bump_rebuild();
        }
        self.scrollbar_active = self.config.show_scrollbar;
        let geometry = self.layout_geometry();
        self.display_geometry = Some(geometry);
        self.wrap_once(geometry);
        if !ScrollGeometry::room_to_travel(self.display.len(), usize::from(self.visible_rows())) {
            self.scrollbar_active = false;
            let geometry = self.layout_geometry();
            self.display_geometry = Some(geometry);
            self.wrap_once(geometry);
        }
    }

    /// Lay out the wrapped `display` rows from a geometry, counting each pass
    /// in tests so a regression test can pin the single-wrap overflow path.
    fn wrap_once(&mut self, geometry: LayoutGeometry) {
        #[cfg(test)]
        {
            super::test_metrics::bump_wrap();
        }
        self.display = self.build_display(geometry);
    }

    fn line_prefix(&self, line_index: usize, chunk_index: usize) -> String {
        if !self.config.show_line_numbers {
            return String::new();
        }
        let width = self.line_count().to_string().len();
        let label = if chunk_index == 0 {
            (line_index + 1).to_string()
        } else {
            "↳".to_string()
        };
        format!("{:>width$} │ ", label, width = width)
    }

    /// Reconcile the display model with the current configuration and
    /// viewport size.
    ///
    /// Rebuilds the wrapped `display` when the geometry changed (resize or
    /// config flip) or the overflow state drifted, then recomputes
    /// `max_offset` and clamps `offset`. All state, no terminal I/O, so it
    /// runs before composing a frame and can be skipped between mouse events
    /// as long as nothing layout-affecting changed since the last draw.
    pub(super) fn refresh_bounds(&mut self) {
        let overflows = self.config.show_scrollbar
            && ScrollGeometry::room_to_travel(self.display.len(), usize::from(self.visible_rows()));
        if overflows != self.scrollbar_active
            || self.display_geometry != Some(self.layout_geometry())
        {
            self.rebuild_display();
        }
        let vr = usize::from(self.visible_rows());
        self.max_offset = self.display.len().saturating_sub(vr);
        self.clamp_offset();
    }

    fn clamp_offset(&mut self) {
        if self.offset > self.max_offset {
            self.offset = self.max_offset;
        }
    }

    /// The thumb geometry for a scrollbar spanning the content rows along its
    /// axis.
    ///
    /// The thumb is capped so it always keeps
    /// [`MIN_TRAVEL`](ScrollGeometry::MIN_TRAVEL) cells of the track free to
    /// travel: a document that barely overflows yields a thumb that gives up
    /// a little proportional accuracy rather than a solid block with no room
    /// to show position. Both the forward mapping (offset to top) and the
    /// inverse (top to offset) round to the nearest cell, so the thumb sits
    /// centered on the true fraction instead of always under-stepping it.
    ///
    /// Whether a bar exists is decided by [`TxtView::scrollbar_reserved`]
    /// alone: the scrollbar is composed exactly when the flag is set and the
    /// text keeps at least three columns, and
    /// [`TxtView::refresh_bounds`] keeps the flag equal to "the document
    /// overflows the viewport AND the track can host a moving thumb", so the
    /// wrap-time decision and the canvas agree about the bar's presence. The
    /// guard is belt-and-braces against a transiently inconsistent frame: it
    /// returns `None`, and `compose` then draws no bar for one frame instead
    /// of faulting.
    pub(super) fn scroll_geometry(&self) -> Option<ScrollGeometry> {
        let visible = usize::from(self.visible_rows());
        if visible == 0 || !self.scrollbar_reserved() {
            return None;
        }
        debug_assert!(
            self.max_offset > 0,
            "a reserved bar must have a scroll range"
        );
        let travel = self.max_offset.max(1);
        let total = self.display.len().max(1);

        let max = visible
            .saturating_sub(ScrollGeometry::MIN_TRAVEL)
            .max(ScrollGeometry::MIN_THUMB);
        let size = (visible * visible / total)
            .max(ScrollGeometry::MIN_THUMB)
            .min(max);
        let thumb_travel = visible - size;
        // The offset is clamped to `max_offset`, which is `travel`, so the
        // rounded proportion can never land past `thumb_travel`: no extra
        // clamp is needed
        let top = self
            .offset
            .saturating_mul(thumb_travel)
            .saturating_add(travel / 2)
            / travel;
        Some(ScrollGeometry { top, size, visible })
    }
}
