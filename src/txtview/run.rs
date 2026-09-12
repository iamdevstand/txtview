use std::io;
use std::io::IsTerminal;
use std::time::{Duration, Instant};

use crossterm::{
    cursor::{Hide, Show},
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseButton,
        MouseEventKind,
    },
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};

use super::TxtView;

const PAGE_JUMP_COOLDOWN: Duration = Duration::from_millis(200);

/// Restores the terminal when dropped, so raw mode, the alternate screen,
/// the hidden cursor and mouse capture are always cleaned up — even when a
/// panic unwinds through the viewer (issue #9).
struct RestoreTerminal;

impl Drop for RestoreTerminal {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = execute!(stdout, DisableMouseCapture, LeaveAlternateScreen, Show);
        let _ = terminal::disable_raw_mode();
    }
}

impl TxtView {
    /// Show the viewer and block until the user quits.
    ///
    /// This takes over the terminal: it enters raw mode, switches to an
    /// alternate screen, hides the cursor, and enables mouse capture. The
    /// terminal is always restored before returning, including on errors or
    /// a panic (cleanup runs from a `Drop` guard).
    ///
    /// # Errors
    ///
    /// Returns [`io::Error`] with [`io::ErrorKind::NotConnected`] when stdin
    /// or stdout is not a terminal (for example when output is piped), and
    /// propagates I/O errors from the terminal itself or the event loop.
    ///
    /// # Keybindings
    ///
    /// | Key               | Action                 |
    /// | ----------------- | ---------------------- |
    /// | `q`, `Esc`, `Ctrl+C` | Quit                |
    /// | `↑`/`↓`, `j`/`k`  | Scroll one line        |
    /// | `PgUp`/`PgDn`      | Scroll one page        |
    /// | `Home`/`g`, `End`/`G` | Jump to start / end |
    /// | Mouse wheel       | Scroll one line per tick |
    /// | Scrollbar track/thumb | Click to jump to position, drag to scroll |
    pub fn run(&mut self) -> io::Result<()> {
        if !io::stdout().is_terminal() {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "txtview requires an interactive terminal: stdout must be a terminal",
            ));
        }

        terminal::enable_raw_mode()?;
        let _guard = RestoreTerminal;
        let mut stdout = io::BufWriter::with_capacity(256 * 1024, io::stdout());

        execute!(stdout, EnterAlternateScreen, Hide, EnableMouseCapture)?;
        self.event_loop(&mut stdout)
    }

    fn event_loop(&mut self, stdout: &mut impl io::Write) -> io::Result<()> {
        self.draw(stdout)?;

        let mut last_page_up_jump = Instant::now();
        let mut last_page_down_jump = Instant::now();

        loop {
            match event::read()? {
                Event::Key(key) if key.kind == event::KeyEventKind::Press => {
                    match (key.code, key.modifiers) {
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
                            if last_page_down_jump.elapsed() >= PAGE_JUMP_COOLDOWN {
                                let delta = self.scroll_down(usize::from(self.visible_rows()));
                                self.apply_scroll(stdout, delta)?;
                                last_page_down_jump = Instant::now();
                            }
                        }
                        (KeyCode::PageUp, _) => {
                            if last_page_up_jump.elapsed() >= PAGE_JUMP_COOLDOWN {
                                let delta = self.scroll_up(usize::from(self.visible_rows()));
                                self.apply_scroll(stdout, delta)?;
                                last_page_up_jump = Instant::now();
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
                    }
                }
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        let delta = self.scroll_up(1);
                        self.apply_scroll(stdout, delta)?;
                    }
                    MouseEventKind::ScrollDown => {
                        let delta = self.scroll_down(1);
                        self.apply_scroll(stdout, delta)?;
                    }
                    MouseEventKind::Down(MouseButton::Left) => {
                        let visible = usize::from(self.visible_rows());
                        if let Some(g) = self.scroll_geometry(visible)
                            && mouse.column == g.column
                        {
                            let y = usize::from(mouse.row);
                            if y >= g.top && y < g.top + g.size {
                                self.dragging = true;
                                self.drag_grab_offset = y - g.top;
                                self.draw(stdout)?;
                            } else {
                                let size = g.size.max(1);
                                let center = y.saturating_sub(size / 2);
                                let target = self.offset_from_thumb_top(
                                    i64::try_from(center).unwrap_or(i64::MAX),
                                    &g,
                                );
                                let delta = self.scroll_to(target);
                                self.apply_scroll(stdout, delta)?;
                                self.dragging = true;
                                self.drag_grab_offset = size.div_ceil(2).min(size - 1);
                            }
                        }
                    }
                    MouseEventKind::Drag(MouseButton::Left) if self.dragging => {
                        if let Some(g) = self.scroll_geometry(usize::from(self.visible_rows())) {
                            let top = i64::from(mouse.row)
                                - i64::try_from(self.drag_grab_offset).unwrap_or(i64::MAX);
                            let target = self.offset_from_thumb_top(top, &g);
                            let delta = self.scroll_to(target);
                            self.apply_scroll(stdout, delta)?;
                        }
                    }
                    MouseEventKind::Up(_) if self.dragging => {
                        self.dragging = false;
                        self.draw(stdout)?;
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
