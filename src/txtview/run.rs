use std::io;
use std::time::{Duration, Instant};

use crossterm::{
    cursor::{Hide, Show},
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseEventKind,
    },
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};

use super::TxtView;

const PAGE_JUMP_COOLDOWN: Duration = Duration::from_millis(250);

impl TxtView {
    pub fn run(&mut self) -> io::Result<()> {
        terminal::enable_raw_mode()?;
        let mut stdout = io::BufWriter::with_capacity(256 * 1024, io::stdout());

        execute!(stdout, EnterAlternateScreen, Hide, EnableMouseCapture)?;

        let result = self.event_loop(&mut stdout);

        execute!(stdout, DisableMouseCapture, LeaveAlternateScreen, Show)?;
        terminal::disable_raw_mode()?;

        result
    }

    fn event_loop(&mut self, stdout: &mut impl io::Write) -> io::Result<()> {
        self.draw(stdout)?;

        let mut last_page_jump = Instant::now() - PAGE_JUMP_COOLDOWN;

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
                        if last_page_jump.elapsed() >= PAGE_JUMP_COOLDOWN {
                            let delta = self.scroll_down(self.visible_rows() as usize);
                            self.apply_scroll(stdout, delta)?;
                            last_page_jump = Instant::now();
                        }
                    }
                    (KeyCode::PageUp, _) => {
                        if last_page_jump.elapsed() >= PAGE_JUMP_COOLDOWN {
                            let delta = self.scroll_up(self.visible_rows() as usize);
                            self.apply_scroll(stdout, delta)?;
                            last_page_jump = Instant::now();
                        }
                    }
                    (KeyCode::Home, _) | (KeyCode::Char('g'), KeyModifiers::NONE) => {
                        let delta = self.jump_to_start();
                        self.apply_scroll(stdout, delta)?;
                    }
                    (KeyCode::End, _) | (KeyCode::Char('G'), _) => {
                        let delta = self.jump_to_end();
                        self.apply_scroll(stdout, delta)?;
                    }
                    _ => {}
                },
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        let delta = self.scroll_up(1);
                        self.apply_scroll(stdout, delta)?;
                    }
                    MouseEventKind::ScrollDown => {
                        let delta = self.scroll_down(1);
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

    fn apply_scroll(&mut self, stdout: &mut impl io::Write, delta: isize) -> io::Result<()> {
        if delta != 0 {
            self.draw(stdout)?;
        }
        Ok(())
    }
}
