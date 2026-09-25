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
/// decides its own footprint: [`HelpBar::height`] shows it in full when the
/// wrapped text fits its budget or hides it, and [`HelpBar::area`] anchors
/// that footprint to the bottom edge of the box it is handed. The canvas
/// grants it as far as the free space allows and draws the bar into the
/// area it was granted.
pub(crate) struct HelpBar {
    /// The wrapped help text, one `String` per displayed line.
    rows: Vec<String>,
}

impl HelpBar {
    pub(crate) fn new(rows: Vec<String>) -> Self {
        HelpBar { rows }
    }

    /// The rows the bar needs when pinned to the bottom of a `rows`-high
    /// viewport, or zero when it should hide itself.
    ///
    /// The help is a footnote, so it gets a budget of a quarter of the
    /// viewport rows, a share that scales with the viewport and never locks
    /// the current text to a fixed row count. The bar shows in full only
    /// while the wrapped text fits that budget. When it does not, which a
    /// narrow viewport provokes by wrapping the text into many rows, the
    /// bar hides instead of showing a truncated flood. Either way the
    /// document keeps at least three quarters of the rows.
    pub(crate) fn height(&self, rows: u16) -> u16 {
        let budget = rows / 4;
        let wrapped = u16::try_from(self.rows.len()).unwrap_or(u16::MAX);
        if wrapped < budget { wrapped + 1 } else { 0 }
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

    #[test]
    fn hides_when_the_wrapped_help_would_flood() {
        let out = render(
            HelpBar::new(rows("a long string of keybindings that wraps", 1)),
            1,
            10,
        );
        assert!(
            out.is_empty(),
            "a bar the viewport cannot fit must hide, not render: {out:?}"
        );
    }

    #[test]
    fn skipped_when_viewport_is_too_short() {
        let out = render(HelpBar::new(rows("help", 20)), 20, 1);
        assert!(out.is_empty(), "expected no output: {out:?}");
    }

    #[test]
    fn draws_separator_then_lines() {
        let out = render(HelpBar::new(rows("ab", 2)), 2, 24);
        assert!(out.contains("──"), "expected the separator row: {out:?}");
        assert!(out.contains("ab"), "expected the help text: {out:?}");
    }

    #[test]
    fn area_anchors_the_footprint_to_the_bottom() {
        let bar = HelpBar::new(rows("ab", 2));
        let area = bar.area(box_area(0, 0, 2, 24));
        assert_eq!(area.height, 2);
        assert_eq!(area.row, 22, "footprint must sit on the last rows");
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
        assert_eq!(bar.height(24), 3, "separator plus the two wrapped lines");
        assert_eq!(bar.height(12), 3, "still within the budget at half height");
        assert_eq!(bar.height(8), 0, "no budget for the footprint, so it hides");
        assert_eq!(bar.height(1), 0, "no room on a one-row viewport");
        assert_eq!(bar.height(2), 0, "no room on a two-row viewport");
    }

    #[test]
    fn wraps_by_visual_width_not_char_count() {
        let bar = HelpBar::new(rows("🙂🙂ab", 4));
        assert_eq!(
            bar.height(24),
            3,
            "two 2-column wide emoji must share one row"
        );
    }

    #[test]
    fn budget_scales_with_the_viewport_not_the_text() {
        let short = HelpBar::new(rows("q: quit", 40));
        let long = HelpBar::new(rows("a longer help string that wraps past one line", 40));
        assert!(long.rows.len() > short.rows.len());
        assert_eq!(usize::from(short.height(24)), 1 + short.rows.len());
        assert_eq!(usize::from(long.height(24)), 1 + long.rows.len());
        let tall = HelpBar::new(rows("wrap me one column at a time please", 1));
        assert!(tall.rows.len() >= 10, "fixture must outgrow the budget");
        assert_eq!(tall.height(8), 0);
        assert_eq!(tall.height(4), 0);
    }

    #[test]
    fn hides_instead_of_showing_a_cut_down_bar() {
        let bar = HelpBar::new(rows(
            "keybindings that wrap often on a tiny column width",
            4,
        ));
        assert!(bar.rows.len() > 8, "fixture must outgrow the budget");
        assert_eq!(bar.height(24), 0, "a bar that cannot fit hides entirely");
        let compact = HelpBar::new(rows("q: quit | j/k: scroll", 40));
        assert_eq!(
            usize::from(compact.height(24)),
            1 + compact.rows.len(),
            "a short text still earns its footnote on the same viewport"
        );
    }

    #[test]
    fn never_takes_more_than_a_quarter_of_the_viewport() {
        let bar = HelpBar::new(rows("wordy help text", 5));
        for rows in 1..=30u16 {
            let taken = bar.height(rows);
            assert!(taken <= rows / 4, "the bar took {taken} of {rows} rows");
        }
    }
}
