//! Copied from freemapper 743ce4b (ADR 0014).
//! The training map: OOM symbols drawn the way OOM draws them, written as PDF.

mod dash;
pub mod draw;
pub mod fonts;
pub mod geometry;
mod line;
mod pattern;
pub mod pdf;
pub mod symbols;

use draw::Primitive;
use geometry::Pt;
use symbols::RenderColour;

/// An axis-parallel rectangle on paper (mm, y down).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaperBox {
    pub min: Pt,
    pub max: Pt,
}

/// One page as every backend needs it, in page millimetres (origin top left, y down).
///
/// The PDF writer takes nothing else, so a later SVG or tile backend for the browser editors
/// draws the same value.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderedPage {
    /// Painted by colour priority, clipped to `clip`.
    pub primitives: Vec<Primitive>,
    pub clip: PaperBox,
    /// Width and height of the page: the paper size.
    pub page_mm: (f64, f64),
    pub colours: Vec<RenderColour>,
}
