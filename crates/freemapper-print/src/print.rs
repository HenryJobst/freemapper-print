//! Exact-scale page output (design §5.2, ADR 0018).
//!
//! One millimetre at print scale is one page millimetre, and the page is exactly the paper. When the
//! print scale differs from the map scale, the geometry scales by map scale / print scale and the
//! symbols by the symbol factor the IOF standard sets for the map kind. Nothing is fitted to the page: a map larger than
//! the page is clipped, and the report says by how much it overflows.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use crate::omap::enlarge::enlarge_symbols;
use crate::omap::{MapCoord, MapDocument, MapObject, MapObjectKind, OmapError, SymbolSet};
use crate::project::SheetLang;
use crate::render::bounds::{Bounds, painted_bounds};
use crate::render::draw::{MAX_UNITS_PER_OBJECT, Primitive, Shape, primitives_within};
use crate::render::fonts::substituted;
use crate::render::geometry::Pt;
use crate::render::page::{PaperBox, RenderedPage};
use crate::render::pdf::{PdfError, PdfMeta, write_pdf};
use crate::render::symbols::{RenderSymbols, Symbol};
use crate::scale::{MapKind, ScaleError, geometry_factor, is_standard, symbol_factor};
use crate::sheet::SheetNote;

/// White margin on each side of the page for the `render` command; the map is clipped to the page
/// minus this margin (mm). A layout sets its own ([`PrintOptions::frame_margin_mm`]).
pub const PAGE_MARGIN_MM: f64 = 5.0;

/// Most units of work one page may use, counted as for one object (`MAX_UNITS_PER_OBJECT`): a
/// primitive costs one unit plus one per eight points it keeps, and a clip outline one per
/// eight points for each clip block the PDF writes. Lübeck (2173 objects, about 4400
/// primitives) uses about 6500 units and the forest sample (539 objects, about 1500
/// primitives) at most about 6400 at every allowed scale, so a dense real map stays far below
/// it. A file that asks for more is broken or hostile; the objects after the limit are not
/// drawn and counted ([`PrintReport::objects_not_drawn_page_limit`]).
///
/// A page keeps at most four million points (500 000 units of at most eight points each),
/// clip outlines counted once per block. Measured peak memory (release build, macOS): about
/// 200 MB on a 134 KB file of 500 000 strokes of seven points, the largest of the hostile
/// files tried (ADR 0019).
pub const MAX_UNITS_PER_PAGE: usize = 500_000;

/// The tag freemapper puts on the objects of a map's peripheral information.
const SHEET_TAG: (&str, &str) = ("freemapper:source", "sheet");

/// Micrometres (OOM map units) per millimetre.
const UM_PER_MM: f64 = 1000.0;

/// Paper sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
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
    /// takes the centre of the box everything drawn paints in. A point far outside the map moves
    /// the map off the page: the overflow then counts from the page edge to the far side of the
    /// map and can exceed the map's own size.
    pub centre: Option<MapCoord>,
    /// Draw freemapper's information block (title, scale, scale bar, credits): the objects tagged
    /// `freemapper:source=sheet`. North lines (601) carry the tag too but belong to the map and are
    /// always drawn (ADR 0023).
    pub show_sheet_objects: bool,
    /// Width (mm) of the white border between the page edge and the map frame, the same on every
    /// side: the map is clipped to the page minus this border, centred in what is left, and the
    /// overflow is measured against it. `None` takes [`PAGE_MARGIN_MM`] (the `render` command);
    /// a layout passes its passe-partout. A margin wider than half the page leaves a frame of
    /// zero width or height.
    pub frame_margin_mm: Option<f64>,
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
    /// How far (mm) the box the drawn primitives paint in (line widths, symbols and texts
    /// included) reaches beyond the clip rectangle in x and in y, both sides together; `None`
    /// when everything fits.
    pub overflow_mm: Option<(f64, f64)>,
    /// Objects not drawn because their symbol is unknown or hidden (`is_hidden` in the file).
    /// Sheet objects left out by [`PrintOptions::show_sheet_objects`] do not count.
    pub skipped_objects: usize,
    /// Objects of a type the reader does not know ([`MapDocument::unknown_objects`]).
    pub unknown_type_objects: usize,
    /// Objects whose drawing a limit cut short: the per-object budget (`MAX_UNITS_PER_OBJECT`),
    /// the symbol nesting depth, a pattern's line cap, or the rest of the page budget.
    pub truncated_objects: usize,
    /// Objects not drawn at all because the page budget ([`MAX_UNITS_PER_PAGE`]) was spent.
    pub objects_not_drawn_page_limit: usize,
    /// Drawn objects whose symbol lost an area pattern when read, because the pattern broke a
    /// limit (spacing or point distance below 0.05 mm, a number that is not finite).
    pub dropped_pattern_objects: usize,
    /// Font families of drawn texts that Arimo stands in for although it does not match them.
    pub substituted_fonts: Vec<String>,
    /// Characters of drawn texts that the embedded font has no glyph for (filled in by
    /// [`print_map`]; [`render_page`] leaves it empty).
    pub missing_glyphs: Vec<char>,
}

