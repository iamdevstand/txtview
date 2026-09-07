use std::io::{self, Write};

use crossterm::{
    cursor::MoveTo,
    queue,
    terminal::{self, Clear, ClearType, ScrollDown, ScrollUp},
};

use super::TxtView;

impl TxtView {
    pub(super) fn draw(&mut self, stdout: &mut io::Stdout) -> io::Result<()> {
        let (cols, rows) = terminal::size()?;
        self.refresh_bounds();

        let visible = self.visible_rows() as usize;
        let end = (self.offset + visible).min(self.display.len());

        for (i, text) in self.display[self.offset..end].iter().enumerate() {
            queue!(stdout, MoveTo(0, i as u16), Clear(ClearType::CurrentLine))?;
            write!(stdout, "{}", text)?;
        }

        self.render_status_bar(stdout, cols, rows)?;

        stdout.flush()?;
        Ok(())
    }

    pub(super) fn draw_scroll(&mut self, stdout: &mut io::Stdout, delta: isize) -> io::Result<()> {
        let (cols, rows) = terminal::size()?;
        self.refresh_bounds();

        let visible = self.visible_rows() as usize;

        if delta > 0 {
            let n = delta as usize;
            queue!(stdout, ScrollUp(delta as u16))?;
            self.render_rows(stdout, visible.saturating_sub(n)..visible)?;
        } else {
            let n = (-delta) as usize;
            queue!(stdout, ScrollDown(n as u16))?;
            self.render_rows(stdout, 0..n.min(visible))?;
        }

        self.render_status_bar(stdout, cols, rows)?;

        stdout.flush()?;
        Ok(())
    }

    fn render_rows(
        &mut self,
        stdout: &mut io::Stdout,
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

    fn render_status_bar(
        &mut self,
        stdout: &mut io::Stdout,
        cols: u16,
        rows: u16,
    ) -> io::Result<()> {
        if !self.config.status_bar_visible {
            return Ok(());
        }

        let status = format!(
            "q: quit | ↑/↓, j/k: scroll | PgUp/PgDn: page | g/h: start/end | Mouse: scroll  [{}/{}]",
            self.current_line_index() + 1,
            self.lines.len()
        );

        queue!(
            stdout,
            MoveTo(0, rows.saturating_sub(2)),
            Clear(ClearType::CurrentLine)
        )?;
        write!(stdout, "{}", "─".repeat(cols as usize))?;

        queue!(
            stdout,
            MoveTo(0, rows.saturating_sub(1)),
            Clear(ClearType::CurrentLine)
        )?;
        write!(stdout, "{}", status)?;

        Ok(())
    }
}
