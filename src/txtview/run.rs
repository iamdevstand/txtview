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
                Event::Key(key) => {
                    match (key.code, key.modifiers) {
                        (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                            break;
                        }
                        (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
                            self.scroll_down(1);
                        }
                        (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
                            self.scroll_up(1);
                        }
                        (KeyCode::PageDown, _) => {
                            self.scroll_down(self.visible_rows() as usize);
                        }
                        (KeyCode::PageUp, _) => {
                            self.scroll_up(self.visible_rows() as usize);
                        }
                        (KeyCode::Home, _) | (KeyCode::Char('g'), KeyModifiers::NONE) => {
                            self.jump_to_start();
                        }
                        (KeyCode::End, _) | (KeyCode::Char('h'), KeyModifiers::NONE) => {
                            self.jump_to_end();
                        }
                        (KeyCode::Esc, _) => break,
                        _ => {}
                    }
                    self.draw(stdout)?;
                }
                Event::Mouse(mouse) => {
                    match mouse.kind {
                        MouseEventKind::ScrollUp => {
                            self.scroll_up(3);
                        }
                        MouseEventKind::ScrollDown => {
                            self.scroll_down(3);
                        }
                        _ => {}
                    }
                    self.draw(stdout)?;
                }
                Event::Resize(_cols, _rows) => {
                    self.draw(stdout)?;
                }
                _ => {}
            }
        }

        Ok(())
    }
}
