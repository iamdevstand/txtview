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

        self.render_status_bar(stdout, rows)?;

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

    fn render_status_bar(&mut self, stdout: &mut impl io::Write, rows: u16) -> io::Result<()> {
        if !self.config.status_bar_visible {
            return Ok(());
        }

        let cols = self.status_wrap_cols();
        let lines = wrap_lines(&self.status_text(), cols);
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
