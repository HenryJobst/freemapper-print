//! One rendered page as every backend needs it: drawing primitives in page millimetres (origin top
//! left, y down), the clip rectangle, the page size and the colours.

use std::sync::Arc;

pub use super::draw::{FillRule, Primitive, Shape};
pub use super::geometry::Pt;
pub use super::image::{ImageError, RasterImage};
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
    /// The sheet elements (title, texts, logos, scale bar, footer), painted after all map
    /// primitives in this order, over the whole page: they are not clipped to `clip`.
    pub sheet: Vec<SheetItem>,
}

/// A colour of a sheet element, given directly instead of by map colour priority. CMYK, so that
/// print colours match the map's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DirectColour {
    /// Cyan, magenta, yellow and black, each 0 to 1 (as [`RenderColour::cmyk`]).
    pub cmyk: [f64; 4],
}

impl DirectColour {
    pub const BLACK: DirectColour = DirectColour {
        cmyk: [0.0, 0.0, 0.0, 1.0],
    };
    pub const WHITE: DirectColour = DirectColour {
        cmyk: [0.0, 0.0, 0.0, 0.0],
    };
}

/// One element of the sheet layer.
#[derive(Debug, Clone, PartialEq)]
pub enum SheetItem {
    /// `shape` painted in `colour`. Text uses the same embedded fonts as map text.
    Shape { colour: DirectColour, shape: Shape },
    /// `image` stretched to fill `rect` exactly.
    Image {
        rect: PaperBox,
        image: Arc<RasterImage>,
    },
}
