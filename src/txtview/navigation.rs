use super::TxtView;

impl TxtView {
    pub fn scroll_down(&mut self, amount: usize) {
        self.refresh_bounds();
        self.offset = self.offset.saturating_add(amount).min(self.max_offset);
    }

    pub fn scroll_up(&mut self, amount: usize) {
        self.refresh_bounds();
        self.offset = self.offset.saturating_sub(amount);
    }

    pub fn jump_to_start(&mut self) {
        self.offset = 0;
    }

    pub fn jump_to_end(&mut self) {
        self.refresh_bounds();
        self.offset = self.max_offset;
    }

    pub(super) fn current_line_index(&self) -> usize {
        let mut acc = 0usize;
        for (i, n) in self.rows_per_line.iter().enumerate() {
            if self.offset < acc + n {
                return i;
            }
            acc += n;
        }
        self.lines.len().saturating_sub(1)
    }
}
