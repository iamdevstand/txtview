use std::io;

use crossterm::{
    cursor::MoveTo,
    queue,
    terminal::{self, Clear, ClearType},
};

use super::TxtView;

impl TxtView {
    pub(super) fn draw(&mut self, stdout: &mut impl io::Write) -> io::Result<()> {
        let rows = terminal::size()?.1;
        self.refresh_bounds();

        let visible = self.visible_rows() as usize;

        self.render_rows(stdout, 0..visible)?;
        self.render_scrollbar(stdout, visible)?;

        self.render_help_bar(stdout, rows)?;

        stdout.flush()?;
        Ok(())
    }

    fn render_rows(
        &mut self,
        stdout: &mut impl io::Write,
        range: std::ops::Range<usize>,
    ) -> io::Result<()> {
        for i in range {
            let text = self
                .display
                .get(self.offset + i)
                .map(String::as_str)
                .unwrap_or("");
            queue!(stdout, MoveTo(0, i as u16), Clear(ClearType::CurrentLine))?;
            write!(stdout, "{}", text)?;
        }
        Ok(())
    }

    fn render_scrollbar(&mut self, stdout: &mut impl io::Write, visible: usize) -> io::Result<()> {
        if !self.config.show_progress || self.max_offset == 0 || visible == 0 {
            return Ok(());
        }
        let cols = self.help_wrap_cols();
        let gutter = cols.saturating_sub(1) as u16;
        let total = self.display.len().max(1);
        let thumb = (visible * visible / total).max(1).min(visible);
        let top = (self.offset * (visible - thumb) / self.max_offset).min(visible - thumb);
        for i in 0..visible {
            let ch = if i >= top && i < top + thumb {
                '█'
            } else {
                '░'
            };
            queue!(stdout, MoveTo(gutter, i as u16))?;
            write!(stdout, "{}", ch)?;
        }
        Ok(())
    }

    fn render_help_bar(&mut self, stdout: &mut impl io::Write, rows: u16) -> io::Result<()> {
        if !self.config.show_help_bar {
            return Ok(());
        }

        let cols = self.help_wrap_cols();
        let lines = wrap_lines(&self.help_text(), cols);
        let reserve = 1 + lines.len();
        if (rows as usize) < reserve {
            return Ok(());
        }
        let base = rows as usize - reserve;

        queue!(
            stdout,
            MoveTo(0, base as u16),
            Clear(ClearType::CurrentLine)
        )?;
        write!(stdout, "{}", "─".repeat(cols))?;

        for (i, line) in lines.iter().enumerate() {
            queue!(
                stdout,
                MoveTo(0, (base + 1 + i) as u16),
                Clear(ClearType::CurrentLine)
            )?;
            write!(stdout, "{}", line)?;
        }

        Ok(())
    }
}

fn wrap_lines(s: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let chars: Vec<char> = s.chars().collect();
    if chars.is_empty() {
        return vec![String::new()];
    }
    chars
        .chunks(width)
        .map(|chunk| chunk.iter().collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::TxtView;
    use crate::TxtViewConfig;

    fn viewer(n: usize) -> TxtView {
        let text = (0..n)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let config = TxtViewConfig {
            viewport_height: Some(10),
            viewport_width: Some(80),
            show_help_bar: false,
            ..TxtViewConfig::default()
        };
        TxtView::new(&text).with_config(config)
    }

    #[test]
    fn scrollbar_emits_thumb_blocks() {
        let mut v = viewer(25);
        assert!(v.max_offset > 0);
        let mut out = Vec::new();
        v.render_scrollbar(&mut out, v.visible_rows() as usize)
            .unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains('█'), "expected a thumb in output: {s:?}");
    }

    #[test]
    fn scrollbar_skipped_when_everything_fits() {
        let mut v = viewer(5);
        assert_eq!(v.max_offset, 0);
        let mut out = Vec::new();
        v.render_scrollbar(&mut out, v.visible_rows() as usize)
            .unwrap();
        assert!(out.is_empty(), "expected no output: {out:?}");
    }
}
