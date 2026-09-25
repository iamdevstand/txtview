//! Frame composition and drawing: each redraw goes through this module, which
//! (re)builds the display bounds, places the viewer's pieces on a [`Canvas`],
//! paints them and flushes the buffer.

use std::io;

use super::TxtView;
use crate::components::{Content, HelpBar, Orientation, ScrollBar};
use crate::surface::Canvas;

impl TxtView {
    /// Compose the viewer's pieces into a canvas and paint it.
    ///
    /// This is the junction point every redraw goes through. The full terminal
    /// box (config-overridable) is the [`Canvas`], the pieces live in it and
    /// it is what gets drawn. Each piece works out its own area inside the
    /// space that is still free. [`HelpBar`] anchors its wrapped text to the
    /// bottom edge, [`ScrollBar`] claims the rightmost column, and [`Content`]
    /// fills whatever remains. A piece with nothing to show vanishes without
    /// taking space. Once every piece is placed the canvas draws them in the
    /// order they were added. The same canvas also answers mouse presses, so
    /// the scrollbar behaves like the piece it is instead of being bolted on.
    pub(super) fn draw(&mut self, stdout: &mut impl io::Write) -> io::Result<()> {
        self.refresh_bounds();
        self.compose().render(stdout)?;
        stdout.flush()?;
        Ok(())
    }

    /// Rebuild the current frame's [`Canvas`] from the viewer's last-drawn
    /// state.
    ///
    /// This must follow a [`TxtView::draw`]: composing alone does not refresh
    /// the display bounds, so the canvas it builds describes the state after
    /// the most recent draw. The mouse handlers recompose between events,
    /// when no layout-affecting change can have happened since the last draw,
    /// so the composed canvas always agrees with what is on screen.
    pub(super) fn compose(&self) -> Canvas<'_> {
        let cols = self.content_cols();
        let viewport_rows = self.resolved_height();

        let mut canvas = Canvas::new(cols, viewport_rows);
        if self.config.show_help_bar {
            canvas.place(HelpBar::new(self.help_rows()));
        }
        if let Some(geometry) = self.scroll_geometry() {
            canvas.place(ScrollBar::new(
                geometry,
                Orientation::Vertical,
                self.drag_grab_offset,
            ));
        }
        canvas.fill(Content::new(&self.display, self.offset));

        canvas
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TxtViewConfig;
    use crate::surface::{Area, Gesture};

    fn max_move_to_row(out: &[u8]) -> usize {
        parse_moves(out)
            .into_iter()
            .map(|(row, _)| usize::from(row))
            .max()
            .unwrap_or(0)
    }

    fn max_move_to_col(out: &[u8]) -> usize {
        parse_moves(out)
            .into_iter()
            .map(|(_, col)| usize::from(col))
            .max()
            .unwrap_or(0)
    }

    /// The 1-based `(row, col)` of every `MoveTo` escape in the output.
    fn parse_moves(out: &[u8]) -> Vec<(u16, u16)> {
        String::from_utf8_lossy(out)
            .split("\x1b[")
            .filter_map(|m| {
                let rest = m.strip_suffix('H')?;
                let (row, col) = rest.split_once(';')?;
                Some((row.parse().ok()?, col.parse().ok()?))
            })
            .collect()
    }

    fn scrolling_viewer(lines: usize, rows: u16, cols: u16) -> TxtView {
        let text = (0..lines)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let config = TxtViewConfig {
            viewport_height: Some(rows),
            viewport_width: Some(cols),
            ..TxtViewConfig::default()
        };
        TxtView::new(&text).with_config(config)
    }

    #[test]
    fn draw_clamps_oversized_viewport_to_terminal() {
        let text = (0..100)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let config = TxtViewConfig {
            viewport_height: Some(80),
            viewport_width: Some(5000),
            show_help_bar: true,
            show_scrollbar: false,
            ..TxtViewConfig::default()
        };
        let mut v = TxtView::new(&text).with_config(config);

        let (cols, rows) = TxtView::term_size();
        assert!(
            usize::from(v.visible_rows()) <= usize::from(rows),
            "viewport {} exceeds terminal height {rows}",
            v.visible_rows()
        );

        let mut out = Vec::new();
        v.draw(&mut out).unwrap();
        assert!(
            max_move_to_row(&out) <= usize::from(rows),
            "draw wrote past terminal height {rows}: {out:?}"
        );
        assert!(
            max_move_to_col(&out) <= usize::from(cols),
            "draw wrote past terminal width {cols}: {out:?}"
        );
    }

