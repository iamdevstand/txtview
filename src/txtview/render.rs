use std::io::{self, Write};

use crossterm::{
    cursor::MoveTo,
    execute,
    terminal::{self, Clear, ClearType},
};

use super::TxtView;

impl TxtView {
    pub(super) fn draw(&mut self, stdout: &mut io::Stdout) -> io::Result<()> {
        let (cols, rows) = terminal::size()?;
        self.refresh_bounds();

        execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;

        let visible = self.visible_rows() as usize;
        let end = (self.offset + visible).min(self.display.len());

        for (i, text) in self.display[self.offset..end].iter().enumerate() {
            execute!(stdout, MoveTo(0, i as u16))?;
            write!(stdout, "{}", text)?;
        }

        if self.config.status_bar_visible {
            let status = format!(
                "q: quit | ↑/↓, j/k: scroll | PgUp/PgDn: page | g/h: start/end | Mouse: scroll  [{}/{}]",
                self.current_line_index() + 1,
                self.lines.len()
            );

            execute!(
                stdout,
                MoveTo(0, rows.saturating_sub(2)),
                Clear(ClearType::CurrentLine)
            )?;
            write!(stdout, "{}", "─".repeat(cols as usize))?;

            execute!(
                stdout,
                MoveTo(0, rows.saturating_sub(1)),
                Clear(ClearType::CurrentLine)
            )?;
            write!(stdout, "{}", status)?;
        }

        stdout.flush()?;
        Ok(())
    }
}
