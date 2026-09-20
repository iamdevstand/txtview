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
    /// The smallest thumb worth painting. The thumb is capped on the other
    /// end too, so the bar can neither thin to nothing nor fill the track.
    pub(crate) const MIN_THUMB: usize = 1;

    /// The thumb must leave at least this many cells of the track free to
    /// travel through: the thumb is capped to guarantee it, so the bar never
    /// collapses into one solid block when the document barely overflows the
    /// viewport. Only a track shorter than `MIN_THUMB + MIN_TRAVEL` cells (a
    /// one- or two-cell viewport) cannot host both, and the bar vanishes.
    pub(crate) const MIN_TRAVEL: usize = 2;

    /// Whether a `visible`-cell track over a `total`-cell document can host a
    /// usable bar at all: the document must overflow the track and the track
    /// must be long enough for a thumb plus [`MIN_TRAVEL`](Self::MIN_TRAVEL)
    /// cells of travel. The layout uses this before reserving the scrollbar
    /// column, so a bar with no room to move is not drawn and its column goes
    /// back to the content.
    pub(crate) fn room_to_travel(total: usize, visible: usize) -> bool {
        total > visible && visible >= Self::MIN_THUMB + Self::MIN_TRAVEL
    }

    /// Map a thumb position (`0..visible`, along the scrollbar's axis) back
    /// to a scroll offset using the same proportion that produced the
    /// geometry in the first place, rounding to the nearest offset so an
    /// inverted drag keeps the thumb under the pointer instead of always
    /// under-stepping it.
    pub(crate) fn offset_from_thumb_top(&self, top: i64, max_offset: usize) -> usize {
        let travel = self.visible.saturating_sub(self.size).max(1);
        let clamped = usize::try_from(top).unwrap_or(0).min(travel);
        let numerator = clamped
            .saturating_mul(max_offset)
            .saturating_add(travel / 2);
        numerator / travel
    }
}

/// Which axis a scrollbar runs along.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Orientation {
    /// Right-hand column of the box, spanning its rows.
    Vertical,
    /// Bottom row of the box, spanning its columns. Not yet offered by the
    /// layout, so a horizontal bar is only ever built by the component tests;
    /// kept as the planned next feature.
    #[allow(dead_code)]
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
        // `geometry.visible` comes from a u16 viewport row count, so this
        // cannot truncate. If it ever did, falling back to the box dimension
        // keeps the thumb in-bounds instead of silently spanning the world
        match self.orientation {
            Orientation::Vertical => {
                let extent = u16::try_from(self.geometry.visible)
                    .unwrap_or(boxed.height)
                    .min(boxed.height);
                boxed.place(1, extent, Anchor::TopRight)
            }
            Orientation::Horizontal => {
                let extent = u16::try_from(self.geometry.visible)
                    .unwrap_or(boxed.width)
                    .min(boxed.width);
                boxed.place(extent, 1, Anchor::BottomLeft)
            }
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
                // A track press grabs the thumb at the center it just used to
                // aim, so a drag that never leaves the cell keeps it still.
                let center = y.saturating_sub(size / 2);
                let target = self
                    .geometry
                    .offset_from_thumb_top(i64::try_from(center).unwrap_or(i64::MAX), max_offset);
                Some(Request::DragTo {
                    target,
                    grab_offset: size / 2,
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
                // `geometry.visible` is bounded by a u16 viewport, so this
                // cannot truncate. A fallback of the full area keeps every
                // painted cell on the track
                let cells = u16::try_from(self.geometry.visible)
                    .unwrap_or(area.height)
                    .min(area.height);
                for i in 0..cells {
                    let ch = self.cell(usize::from(i));
                    QueueableCommand::queue(out, MoveTo(area.col, area.row + i))?;
                    write!(out, "{}", ch)?;
                }
            }
            Orientation::Horizontal => {
                let cells = u16::try_from(self.geometry.visible)
                    .unwrap_or(area.width)
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

    #[test]
    fn maps_thumb_top_to_offset_rounded_to_nearest() {
        let g = ScrollGeometry {
            top: 0,
            size: 6,
            visible: 10,
        };
        assert_eq!(g.offset_from_thumb_top(0, 5), 0, "track top is offset 0");
        assert_eq!(g.offset_from_thumb_top(1, 5), 1, "1.25 travels rounds to 1");
        assert_eq!(g.offset_from_thumb_top(2, 5), 3, "halfway rounds up");
        assert_eq!(
            g.offset_from_thumb_top(4, 5),
            5,
            "track bottom is max_offset"
        );
        assert_eq!(
            g.offset_from_thumb_top(-2, 5),
            0,
            "above the track clamps to 0"
        );
    }

    #[test]
    fn track_too_short_for_thumb_plus_travel_has_no_room() {
        assert!(!ScrollGeometry::room_to_travel(10, 1));
        assert!(!ScrollGeometry::room_to_travel(10, 2));
        assert!(ScrollGeometry::room_to_travel(10, 3));
        assert!(
            !ScrollGeometry::room_to_travel(2, 10),
            "without overflow no bar"
        );
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
    fn horizontal_bar_answers_presses_along_its_track() {
        let bar = ScrollBar::new(geometry(), Orientation::Horizontal, None);
        let area = placed(&bar);
        let press = bar
            .press(area, (area.col, area.row), 100, Gesture::Press)
            .expect("a horizontal track press must answer");
        assert!(
            matches!(press, Request::DragTo { target, .. } if target == 0),
            "the pointer above the thumb must jump to the start: {press:?}"
        );

        let grabbed = ScrollBar::new(geometry(), Orientation::Horizontal, Some(1));
        let drag = grabbed
            .press(area, (area.col, area.row), 100, Gesture::Drag)
            .expect("a grabbed horizontal bar must answer");
        assert_eq!(
            drag,
            Request::DragTo {
                target: 0,
                grab_offset: 1
            },
            "grab at offset 1 over the track's first cell pins the thumb at the start"
        );
    }

    #[test]
    fn stationary_drag_after_a_track_press_keeps_the_thumb_still() {
        // A track press centers the thumb under the pointer and grabs it at
        // that center, so a drag that never leaves the cell must not move the
        // thumb again. The press and the drag have to agree on the offset for
        // every thumb size, not just even ones.
        for size in 1..=5 {
            let geometry = ScrollGeometry {
                top: 0,
                size,
                visible: 10,
            };
            let bar = ScrollBar::new(geometry, Orientation::Vertical, None);
            let area = placed(&bar);
            let y = 7;
            let (target, grab_offset) = match bar
                .press(area, (area.col, y), 30, Gesture::Press)
                .expect("a track press must answer")
            {
                Request::DragTo {
                    target,
                    grab_offset,
                } => (target, grab_offset),
                request => panic!("expected a drag request, got {request:?}"),
            };
            assert_eq!(
                grab_offset,
                size / 2,
                "a track press grabs the thumb at its center"
            );
            let grabbed = ScrollBar::new(geometry, Orientation::Vertical, Some(grab_offset));
            let drag = grabbed
                .press(area, (area.col, y), 30, Gesture::Drag)
                .expect("a grabbed bar must answer");
            match drag {
                Request::DragTo { target: t, .. } => assert_eq!(
                    t, target,
                    "a drag over the pressed cell must not move the thumb"
                ),
                request => panic!("expected a drag request, got {request:?}"),
            }
        }
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