    #[test]
    fn draw_keeps_small_viewport_self_contained() {
        let config = TxtViewConfig {
            viewport_height: Some(5),
            viewport_width: Some(80),
            show_help_bar: true,
            show_scrollbar: false,
            ..TxtViewConfig::default()
        };
        let mut v = TxtView::new("a\nb\nc\nd\ne\nf").with_config(config);

        let mut out = Vec::new();
        v.draw(&mut out).unwrap();
        assert!(
            max_move_to_row(&out) <= 5,
            "draw escaped the 5-row viewport: {out:?}"
        );
    }

    #[test]
    fn draw_renders_scrollbar_glyphs() {
        let text = (0..500)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let config = TxtViewConfig {
            viewport_height: Some(20),
            viewport_width: Some(40),
            show_help_bar: false,
            show_scrollbar: true,
            ..TxtViewConfig::default()
        };
        let mut v = TxtView::new(&text).with_config(config);

        let mut out = Vec::new();
        v.draw(&mut out).unwrap();
        let s = String::from_utf8_lossy(&out);
        assert!(
            s.contains(['█', '░', '▓']),
            "the scrollbar column paints its thumb and track glyphs: {s:?}"
        );
    }

    #[test]
    fn compose_scrollbar_area_matches_the_content_rows() {
        let v = scrolling_viewer(100, 20, 40);
        let canvas = v.compose();
        let content_rows = v.visible_rows();
        let viewport_rows = v.resolved_height();

        let areas = canvas.areas();
        assert_eq!(
            areas
                .iter()
                .find(|area| area.width == 1 && area.col == 39)
                .copied(),
            Some(Area {
                col: 39,
                row: 0,
                width: 1,
                height: content_rows,
            }),
            "the scrollbar grant must be exactly the region it paints"
        );
        assert_eq!(
            areas.iter().find(|area| area.row == content_rows).copied(),
            Some(Area {
                col: 0,
                row: content_rows,
                width: 40,
                height: viewport_rows - content_rows,
            }),
            "the help bar must own the rows below the content"
        );
        assert_eq!(
            areas.iter().max_by_key(|area| area.height).copied(),
            Some(Area {
                col: 0,
                row: 0,
                width: 39,
                height: content_rows,
            }),
            "content must cover the box minus the help bar and scrollbar"
        );
    }

    #[test]
    fn scrollbar_hit_region_stops_where_the_bar_renders() {
        let v = scrolling_viewer(100, 20, 40);
        let canvas = v.compose();
        assert!(
            canvas.press(39, 0, v.max_offset, Gesture::Press).is_some(),
            "the thumb column inside the content rows is scrollbar for clicks"
        );
        assert!(
            canvas
                .press(39, v.visible_rows(), v.max_offset, Gesture::Press)
                .is_none(),
            "the help bar's row in the last column must not be scrollbar"
        );
        assert!(
            canvas.press(38, 0, v.max_offset, Gesture::Press).is_none(),
            "the content column must not be scrollbar"
        );
    }

    #[test]
    fn compose_releases_the_scrollbar_when_everything_fits() {
        let v = scrolling_viewer(5, 20, 40);
        assert!(
            v.compose().areas().iter().all(|area| area.width != 1),
            "a document that fits must not reserve a scrollbar"
        );
    }

    #[test]
    fn compose_grants_a_scrollbar_area_when_and_only_when_the_column_is_reserved() {
        for lines in [5usize, 10, 50, 100] {
            let v = scrolling_viewer(lines, 20, 40);
            let overflows = v.display.len() > usize::from(v.visible_rows());
            let granted = v
                .compose()
                .areas()
                .iter()
                .any(|area| area.width == 1 && area.col == 39);
            assert_eq!(
                granted, v.scrollbar_active,
                "the canvas must grant a scrollbar iff the column was reserved ({lines} lines)"
            );
            assert_eq!(
                overflows, v.scrollbar_active,
                "the reserved column must track exactly the overflow ({lines} lines)"
            );
        }
    }

    #[test]
    fn draw_writes_every_piece_inside_its_granted_area() {
        let mut v = scrolling_viewer(100, 20, 40);
        let mut out = Vec::new();
        v.draw(&mut out).unwrap();
        let areas = v.compose().areas();

        let moves = parse_moves(&out);
        assert!(!moves.is_empty(), "draw must emit cursor moves");
        for (row, col) in moves {
            let cell = (col - 1, row - 1);
            assert!(
                areas.iter().any(|area| area.contains(cell.0, cell.1)),
                "draw moved the cursor to ({row}, {col}), outside every granted area: {out:?}"
            );
        }
    }

