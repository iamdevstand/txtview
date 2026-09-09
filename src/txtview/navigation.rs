use super::TxtView;

impl TxtView {
    pub(crate) fn scroll_down(&mut self, amount: usize) -> isize {
        self.refresh_bounds();
        let old = self.offset as isize;
        self.offset = self.offset.saturating_add(amount).min(self.max_offset);
        self.offset as isize - old
    }

    pub(crate) fn scroll_up(&mut self, amount: usize) -> isize {
        self.refresh_bounds();
        let old = self.offset as isize;
        self.offset = self.offset.saturating_sub(amount);
        self.offset as isize - old
    }

    pub(crate) fn jump_to_start(&mut self) -> isize {
        self.refresh_bounds();
        let old = self.offset as isize;
        self.offset = 0;
        self.offset as isize - old
    }

    pub(crate) fn jump_to_end(&mut self) -> isize {
        self.refresh_bounds();
        let old = self.offset as isize;
        self.offset = self.max_offset;
        self.offset as isize - old
    }
}

#[cfg(test)]
mod tests {
    use super::super::TxtView;
    use crate::TxtViewConfig;

    fn text(n: usize) -> String {
        (0..n)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn viewer(input: &str, height: u16) -> TxtView {
        let config = TxtViewConfig {
            viewport_height: Some(height),
            viewport_width: Some(80),
            ..TxtViewConfig::default()
        };
        TxtView::new(input).with_config(config)
    }

    #[test]
    fn scroll_down_moves_offset() {
        let mut v = viewer(&text(25), 10);
        v.scroll_down(5);
        assert_eq!(v.offset(), 5);
    }

    #[test]
    fn scroll_down_clamps_at_end() {
        let mut v = viewer(&text(25), 10);
        v.scroll_down(100);
        assert_eq!(v.offset(), v.max_offset());
        assert_eq!(v.max_offset(), 17);
    }

    #[test]
    fn scroll_up_saturates_at_zero() {
        let mut v = viewer(&text(25), 10);
        v.scroll_down(5);
        v.scroll_up(100);
        assert_eq!(v.offset(), 0);
    }

    #[test]
    fn scroll_up_moves_offset_back() {
        let mut v = viewer(&text(25), 10);
        v.scroll_down(7);
        v.scroll_up(2);
        assert_eq!(v.offset(), 5);
    }

    #[test]
    fn jump_to_end_stops_at_max_offset() {
        let mut v = viewer(&text(25), 10);
        v.jump_to_end();
        assert_eq!(v.offset(), v.max_offset());
        assert_eq!(v.offset(), 17);
    }

    #[test]
    fn jump_to_start_resets_offset() {
        let mut v = viewer(&text(25), 10);
        v.scroll_down(6);
        v.jump_to_start();
        assert_eq!(v.offset(), 0);
    }

    #[test]
    fn small_file_cannot_scroll() {
        let mut v = viewer(&text(5), 10);
        assert_eq!(v.max_offset(), 0);
        v.scroll_down(2);
        assert_eq!(v.offset(), 0);
    }

    #[test]
    fn hidden_help_bar_increases_viewport() {
        let config = TxtViewConfig {
            show_help_bar: false,
            viewport_height: Some(10),
            viewport_width: Some(80),
            ..TxtViewConfig::default()
        };
        let mut v = TxtView::new(&text(25)).with_config(config);
        v.jump_to_end();
        assert_eq!(v.offset(), 15);
    }

    #[test]
    fn long_lines_wrap_into_multiple_rows() {
        let row = "12345678901234567890";
        let input = (0..5)
            .map(|_| row.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let config = TxtViewConfig {
            show_help_bar: false,
            viewport_height: Some(8),
            viewport_width: Some(10),
            ..TxtViewConfig::default()
        };
        let mut v = TxtView::new(&input).with_config(config);

        assert_eq!(v.line_count(), 5);
        assert_eq!(v.max_offset(), 2);

        v.scroll_down(1);
        assert_eq!(v.offset(), 1);
        v.scroll_down(4);
        assert_eq!(v.offset(), v.max_offset());
    }

    #[test]
    fn empty_lines_still_occupy_a_row() {
        let config = TxtViewConfig {
            show_help_bar: false,
            viewport_height: Some(1),
            viewport_width: Some(10),
            ..TxtViewConfig::default()
        };
        let mut v = TxtView::new("\n\n\n").with_config(config);
        assert_eq!(v.line_count(), 3);
        assert_eq!(v.max_offset(), 2);
        v.scroll_down(1);
        assert_eq!(v.offset(), 1);
    }

    #[test]
    fn line_number_prefix_counts_in_width() {
        let config = TxtViewConfig {
            show_line_numbers: true,
            show_help_bar: false,
            show_progress: false,
            viewport_height: Some(3),
            viewport_width: Some(10),
        };
        let input = "12345678901234567890\n12345678901234567890";
        let mut v = TxtView::new(input).with_config(config);

        assert_eq!(v.line_count(), 2);
        assert_eq!(v.max_offset(), 5);
        v.jump_to_end();
        assert_eq!(v.offset(), 5);
    }
}
