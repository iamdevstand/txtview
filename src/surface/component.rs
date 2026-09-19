use std::io;

use super::Area;

/// What a piece asks the viewer to do after handling a pointer gesture. The
/// variants are the viewer's verbs; a piece answers with the one it wants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Request {
    /// Start dragging this piece: grab it at the offset and keep the current
    /// viewport position for now.
    Grab { grab_offset: usize },
    /// Move the viewport so `target` is at the top and keep dragging, with
    /// the grab offset.
    DragTo { target: usize, grab_offset: usize },
}

/// The gesture event the canvas is asking a piece about.
///
/// A drag is a press on a piece that is already grabbed: the canvas asks
/// every piece and only the grabbed one answers, so a drag follows the piece
/// even when the pointer wanders off its cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Gesture {
    /// A fresh press, routed to the piece whose granted area covers the cell.
    Press,
    /// The pointer moving while a piece is grabbed.
    Drag,
}

/// A piece of the viewer: it works out the [`Area`] it wants to paint inside
/// a box it is handed (sizing its footprint and anchoring it to a corner or
/// edge via [`Area::place`]) and renders itself into the area the canvas
/// grants. The piece never holds or manages its granted area, the
/// [`Canvas`](super::Canvas) owns the association.
pub(crate) trait Component {
    /// The area this piece will paint, anchored inside the `box` it is handed.
    ///
    /// The canvas calls this with the box that is still free (cells other
    /// pieces already hold are carved away), so a piece never needs to know
    /// about the pieces around it.
    fn area(&self, boxed: Area) -> Area;

    /// Paint into `area`, the rectangle the canvas granted this piece.
    ///
    /// A piece must not write outside `area`: the canvas trusts it to honor
    /// its bounds, the way the bundled components do. The area may be empty
    /// when the frame has no room for the piece, in which case the render is
    /// expected to do nothing.
    fn render(&self, area: Area, out: &mut dyn io::Write) -> io::Result<()>;

    /// What a press or drag at `at` (a cell of the granted `area`) asks the
    /// viewer to do. A piece that handles input overrides this and answers
    /// with the [`Request`] it wants; every other piece inherits the default
    /// `None`.
    ///
    /// A piece maps the pointer from its own layout. On a
    /// [`Gesture::Press`] the canvas matched the cell to this piece's area,
    /// on a [`Gesture::Drag`] the piece is being dragged, so it maps from the
    /// grab offset it was created with.
    ///
    /// `max_offset` is the viewer's current range: the largest value a piece
    /// may ask the viewport to reach, which pieces calibrate positions
    /// against.
    fn press(
        &self,
        _area: Area,
        _at: (u16, u16),
        _max_offset: usize,
        _gesture: Gesture,
    ) -> Option<Request> {
        None
    }
}
