//! One rendered page as every backend needs it: drawing primitives in page millimetres (origin top
//! left, y down), the clip rectangle, the page size and the colours.

pub use super::draw::{Primitive, Shape};
pub use super::geometry::Pt;
pub use super::symbols::{Cap, Join, RenderColour};

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
    /// The primitives in paint order: each one is painted over the ones before it. They are
    /// sorted by colour priority (highest number first, priority 0 on top) and keep the order of
    /// the map objects within one priority. A backend paints them as they come and clips them to
    /// `clip`.
    pub primitives: Vec<Primitive>,
    pub clip: PaperBox,
    /// Width and height of the page: the paper size.
    pub page_mm: (f64, f64),
    pub colours: Vec<RenderColour>,
}
