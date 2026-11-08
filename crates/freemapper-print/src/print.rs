//! Exact-scale page output (design §5.2, ADR 0018).
//!
//! One millimetre at print scale is one page millimetre, and the page is exactly the paper. When the
//! print scale differs from the map scale, the geometry scales by map scale / print scale and the
//! symbols by the symbol factor the IOF standard sets for the map kind. Nothing is fitted to the page: a map larger than
//! the page is clipped, and the report says by how much it overflows.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::omap::enlarge::enlarge_symbols;
use crate::omap::{MapCoord, MapDocument, MapObject, MapObjectKind, OmapError, SymbolSet};
use crate::render::draw::{Primitive, Shape, primitives_counted};
use crate::render::fonts::substituted;
use crate::render::geometry::Pt;
use crate::render::pdf::{PdfMeta, write_pdf};
use crate::render::symbols::{RenderSymbols, Symbol};
use crate::scale::{MapKind, ScaleError, geometry_factor, is_standard, symbol_factor};

pub use crate::render::{PaperBox, RenderedPage};

/// White margin on each side of the page; the map is clipped to the page minus this margin (mm).
pub const PAGE_MARGIN_MM: f64 = 5.0;

/// The tag freemapper puts on the objects of a map's peripheral information.
const SHEET_TAG: (&str, &str) = ("freemapper:source", "sheet");

/// Micrometres (OOM map units) per millimetre.
const UM_PER_MM: f64 = 1000.0;

/// Paper sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paper {
    A4,
    A3,
}

impl Paper {
    /// Width and height in portrait orientation (mm).
    pub fn size_mm(self) -> (f64, f64) {
        match self {
            Paper::A4 => (210.0, 297.0),
            Paper::A3 => (297.0, 420.0),
        }
    }
}

/// Page orientation. `Auto` takes the orientation in which less of the map lies outside the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Auto,
    Portrait,
    Landscape,
}

/// How a map is put on a page.
#[derive(Debug, Clone, PartialEq)]
pub struct PrintOptions {
    /// Denominator of the print scale; `None` prints at the map scale.
    pub print_scale: Option<u32>,
    pub kind: MapKind,
    pub paper: Paper,
    pub orientation: Orientation,
    /// The map point (in the file's map coordinates) that lands at the centre of the page; `None`
    /// takes the centre of the bounding box of everything drawn.
    pub centre: Option<MapCoord>,
    /// Draw freemapper's information block (title, scale, scale bar, credits): the objects tagged
    /// `freemapper:source=sheet`. North lines (601) carry the tag too but belong to the map and are
    /// always drawn (ADR 0022).
    pub show_sheet_objects: bool,
}

/// What a page shows and what it could not show.
#[derive(Debug, Clone, PartialEq)]
pub struct PrintReport {
    /// Width and height of the page (mm): the paper in the chosen orientation.
    pub page_mm: (f64, f64),
    pub print_scale: u32,
    pub geometry_factor: f64,
    pub symbol_factor: f64,
    /// The standard does not foresee the print scale for the map kind (a reduction below the base
    /// scale): the symbols keep their base size, and the print is meant for training only.
    pub non_standard_scale: bool,
    /// How far (mm) the bounding box of the drawn objects reaches beyond the clip rectangle in x
    /// and in y, both sides together; `None` when everything fits.
    pub overflow_mm: Option<(f64, f64)>,
    /// Objects not drawn because their symbol is unknown or hidden (`is_hidden` in the file).
    /// Sheet objects left out by [`PrintOptions::show_sheet_objects`] do not count.
    pub skipped_objects: usize,
    /// Objects whose drawing [`crate::render::draw::MAX_PRIMITIVES_PER_OBJECT`] cut short.
    pub truncated_objects: usize,
    /// Font families of drawn texts that Arimo stands in for although it does not match them.
    pub substituted_fonts: Vec<String>,
    /// Characters of drawn texts that the embedded font has no glyph for (filled in by
    /// [`print_map`]; [`render_page`] leaves it empty).
    pub missing_glyphs: Vec<char>,
}

