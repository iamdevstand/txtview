use std::io;

use crossterm::{
    QueueableCommand,
    cursor::MoveTo,
    terminal::{Clear, ClearType},
};

use crate::surface::{Anchor, Area, Component};
use crate::text::wrap_line_ansi;

/// The help bar: a separator rule followed by the wrapped help
/// text, pinned to the very bottom of the frame.
///
/// The bar decides its own footprint. [`HelpBar::height`] announces how many
/// rows the wrapped text needs, and [`HelpBar::area`] anchors that footprint
/// to the bottom edge of the box it is handed. The canvas grants it as far as
/// the free space allows and draws the bar into the area it was granted.
pub(crate) struct HelpBar {
    text: String,
}

impl HelpBar {
    pub(crate) fn new(text: impl Into<String>) -> Self {
        HelpBar { text: text.into() }
    }

    /// The rows the bar needs when pinned to the bottom of a `rows`-high
    /// viewport: the separator plus the wrapped help lines that fit. Zero
    /// when there is no room for it at all.
    pub(crate) fn height(&self, cols: usize, rows: u16) -> usize {
        if rows < 2 {
            return 0;
        }
        let lines = wrap_line_ansi(&self.text, cols.max(1), 0);
        1 + lines.len().min(usize::from(rows) - 2)
    }
}

impl Component for HelpBar {
    fn area(&self, boxed: Area) -> Area {
        let height = self.height(usize::from(boxed.width), boxed.height);
        let height = u16::try_from(height).unwrap_or(u16::MAX);
        boxed.place(boxed.width, height, Anchor::BottomLeft)
    }

    fn render(&self, area: Area, out: &mut dyn io::Write) -> io::Result<()> {
        if area.is_empty() {
            return Ok(());
        }

        let cols = usize::from(area.width).max(1);
        let lines = wrap_line_ansi(&self.text, cols, 0);
        let shown = lines.len().min(usize::from(area.height) - 1);

        QueueableCommand::queue(out, MoveTo(area.col, area.row))?;
        QueueableCommand::queue(out, Clear(ClearType::CurrentLine))?;
        write!(out, "{}", "─".repeat(cols))?;

        for (i, line) in lines.iter().take(shown).enumerate() {
            let row = area.row + 1 + u16::try_from(i).unwrap_or(u16::MAX);
            QueueableCommand::queue(out, MoveTo(area.col, row))?;
            QueueableCommand::queue(out, Clear(ClearType::CurrentLine))?;
            write!(out, "{}", line)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(bar: HelpBar, cols: u16, rows: u16) -> String {
        let mut out = Vec::new();
        let area = bar.area(Area {
            col: 0,
            row: 0,
            width: cols,
            height: rows,
        });
        bar.render(area, &mut out).unwrap();
        String::from_utf8(out).unwrap()
    }

    fn max_move_to_row(out: &str) -> usize {
        out.split("\x1b[")
            .filter_map(|m| {
                let rest = m.strip_suffix('H')?;
                rest.split_once(';')?.0.parse::<usize>().ok()
            })
            .max()
            .unwrap_or(0)
    }

    #[test]
    fn renders_on_narrow_terminal() {
        let out = render(
            HelpBar::new("a long string of keybindings that wraps"),
            1,
            10,
        );
        assert!(!out.is_empty(), "help bar must render on a narrow terminal");
        assert!(
            max_move_to_row(&out) <= 10,
            "help bar escaped its 10-row viewport: {out:?}"
        );
    }

    #[test]
    fn skipped_when_viewport_is_too_short() {
        let out = render(HelpBar::new("help"), 20, 1);
        assert!(out.is_empty(), "expected no output: {out:?}");
    }

    #[test]
    fn draws_separator_then_lines() {
        let out = render(HelpBar::new("ab"), 2, 5);
        assert!(out.contains("──"), "expected the separator row: {out:?}");
        assert!(out.contains("ab"), "expected the help text: {out:?}");
    }

    #[test]
    fn area_anchors_the_footprint_to_the_bottom() {
        let bar = HelpBar::new("ab");
        let area = bar.area(box_area(0, 0, 2, 5));
        assert_eq!(area.height, 2);
        assert_eq!(area.row, 3, "footprint must sit on the last rows");
    }

    fn box_area(col: u16, row: u16, width: u16, height: u16) -> Area {
        Area {
            col,
            row,
            width,
            height,
        }
    }

    #[test]
    fn height_matches_rendered_footprint() {
        let bar = HelpBar::new("abcd");
        assert_eq!(bar.height(2, 10), 1 + 2);
        assert_eq!(bar.height(2, 1), 0, "no room on a one-row viewport");
        assert_eq!(bar.height(2, 2), 1, "separator only when nothing fits");
    }

    #[test]
    fn wraps_by_visual_width_not_char_count() {
        let bar = HelpBar::new("🙂🙂ab");
        assert_eq!(
            bar.height(4, 10),
            1 + 2,
            "two 2-column wide emoji must share one row"
        );
    }
}
