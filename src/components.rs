//! Concrete renderable pieces of the viewer, composed at one junction point.
//!
//! Each piece implements [`crate::surface::Component`]: it decides the area
//! it will paint (sizing its footprint and anchoring it to a corner or edge
//! of the box with an [`crate::surface::Anchor`]) and paints into the area
//! the [`crate::surface::Canvas`] grants. The pieces all divide the same
//! layout between them and influence each other's areas: [`HelpBar`]
//! announces how many rows it needs, so [`Content`] fills what is left at
//! the top, and [`ScrollBar`] reserves its right-hand column from the
//! content's wrap width.

pub(crate) mod content;
pub(crate) mod helpbar;
pub(crate) mod scrollbar;

pub(crate) use self::{
    content::Content,
    helpbar::HelpBar,
    scrollbar::{Orientation, ScrollBar},
};