#[derive(Debug, thiserror::Error)]
pub enum PrintError {
    #[error(transparent)]
    Scale(#[from] ScaleError),
    #[error("could not resize the map symbols for the print scale: {0}")]
    Symbols(#[from] OmapError),
}

/// The page for `map` at the print scale of `options`, in page millimetres, ready for any backend.
///
/// Templates are never drawn. Objects of hidden or unknown symbols are skipped and counted, sheet
/// objects are left out unless [`PrintOptions::show_sheet_objects`] is set.
pub fn render_page(
    map: &MapDocument,
    options: &PrintOptions,
) -> Result<(RenderedPage, PrintReport), PrintError> {
    let map_scale = map.georef.scale_denominator;
    let print_scale = options.print_scale.unwrap_or(map_scale);
    let symbol_factor = symbol_factor(options.kind, map_scale, print_scale)?;
    let geometry_factor = geometry_factor(map_scale, print_scale);
    let symbols = render_symbols(&map.symbol_set, symbol_factor)?;

    let mut skipped_objects = 0;
    let mut objects = Vec::new();
    for object in map.parts.iter().flat_map(|p| &p.objects) {
        if is_sheet_object(object)
            && !options.show_sheet_objects
            && !is_north_line(&map.symbol_set, object)
        {
            continue;
        }
        if map.symbol_set.is_hidden(object.symbol_id)
            || !symbols.symbols.contains_key(&object.symbol_id)
        {
            skipped_objects += 1;
            continue;
        }
        objects.push(scaled(object, geometry_factor));
    }

    let extent = extent(&objects);
    let centre = match (options.centre, extent) {
        (Some(c), _) => (
            scale_um(c.x, geometry_factor),
            scale_um(c.y, geometry_factor),
        ),
        (None, Some(e)) => (
            ((e.min.0 + e.max.0) as f64 / 2.0).round() as i64,
            ((e.min.1 + e.max.1) as f64 / 2.0).round() as i64,
        ),
        (None, None) => (0, 0),
    };
    let page_um = page_um(options.paper, options.orientation, extent, centre);
    let offset = (page_um.0 / 2 - centre.0, page_um.1 / 2 - centre.1);
    let overflow_mm = extent
        .map(|e| overflow(e, offset, page_um))
        .filter(|o| o.dx > 0 || o.dy > 0)
        .map(|o| (o.dx as f64 / UM_PER_MM, o.dy as f64 / UM_PER_MM));

    let mut primitives = Vec::new();
    let mut truncated_objects = 0;
    let mut fonts = BTreeSet::new();
    for object in &objects {
        let (mut drawn, cut) = primitives_counted(object, &symbols);
        truncated_objects += usize::from(cut);
        primitives.append(&mut drawn);
        if let (MapObjectKind::Text { .. }, Some(Symbol::Text(def))) =
            (&object.kind, symbols.symbols.get(&object.symbol_id))
            && substituted(&def.family)
        {
            fonts.insert(def.family.trim().to_string());
        }
    }
    translate(
        &mut primitives,
        (offset.0 as f64 / UM_PER_MM, offset.1 as f64 / UM_PER_MM),
    );

    let page_mm = (page_um.0 as f64 / UM_PER_MM, page_um.1 as f64 / UM_PER_MM);
    let page = RenderedPage {
        primitives,
        clip: PaperBox {
            min: (PAGE_MARGIN_MM, PAGE_MARGIN_MM),
            max: (page_mm.0 - PAGE_MARGIN_MM, page_mm.1 - PAGE_MARGIN_MM),
        },
        page_mm,
        colours: symbols.colours,
    };
    let report = PrintReport {
        page_mm,
        print_scale,
        geometry_factor,
        symbol_factor,
        non_standard_scale: !is_standard(options.kind, print_scale),
        overflow_mm,
        skipped_objects,
        truncated_objects,
        substituted_fonts: fonts.into_iter().collect(),
        missing_glyphs: Vec::new(),
    };
    Ok((page, report))
}

/// The office PDF of `map`: [`render_page`] written by [`write_pdf`].
pub fn print_map(
    map: &MapDocument,
    options: &PrintOptions,
    title: &str,
) -> Result<(Vec<u8>, PrintReport), PrintError> {
    let (page, mut report) = render_page(map, options)?;
    let (pdf, info) = write_pdf(&page, &PdfMeta::new(title));
    report.missing_glyphs = info.missing_glyphs.into_iter().collect();
    Ok((pdf, report))
}

/// The symbols of `set` with every dimension multiplied by `factor`.
fn render_symbols(set: &SymbolSet, factor: f64) -> Result<RenderSymbols, OmapError> {
    if factor == 1.0 {
        return Ok(RenderSymbols::from_set(set));
    }
    // Enlarging keeps every id, so the code table and the hidden list stay valid.
    let mut enlarged = set.clone();
    enlarged.symbols_xml = enlarge_symbols(&set.symbols_xml, factor)?;
    Ok(RenderSymbols::from_set(&enlarged))
}

fn is_sheet_object(object: &MapObject) -> bool {
    object
        .tags
        .iter()
        .any(|(k, v)| k == SHEET_TAG.0 && v == SHEET_TAG.1)
}

/// A magnetic north line (601, or a variant 601.x): part of the map even where freemapper tags it
/// as a sheet object (ADR 0022).
fn is_north_line(set: &SymbolSet, object: &MapObject) -> bool {
    set.code_of(object.symbol_id)
        .is_some_and(|code| code == "601" || code.starts_with("601."))
}

/// `v` µm times `factor`, rounded to whole µm.
fn scale_um(v: i32, factor: f64) -> i64 {
    (f64::from(v) * factor).round() as i64
}

fn scale_coord(c: MapCoord, factor: f64) -> MapCoord {
    MapCoord {
        x: (f64::from(c.x) * factor).round() as i32,
        y: (f64::from(c.y) * factor).round() as i32,
        ..c
    }
}

/// The coordinates the geometry factor moves: all of them, but only the anchor of a text (its
/// size follows the symbol).
fn placed_coords(object: &MapObject) -> &[MapCoord] {
    match object.kind {
        MapObjectKind::Text { .. } => &object.coords[..object.coords.len().min(1)],
        _ => &object.coords,
    }
}

/// `object` with its geometry multiplied by `factor` (Bézier control points and pattern origin
/// included).
fn scaled(object: &MapObject, factor: f64) -> MapObject {
    let mut object = object.clone();
    if factor == 1.0 {
        return object;
    }
    let anchors = placed_coords(&object).len();
    for c in &mut object.coords[..anchors] {
        *c = scale_coord(*c, factor);
    }
    if let Some(p) = &mut object.pattern {
        p.origin = scale_coord(p.origin, factor);
    }
    object
}

/// An axis-parallel rectangle in µm.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Extent {
    min: (i64, i64),
    max: (i64, i64),
}

/// The bounding box of the placed coordinates of `objects`.
fn extent(objects: &[MapObject]) -> Option<Extent> {
    let mut points = objects
        .iter()
        .flat_map(placed_coords)
        .map(|c| (i64::from(c.x), i64::from(c.y)));
    let first = points.next()?;
    Some(points.fold(
        Extent {
            min: first,
            max: first,
        },
        |e, p| Extent {
            min: (e.min.0.min(p.0), e.min.1.min(p.1)),
            max: (e.max.0.max(p.0), e.max.1.max(p.1)),
        },
    ))
}

/// How far an extent reaches beyond the clip rectangle (µm), and the area outside it (µm²).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Overflow {
    dx: i64,
    dy: i64,
    area: f64,
}

