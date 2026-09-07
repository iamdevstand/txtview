use std::io;

use crossterm::{
    cursor::{Hide, Show},
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseEventKind,
    },
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};

use super::TxtView;

impl TxtView {
    pub fn run(&mut self) -> io::Result<()> {
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();

        execute!(stdout, EnterAlternateScreen, Hide, EnableMouseCapture)?;

        let result = self.event_loop(&mut stdout);

        execute!(stdout, DisableMouseCapture, LeaveAlternateScreen, Show)?;
        terminal::disable_raw_mode()?;

        result
    }

    fn event_loop(&mut self, stdout: &mut io::Stdout) -> io::Result<()> {
        self.draw(stdout)?;

        loop {
            match event::read()? {
                Event::Key(key) => match (key.code, key.modifiers) {
                    (KeyCode::Char('q'), _)
                    | (KeyCode::Char('c'), KeyModifiers::CONTROL)
                    | (KeyCode::Esc, _) => {
                        break;
                    }
                    (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
                        let delta = self.scroll_down(1);
                        self.apply_scroll(stdout, delta)?;
                    }
                    (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
                        let delta = self.scroll_up(1);
                        self.apply_scroll(stdout, delta)?;
                    }
                    (KeyCode::PageDown, _) => {
                        let delta = self.scroll_down(self.visible_rows() as usize);
                        self.apply_scroll(stdout, delta)?;
                    }
                    (KeyCode::PageUp, _) => {
                        let delta = self.scroll_up(self.visible_rows() as usize);
                        self.apply_scroll(stdout, delta)?;
                    }
                    (KeyCode::Home, _) | (KeyCode::Char('g'), KeyModifiers::NONE) => {
                        self.jump_to_start();
                        self.draw(stdout)?;
                    }
                    (KeyCode::End, _) | (KeyCode::Char('h'), KeyModifiers::NONE) => {
                        self.jump_to_end();
                        self.draw(stdout)?;
                    }
                    _ => {}
                },
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        let delta = self.scroll_up(3);
                        self.apply_scroll(stdout, delta)?;
                    }
                    MouseEventKind::ScrollDown => {
                        let delta = self.scroll_down(3);
                        self.apply_scroll(stdout, delta)?;
                    }
                    _ => {}
                },
                Event::Resize(_cols, _rows) => {
                    self.draw(stdout)?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn apply_scroll(&mut self, stdout: &mut io::Stdout, delta: isize) -> io::Result<()> {
        if delta == 0 {
            return Ok(());
        }
        if delta.unsigned_abs() > self.visible_rows() as usize {
            self.draw(stdout)
        } else {
            self.draw_scroll(stdout, delta)
        }
    }
}
