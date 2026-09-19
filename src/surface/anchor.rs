/// Where a piece pins the rectangle it will paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Anchor {
    /// The box's top-left corner.
    #[allow(dead_code)]
    TopLeft,
    /// The box's top-right corner.
    TopRight,
    /// The box's bottom-left corner.
    BottomLeft,
    /// The box's bottom-right corner.
    #[allow(dead_code)]
    BottomRight,
    /// The middle of the box, both horizontally and vertically.
    #[allow(dead_code)]
    Center,
}