    #[test]
    fn draw_survives_narrow_viewport_with_wide_content() {
        let text = "🎉\t🎉";
        for cols in [1u16, 5] {
            let config = TxtViewConfig {
                viewport_height: Some(8),
                viewport_width: Some(cols),
                show_help_bar: false,
                show_scrollbar: false,
                ..TxtViewConfig::default()
            };
            let mut v = TxtView::new(text).with_config(config);
            let mut out = Vec::new();
            assert!(
                v.draw(&mut out).is_ok(),
                "a {cols}-column viewport must not fault on 2-wide chars and tabs"
            );
            assert!(
                max_move_to_row(&out) <= 8,
                "writes must stay inside the 8-row viewport at {cols} columns"
            );
        }
    }

    #[test]
    fn tab_gap_is_repainted_after_a_scroll() {
        let text = ["01234567", "ab\tcd", "01234567", "again"].join("\n");
        let config = TxtViewConfig {
            viewport_height: Some(2),
            viewport_width: Some(8),
            show_help_bar: false,
            show_scrollbar: false,
            ..TxtViewConfig::default()
        };
        let mut v = TxtView::new(&text).with_config(config);

        let mut frame = Vec::new();
        v.draw(&mut frame).unwrap();
        let first = String::from_utf8_lossy(&frame);
        assert_eq!(
            first, "\x1b[1;1H01234567\x1b[2;1Hab      ",
            "frame 1 must paint the tab gap as spaces: {first:?}"
        );

        v.scroll_down(1);
        let mut frame = Vec::new();
        v.draw(&mut frame).unwrap();
        let second = String::from_utf8_lossy(&frame);
        assert_eq!(
            second, "\x1b[1;1Hab      \x1b[2;1Hcd      ",
            "frame 2 must repaint the gap a literal tab byte would have skipped: {second:?}"
        );
    }

    #[test]
    fn tab_stop_lines_up_under_a_line_number_prefix() {
        let config = TxtViewConfig {
            viewport_height: Some(2),
            viewport_width: Some(10),
            show_line_numbers: true,
            show_help_bar: false,
            show_scrollbar: false,
        };
        let mut v = TxtView::new("ab\tcd\nef").with_config(config);

        let mut out = Vec::new();
        v.draw(&mut out).unwrap();
        let s = String::from_utf8_lossy(&out);
        assert_eq!(
            s, "\x1b[1;1H1 │ ab  cd\x1b[2;1H2 │ ef    ",
            "the tab must reach the same stop the wrap measured under the prefix: {s:?}"
        );
    }

    #[test]
    fn one_column_viewport_keeps_content_above_the_scrollbar() {
        let config = TxtViewConfig {
            viewport_height: Some(3),
            viewport_width: Some(1),
            show_help_bar: false,
            ..TxtViewConfig::default()
        };
        let mut v = TxtView::new("a\nb\nc\nd").with_config(config);
        assert!(v.scrollbar_active, "fixture must overflow the viewport");

        let mut out = Vec::new();
        v.draw(&mut out).unwrap();
        let s = String::from_utf8_lossy(&out);
        assert_eq!(
            s, "\x1b[1;1Ha\x1b[2;1Hb\x1b[3;1Hc",
            "a one-column viewport must paint content, not hand it a zero-width box: {s:?}"
        );
    }

    #[test]
    fn resize_during_drag_keeps_the_grab_consistent() {
        let mut v = scrolling_viewer(100, 20, 40);
        assert!(v.scrollbar_active, "fixture must overflow");
        v.drag_grab_offset = Some(1);

        assert!(
            v.compose()
                .press(39, 5, v.max_offset, Gesture::Drag)
                .is_some(),
            "a grabbed bar must keep answering drags"
        );

        let config = TxtViewConfig {
            viewport_height: Some(10),
            ..v.config().clone()
        };
        v = v.with_config(config);
        assert!(v.scrollbar_active, "still overflows after the shrink");
        assert!(
            v.drag_grab_offset.is_some(),
            "an Up event that never arrived must leave the grab armed"
        );
        assert!(
            v.compose()
                .press(39, 2, v.max_offset, Gesture::Drag)
                .is_some(),
            "a drag must keep working across the resize"
        );

        v.drag_grab_offset = None;
        assert_eq!(
            v.compose().press(39, 2, v.max_offset, Gesture::Drag),
            None,
            "releasing the grab must silence further drags"
        );
    }
}