fn overflow(extent: Extent, offset: (i64, i64), page_um: (i64, i64)) -> Overflow {
    let margin = (PAGE_MARGIN_MM * UM_PER_MM) as i64;
    let axis = |min: i64, max: i64, page: i64| {
        let (lo, hi) = (margin, page - margin);
        let out = (lo - min).max(0) + (max - hi).max(0);
        let inside = (max.min(hi) - min.max(lo)).max(0);
        (out, max - min, inside)
    };
    let (dx, w, w_in) = axis(extent.min.0 + offset.0, extent.max.0 + offset.0, page_um.0);
    let (dy, h, h_in) = axis(extent.min.1 + offset.1, extent.max.1 + offset.1, page_um.1);
    let area = w as f64 * h as f64 - w_in as f64 * h_in as f64;
    Overflow { dx, dy, area }
}

/// The page (µm) in the requested orientation. `Auto` takes the one whose overflowing area is
/// smaller, then the one with the smaller overflow lengths (a map of a single straight line has no
/// area), and portrait on a tie.
fn page_um(
    paper: Paper,
    orientation: Orientation,
    extent: Option<Extent>,
    centre: (i64, i64),
) -> (i64, i64) {
    let (w, h) = paper.size_mm();
    let portrait = ((w * UM_PER_MM) as i64, (h * UM_PER_MM) as i64);
    let landscape = (portrait.1, portrait.0);
    match (orientation, extent) {
        (Orientation::Portrait, _) | (Orientation::Auto, None) => portrait,
        (Orientation::Landscape, _) => landscape,
        (Orientation::Auto, Some(e)) => {
            let cost = |page: (i64, i64)| {
                let o = overflow(e, (page.0 / 2 - centre.0, page.1 / 2 - centre.1), page);
                (o.area, o.dx + o.dy)
            };
            let (p, l) = (cost(portrait), cost(landscape));
            if l.0 < p.0 || (l.0 == p.0 && l.1 < p.1) {
                landscape
            } else {
                portrait
            }
        }
    }
}

