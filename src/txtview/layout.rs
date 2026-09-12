//! Display layout: terminal/viewport geometry, scrolling bounds and the
//! rebuild of the wrapped `display` rows.

use crossterm::terminal;

use super::TxtView;
use super::wrap::wrap_line_ansi;

pub(super) struct ScrollGeometry {
    pub column: u16,
    pub top: usize,
    pub size: usize,
    pub visible: usize,
}

impl TxtView {
    pub(super) fn term_size() -> (u16, u16) {
        terminal::size().unwrap_or((80, 24))
    }

    fn resolved_width(&self) -> usize {
        match self.config.viewport_width {
            Some(w) => usize::from(w),
            None => usize::from(Self::term_size().0),
        }
    }

    pub(super) fn resolved_height(&self) -> u16 {
        let rows = Self::term_size().1;
        match self.config.viewport_height {
            Some(h) => h.min(rows),
            None => rows,
        }
    }

    pub(super) fn help_wrap_cols(&self) -> usize {
        self.resolved_width().max(1)
    }

    pub(super) fn help_text() -> String {
        "q: quit | ↑/↓, j/k, Mouse: scroll | PgUp/PgDn: page | Home/End, g/G: start/end".to_string()
    }

    fn help_height(&self) -> usize {
        if !self.config.show_help_bar {
            return 0;
        }
        let total = usize::from(self.resolved_height());
        if total < 2 {
            return 0;
        }
        let lines = Self::help_text()
            .chars()
            .count()
            .div_ceil(self.help_wrap_cols())
            .max(1)
            .min(total - 2);
        1 + lines
    }

    pub(super) fn visible_rows(&self) -> u16 {
        self.resolved_height()
            .saturating_sub(u16::try_from(self.help_height()).unwrap_or(u16::MAX))
            .max(1)
    }

    fn layout_cols(&self) -> usize {
        self.resolved_width()
    }

    fn layout_geometry(&self) -> (usize, usize) {
        let cols = self.layout_cols().max(1);
        let scrollbar_width = if self.config.show_scrollbar { 1 } else { 0 };
        let prefix_width = if self.config.show_line_numbers {
            self.lines.len().to_string().len() + 3
        } else {
            0
        };
        let avail = cols
            .saturating_sub(prefix_width)
            .saturating_sub(scrollbar_width)
            .max(1);
        (avail, prefix_width)
    }

    fn rebuild_display(&mut self) {
        #[cfg(test)]
        {
            self.rebuild_count += 1;
        }
        let (avail, prefix_width) = self.layout_geometry();
        self.display_geometry = Some((avail, prefix_width));

        let mut display = Vec::new();

        for (i, line) in self.lines.iter().enumerate() {
            if line.is_empty() {
                display.push(self.line_prefix(i, 0));
            } else {
                let chunks = wrap_line_ansi(line, avail, prefix_width);
                for (ci, chunk) in chunks.iter().enumerate() {
                    let mut row = self.line_prefix(i, ci);
                    row.push_str(chunk);
                    display.push(row);
                }
            }
        }

        self.display = display;
    }

    fn line_prefix(&self, line_index: usize, chunk_index: usize) -> String {
        if !self.config.show_line_numbers {
            return String::new();
        }
        let width = self.lines.len().to_string().len();
        let label = if chunk_index == 0 {
            (line_index + 1).to_string()
        } else {
            "↳".to_string()
        };
        format!("{:>width$} │ ", label, width = width)
    }

    pub(super) fn refresh_bounds(&mut self) {
        if self.display_geometry != Some(self.layout_geometry()) {
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

    pub(super) fn scroll_geometry(&self, visible: usize) -> Option<ScrollGeometry> {
        if !self.config.show_scrollbar || self.max_offset == 0 || visible == 0 {
            return None;
        }
        let total = self.display.len().max(1);
        let thumb = (visible * visible / total).max(1).min(visible);
        let top = (self.offset * (visible - thumb) / self.max_offset).min(visible - thumb);
        Some(ScrollGeometry {
            column: u16::try_from(self.help_wrap_cols().saturating_sub(1)).unwrap_or(u16::MAX),
            top,
            size: thumb,
            visible,
        })
    }

    /// Map a thumb-top row (0..visible) to a scroll offset using the same
    /// proportion that [`TxtView::scroll_geometry`] uses in reverse.
    pub(super) fn offset_from_thumb_top(&self, top: i64, g: &ScrollGeometry) -> usize {
        let travel = g.visible.saturating_sub(g.size).max(1);
        let clamped = usize::try_from(top).unwrap_or(0).min(travel);
        clamped.saturating_mul(self.max_offset) / travel
    }
}
