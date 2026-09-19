//! The building blocks of the render pipeline: [`Area`], the rectangle of
//! cells a piece paints; [`Anchor`], the corner or edge of a box a piece pins
//! its footprint to; [`Component`], a piece that decides its area and paints
//! into it; and [`Canvas`], the central place that keeps every component with
//! the area it was granted, sizes those grants against the space still free,
//! and draws them all. The concrete pieces live in [`crate::components`];
//! [`crate::txtview::render`] composes them into a canvas.

pub(crate) mod anchor;
pub(crate) mod area;
pub(crate) mod canvas;
pub(crate) mod component;

pub(crate) use self::{
    anchor::Anchor,
    area::Area,
    canvas::Canvas,
    component::{Component, Gesture, Request},
};
