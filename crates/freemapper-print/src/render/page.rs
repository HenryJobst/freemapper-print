//! One rendered page as every backend needs it: drawing primitives in page millimetres (origin top
//! left, y down), the clip rectangle, the page size and the colours.

use std::sync::Arc;

pub use super::draw::{FillRule, Primitive, Shape, SymbolClass};
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
    /// The point of the map that lies at the centre of the page, in mm at the print scale (the
    /// map's own coordinates times the geometry factor; y down as the page).
    pub centre_mm: Pt,
    pub colours: Vec<RenderColour>,
    /// The sheet elements (title, texts, logos, scale bar, footer), painted after all map
    /// primitives in this order, over the whole page: they are not clipped to `clip`.
    pub sheet: Vec<SheetItem>,
    /// The course overprint, clipped to `clip` like the map: its lower layer among the map primitives
    /// ([`Overprint::below_priority`]), its upper layer after them, both before the sheet layer. Empty
    /// on a page without a course.
    pub overprint: Overprint,
}

/// Shapes painted over the map in one colour with the blend mode Multiply, in two layers: the course
/// overprint (ADR 0034). The renderer knows only the shapes; what they show is decided elsewhere.
#[derive(Debug, Clone, PartialEq)]
pub struct Overprint {
    /// Painted among the map primitives, before the first one of [`Overprint::below_priority`].
    pub lower: Vec<Shape>,
    /// Painted over all map primitives.
    pub upper: Vec<Shape>,
    /// The priority before whose first primitive the lower layer is painted. `None` paints it after all map
    /// primitives, just before the upper layer.
    pub below_priority: Option<i32>,
    pub colour: DirectColour,
    /// A limit left out some of the shapes (hatch lines, dashes or crosses).
    pub cut: bool,
}

impl Default for Overprint {
    /// Nothing to paint, in black.
    fn default() -> Self {
        Overprint {
            lower: Vec::new(),
            upper: Vec::new(),
            below_priority: None,
            colour: DirectColour::BLACK,
            cut: false,
        }
    }
}

impl Overprint {
    /// Nothing to paint.
    pub fn is_empty(&self) -> bool {
        self.lower.is_empty() && self.upper.is_empty()
    }
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
