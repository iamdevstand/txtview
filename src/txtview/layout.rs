use crossterm::terminal;

use super::TxtView;

pub(super) struct ScrollGeometry {
    pub column: u16,
    pub top: usize,
    pub size: usize,
    pub visible: usize,
}

impl TxtView {
    fn term_size(&self) -> (u16, u16) {
        terminal::size().unwrap_or((80, 24))
    }

    pub(super) fn help_wrap_cols(&self) -> usize {
        match self.config.viewport_width {
            Some(w) => w as usize,
            None => self.term_size().0 as usize,
        }
        .max(1)
    }

    pub(super) fn help_text(&self) -> String {
        "q: quit | ↑/↓, j/k, Mouse: scroll | PgUp/PgDn: page | Home/End, g/G: start/end".to_string()
    }

    fn help_height(&self) -> usize {
        if !self.config.show_help_bar {
            return 0;
        }
        let lines = self
            .help_text()
            .chars()
            .count()
            .div_ceil(self.help_wrap_cols())
            .max(1);
        1 + lines
    }

    pub(super) fn visible_rows(&self) -> u16 {
        let h = match self.config.viewport_height {
            Some(h) => h,
            None => self.term_size().1,
        };
        h.saturating_sub(self.help_height() as u16)
    }

    fn layout_cols(&self) -> usize {
        match self.config.viewport_width {
            Some(w) => w as usize,
            None => self.term_size().0 as usize,
        }
    }

    fn rebuild_display(&mut self) {
        let cols = self.layout_cols().max(1);
        let prefix_width = if self.config.show_line_numbers {
            self.lines.len().to_string().len() + 3
        } else {
            0
        };
        let avail = cols.saturating_sub(prefix_width).max(1);

        let mut display = Vec::new();
        let mut rows_per_line = Vec::with_capacity(self.lines.len());

        for (i, line) in self.lines.iter().enumerate() {
            let chars: Vec<char> = line.chars().collect();
            let mut rows = 0usize;
            if chars.is_empty() {
                display.push(self.line_prefix(i, 0));
                rows = 1;
            } else {
                for (ci, chunk) in chars.chunks(avail).enumerate() {
                    let mut row = self.line_prefix(i, ci);
                    row.extend(chunk.iter());
                    display.push(row);
                    rows += 1;
                }
            }
            rows_per_line.push(rows);
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
        self.rebuild_display();
        let vr = self.visible_rows() as usize;
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
            column: self.help_wrap_cols().saturating_sub(1) as u16,
            top,
            size: thumb,
            visible,
        })
    }

    /// Map a thumb-top row (0..visible) to a scroll offset using the same
    /// proportion that [`TxtView::scroll_geometry`] uses in reverse.
    pub(super) fn offset_from_thumb_top(&self, top: i64, g: &ScrollGeometry) -> usize {
        let travel = (g.visible as i64 - g.size as i64).max(1);
        let clamped = top.clamp(0, travel);
        (clamped * self.max_offset as i64 / travel) as usize
    }
}
