use std::io;

use crossterm::{
    QueueableCommand,
    cursor::MoveTo,
    terminal::{Clear, ClearType},
};

use crate::surface::{Anchor, Area, Component};

/// The help bar: a separator rule followed by the wrapped help
/// text, pinned to the very bottom of the frame.
///
/// The wrapped rows come from the layout already laid out, the same
/// `wrap_line_ansi` step that produces the content rows, so the help text
/// shares the content's wrapping routine and never wraps itself. The bar
/// decides its own footprint: [`HelpBar::height`] announces how many rows the
/// wrapped text needs, and [`HelpBar::area`] anchors that footprint to the
/// bottom edge of the box it is handed. The canvas grants it as far as the
/// free space allows and draws the bar into the area it was granted.
pub(crate) struct HelpBar {
    rows: Vec<String>,
}

impl HelpBar {
    pub(crate) fn new(rows: Vec<String>) -> Self {
        HelpBar { rows }
    }

    /// The rows the bar needs when pinned to the bottom of a `rows`-high
    /// viewport: the separator plus the wrapped help lines that fit. Zero
    /// when there is no room for it at all.
    pub(crate) fn height(&self, rows: u16) -> u16 {
        if rows < 2 {
            return 0;
        }
        let available = rows - 2;
        // `available` is a u16 and `shown` is clamped to it, so this cannot
        // truncate, the fallback keeps the bar small rather than absurd
        let shown = u16::try_from(self.rows.len())
            .unwrap_or(available)
            .min(available);
        1 + shown
    }
}

impl Component for HelpBar {
    fn area(&self, boxed: Area) -> Area {
        let height = self.height(boxed.height);
        boxed.place(boxed.width, height, Anchor::BottomLeft)
    }

    fn render(&self, area: Area, out: &mut dyn io::Write) -> io::Result<()> {
        if area.is_empty() {
            return Ok(());
        }

        let cols = usize::from(area.width).max(1);
        // `rows.len()` is `usize`, the range below needs a `u16` count. The
        // count is clamped to the granted area, so this cannot produce a value
        // that overflows `area.height - 1`, the fallback is the largest bar
        // that the area can hold, never an out-of-bounds row
        let shown = area
            .height
            .saturating_sub(1)
            .min(u16::try_from(self.rows.len()).unwrap_or(area.height.saturating_sub(1)));

        QueueableCommand::queue(out, MoveTo(area.col, area.row))?;
        QueueableCommand::queue(out, Clear(ClearType::CurrentLine))?;
        write!(out, "{}", "─".repeat(cols))?;

        for offset in 1..=shown {
            let row = area.row.saturating_add(offset);
            let line = &self.rows[usize::from(offset - 1)];
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

    /// Wrap once, the way the layout does, to build the bar's fixture rows.
    fn rows(text: &str, cols: usize) -> Vec<String> {
        crate::text::wrap_line_ansi(text, cols.max(1), 0)
    }

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
            HelpBar::new(rows("a long string of keybindings that wraps", 1)),
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
        let out = render(HelpBar::new(rows("help", 20)), 20, 1);
        assert!(out.is_empty(), "expected no output: {out:?}");
    }

    #[test]
    fn draws_separator_then_lines() {
        let out = render(HelpBar::new(rows("ab", 2)), 2, 5);
        assert!(out.contains("──"), "expected the separator row: {out:?}");
        assert!(out.contains("ab"), "expected the help text: {out:?}");
    }

    #[test]
    fn area_anchors_the_footprint_to_the_bottom() {
        let bar = HelpBar::new(rows("ab", 2));
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
        let bar = HelpBar::new(rows("abcd", 2));
        assert_eq!(bar.height(10), 1 + 2);
        assert_eq!(bar.height(1), 0, "no room on a one-row viewport");
        assert_eq!(bar.height(2), 1, "separator only when nothing fits");
    }

    #[test]
    fn wraps_by_visual_width_not_char_count() {
        let bar = HelpBar::new(rows("🙂🙂ab", 4));
        assert_eq!(
            bar.height(10),
            1 + 2,
            "two 2-column wide emoji must share one row"
        );
    }
}