impl PrintReport {
    /// Whether the report has anything to tell beyond the scales and factors: a non-standard
    /// scale, overflow, objects left out or cut, replaced fonts or missing glyphs.
    pub fn has_notes(&self) -> bool {
        self.non_standard_scale
            || self.overflow_mm.is_some_and(|(x, y)| x > 0.0 || y > 0.0)
            || self.skipped_objects > 0
            || self.unknown_type_objects > 0
            || self.truncated_objects > 0
            || self.objects_not_drawn_page_limit > 0
            || self.dropped_pattern_objects > 0
            || !self.substituted_fonts.is_empty()
            || !self.missing_glyphs.is_empty()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PrintError {
    #[error(transparent)]
    Scale(#[from] ScaleError),
    #[error("could not resize the map symbols for the print scale")]
    Symbols(#[from] OmapError),
    #[error(transparent)]
    Pdf(#[from] PdfError),
}

/// The page for `map` at the print scale of `options`, in page millimetres, ready for any backend.
///
/// Templates are never drawn. Objects of hidden or unknown symbols are skipped and counted, sheet
/// objects are left out unless [`PrintOptions::show_sheet_objects`] is set.
pub fn render_page(
    map: &MapDocument,
    options: &PrintOptions,
) -> Result<(RenderedPage, PrintReport), PrintError> {
    render_page_within(map, options, MAX_UNITS_PER_PAGE)
}

/// [`render_page`] with a page budget of `page_budget` units.
fn render_page_within(
    map: &MapDocument,
    options: &PrintOptions,
    page_budget: usize,
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

    // Draw everything in print-scale millimetres of the map, then place it on the page.
    let mut primitives = Vec::new();
    let mut page_left = page_budget;
    let mut truncated_objects = 0;
    let mut objects_not_drawn_page_limit = 0;
    let mut dropped_pattern_objects = 0;
    let mut fonts = BTreeSet::new();
    for object in &objects {
        if page_left == 0 {
            objects_not_drawn_page_limit += 1;
            continue;
        }
        let mut drawn = primitives_within(object, &symbols, page_left.min(MAX_UNITS_PER_OBJECT));
        page_left = page_left.saturating_sub(drawn.units);
        truncated_objects += usize::from(drawn.cut);
        dropped_pattern_objects += usize::from(
            object.kind == MapObjectKind::Path
                && symbols.dropped_patterns.contains(&object.symbol_id),
        );
        primitives.append(&mut drawn.primitives);
        if let (MapObjectKind::Text { .. }, Some(Symbol::Text(def))) =
            (&object.kind, symbols.symbols.get(&object.symbol_id))
            && substituted(&def.family)
        {
            fonts.insert(def.family.trim().to_string());
        }
    }
    // Paint order: highest priority number first, priority 0 last (on top). Stable, so the
    // objects keep their order within a colour.
    primitives.sort_by_key(|p| std::cmp::Reverse(p.priority));

    let bounds = painted_bounds(&primitives);
    let centre = match (options.centre, bounds) {
        (Some(c), _) => (
            f64::from(c.x) * geometry_factor / UM_PER_MM,
            f64::from(c.y) * geometry_factor / UM_PER_MM,
        ),
        (None, Some((min, max))) => ((min.0 + max.0) / 2.0, (min.1 + max.1) / 2.0),
        (None, None) => (0.0, 0.0),
    };
    let margin = options.frame_margin_mm.unwrap_or(PAGE_MARGIN_MM);
    let page_mm = page_size(options.paper, options.orientation, bounds, centre, margin);
    let frame = frame_box(page_mm, margin);
    // The centre of the frame is the centre of the page: the margin is the same on every side.
    let offset = (page_mm.0 / 2.0 - centre.0, page_mm.1 / 2.0 - centre.1);
    let overflow_mm = bounds
        .map(|b| overflow(b, offset, frame))
        .filter(|o| o.dx > 0.0 || o.dy > 0.0)
        .map(|o| (o.dx, o.dy));
    translate(&mut primitives, offset);

    let page = RenderedPage {
        primitives,
        clip: frame,
        page_mm,
        centre_mm: centre,
        colours: symbols.colours,
        sheet: Vec::new(),
    };
    let report = PrintReport {
        page_mm,
        print_scale,
        geometry_factor,
        symbol_factor,
        non_standard_scale: !is_standard(options.kind, print_scale),
        overflow_mm,
        skipped_objects,
        unknown_type_objects: map.unknown_objects,
        truncated_objects,
        objects_not_drawn_page_limit,
        dropped_pattern_objects,
        substituted_fonts: fonts.into_iter().collect(),
        missing_glyphs: Vec::new(),
    };
    Ok((page, report))
}

/// [`render_page`] with the default footer of the `render` command: the copyright lines from the
/// map notes and the visible credit line in `lang` ([`crate::sheet::add_map_footer`]). The map
/// stays where [`render_page`] puts it. The notes are about the footer.
pub fn render_with_footer(
    map: &MapDocument,
    options: &PrintOptions,
    lang: SheetLang,
) -> Result<(RenderedPage, PrintReport, Vec<SheetNote>), PrintError> {
    let (mut page, report) = render_page(map, options)?;
    let notes = crate::sheet::add_map_footer(&mut page, map, lang);
    Ok((page, report, notes))
}

/// The office PDF of `map`: [`render_with_footer`] written by [`write_pdf`], with the notes about
/// the footer.
pub fn print_map(
    map: &MapDocument,
    options: &PrintOptions,
    title: &str,
    lang: SheetLang,
) -> Result<(Vec<u8>, PrintReport, Vec<SheetNote>), PrintError> {
    let (page, mut report, notes) = render_with_footer(map, options, lang)?;
    let (pdf, info) = write_pdf(&page, &PdfMeta::new(title))?;
    report.missing_glyphs = info.missing_glyphs.into_iter().collect();
    Ok((pdf, report, notes))
}

/// The symbols of `set` with every dimension multiplied by `factor`.
fn render_symbols(set: &SymbolSet, factor: f64) -> Result<RenderSymbols, OmapError> {
    if factor == 1.0 {
        return RenderSymbols::from_set(set);
    }
    // Enlarging keeps every id, so the code table and the hidden list stay valid.
    let mut enlarged = set.clone();
    enlarged.symbols_xml = enlarge_symbols(&set.symbols_xml, factor)?;
    RenderSymbols::from_set(&enlarged)
}

fn is_sheet_object(object: &MapObject) -> bool {
    object
        .tags
        .iter()
        .any(|(k, v)| k == SHEET_TAG.0 && v == SHEET_TAG.1)
}

/// A magnetic north line (601, or a variant 601.x): part of the map even where freemapper tags it
/// as a sheet object (ADR 0023).
pub(crate) fn is_north_line(set: &SymbolSet, object: &MapObject) -> bool {
    set.code_of(object.symbol_id)
        .is_some_and(|code| code == "601" || code.starts_with("601."))
}

/// `c` times `factor`, rounded to whole µm. The geometry factor is at most 4 (a forest map drawn
/// at 1:20000 printed at 1:5000), so a coordinate beyond ±537 m of map paper leaves the `i32`
/// range. `as i32` then saturates at ±2147 m: such a point lies kilometres off any page either
/// way, so the page and its overflow report stay as they would be (the overflow counts to the
/// saturated point).
fn scale_coord(c: MapCoord, factor: f64) -> MapCoord {
    MapCoord {
        x: (f64::from(c.x) * factor).round() as i32,
        y: (f64::from(c.y) * factor).round() as i32,
        ..c
    }
}

/// `object` with its geometry multiplied by `factor` (Bézier control points, pattern origin and
/// the box of a box text included).
fn scaled(object: &MapObject, factor: f64) -> MapObject {
    let mut object = object.clone();
    if factor == 1.0 {
        return object;
    }
    for c in &mut object.coords {
        *c = scale_coord(*c, factor);
    }
    if let Some(p) = &mut object.pattern {
        p.origin = scale_coord(p.origin, factor);
    }
    if let MapObjectKind::Text {
        box_size: Some((w, h)),
        ..
    } = &mut object.kind
    {
        *w *= factor;
        *h *= factor;
    }
    object
}

/// How far painted bounds reach beyond the clip rectangle (mm), and the area outside it (mm²).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Overflow {
    dx: f64,
    dy: f64,
    area: f64,
}

/// The map frame on a page of `page_mm`: the page minus `margin_mm` on every side. A margin of
/// more than half the page leaves a frame of zero width or height in the middle of the page.
pub fn frame_box(page_mm: (f64, f64), margin_mm: f64) -> PaperBox {
    // `max` first: a NaN margin becomes 0.
    let mx = margin_mm.max(0.0).min(page_mm.0 / 2.0);
    let my = margin_mm.max(0.0).min(page_mm.1 / 2.0);
    PaperBox {
        min: (mx, my),
        max: (page_mm.0 - mx, page_mm.1 - my),
    }
}

/// The overflow of `bounds` moved by `offset` beyond `frame`.
fn overflow((min, max): Bounds, offset: Pt, frame: PaperBox) -> Overflow {
    let axis = |min: f64, max: f64, lo: f64, hi: f64| {
        let out = (lo - min).max(0.0) + (max - hi).max(0.0);
        let inside = (max.min(hi) - min.max(lo)).max(0.0);
        (out, max - min, inside)
    };
    let (dx, w, w_in) = axis(min.0 + offset.0, max.0 + offset.0, frame.min.0, frame.max.0);
    let (dy, h, h_in) = axis(min.1 + offset.1, max.1 + offset.1, frame.min.1, frame.max.1);
    Overflow {
        dx,
        dy,
        area: w * h - w_in * h_in,
    }
}

/// The page (mm) in the requested orientation. `Auto` takes the one whose overflowing area beyond
/// the frame (the page minus `margin_mm`) is smaller, then the one with the smaller overflow
/// lengths (a map of a single straight line has almost no area), and portrait on a tie.
fn page_size(
    paper: Paper,
    orientation: Orientation,
    bounds: Option<Bounds>,
    centre: Pt,
    margin_mm: f64,
) -> (f64, f64) {
    let portrait = paper.size_mm();
    let landscape = (portrait.1, portrait.0);
    match (orientation, bounds) {
        (Orientation::Portrait, _) | (Orientation::Auto, None) => portrait,
        (Orientation::Landscape, _) => landscape,
        (Orientation::Auto, Some(b)) => {
            let cost = |page: (f64, f64)| {
                let o = overflow(
                    b,
                    (page.0 / 2.0 - centre.0, page.1 / 2.0 - centre.1),
                    frame_box(page, margin_mm),
                );
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

/// Moved outlines by the address of the original, which the entry holds so that no address
/// is reused.
type MovedOutlines = HashMap<usize, (Rings, Rings)>;

/// Moves every primitive by `d` (mm). Clipped shapes that share an outline keep sharing it,
/// at every nesting level: each outline is moved once.
fn translate(primitives: &mut [Primitive], d: Pt) {
    let mut moved = MovedOutlines::new();
    for p in primitives {
        translate_shape(&mut p.shape, d, &mut moved);
    }
}

fn translate_shape(shape: &mut Shape, d: Pt, moved: &mut MovedOutlines) {
    let shift = |p: &mut Pt| {
        p.0 += d.0;
        p.1 += d.1;
    };
    match shape {
        Shape::Fill { rings, .. } => rings.iter_mut().flatten().for_each(shift),
        Shape::Stroke { path, .. } => path.iter_mut().for_each(shift),
        Shape::Text { origin, .. } => shift(origin),
        Shape::Clipped { rings, inner } => {
            let key = Arc::as_ptr(rings).cast::<()>().addr();
            let (_, to) = moved.entry(key).or_insert_with(|| {
                let to: Rings = rings
                    .iter()
                    .map(|r| r.iter().map(|&(x, y)| (x + d.0, y + d.1)).collect())
                    .collect();
                (Arc::clone(rings), to)
            });
            *rings = Arc::clone(to);
            translate_shape(inner, d, moved);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::omap::{MapGeoref, MapPart};
    use crate::render::draw::FillRule;

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
            unknown_objects: 0,
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
            frame_margin_mm: None,
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
        // 250 mm of line plus half its 0.1 mm width (501.1) at each end, on 200 mm of printable
        // width.
        let (dx, dy) = report.overflow_mm.unwrap();
        assert!((dx - 50.1).abs() < 1e-9 && dy == 0.0, "{dx} {dy}");
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
    fn sheet_tagged_north_line_variants_are_drawn() {
        // A variant 601.x (601.5, the blue north line) is a north line too (ADR 0023); a code
        // that only starts with the digits is not.
        let set = SymbolSet::issprom_2019().with_additions(
            r#"<symbol type="2" id="@" code="6011" name="other"><line_symbol color="2" line_width="100"/></symbol>"#,
        );
        let sheet = |code: &str| MapObject {
            tags: vec![(SHEET_TAG.0.into(), SHEET_TAG.1.into())],
            ..line(set.id_for_code(code).unwrap(), (25_000, 0))
        };
        let doc = map(4000, set.clone(), vec![sheet("601.5")]);
        let (page, _) = render(&doc, &options(Orientation::Portrait));
        assert_eq!(page.primitives.len(), 1);
        let doc = map(4000, set.clone(), vec![sheet("6011")]);
        let (page, _) = render(&doc, &options(Orientation::Portrait));
        assert!(page.primitives.is_empty());
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
                box_size: None,
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
    fn box_scales_with_the_geometry() {
        let set = SymbolSet::issprom_2019();
        let text = MapObject {
            symbol_id: set.id_for_code("704").unwrap(),
            kind: MapObjectKind::Text {
                text: "Wald".into(),
                h_align: 0,
                v_align: 1,
                rotation_rad: 0.0,
                box_size: Some((40_000.0, 20_000.0)),
            },
            coords: vec![MapCoord::new(10_000, 0)],
            tags: Vec::new(),
            pattern: None,
        };
        let MapObjectKind::Text { box_size, .. } = scaled(&text, 0.8).kind else {
            panic!()
        };
        assert_eq!(box_size, Some((32_000.0, 16_000.0)));
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
                box_size: None,
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
                box_size: None,
            },
            coords: vec![MapCoord::new(0, 0)],
            tags: Vec::new(),
            pattern: None,
        };
        let doc = map(4000, set, vec![text]);
        let (_, report, _) = print_map(
            &doc,
            &options(Orientation::Portrait),
            "Glyphs",
            SheetLang::En,
        )
        .unwrap();
        assert_eq!(report.missing_glyphs, ['✈']);
    }

    fn texts_of(page: &RenderedPage) -> Vec<(String, (f64, f64))> {
        use crate::render::page::{Shape, SheetItem};
        page.sheet
            .iter()
            .filter_map(|i| match i {
                SheetItem::Shape {
                    shape: Shape::Text { text, origin, .. },
                    ..
                } => Some((text.clone(), *origin)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn render_gets_the_default_footer() {
        let mut doc = map(
            4000,
            SymbolSet::issprom_2019(),
            vec![line(edge(), (190_000, 280_000))],
        );
        doc.notes = "© OpenStreetMap contributors, ODbL\nCC BY 4.0".into();
        let opts = options(Orientation::Portrait);
        let (plain, plain_report) = render(&doc, &opts);
        let (page, report, notes) = render_with_footer(&doc, &opts, SheetLang::En).unwrap();
        // The map does not move: same primitives, frame, centre and report.
        assert_eq!(page.primitives, plain.primitives);
        assert_eq!(page.clip, plain.clip);
        assert_eq!(page.centre_mm, plain.centre_mm);
        assert_eq!(report, plain_report);
        let t = texts_of(&page);
        let names: Vec<&str> = t.iter().map(|x| x.0.as_str()).collect();
        assert_eq!(
            names,
            [
                "© OpenStreetMap contributors, ODbL",
                "CC BY 4.0",
                crate::CREDIT_LINE_EN
            ]
        );
        // The 5 mm margin is too narrow: on a white box in the bottom-left corner of the frame.
        assert!(matches!(
            page.sheet[0],
            crate::render::page::SheetItem::Shape {
                colour: crate::render::page::DirectColour::WHITE,
                ..
            }
        ));
        assert!(t.iter().all(|x| x.1.0 > 5.0 && x.1.1 < 292.0), "{t:?}");
        // The line runs through the bottom-left corner: the footer covers part of it.
        assert_eq!(
            notes,
            [crate::sheet::SheetNote::FooterOverlapsMap {
                content: crate::sheet::FooterContent::CopyrightAndCredit
            }]
        );
        // German, and nothing to note when the map keeps clear of the corner.
        let small = map(
            4000,
            SymbolSet::issprom_2019(),
            vec![line(edge(), (10_000, 0))],
        );
        let (page, _, notes) = render_with_footer(&small, &opts, SheetLang::De).unwrap();
        assert_eq!(texts_of(&page)[0].0, crate::CREDIT_LINE_DE);
        assert_eq!(notes, []);
    }

    /// A sprint set plus `999.1`, an area with a hatch `spacing` µm apart.
    fn hatched(spacing: u32) -> (SymbolSet, u32) {
        let set = SymbolSet::issprom_2019().with_additions(&format!(
            r#"<symbol type="4" id="@" code="999.1" name="hatch"><area_symbol inner_color="-1" min_area="0" patterns="1"><pattern type="1" angle="0" rotatable="false" line_spacing="{spacing}" line_offset="0" offset_along_line="0" color="2" line_width="10"/></area_symbol></symbol>"#
        ));
        let id = set.id_for_code("999.1").unwrap();
        (set, id)
    }

    fn area(symbol_id: u32, w: i32, h: i32) -> MapObject {
        MapObject {
            symbol_id,
            kind: MapObjectKind::Path,
            coords: crate::omap::closed_ring(&[(0, 0), (w, 0), (w, h), (0, h)]),
            tags: Vec::new(),
            pattern: None,
        }
    }

    #[test]
    fn page_budget_bounds_the_page_and_counts_the_rest() {
        // Each area: a 10 mm square hatched 0.1 mm apart, 101 lines.
        let (set, id) = hatched(100);
        let doc = map(4000, set, vec![area(id, 10_000, 10_000); 20]);
        let (page, report) =
            render_page_within(&doc, &options(Orientation::Portrait), 250).unwrap();
        // A unit per line and one per outline: two whole areas (102 units each), 45 lines of the
        // third.
        assert_eq!(page.primitives.len(), 247);
        // Two whole areas, the third cut short, the other 17 not drawn.
        assert_eq!(report.truncated_objects, 1);
        assert_eq!(report.objects_not_drawn_page_limit, 17);
        let (page, report) = render(&doc, &options(Orientation::Portrait));
        assert_eq!(page.primitives.len(), 20 * 101);
        assert_eq!(
            (
                report.truncated_objects,
                report.objects_not_drawn_page_limit
            ),
            (0, 0)
        );
    }

    /// A map of `objects` objects of 20 000 points each, whose combined symbol fills the object
    /// 1000 times (final review of M1: three of them took 1.2 GB).
    fn thousand_fills(objects: usize) -> MapDocument {
        let parts = r#"<part symbol="0"/>"#.repeat(1000);
        let coords: String = (0..20_000)
            .map(|i| format!("{} {};", i % 1000 * 10, i / 1000 * 10))
            .collect();
        let object = format!(
            r#"<object type="1" symbol="1"><coords count="20001">{coords}0 0 18;</coords></object>"#
        );
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<map xmlns="http://openorienteering.org/apps/mapper/xml/v2" version="9">
<georeferencing scale="4000"/>
<colors count="1"><color priority="0" name="Black" c="0" m="0" y="0" k="1" opacity="1"/></colors>
<symbols count="2" id="ISSprOM 2019"><symbol type="4" id="0" code="999" name="a"><area_symbol inner_color="0" min_area="0" patterns="0"/></symbol><symbol type="16" id="1" code="998" name="c"><combined_symbol parts="1000">{parts}</combined_symbol></symbol></symbols>
<parts count="1" current="0"><part name="p"><objects count="{objects}">{}</objects></part></parts>
<templates count="0"/>
</map>"#,
            object.repeat(objects)
        );
        crate::omap::read::read_map(&xml).unwrap()
    }

    /// Points the fills and strokes of `page` keep.
    fn kept_points(page: &RenderedPage) -> usize {
        page.primitives
            .iter()
            .map(|p| match &p.shape {
                Shape::Fill { rings, .. } => rings.iter().map(Vec::len).sum(),
                Shape::Stroke { path, .. } => path.len(),
                _ => 0,
            })
            .sum()
    }

    #[test]
    fn page_budget_counts_work_units() {
        use crate::render::draw::{BUDGET_SLACK, KEPT_POINTS_PER_UNIT, work_units};
        // One object: its own budget cuts it.
        let before = work_units();
        let (page, report) = render(&thousand_fills(1), &options(Orientation::Portrait));
        let work = work_units() - before;
        assert_eq!(
            (
                report.truncated_objects,
                report.objects_not_drawn_page_limit
            ),
            (1, 0)
        );
        assert!(work <= MAX_UNITS_PER_OBJECT + BUDGET_SLACK, "{work}");
        // Three: two use up the page, the third is not drawn.
        let before = work_units();
        let (page3, report) = render(&thousand_fills(3), &options(Orientation::Portrait));
        let work = work_units() - before;
        assert_eq!(
            (
                report.truncated_objects,
                report.objects_not_drawn_page_limit
            ),
            (2, 1)
        );
        assert!(work <= MAX_UNITS_PER_PAGE + 2 * BUDGET_SLACK, "{work}");
        let points = kept_points(&page3);
        assert!(
            points <= MAX_UNITS_PER_PAGE * KEPT_POINTS_PER_UNIT,
            "{points}"
        );
        assert_eq!(page3.primitives.len(), 2 * page.primitives.len());
    }

    #[test]
    fn real_maps_stay_far_below_the_page_budget() {
        use crate::render::draw::work_units;
        let maps = [
            ("luebeck.omap", MapKind::Sprint),
            ("forest-sample.omap", MapKind::Forest),
        ];
        for (file, kind) in maps {
            let path = format!(
                "{}/../../tests/fixtures/maps/{file}",
                env!("CARGO_MANIFEST_DIR")
            );
            let doc = crate::omap::read::read_map_file(std::path::Path::new(&path)).unwrap();
            for scale in kind.allowed_scales() {
                let options = PrintOptions {
                    print_scale: Some(scale),
                    kind,
                    show_sheet_objects: true,
                    ..options(Orientation::Auto)
                };
                let before = work_units();
                let (page, report) = render(&doc, &options);
                let units = work_units() - before;
                eprintln!(
                    "{file} 1:{scale}: {units} units, {} primitives, {} kept points",
                    page.primitives.len(),
                    kept_points(&page)
                );
                assert!(units < MAX_UNITS_PER_PAGE / 10, "{file} 1:{scale}: {units}");
                assert_eq!(
                    (
                        report.truncated_objects,
                        report.objects_not_drawn_page_limit
                    ),
                    (0, 0)
                );
            }
        }
    }

    #[test]
    fn primitives_come_in_paint_order() {
        // An edge line (501.1, black) before a water area (blue, a higher number): water is
        // painted first, the line on top, though the file lists the line first.
        let set = SymbolSet::issprom_2019();
        let water = set.id_for_code("301.1").unwrap();
        let doc = map(
            4000,
            set,
            vec![line(edge(), (25_000, 0)), area(water, 10_000, 10_000)],
        );
        let (page, _) = render(&doc, &options(Orientation::Portrait));
        let priorities: Vec<i32> = page.primitives.iter().map(|p| p.priority).collect();
        assert!(
            priorities.windows(2).all(|w| w[0] >= w[1]),
            "{priorities:?}"
        );
        assert!(priorities.len() >= 2);
    }

    #[test]
    fn overflow_counts_what_is_painted() {
        // A text anchored right at the edge of the printable area still sticks out with its
        // glyphs: the overflow measures them, not the anchor.
        let set = SymbolSet::issprom_2019();
        let id = set.id_for_code("704").unwrap();
        let text = MapObject {
            symbol_id: id,
            kind: MapObjectKind::Text {
                text: "Wald".into(),
                h_align: 0,
                v_align: 0,
                rotation_rad: 0.0,
                box_size: None,
            },
            coords: vec![MapCoord::new(100_000, 0)],
            tags: Vec::new(),
            pattern: None,
        };
        let doc = map(4000, set, vec![line(edge(), (100_000, 0)), text]);
        let centred = PrintOptions {
            centre: Some(MapCoord::new(0, 0)),
            ..options(Orientation::Portrait)
        };
        // The line ends 5 mm inside the page edge (x = 205 mm), on the clip; the text runs on.
        let (_, report) = render(&doc, &centred);
        let (dx, _) = report.overflow_mm.expect("the text sticks out");
        assert!(dx > 3.0, "{dx}");
    }

    #[test]
    fn frame_box_is_the_page_minus_the_margin() {
        let frame = |min, max| PaperBox { min, max };
        assert_eq!(
            frame_box((210.0, 297.0), PAGE_MARGIN_MM),
            frame((5.0, 5.0), (205.0, 292.0))
        );
        // A margin wider than half the page leaves a frame of zero width, then zero height.
        assert_eq!(
            frame_box((210.0, 297.0), 120.0),
            frame((105.0, 120.0), (105.0, 177.0))
        );
        assert_eq!(
            frame_box((210.0, 297.0), 500.0),
            frame((105.0, 148.5), (105.0, 148.5))
        );
        assert_eq!(
            frame_box((210.0, 297.0), f64::NAN),
            frame((0.0, 0.0), (210.0, 297.0))
        );
    }

    #[test]
    fn auto_orientation_measures_against_the_frame() {
        // 196 mm wide, 150 mm tall: fits in portrait with the 5 mm margin (200 mm), and only in
        // landscape with a 10 mm margin (190 mm wide in portrait).
        let doc = map(
            4000,
            SymbolSet::issprom_2019(),
            vec![line(edge(), (196_000, 150_000))],
        );
        let (page, _) = render(&doc, &options(Orientation::Auto));
        assert_eq!(page.page_mm, (210.0, 297.0));
        let wide = PrintOptions {
            frame_margin_mm: Some(10.0),
            ..options(Orientation::Auto)
        };
        let (page, report) = render(&doc, &wide);
        assert_eq!(page.page_mm, (297.0, 210.0));
        assert_eq!(page.clip, frame_box((297.0, 210.0), 10.0));
        assert_eq!(report.overflow_mm, None);
    }

    #[test]
    fn dropped_patterns_and_unknown_objects_are_reported() {
        // A spacing of 0.01 mm breaks the 0.05 mm limit: the pattern is dropped when read.
        let (set, id) = hatched(10);
        let mut doc = map(
            4000,
            set,
            vec![area(id, 10_000, 10_000), line(edge(), (5_000, 0))],
        );
        doc.unknown_objects = 3;
        let (page, report) = render(&doc, &options(Orientation::Portrait));
        assert_eq!(report.dropped_pattern_objects, 1);
        assert_eq!(report.unknown_type_objects, 3);
        assert_eq!(page.primitives.len(), 1, "only the line");
    }

    #[test]
    fn nested_pattern_outlines_stay_shared_on_the_page() {
        // A hatched square at every point of an outer pattern: each hatch line is clipped to
        // the outer area and, inside that, to its own square. Moving the page must not copy
        // either outline per line.
        let set = SymbolSet::issprom_2019().with_additions(concat!(
            r#"<symbol type="4" id="@" code="999.2" name="nested"><area_symbol inner_color="-1" min_area="0" patterns="1">"#,
            r#"<pattern type="2" angle="0" line_spacing="2000" line_offset="0" offset_along_line="0" point_distance="2000">"#,
            r#"<symbol type="1" code=""><point_symbol inner_radius="0" inner_color="-1" outer_width="0" outer_color="-1" elements="1"><element>"#,
            r#"<symbol type="4" code=""><area_symbol inner_color="-1" min_area="0" patterns="1"><pattern type="1" angle="0" line_spacing="100" line_offset="0" offset_along_line="0" color="2" line_width="10"/></area_symbol></symbol>"#,
            r#"<object type="1"><coords count="5">-500 -500;500 -500;500 500;-500 500;-500 -500 18;</coords></object>"#,
            r#"</element></point_symbol></symbol></pattern></area_symbol></symbol>"#,
        ));
        let id = set.id_for_code("999.2").unwrap();
        let doc = map(4000, set, vec![area(id, 10_000, 10_000)]);
        let (page, _) = render(&doc, &options(Orientation::Portrait));
        // Every outline (by content) is one Arc.
        let mut outlines: Vec<&Rings> = Vec::new();
        let mut clipped = 0;
        for p in &page.primitives {
            let mut shape = &p.shape;
            while let Shape::Clipped { rings, inner } = shape {
                clipped += 1;
                match outlines.iter().find(|o| o[..] == rings[..]) {
                    Some(o) => assert!(Arc::ptr_eq(o, rings), "an outline was copied"),
                    None => outlines.push(rings),
                }
                shape = inner;
            }
        }
        // A square at each of the 36 placements, ten lines each, two clips per line: the outer
        // outline and one per square.
        assert!(clipped > 400, "{clipped}");
        assert_eq!(outlines.len(), 37);
    }

    /// A map of `objects` circles of `points` points, each with `patterns` line patterns of one
    /// colour, clipped and unclipped (`no_clipping="2"`) in turn, one line each.
    fn interleaved(patterns: usize, points: u32, objects: usize) -> MapDocument {
        let xml: String = (0..patterns)
            .map(|i| {
                format!(
                    r#"<pattern type="1" angle="0" no_clipping="{}" line_spacing="1000000" line_offset="0" offset_along_line="0" color="2" line_width="10"/>"#,
                    if i % 2 == 0 { 0 } else { 2 }
                )
            })
            .collect();
        let set = SymbolSet::issprom_2019().with_additions(&format!(
            r#"<symbol type="4" id="@" code="999.3" name="interleaved"><area_symbol inner_color="-1" min_area="0" patterns="{patterns}">{xml}</area_symbol></symbol>"#
        ));
        let circle: Vec<(i32, i32)> = (0..points)
            .map(|i| {
                let a = f64::from(i) / f64::from(points) * std::f64::consts::TAU;
                (
                    (50_000.0 * a.cos()).round() as i32,
                    (50_000.0 * a.sin()).round() as i32,
                )
            })
            .collect();
        let object = MapObject {
            coords: crate::omap::closed_ring(&circle),
            ..area(set.id_for_code("999.3").unwrap(), 0, 0)
        };
        map(4000, set, vec![object; objects])
    }

    #[test]
    fn every_clip_block_the_pdf_writes_is_charged() {
        // 40 patterns on a 2000 point circle: the PDF writes the outline 20 times.
        use crate::render::draw::{KEPT_POINTS_PER_UNIT, work_units};
        let doc = interleaved(40, 2000, 1);
        let before = work_units();
        let (page, report) = render(&doc, &options(Orientation::Portrait));
        let work = work_units() - before;
        assert_eq!(report.truncated_objects, 0);
        let (pdf, _) = write_pdf(&page, &PdfMeta::new("clips")).unwrap();
        let content = crate::render::pdf::content_of(&pdf);
        let blocks = content.windows(4).filter(|w| w == b"W*\nn").count();
        assert_eq!(blocks, 20);
        // Each block writes the 2000 point outline: the units pay for all of them.
        assert!(
            work * KEPT_POINTS_PER_UNIT >= blocks * 2000,
            "{work} units for {blocks} blocks"
        );
    }

    #[test]
    fn a_spent_object_asks_for_no_more_outlines() {
        // The reviewer's probe, smaller: 300 clipped patterns on a 20 000 point circle need
        // 750 000 units. Each object is cut at its own budget, and the patterns after that do not
        // ask for their outlines, so the second object still gets the rest of the page.
        use crate::render::draw::{BUDGET_SLACK, work_units};
        let doc = interleaved(600, 20_000, 4);
        let before = work_units();
        let (_, report) = render(&doc, &options(Orientation::Portrait));
        let work = work_units() - before;
        assert_eq!(
            (
                report.truncated_objects,
                report.objects_not_drawn_page_limit
            ),
            (2, 2)
        );
        // One outline (2500 units) may be asked for beyond each object's budget.
        assert!(
            work <= MAX_UNITS_PER_PAGE + 2 * (2_500 + BUDGET_SLACK),
            "{work}"
        );
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
                    rule: FillRule::EvenOdd,
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
                rings: vec![vec![(10.0, 20.0), (11.0, 21.0), (10.0, 21.0)]],
                rule: FillRule::EvenOdd,
            }
        );
    }
}
