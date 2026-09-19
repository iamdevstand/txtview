use std::io;

use crossterm::{QueueableCommand, cursor::MoveTo};

use crate::surface::{Anchor, Area, Component, Gesture, Request};

/// Geometric data for a single scrollbar frame: where the thumb sits and how
/// far it stretches, measured along the scrollbar's own axis. Produced by
/// [`TxtView::scroll_geometry`](crate::txtview::TxtView) and consumed by the
/// [`ScrollBar`] component, which renders it and turns presses and drags along
/// it back into scroll offsets. The fields are axis-agnostic: for a vertical
/// scrollbar they are rows, for a horizontal scrollbar they are columns.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ScrollGeometry {
    pub(crate) top: usize,
    pub(crate) size: usize,
    pub(crate) visible: usize,
}

impl ScrollGeometry {
    /// Map a thumb position (`0..visible`, along the scrollbar's axis) back
    /// to a scroll offset using the same proportion that produced the
    /// geometry in the first place.
    pub(crate) fn offset_from_thumb_top(&self, top: i64, max_offset: usize) -> usize {
        let travel = self.visible.saturating_sub(self.size).max(1);
        let clamped = usize::try_from(top).unwrap_or(0).min(travel);
        clamped.saturating_mul(max_offset) / travel
    }
}

/// Which axis a scrollbar runs along.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Orientation {
    /// Right-hand column of the box, spanning its rows.
    Vertical,
    /// Bottom row of the box, spanning its columns.
    #[allow(dead_code)]
    // This is deliberately dead code i made during the rewrite since its probably gonna be a feature later
    Horizontal,
}

/// A scrollbar drawn against the edge of its granted area.
///
/// The bar belongs to the base layout: its area is the edge the scrollbar
/// runs along, sized to the [`ScrollGeometry`] it holds, which the canvas
/// grants out of the free space. It positions itself entirely from its
/// [`Orientation`] and the area it is handed, a vertical scrollbar anchors
/// the right edge, a horizontal one the bottom row. The thumb (draggable,
/// highlighted while dragged) sits between blank track runs.
/// [`TxtView::scroll_geometry`](crate::txtview::TxtView) decides whether the
/// scrollbar is needed and how the thumb is shaped, this component only draws
/// a given geometry. A `grab_offset` of `Some(..)` marks the bar as being
/// dragged, the thumb is painted with the dragging fill and drags along the
/// bar map back to scroll offsets from that same grab. When both scrollbars
/// are shown at once the corner cell is painted by whichever renders last.
pub(crate) struct ScrollBar {
    geometry: ScrollGeometry,
    orientation: Orientation,
    grab_offset: Option<usize>,
}

impl ScrollBar {
    pub(crate) fn new(
        geometry: ScrollGeometry,
        orientation: Orientation,
        grab_offset: Option<usize>,
    ) -> Self {
        ScrollBar {
            geometry,
            orientation,
            grab_offset,
        }
    }

    fn cell(&self, i: usize) -> char {
        if i >= self.geometry.top && i < self.geometry.top + self.geometry.size {
            if self.grab_offset.is_some() {
                '▓'
            } else {
                '█'
            }
        } else {
            '░'
        }
    }
}

impl Component for ScrollBar {
    fn area(&self, boxed: Area) -> Area {
        let extent = u16::try_from(self.geometry.visible).unwrap_or(u16::MAX);
        match self.orientation {
            Orientation::Vertical => boxed.place(1, extent.min(boxed.height), Anchor::TopRight),
            Orientation::Horizontal => boxed.place(extent.min(boxed.width), 1, Anchor::BottomLeft),
        }
    }

