//! Exact-scale page output (design §5.2, ADR 0018).
//!
//! One millimetre at print scale is one page millimetre, and the page is exactly the paper. When the
//! print scale differs from the map scale, the geometry scales by map scale / print scale and the
//! symbols by the symbol factor the IOF standard sets for the map kind. Nothing is fitted to the page: a map larger than
//! the page is clipped, and the report says by how much it overflows.

use crate::omap::{MapCoord, MapDocument, OmapError};
use crate::project::SheetLang;
use crate::render::build::{BuildError, PageSpec, build_page_within};
use crate::render::page::RenderedPage;
use crate::render::pdf::{PdfError, PdfMeta, write_pdf};
use crate::scale::{MapKind, ScaleError, geometry_factor, is_standard, symbol_factor};
use crate::sheet::SheetNote;

#[cfg(test)]
pub(crate) use crate::render::build::BASE_TAG;
pub use crate::render::build::{
    BASE_PART, MAX_UNITS_PER_PAGE, Orientation, PAGE_MARGIN_MM, Paper, frame_box, is_base_object,
    is_north_line, is_sheet_object,
};

/// How a map is put on a page.
#[derive(Debug, Clone, PartialEq)]
pub struct PrintOptions {
    /// Denominator of the print scale; `None` prints at the map scale.
    pub print_scale: Option<u32>,
    pub kind: MapKind,
    pub paper: Paper,
    /// A page of this size (width and height in portrait, mm) instead of `paper`; see `PageSpec::page_mm` in
    /// freemapper-render for the limits (ADR 0058).
    pub page_mm: Option<(f64, f64)>,
    pub orientation: Orientation,
    /// The map point (in the file's map coordinates) that lands at the centre of the page; `None`
    /// takes the centre of the box everything drawn paints in, the base area of a sprint map left
    /// out (ADR 0044). A point far outside the map moves
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
    /// included, the base area of a sprint map not) reaches beyond the clip rectangle in x and in
    /// y, both sides together; `None` when everything fits.
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
    /// For each object a page's primitives name (the number a hit carries), its position in the map the page was
    /// drawn from (`source_objects` of the page builder's report).
    pub source_objects: Vec<usize>,
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
    Symbols(OmapError),
    /// The page builder refused the size. The layouts only name A4 and A3, so no print command reaches it.
    #[error("a page of {width_mm} x {height_mm} mm is not possible")]
    PageSize { width_mm: f64, height_mm: f64 },
    #[error(transparent)]
    Pdf(#[from] PdfError),
}

impl From<BuildError> for PrintError {
    fn from(e: BuildError) -> Self {
        match e {
            BuildError::Symbols(e) => PrintError::Symbols(e),
            BuildError::PageSize {
                width_mm,
                height_mm,
            } => PrintError::PageSize {
                width_mm,
                height_mm,
            },
        }
    }
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
///
/// The factors are computed here (the scale policy stays in this crate, ADR 0053); the page itself comes from
/// the page builder in core.
fn render_page_within(
    map: &MapDocument,
    options: &PrintOptions,
    page_budget: usize,
) -> Result<(RenderedPage, PrintReport), PrintError> {
    let map_scale = map.georef.scale_denominator;
    let print_scale = options.print_scale.unwrap_or(map_scale);
    let symbol_factor = symbol_factor(options.kind, map_scale, print_scale)?;
    let geometry_factor = geometry_factor(map_scale, print_scale);
    let spec = PageSpec {
        symbol_factor,
        geometry_factor,
        paper: options.paper,
        page_mm: options.page_mm,
        orientation: options.orientation,
        centre: options.centre,
        frame_margin_mm: options.frame_margin_mm,
        show_sheet_objects: options.show_sheet_objects,
    };
    let (page, built) = build_page_within(map, &spec, page_budget)?;
    let report = PrintReport {
        page_mm: built.page_mm,
        print_scale,
        geometry_factor: built.geometry_factor,
        symbol_factor: built.symbol_factor,
        non_standard_scale: !is_standard(options.kind, print_scale),
        overflow_mm: built.overflow_mm,
        skipped_objects: built.skipped_objects,
        unknown_type_objects: built.unknown_type_objects,
        truncated_objects: built.truncated_objects,
        objects_not_drawn_page_limit: built.objects_not_drawn_page_limit,
        dropped_pattern_objects: built.dropped_pattern_objects,
        substituted_fonts: built.substituted_fonts,
        missing_glyphs: built.missing_glyphs,
        source_objects: built.source_objects,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::omap::{MapGeoref, MapObject, MapObjectKind, MapPart, SymbolSet};
    use crate::render::draw::Shape;

    fn map(scale: u32, set: SymbolSet, objects: Vec<MapObject>) -> MapDocument {
        MapDocument {
            notes: String::new(),
            georef: MapGeoref {
                scale_denominator: scale,
                declination_deg: 0.0,
                grivation_deg: 0.0,
                ref_projected: None,
                crs_spec: None,
                combined_scale_factor: 1.0,
                map_ref_mm: (0.0, 0.0),
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
            page_mm: None,
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

    fn area(symbol_id: u32, w: i32, h: i32) -> MapObject {
        MapObject {
            symbol_id,
            kind: MapObjectKind::Path,
            coords: crate::omap::closed_ring(&[(0, 0), (w, 0), (w, h), (0, h)]),
            tags: Vec::new(),
            pattern: None,
        }
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
            for scale in crate::scale::allowed_scales(kind) {
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

    // --- The base area of sprint maps (ADR 0044) ---

    /// A small sprint map near the origin on the walkable-area palette: an edge line from (0, 0) to `to`
    /// and a building, with freemapper's base rectangle when `base` is set. The rectangle is far larger
    /// than the map and lies off-centre, so it would move the centre and the overflow if it counted.
    fn sprint_map(to: (i32, i32), base: bool) -> MapDocument {
        let set = base_fixture::sprint_set();
        let building = area(set.id_for_code("521").unwrap(), 20_000, 10_000);
        let mut doc = map(
            4000,
            set.clone(),
            vec![line(set.id_for_code("501.1").unwrap(), to), building],
        );
        if base {
            let part = base_fixture::base_part(&set, (-900_000, -300_000), (400_000, 700_000));
            doc.parts.insert(0, part);
        }
        doc
    }

    #[test]
    fn base_area_does_not_count_under_the_footer() {
        // The footer sits in the bottom-left corner of the frame, on the base area but off the map.
        let footer_notes = |base: bool| {
            let mut doc = sprint_map((60_000, 40_000), base);
            doc.notes = "© Test".into();
            render_with_footer(&doc, &options(Orientation::Portrait), SheetLang::En)
                .unwrap()
                .2
        };
        assert_eq!(footer_notes(true), footer_notes(false));
        assert!(
            !footer_notes(true)
                .iter()
                .any(|n| matches!(n, SheetNote::FooterOverlapsMap { .. }))
        );
    }
}

/// Maps with freemapper's base area for tests (ADR 0044): the colour and symbol as freemapper's
/// `SymbolSet::with_walkable_area` appends them (its ADR 0069), and the base rectangle in its own part.
#[cfg(test)]
pub(crate) mod base_fixture {
    use crate::omap::{MapObject, MapObjectKind, MapPart, SymbolSet, closed_ring};

    /// freemapper's colour of the walkable area, the mix of "Lower brown 30%".
    const COLOUR: &str = r#"<color priority="{priority}" name="Lower brown 30% for walkable area" c="0.06" m="0.23" y="0.33" k="0" opacity="1"><spotcolors knockout="true"><component factor="0.3" spotcolor="8"/></spotcolors><cmyk method="custom"/><rgb method="cmyk" r="0.94" g="0.77" b="0.67"/></color>"#;

    /// freemapper's symbol 990 in the colour `{priority}`.
    const SYMBOL: &str = r#"<symbol type="4" id="@" code="990" name="Walkable area (base fill)"><area_symbol inner_color="{priority}" min_area="0" patterns="0"/></symbol>"#;

    /// `set` with the walkable-area colour appended behind all others (priority 32 on ISSprOM 2019) and
    /// symbol 990 in it. Existing priorities and ids stay.
    pub(crate) fn with_walkable_area(set: &SymbolSet) -> SymbolSet {
        let count = set.colors_xml.matches("<color ").count();
        let colour = COLOUR.replace("{priority}", &count.to_string());
        let closing = set.colors_xml.rfind("</colors>").unwrap();
        let mut with = set.clone();
        with.colors_xml = format!("{}{colour}\n</colors>", &set.colors_xml[..closing]).replacen(
            &format!(r#"<colors count="{count}""#),
            &format!(r#"<colors count="{}""#, count + 1),
            1,
        );
        with.with_additions(&SYMBOL.replace("{priority}", &count.to_string()))
    }

    /// ISSprOM 2019 with the walkable area: the palette of a freemapper sprint map.
    pub(crate) fn sprint_set() -> SymbolSet {
        with_walkable_area(&SymbolSet::issprom_2019())
    }

    /// The part `base` with one 990 rectangle from `min` to `max` (µm), tagged as freemapper tags it.
    pub(crate) fn base_part(set: &SymbolSet, min: (i32, i32), max: (i32, i32)) -> MapPart {
        MapPart {
            name: super::BASE_PART.into(),
            objects: vec![MapObject {
                symbol_id: set.id_for_code("990").unwrap(),
                kind: MapObjectKind::Path,
                coords: closed_ring(&[min, (max.0, min.1), max, (min.0, max.1)]),
                tags: vec![(super::BASE_TAG.0.into(), super::BASE_TAG.1.into())],
                pattern: None,
            }],
        }
    }
}