type Rings = Arc<[Vec<Pt>]>;

/// Moves every primitive by `d` (mm). The clipped shapes of one area pattern keep sharing their
/// rings.
fn translate(primitives: &mut [Primitive], d: Pt) {
    let mut shared: Option<(Rings, Rings)> = None;
    for p in primitives {
        translate_shape(&mut p.shape, d, &mut shared);
    }
}

fn translate_shape(shape: &mut Shape, d: Pt, shared: &mut Option<(Rings, Rings)>) {
    let shift = |p: &mut Pt| {
        p.0 += d.0;
        p.1 += d.1;
    };
    match shape {
        Shape::Fill { rings } => rings.iter_mut().flatten().for_each(shift),
        Shape::Stroke { path, .. } => path.iter_mut().for_each(shift),
        Shape::Text { origin, .. } => shift(origin),
        Shape::Clipped { rings, inner } => {
            let moved = match shared {
                Some((from, to)) if Arc::ptr_eq(from, rings) => Arc::clone(to),
                _ => {
                    let to: Rings = rings
                        .iter()
                        .map(|r| r.iter().map(|&(x, y)| (x + d.0, y + d.1)).collect())
                        .collect();
                    *shared = Some((Arc::clone(rings), Arc::clone(&to)));
                    to
                }
            };
            *rings = moved;
            translate_shape(inner, d, shared);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::omap::{MapGeoref, MapPart};

    fn map(scale: u32, set: SymbolSet, objects: Vec<MapObject>) -> MapDocument {
        MapDocument {
            notes: String::new(),
            georef: MapGeoref {
                scale_denominator: scale,
                declination_deg: 0.0,
                grivation_deg: 0.0,
                ref_projected: None,
                crs_spec: None,
            },
            symbol_set: set,
            parts: vec![MapPart {
                name: "default part".into(),
                objects,
            }],
            templates: Vec::new(),
        }
    }

    fn line(symbol_id: u32, to: (i32, i32)) -> MapObject {
        MapObject {
            symbol_id,
            kind: MapObjectKind::Path,
            coords: vec![MapCoord::new(0, 0), MapCoord::new(to.0, to.1)],
            tags: Vec::new(),
            pattern: None,
        }
    }

    fn edge() -> u32 {
        SymbolSet::issprom_2019().id_for_code("501.1").unwrap()
    }

    fn options(orientation: Orientation) -> PrintOptions {
        PrintOptions {
            print_scale: None,
            kind: MapKind::Sprint,
            paper: Paper::A4,
            orientation,
            centre: None,
            show_sheet_objects: false,
        }
    }

    fn render(doc: &MapDocument, options: &PrintOptions) -> (RenderedPage, PrintReport) {
        render_page(doc, options).unwrap()
    }

    #[test]
    fn auto_turns_a_wide_map_to_landscape() {
        let doc = map(
            4000,
            SymbolSet::issprom_2019(),
            vec![line(edge(), (250_000, 150_000))],
        );
        let (page, report) = render(&doc, &options(Orientation::Auto));
        assert_eq!(page.page_mm, (297.0, 210.0));
        assert_eq!(report.overflow_mm, None);
        let (_, report) = render(&doc, &options(Orientation::Portrait));
        assert_eq!(report.overflow_mm, Some((50.0, 0.0)));
    }

    #[test]
    fn auto_turns_a_long_straight_line_too() {
        // No overflowing area in either orientation; the overflow length decides.
        let doc = map(
            4000,
            SymbolSet::issprom_2019(),
            vec![line(edge(), (250_000, 0))],
        );
        let (page, _) = render(&doc, &options(Orientation::Auto));
        assert_eq!(page.page_mm, (297.0, 210.0));
    }

    #[test]
    fn auto_prefers_portrait_on_a_tie() {
        let doc = map(
            4000,
            SymbolSet::issprom_2019(),
            vec![line(edge(), (20_000, 20_000))],
        );
        let (page, _) = render(&doc, &options(Orientation::Auto));
        assert_eq!(page.page_mm, (210.0, 297.0));
        let empty = map(4000, SymbolSet::issprom_2019(), Vec::new());
        let (page, report) = render(&empty, &options(Orientation::Auto));
        assert_eq!(page.page_mm, (210.0, 297.0));
        assert!(page.primitives.is_empty() && report.overflow_mm.is_none());
    }

    #[test]
    fn hidden_symbols_are_skipped() {
        let set = SymbolSet::issprom_2019().with_additions(
            r#"<symbol type="2" id="@" code="999.1" name="hidden" is_hidden="true"><line_symbol color="2" line_width="100"/></symbol>"#,
        );
        let hidden = set.id_for_code("999.1").unwrap();
        assert!(set.is_hidden(hidden));
        let doc = map(4000, set, vec![line(hidden, (25_000, 0))]);
        let (page, report) = render(&doc, &options(Orientation::Portrait));
        assert!(page.primitives.is_empty());
        assert_eq!(report.skipped_objects, 1);
    }

    #[test]
    fn centre_option_puts_a_map_point_in_the_middle() {
        let doc = map(
            4000,
            SymbolSet::issprom_2019(),
            vec![line(edge(), (25_000, 0))],
        );
        let at_origin = PrintOptions {
            print_scale: Some(5000),
            centre: Some(MapCoord::new(0, 0)),
            ..options(Orientation::Portrait)
        };
        let (page, _) = render(&doc, &at_origin);
        let Shape::Stroke { path, .. } = &page.primitives[0].shape else {
            panic!()
        };
        assert!((path[0].0 - 105.0).abs() < 1e-9 && (path[0].1 - 148.5).abs() < 1e-9);
        assert!((path[1].0 - 125.0).abs() < 1e-9, "{path:?}");
    }

    #[test]
    fn text_anchor_scales_and_size_follows_the_symbol() {
        let set = SymbolSet::issprom_2019();
        let id = set.id_for_code("704").unwrap();
        let text = MapObject {
            symbol_id: id,
            kind: MapObjectKind::Text {
                text: "Wald".into(),
                h_align: 0,
                v_align: 0,
                rotation_rad: 0.0,
            },
            coords: vec![MapCoord::new(10_000, 0)],
            tags: Vec::new(),
            pattern: None,
        };
        let doc = map(15000, set, vec![text]);
        let at = |print_scale| {
            let options = PrintOptions {
                print_scale: Some(print_scale),
                kind: MapKind::Forest,
                centre: Some(MapCoord::new(0, 0)),
                ..options(Orientation::Portrait)
            };
            let (page, _) = render(&doc, &options);
            match &page.primitives[..] {
                [p] => match &p.shape {
                    Shape::Text { origin, size, .. } => (origin.0 - 105.0, *size),
                    other => panic!("{other:?}"),
                },
                other => panic!("{other:?}"),
            }
        };
        let (x_base, size_base) = at(15000);
        let (x_big, size_big) = at(10000);
        assert!((x_big / x_base - 1.5).abs() < 1e-6, "{x_base} {x_big}");
        assert!(
            (size_big / size_base - 1.5).abs() < 1e-3,
            "{size_base} {size_big}"
        );
    }

    #[test]
    fn substituted_fonts_are_reported() {
        let mut set = SymbolSet::issprom_2019();
        set.symbols_xml = set
            .symbols_xml
            .replace(r#"family="Arial""#, r#"family="Frutiger""#);
        let id = set.id_for_code("704").unwrap();
        let text = MapObject {
            symbol_id: id,
            kind: MapObjectKind::Text {
                text: "Wald".into(),
                h_align: 0,
                v_align: 0,
                rotation_rad: 0.0,
            },
            coords: vec![MapCoord::new(0, 0)],
            tags: Vec::new(),
            pattern: None,
        };
        let doc = map(4000, set, vec![text.clone(), text]);
        let (_, report) = render(&doc, &options(Orientation::Portrait));
        assert_eq!(report.substituted_fonts, ["Frutiger"]);
    }

    #[test]
    fn print_scale_outside_the_table_is_refused() {
        let doc = map(4000, SymbolSet::issprom_2019(), Vec::new());
        let options = PrintOptions {
            print_scale: Some(10000),
            ..options(Orientation::Portrait)
        };
        let err = render_page(&doc, &options).unwrap_err();
        assert!(matches!(err, PrintError::Scale(_)));
        assert!(err.to_string().contains("1:3000, 1:4000, 1:5000"), "{err}");
    }

    #[test]
    fn missing_glyphs_reach_the_report() {
        let set = SymbolSet::issprom_2019();
        let id = set.id_for_code("704").unwrap();
        let text = MapObject {
            symbol_id: id,
            kind: MapObjectKind::Text {
                text: "Wald ✈".into(),
                h_align: 0,
                v_align: 0,
                rotation_rad: 0.0,
            },
            coords: vec![MapCoord::new(0, 0)],
            tags: Vec::new(),
            pattern: None,
        };
        let doc = map(4000, set, vec![text]);
        let (_, report) = print_map(&doc, &options(Orientation::Portrait), "Glyphs").unwrap();
        assert_eq!(report.missing_glyphs, ['✈']);
    }

    #[test]
    fn pattern_rings_stay_shared_after_moving() {
        let rings: Rings = vec![vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)]].into();
        let clipped = |rings: &Rings| Primitive {
            priority: 2,
            shape: Shape::Clipped {
                rings: Arc::clone(rings),
                inner: Box::new(Shape::Fill {
                    rings: vec![vec![(0.0, 0.0), (1.0, 1.0), (0.0, 1.0)]],
                }),
            },
        };
        let mut primitives = vec![clipped(&rings), clipped(&rings)];
        translate(&mut primitives, (10.0, 20.0));
        let (Shape::Clipped { rings: a, inner }, Shape::Clipped { rings: b, .. }) =
            (&primitives[0].shape, &primitives[1].shape)
        else {
            panic!()
        };
        assert!(Arc::ptr_eq(a, b));
        assert_eq!(a[0][1], (11.0, 20.0));
        assert_eq!(
            **inner,
            Shape::Fill {
                rings: vec![vec![(10.0, 20.0), (11.0, 21.0), (10.0, 21.0)]]
            }
        );
    }
}
