use super::Anchor;

/// A rectangle of terminal cells.
///
/// Every `Area` is handed out by the [`Canvas`](super::Canvas) the frame
/// paints into. A piece renders only inside its `Area` and never outside it:
/// the canvas does not clip a component's output, so the bundled components
/// honor the rectangle they are given, and the tests check that no piece
/// writes past it.
///
/// An `Area` also does the anchoring job: a piece decides the footprint it
/// wants, pins it to an [`Anchor`], and [`Area::place`] turns that footprint
/// into the concrete rectangle that will be granted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Area {
    pub col: u16,
    pub row: u16,
    pub width: u16,
    pub height: u16,
}

impl Area {
    /// Whether the rectangle covers no cells, the viewport never hands an
    /// empty area to a base piece, but a component can receive one when the
    /// frame has no room for it.
    pub(crate) fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Whether the `(col, row)` cell lies inside this rectangle.
    pub(crate) fn contains(self, col: u16, row: u16) -> bool {
        col >= self.col
            && row >= self.row
            && col.saturating_sub(self.col) < self.width
            && row.saturating_sub(self.row) < self.height
    }

    /// The rectangle a `width` × `height` footprint describes inside this
    /// box once it is pinned to `anchor`: the footprint placed and clipped to
    /// this area. A footprint taller, wider, or elsewhere than the box shrinks
    /// to fit.
    pub(crate) fn place(self, width: u16, height: u16, anchor: Anchor) -> Area {
        let width = width.min(self.width);
        let height = height.min(self.height);
        let (col, row) = match anchor {
            Anchor::TopLeft => (self.col, self.row),
            Anchor::TopRight => (self.col + self.width - width, self.row),
            Anchor::BottomLeft => (self.col, self.row + self.height - height),
            Anchor::BottomRight => (
                self.col + self.width - width,
                self.row + self.height - height,
            ),
            Anchor::Center => (
                self.col + (self.width - width) / 2,
                self.row + (self.height - height) / 2,
            ),
        };
        Area {
            col,
            row,
            width,
            height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn box_area() -> Area {
        Area {
            col: 1,
            row: 2,
            width: 10,
            height: 6,
        }
    }

    fn box_area_with(col: u16, row: u16) -> Area {
        Area {
            col,
            row,
            width: 2,
            height: 3,
        }
    }

    #[test]
    fn place_pins_to_the_anchored_corner() {
        let place = |anchor| box_area().place(2, 3, anchor);
        assert_eq!(place(Anchor::TopLeft), box_area_with(1, 2));
        assert_eq!(place(Anchor::TopRight), box_area_with(9, 2));
        assert_eq!(place(Anchor::BottomLeft), box_area_with(1, 5));
        assert_eq!(place(Anchor::BottomRight), box_area_with(9, 5));
        assert_eq!(
            place(Anchor::Center),
            box_area_with(5, 3),
            "the footprint must sit in the middle of the box"
        );
    }

    #[test]
    fn place_clips_a_larger_footprint_to_the_box() {
        assert_eq!(
            box_area().place(u16::MAX, u16::MAX, Anchor::BottomRight),
            box_area()
        );
    }
}
