use crossterm::terminal;

use super::TxtView;

impl TxtView {
    pub(super) fn visible_rows(&self) -> u16 {
        let h = match self.config.viewport_height {
            Some(h) => h,
            None => terminal::size().unwrap_or((80, 24)).1,
        };
        if self.config.status_bar_visible {
            h.saturating_sub(2)
        } else {
            h
        }
    }

    fn layout_cols(&self) -> usize {
        match self.config.viewport_width {
            Some(w) => w as usize,
            None => terminal::size().unwrap_or((80, 24)).0 as usize,
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
            let prefix = if self.config.show_line_numbers {
                format!("{:>width$} │ ", i + 1, width = prefix_width - 3)
            } else {
                String::new()
            };

            let chars: Vec<char> = line.chars().collect();
            let mut rows = 0usize;
            if chars.is_empty() {
                display.push(prefix);
                rows = 1;
            } else {
                for chunk in chars.chunks(avail) {
                    let mut row = prefix.clone();
                    row.extend(chunk.iter());
                    display.push(row);
                    rows += 1;
                }
            }
            rows_per_line.push(rows);
        }

        self.display = display;
        self.rows_per_line = rows_per_line;
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
}
