use std::io;

use super::{Area, Component, Gesture, Request};

/// The canvas a whole frame paints into: the entire visible terminal area, or
/// its config-overridden size.
///
/// The canvas is the one place that keeps every component together with the
/// area it was granted, sizes those grants against the space that is still
/// free, and draws them all at render time in the order they were added. A
/// component is handed to it with [`Canvas::place`]: the canvas asks the
/// component for the area it will paint ([`Component::area`]), fits it into
/// the rectangle of free cells that is still left (it never grants a cell
/// already given to another piece, so the visual layout survives), and keeps
/// the component together with the area it was awarded. The content piece is
/// added with [`Canvas::fill`]: it takes whatever the canvas still holds, and
/// the canvas guarantees it can never be squeezed away entirely.
pub(crate) struct Canvas<'a> {
    pieces: Vec<Piece<'a>>,
    free_box: Area,
}

/// A component the canvas owns, with the area granted to it.
struct Piece<'a> {
    component: Box<dyn Component + 'a>,
    area: Area,
}

/// Rows a [`Canvas`] refuses to allocate: the content piece must always be
/// able to show at least one row, so pieces are trimmed rather than crowding
/// it out.
const CONTENT_FLOOR: u16 = 1;

impl<'a> Canvas<'a> {
    /// A canvas covering the whole `width` × `height` box before anything is
    /// placed in it.
    pub(crate) fn new(width: u16, height: u16) -> Canvas<'a> {
        Canvas {
            pieces: Vec::new(),
            free_box: Area {
                col: 0,
                row: 0,
                width,
                height,
            },
        }
    }

    /// Move a component into this canvas.
    ///
    /// The area comes from the component's [`Component::area`]: the canvas
    /// hands the piece the box that is still free (cells other components
    /// already hold are carved away), and the component anchors the area it
    /// wants inside that box. The piece then claims the granted area for
    /// later components. A piece pinned to the bottom edge of the free
    /// space gives up rows rather than let the content piece fall below its
    /// floor. The granted area is handed back to the caller.
    ///
    /// The canvas models the free space as the rectangle of still-ungranted
    /// cells anchored to the box's top-left corner, so a piece must anchor
    /// its footprint to the bottom or right edge of the box it is handed (or
    /// claim the whole box); otherwise the rectangle the next piece sees
    /// would lose the top-left corner the model builds on. A piece that
    /// violates this is a bug in the piece, caught by the debug assertion.
    pub(crate) fn place(&mut self, component: impl Component + 'a) -> Area {
        let free = self.free_box;
        let mut granted = component.area(free);
        let spans_full_w = granted.width == free.width;
        let spans_full_h = granted.height == free.height;
        if (spans_full_w || spans_full_h) && (granted.height > 0 || granted.width > 0) {
            // Bar spanning full width OR full height: check if it would squeeze content below floor
            let mut test_granted = granted;
            let mut squeezed = Self::carve_free_box(free, test_granted);
            if squeezed.height < CONTENT_FLOOR {
                let deficit = CONTENT_FLOOR.saturating_sub(squeezed.height);
                test_granted.height = test_granted.height.saturating_sub(deficit);
                test_granted.row = free.row + free.height - test_granted.height;
                squeezed = Self::carve_free_box(free, test_granted);
            }
            if spans_full_h && squeezed.width < 1 {
                // Vertical bar squeezing content horizontally (not expected but handle)
                let deficit = 1u16.saturating_sub(squeezed.width);
                test_granted.width = test_granted.width.saturating_sub(deficit);
                test_granted.col = free.col + free.width - test_granted.width;
            }
            granted = test_granted;
        }
        debug_assert!(
            granted.is_empty() || granted.width == free.width || granted.height == free.height,
            "component must span full width OR full height (or both)"
        );
        self.free_box = Self::carve_free_box(free, granted);
        self.pieces.push(Piece {
            component: Box::new(component),
            area: granted,
        });
        granted
    }

    /// Move the content component into this canvas: give it everything the
    /// canvas still holds.
    ///
    /// Call this after the other components are placed: the content piece
    /// draws in whatever free cells they left. Placing another component after
    /// the fill leaves it nothing to grant.
    pub(crate) fn fill(&mut self, component: impl Component + 'a) -> Area {
        let area = self.free_box;
        self.free_box = Area {
            col: 0,
            row: 0,
            width: 0,
            height: 0,
        };
        self.pieces.push(Piece {
            component: Box::new(component),
            area,
        });
        area
    }

    /// Draw every placed component into the area it was granted, in the order
    /// they were added to the canvas.
    pub(crate) fn render(&self, out: &mut dyn io::Write) -> io::Result<()> {
        for piece in &self.pieces {
            piece.component.render(piece.area, out)?;
        }
        Ok(())
    }

    #[cfg(test)]
    /// The areas the pieces were granted, in the order they were added: the
    /// single source of truth for what each component may paint and where the
    /// mouse lands on it.
    pub(crate) fn areas(&self) -> Vec<Area> {
        self.pieces.iter().map(|piece| piece.area).collect()
    }

    /// What the pointer at `(col, row)` asks the viewer to do, from the piece
    /// that owns the gesture.
    ///
    /// A [`Gesture::Press`] is routed by containment: the first piece whose
    /// granted area covers the cell decides, and `None` when the cell belongs
    /// to no interactive piece. A [`Gesture::Drag`] is routed by who is
    /// grabbed, not by which cell the pointer lands on: the pieces are asked
    /// in order and the dragged one, the only one that answers, maps the
    /// pointer even when it has left its cells, so the thumb keeps following
    /// the mouse across the whole drag.
    pub(crate) fn press(
        &self,
        col: u16,
        row: u16,
        max_offset: usize,
        gesture: Gesture,
    ) -> Option<Request> {
        self.pieces.iter().find_map(|piece| {
            if gesture == Gesture::Drag || piece.area.contains(col, row) {
                piece
                    .component
                    .press(piece.area, (col, row), max_offset, gesture)
            } else {
                None
            }
        })
    }

    /// Carve a granted area from the free box, returning the new free box.
    /// Assumes granted is within free and spans full width OR full height
    /// (or both, for a greedy piece that yields to CONTENT_FLOOR).
    fn carve_free_box(free: Area, granted: Area) -> Area {
        let spans_full_w = granted.width == free.width;
        let spans_full_h = granted.height == free.height;

        debug_assert!(
            granted.is_empty() || spans_full_w || spans_full_h,
            "carve_free_box requires granted to span full width or full height"
        );

        if spans_full_w && spans_full_h {
            // Greedy piece spanning both dimensions
            if granted.row == free.row {
                // Top edge - carve down
                Area {
                    row: granted.row + granted.height,
                    height: free.height - granted.height,
                    ..free
                }
            } else {
                // Bottom edge (or middle, treat as bottom bar)
                Area {
                    height: granted.row - free.row,
                    ..free
                }
            }
        } else if spans_full_w {
            // Horizontal bar, carves vertically
            if granted.row == free.row {
                // Top bar
                Area {
                    row: granted.row + granted.height,
                    height: free.height - granted.height,
                    ..free
                }
            } else {
                // Bottom bar (must touch bottom)
                Area {
                    height: granted.row - free.row,
                    ..free
                }
            }
        } else {
            // Vertical bar, carves horizontally (spans_full_h must be true)
            if granted.col == free.col {
                // Left bar
                Area {
                    col: granted.col + granted.width,
                    width: free.width - granted.width,
                    ..free
                }
            } else {
                // Right bar (must touch right)
                Area {
                    width: granted.col - free.col,
                    ..free
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::Anchor;

    const COLS: u16 = 80;
    const ROWS: u16 = 24;

    struct Fake {
        footprint: (u16, u16, Anchor),
        mark: char,
    }

    impl Component for Fake {
        fn area(&self, boxed: Area) -> Area {
            boxed.place(self.footprint.0, self.footprint.1, self.footprint.2)
        }

        fn render(&self, area: Area, out: &mut dyn io::Write) -> io::Result<()> {
            write!(out, "{}{}", self.mark, area.width)
        }
    }

    fn base(footprint: (u16, u16, Anchor)) -> Fake {
        Fake {
            footprint,
            mark: 'B',
        }
    }

    fn box_area(col: u16, row: u16, width: u16, height: u16) -> Area {
        Area {
            col,
            row,
            width,
            height,
        }
    }

    fn helpbar_footprint() -> (u16, u16, Anchor) {
        (COLS, 2, Anchor::BottomLeft)
    }

    fn scrollbar_footprint() -> (u16, u16, Anchor) {
        (1, ROWS, Anchor::TopRight)
    }

    #[test]
    fn place_grants_the_area_a_piece_claims() {
        let mut frame = Canvas::new(COLS, ROWS);
        assert_eq!(
            frame.place(base(helpbar_footprint())),
            box_area(0, ROWS - 2, COLS, 2),
            "a piece that fits gets exactly the area it anchored, cut to the box"
        );
    }

    #[test]
    fn place_clips_a_colliding_area_to_the_free_space() {
        let mut frame = Canvas::new(COLS, ROWS);
        frame.place(base(helpbar_footprint()));
        assert_eq!(
            frame.place(base(scrollbar_footprint())),
            box_area(COLS - 1, 0, 1, ROWS - 2),
            "the corner already held by the help bar is carved off"
        );
    }

    #[test]
    fn fill_takes_everything_the_pieces_left() {
        let mut frame = Canvas::new(COLS, ROWS);
        frame.place(base(helpbar_footprint()));
        frame.place(base(scrollbar_footprint()));
        assert_eq!(
            frame.fill(base(footprint_everything())),
            box_area(0, 0, COLS - 1, ROWS - 2)
        );
    }

    fn footprint_everything() -> (u16, u16, Anchor) {
        (COLS, ROWS, Anchor::TopLeft)
    }

    #[test]
    fn placement_order_does_not_change_the_content_area() {
        let mut a = Canvas::new(COLS, ROWS);
        a.place(base(helpbar_footprint()));
        a.place(base(scrollbar_footprint()));
        let a = a.fill(base(footprint_everything()));

        let mut b = Canvas::new(COLS, ROWS);
        b.place(base(scrollbar_footprint()));
        b.place(base(helpbar_footprint()));
        let b = b.fill(base(footprint_everything()));

        assert_eq!(a, b);
    }

    #[test]
    fn an_empty_area_vanishes_and_takes_no_space() {
        let mut frame = Canvas::new(COLS, ROWS);
        let (w, _h, anchor) = footprint_everything();
        let empty = (w, 0, anchor);
        assert_eq!(frame.place(base(empty)).height, 0);
        assert_eq!(
            frame.fill(base(footprint_everything())).height,
            ROWS,
            "the vanished piece freed its space"
        );
    }

    #[test]
    fn a_piece_that_wants_everything_is_trimmed_for_content() {
        let mut frame = Canvas::new(COLS, 3);
        assert_eq!(
            frame.place(base(footprint_everything())).height,
            2,
            "the piece gives up one row"
        );
        assert_eq!(
            frame.fill(base(footprint_everything())).height,
            1,
            "content keeps its row"
        );
    }

    #[test]
    fn render_draws_in_insertion_order() {
        let mut frame = Canvas::new(COLS, ROWS);
        frame.place(Fake {
            footprint: box_area_footprint(0),
            mark: 'A',
        });
        frame.place(Fake {
            footprint: box_area_footprint(0),
            mark: 'B',
        });
        let mut out = Vec::new();
        frame.render(&mut out).unwrap();
        let out = String::from_utf8(out).unwrap();
        assert!(
            out.find('A').unwrap() < out.find('B').unwrap(),
            "pieces must draw in the order they were placed: {out:?}"
        );
    }

    fn box_area_footprint(_nothing: u8) -> (u16, u16, Anchor) {
        (COLS, 1, Anchor::BottomLeft)
    }

    #[test]
    fn render_hands_each_piece_the_area_it_was_granted() {
        let mut frame = Canvas::new(COLS, ROWS);
        let granted = frame.place(base(box_area_footprint(0)));
        let mut out = Vec::new();
        frame.render(&mut out).unwrap();
        let out = String::from_utf8(out).unwrap();
        assert!(
            out.contains(&format!("B{}", granted.width)),
            "the piece received its granted area: {out:?}"
        );
    }

    #[test]
    fn filling_before_placing_leaves_nothing_to_grant() {
        let mut frame = Canvas::new(COLS, ROWS);
        let filled = frame.fill(base(footprint_everything()));
        assert_eq!(
            filled,
            box_area(0, 0, COLS, ROWS),
            "fill takes the whole box"
        );
        assert_eq!(
            frame.place(base(box_area_footprint(0))).height,
            0,
            "a piece placed after the fill is granted nothing"
        );
    }
}