    fn press(
        &self,
        area: Area,
        at: (u16, u16),
        max_offset: usize,
        gesture: Gesture,
    ) -> Option<Request> {
        if area.is_empty() {
            return None;
        }
        let y = usize::from(at.1.saturating_sub(area.row));
        match gesture {
            Gesture::Press => {
                if y >= self.geometry.top && y < self.geometry.top + self.geometry.size {
                    return Some(Request::Grab {
                        grab_offset: y - self.geometry.top,
                    });
                }
                let size = self.geometry.size.max(1);
                let center = y.saturating_sub(size / 2);
                let target = self
                    .geometry
                    .offset_from_thumb_top(i64::try_from(center).unwrap_or(i64::MAX), max_offset);
                Some(Request::DragTo {
                    target,
                    grab_offset: size.div_ceil(2).min(size - 1),
                })
            }
            Gesture::Drag => {
                let grab_offset = self.grab_offset?;
                let top = i64::try_from(y).unwrap_or(i64::MAX)
                    - i64::try_from(grab_offset).unwrap_or(i64::MAX);
                let target = self.geometry.offset_from_thumb_top(top, max_offset);
                Some(Request::DragTo {
                    target,
                    grab_offset,
                })
            }
        }
    }

    fn render(&self, area: Area, out: &mut dyn io::Write) -> io::Result<()> {
        if area.is_empty() {
            return Ok(());
        }
        match self.orientation {
            Orientation::Vertical => {
                let cells = u16::try_from(self.geometry.visible)
                    .unwrap_or(u16::MAX)
                    .min(area.height);
                for i in 0..cells {
                    let ch = self.cell(usize::from(i));
                    QueueableCommand::queue(out, MoveTo(area.col, area.row + i))?;
                    write!(out, "{}", ch)?;
                }
            }
            Orientation::Horizontal => {
                let cells = u16::try_from(self.geometry.visible)
                    .unwrap_or(u16::MAX)
                    .min(area.width);
                for i in 0..cells {
                    let ch = self.cell(usize::from(i));
                    QueueableCommand::queue(out, MoveTo(area.col + i, area.row))?;
                    write!(out, "{}", ch)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry() -> ScrollGeometry {
        ScrollGeometry {
            top: 1,
            size: 3,
            visible: 10,
        }
    }

    const COLS: u16 = 5;
    const ROWS: u16 = 10;

    fn box_area() -> Area {
        Area {
            col: 0,
            row: 0,
            width: COLS,
            height: ROWS,
        }
    }

    fn render(orientation: Orientation) -> String {
        let mut out = Vec::new();
        let bar = ScrollBar::new(geometry(), orientation, None);
        bar.render(placed(&bar), &mut out).unwrap();
        String::from_utf8(out).unwrap()
    }

    fn placed(bar: &ScrollBar) -> Area {
        bar.area(box_area())
    }

    #[test]
    fn emits_thumb_and_track_runs() {
        let out = render(Orientation::Vertical);
        assert!(out.contains('█'), "expected a thumb in output: {out:?}");
        assert!(out.contains('░'), "expected track runs in output: {out:?}");
    }

    #[test]
    fn uses_dragging_fill_while_dragging() {
        let mut out = Vec::new();
        let bar = ScrollBar::new(geometry(), Orientation::Vertical, Some(0));
        bar.render(placed(&bar), &mut out).unwrap();
        let out = String::from_utf8(out).unwrap();
        assert!(
            out.contains('▓'),
            "expected the dragging fill in output: {out:?}"
        );
    }

    #[test]
    fn vertical_draws_in_the_reserved_right_most_column() {
        let out = render(Orientation::Vertical);
        assert!(
            out.contains("\x1b[10;5H"),
            "vertical scrollbar must stay in the reserved last column: {out:?}"
        );
    }

    #[test]
    fn horizontal_draws_in_the_bottom_row() {
        let out = render(Orientation::Horizontal);
        assert!(
            out.contains("\x1b[10;1H"),
            "horizontal scrollbar must start in its row: {out:?}"
        );
        assert!(
            out.contains("\x1b[10;5H"),
            "horizontal scrollbar must stop at the granted width: {out:?}"
        );
    }

    #[test]
    fn clamps_to_the_granted_area() {
        let geometry = ScrollGeometry {
            top: 0,
            size: 1,
            visible: COLS as usize,
        };
        let mut out = Vec::new();
        let bar = ScrollBar::new(geometry, Orientation::Horizontal, None);
        let area = Area {
            col: 2,
            row: 3,
            width: 3,
            height: 1,
        };
        bar.render(area, &mut out).unwrap();
        let out = String::from_utf8(out).unwrap();
        assert!(
            !out.contains("\x1b[4;7H"),
            "must not paint past the granted width: {out:?}"
        );
        assert!(
            out.contains("\x1b[4;5H"),
            "must paint the cells inside its area: {out:?}"
        );
    }

    #[test]
    fn renders_nothing_in_an_empty_area() {
        let mut out = Vec::new();
        let bar = ScrollBar::new(geometry(), Orientation::Vertical, None);
        bar.render(
            Area {
                col: 0,
                row: 0,
                width: 0,
                height: 4,
            },
            &mut out,
        )
        .unwrap();
        assert!(out.is_empty(), "a zero-width area must not paint: {out:?}");
    }

    #[test]
    fn area_matches_rendered_position() {
        let bar = |orientation| ScrollBar::new(geometry(), orientation, None);
        let vertical = placed(&bar(Orientation::Vertical));
        assert_eq!(
            vertical,
            Area {
                col: COLS - 1,
                row: 0,
                width: 1,
                height: ROWS,
            },
            "vertical track must sit on the box's last column"
        );
        let horizontal = placed(&bar(Orientation::Horizontal));
        assert_eq!(
            horizontal,
            Area {
                col: 0,
                row: ROWS - 1,
                width: COLS,
                height: 1,
            },
            "horizontal track must sit on the box's last row"
        );
    }

    #[test]
    fn press_on_the_thumb_starts_a_grab_and_keeps_the_offset() {
        let bar = ScrollBar::new(geometry(), Orientation::Vertical, None);
        let area = placed(&bar);
        assert_eq!(
            bar.press(area, (area.col, 2), 100, Gesture::Press),
            Some(Request::Grab { grab_offset: 1 }),
            "a press inside the thumb (rows 1..4) must grab at its offset"
        );
    }

    #[test]
    fn press_on_the_track_jumps_to_the_centered_target() {
        let bar = ScrollBar::new(geometry(), Orientation::Vertical, None);
        let area = placed(&bar);
        let target = bar
            .press(area, (area.col, 8), 100, Gesture::Press)
            .expect("track press must answer");
        assert!(
            matches!(
                target,
                Request::DragTo {
                    target,
                    grab_offset
                } if grab_offset < 3 && target > 0
            ),
            "a press below the thumb must jump forward: {target:?}"
        );
    }

    #[test]
    fn drag_answers_from_its_grab_offset_only_while_grabbing() {
        let idle = ScrollBar::new(geometry(), Orientation::Vertical, None);
        let area = placed(&idle);
        assert_eq!(
            idle.press(area, (area.col, 5), 100, Gesture::Drag),
            None,
            "a bar that never grabbed must not consume drags"
        );

        let grabbed = ScrollBar::new(geometry(), Orientation::Vertical, Some(1));
        let travel = grabbed.geometry.visible - grabbed.geometry.size;
        let at_top = grabbed
            .press(area, (area.col, 1), 100, Gesture::Drag)
            .expect("a grabbed bar must answer");
        assert_eq!(
            at_top,
            Request::DragTo {
                target: 0,
                grab_offset: 1
            },
            "grab at offset 1 with the pointer at row 1 pins the thumb at the top"
        );
        let at_bottom = grabbed
            .press(area, (area.col, area.height - 1), 100, Gesture::Drag)
            .expect("a grabbed bar must answer");
        assert_eq!(
            at_bottom,
            Request::DragTo {
                target: 100,
                grab_offset: 1
            },
            "the thumb travel must reach max_offset"
        );
        assert!(travel >= 2, "the geometry test relies on a wide travel");
    }
}
