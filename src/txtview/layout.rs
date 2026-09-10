use crossterm::terminal;
use unicode_width::UnicodeWidthChar;

use super::TxtView;

pub(super) struct ScrollGeometry {
    pub column: u16,
    pub top: usize,
    pub size: usize,
    pub visible: usize,
}

impl TxtView {
    fn term_size() -> (u16, u16) {
        terminal::size().unwrap_or((80, 24))
    }

    pub(super) fn help_wrap_cols(&self) -> usize {
        match self.config.viewport_width {
            Some(w) => w as usize,
            None => Self::term_size().0 as usize,
        }
        .max(1)
    }

    pub(super) fn help_text() -> String {
        "q: quit | ↑/↓, j/k, Mouse: scroll | PgUp/PgDn: page | Home/End, g/G: start/end".to_string()
    }

    fn help_height(&self) -> usize {
        if !self.config.show_help_bar {
            return 0;
        }
        let lines = Self::help_text()
            .chars()
            .count()
            .div_ceil(self.help_wrap_cols())
            .max(1);
        1 + lines
    }

    pub(super) fn visible_rows(&self) -> u16 {
        let h = match self.config.viewport_height {
            Some(h) => h,
            None => Self::term_size().1,
        };
        h.saturating_sub(self.help_height() as u16)
    }

    fn layout_cols(&self) -> usize {
        match self.config.viewport_width {
            Some(w) => w as usize,
            None => Self::term_size().0 as usize,
        }
    }

    fn rebuild_display(&mut self) {
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

        let mut display = Vec::new();

        for (i, line) in self.lines.iter().enumerate() {
            if line.is_empty() {
                display.push(self.line_prefix(i, 0));
            } else {
                let chunks = wrap_line_ansi(line, avail);
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

fn wrap_line_ansi(line: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let bytes = line.as_bytes();
    let mut chunks = Vec::new();
    let mut current_chunk = String::new();
    let mut visible_width = 0;
    let mut ansi_state = String::new();

    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b {
            let seq_start = i;
            i += 1;
            if i < bytes.len() && bytes[i] == b'[' {
                i += 1;
                while i < bytes.len() && !bytes[i].is_ascii_alphabetic() {
                    i += 1;
                }
                if i < bytes.len() {
                    i += 1;
                }
            }
            let seq = &line[seq_start..i];
            current_chunk.push_str(seq);
            if seq.ends_with('m') {
                if seq == "\x1b[0m" {
                    ansi_state.clear();
                } else {
                    ansi_state.push_str(seq);
                }
            }
        } else {
            let ch_start = i;
            i += 1;
            while i < bytes.len() && (bytes[i] & 0xC0) == 0x80 {
                i += 1;
            }
            let ch = &line[ch_start..i];
            let ch_width = ch.chars().next().map_or(1, |c| c.width().unwrap_or(1));
            if visible_width + ch_width > width && !current_chunk.is_empty() {
                if !ansi_state.is_empty() {
                    current_chunk.push_str("\x1b[0m");
                }
                chunks.push(current_chunk);
                current_chunk = String::new();
                visible_width = 0;
                if !ansi_state.is_empty() {
                    current_chunk.push_str(&ansi_state);
                }
            }
            current_chunk.push_str(ch);
            visible_width += ch_width;
        }
    }

    if !current_chunk.is_empty() {
        if !ansi_state.is_empty() {
            current_chunk.push_str("\x1b[0m");
        }
        chunks.push(current_chunk);
    }

    if chunks.is_empty() {
        chunks.push(String::new());
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_single_style_fits_one_row() {
        let line = "\x1b[31mhello\x1b[0m";
        let chunks = wrap_line_ansi(line, 10);
        assert_eq!(chunks, vec!["\x1b[31mhello\x1b[0m"]);
    }

    #[test]
    fn ansi_wraps_without_splitting_codes() {
        let line = "\x1b[31m12345\x1b[0m";
        let chunks = wrap_line_ansi(line, 3);
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].contains("\x1b[31m"));
        assert!(chunks[0].contains("123"));
        assert!(chunks[0].ends_with("\x1b[0m"));
        assert!(chunks[1].contains("\x1b[31m"));
        assert!(chunks[1].contains("45"));
        assert!(chunks[1].ends_with("\x1b[0m"));
    }

    #[test]
    fn ansi_state_cleared_by_reset() {
        let line = "\x1b[31mabc\x1b[0mdefghi";
        let chunks = wrap_line_ansi(line, 3);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], "\x1b[31mabc\x1b[0m");
        assert_eq!(chunks[1], "def");
        assert_eq!(chunks[2], "ghi");
    }

    #[test]
    fn plain_text_unchanged() {
        let line = "hello world";
        let chunks = wrap_line_ansi(line, 5);
        assert_eq!(chunks, vec!["hello", " worl", "d"]);
    }

    #[test]
    fn empty_line_returns_empty_string() {
        let chunks = wrap_line_ansi("", 10);
        assert_eq!(chunks, vec![""]);
    }

    #[test]
    fn cjk_chars_wrap_by_visual_width() {
        let chunks = wrap_line_ansi("一二三四五六七八九十", 10);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], "一二三四五");
        assert_eq!(chunks[1], "六七八九十");
    }

    #[test]
    fn mixed_ascii_cjk_wrap_correctly() {
        let chunks = wrap_line_ansi("A中B日C", 5);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], "A中B");
        assert_eq!(chunks[1], "日C");
    }
}
