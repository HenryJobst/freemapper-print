//! Exact-scale page output: one millimetre at print scale is one PDF millimetre, the page is the
//! paper, and symbols have the size the standard sets for the print scale (ADR 0018).

use freemapper_print::omap::{
    MapCoord, MapDocument, MapGeoref, MapObject, MapObjectKind, MapPart, SymbolSet,
};
use freemapper_print::print::{Orientation, Paper, PrintOptions, print_map, render_page};
use freemapper_print::project::SheetLang;
use freemapper_print::render::page::{PaperBox, RenderedPage, Shape};
use freemapper_print::scale::MapKind;

/// Points per millimetre.
const PT_PER_MM: f64 = 72.0 / 25.4;

fn map(scale: u32, objects: Vec<MapObject>) -> MapDocument {
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
        symbol_set: SymbolSet::issprom_2019(),
        parts: vec![MapPart {
            name: "default part".into(),
            objects,
        }],
        templates: Vec::new(),
        unknown_objects: 0,
    }
}

fn symbol(code: &str) -> u32 {
    SymbolSet::issprom_2019()
        .id_for_code(code)
        .unwrap_or_else(|| panic!("ISSprOM 2019 defines {code}"))
}

/// A horizontal line `length_mm` long on the map, starting at the map origin.
fn line(symbol_id: u32, length_mm: f64) -> MapObject {
    MapObject {
        symbol_id,
        kind: MapObjectKind::Path,
        coords: vec![
            MapCoord::new(0, 0),
            MapCoord::new((length_mm * 1000.0).round() as i32, 0),
        ],
        tags: Vec::new(),
        pattern: None,
    }
}

fn options(kind: MapKind, print_scale: Option<u32>) -> PrintOptions {
    PrintOptions {
        print_scale,
        kind,
        paper: Paper::A4,
        orientation: Orientation::Portrait,
        centre: None,
        show_sheet_objects: false,
        frame_margin_mm: None,
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn media_box(pdf: &[u8]) -> Vec<f64> {
    let start = find(pdf, b"/MediaBox [").expect("MediaBox") + 11;
    let end = start + find(&pdf[start..], b"]").expect("end of MediaBox");
    std::str::from_utf8(&pdf[start..end])
        .unwrap()
        .split_whitespace()
        .map(|v| v.parse().unwrap())
        .collect()
}

/// The decompressed page content (the first stream).
fn content(pdf: &[u8]) -> String {
    let start = find(pdf, b"stream\n").expect("content stream") + 7;
    let end = find(pdf, b"\nendstream").expect("end of stream");
    let raw = miniz_oxide::inflate::decompress_to_vec_zlib(&pdf[start..end]).expect("FlateDecode");
    String::from_utf8_lossy(&raw).into_owned()
}

/// The operands of the first content line that ends with ` op`.
fn operands(content: &str, op: &str) -> Vec<f64> {
    let suffix = format!(" {op}");
    let line = content
        .lines()
        .find(|l| l.ends_with(&suffix))
        .unwrap_or_else(|| panic!("no `{op}` in\n{content}"));
    line.split_whitespace()
        .rev()
        .skip(1)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|v| v.parse().unwrap())
        .collect()
}

fn close(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance
}

#[test]
fn a4_page_is_exact() {
    let doc = map(4000, vec![line(symbol("501.1"), 25.0)]);
    let (pdf, report, _) =
        print_map(&doc, &options(MapKind::Sprint, None), "A4", SheetLang::En).unwrap();
    let mb = media_box(&pdf);
    assert_eq!(mb.len(), 4);
    assert!(close(mb[0], 0.0, 1e-9) && close(mb[1], 0.0, 1e-9), "{mb:?}");
    assert!(close(mb[2], 595.2756, 0.01), "{mb:?}");
    assert!(close(mb[3], 841.8898, 0.01), "{mb:?}");
    assert_eq!(report.page_mm, (210.0, 297.0));

    let a3 = PrintOptions {
        paper: Paper::A3,
        orientation: Orientation::Landscape,
        ..options(MapKind::Sprint, None)
    };
    let (pdf, report, _) = print_map(&doc, &a3, "A3", SheetLang::En).unwrap();
    let mb = media_box(&pdf);
    assert!(close(mb[2], 1190.55, 0.01), "{mb:?}");
    assert!(close(mb[3], 841.89, 0.01), "{mb:?}");
    assert_eq!(report.page_mm, (420.0, 297.0));
}

#[test]
fn line_length_at_print_scale() {
    // 25 mm on the map = 100 m at 1:4000 = 20 mm at 1:5000.
    let doc = map(4000, vec![line(symbol("501.1"), 25.0)]);
    let (pdf, report, _) = print_map(
        &doc,
        &options(MapKind::Sprint, Some(5000)),
        "Line",
        SheetLang::En,
    )
    .unwrap();
    assert_eq!(report.print_scale, 5000);
    assert!(close(report.geometry_factor, 0.8, 1e-12));
    let content = content(&pdf);
    let from = operands(&content, "m");
    let to = operands(&content, "l");
    assert!(close(from[1], to[1], 1e-6), "horizontal: {from:?} {to:?}");
    assert!(
        close(to[0] - from[0], 20.0 * PT_PER_MM, 0.01),
        "{} pt",
        to[0] - from[0]
    );
    assert!(close(to[0] - from[0], 56.693, 0.01));
}

#[test]
fn reduction_keeps_symbol_width_and_is_flagged() {
    // 1:5000 is below the sprint base scale 1:4000: not standard, symbols keep their size.
    let doc = map(4000, vec![line(symbol("501.1"), 25.0)]);
    let (pdf, report, _) = print_map(
        &doc,
        &options(MapKind::Sprint, Some(5000)),
        "Width",
        SheetLang::En,
    )
    .unwrap();
    assert_eq!(report.symbol_factor, 1.0);
    assert!(report.non_standard_scale);
    let width = operands(&content(&pdf), "w");
    assert!(close(width[0], 0.2835, 0.01), "{width:?}");
}

/// The widths of all strokes on the page (mm).
fn stroke_widths(page: &RenderedPage) -> Vec<f64> {
    page.primitives
        .iter()
        .filter_map(|p| match &p.shape {
            Shape::Stroke { width, .. } => Some(*width),
            _ => None,
        })
        .collect()
}

#[test]
fn sprint_symbols_grow_to_133_percent_at_3000() {
    // 506, the small footpath, is 0.27 mm wide at 1:4000 (ISSprOM 2019-2 §4.7: 133 % at 1:3000).
    let doc = map(4000, vec![line(symbol("506"), 25.0)]);
    let (page, report) = render_page(&doc, &options(MapKind::Sprint, Some(3000))).unwrap();
    assert!(close(report.symbol_factor, 4.0 / 3.0, 1e-12));
    assert!(!report.non_standard_scale);
    let widths = stroke_widths(&page);
    assert!(!widths.is_empty());
    assert!(widths.iter().all(|w| close(*w, 0.36, 1e-9)), "{widths:?}");
    let (page, report) = render_page(&doc, &options(MapKind::Sprint, None)).unwrap();
    assert!(!report.non_standard_scale);
    assert!(stroke_widths(&page).iter().all(|w| close(*w, 0.27, 1e-9)));
}

#[test]
fn forest_symbols_grow_at_10000_and_5000() {
    // 301.6 is a plain 0.18 mm line; the map is a forest map drawn at 1:15000.
    let doc = map(15000, vec![line(symbol("301.6"), 25.0)]);
    for (scale, factor) in [(10000, 1.5), (7500, 2.0), (5000, 3.0)] {
        let (page, report) = render_page(&doc, &options(MapKind::Forest, Some(scale))).unwrap();
        assert_eq!(report.symbol_factor, factor);
        assert!(!report.non_standard_scale);
        assert!(close(
            report.geometry_factor,
            15000.0 / f64::from(scale),
            1e-12
        ));
        let widths = stroke_widths(&page);
        assert_eq!(widths.len(), 1, "{:?}", page.primitives);
        assert!(close(widths[0], 0.18 * factor, 1e-9), "{scale}: {widths:?}");
    }
}

#[test]
fn overflow_is_reported_not_shrunk() {
    let doc = map(4000, vec![line(symbol("501.1"), 300.0)]);
    let (page, report) = render_page(&doc, &options(MapKind::Sprint, None)).unwrap();
    let (dx, dy) = report
        .overflow_mm
        .expect("a 300 mm map overflows A4 portrait");
    // The painted line: 300 mm plus half its 0.1 mm width at each end, on 200 mm of printable
    // width.
    assert!(close(dx, 100.1, 1e-9), "{dx}");
    assert!(close(dy, 0.0, 1e-9), "{dy}");
    assert_eq!(
        page.clip,
        PaperBox {
            min: (5.0, 5.0),
            max: (205.0, 292.0)
        }
    );
    let path = match &page.primitives[..] {
        [p] => match &p.shape {
            Shape::Stroke { path, .. } => path.clone(),
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    };
    let (first, last) = (path[0], *path.last().unwrap());
    assert!(close(last.0 - first.0, 300.0, 1e-9), "full scale: {path:?}");
    assert!(
        close(first.0, -45.0, 1e-9) && close(last.0, 255.0, 1e-9),
        "{path:?}"
    );

    // The PDF clips to the same rectangle.
    let (pdf, _, _) = print_map(
        &doc,
        &options(MapKind::Sprint, None),
        "Overflow",
        SheetLang::En,
    )
    .unwrap();
    let rect = operands(&content(&pdf), "re");
    let expected = [5.0, 5.0, 200.0, 287.0].map(|v| v * PT_PER_MM);
    for (a, b) in rect.iter().zip(expected) {
        assert!(close(*a, b, 0.01), "{rect:?}");
    }
}

#[test]
fn sheet_objects_hidden_by_default() {
    let mut sheet = line(symbol("501.1"), 25.0);
    sheet.tags = vec![("freemapper:source".into(), "sheet".into())];
    let doc = map(4000, vec![sheet]);
    let (page, report) = render_page(&doc, &options(MapKind::Sprint, None)).unwrap();
    assert!(page.primitives.is_empty(), "{:?}", page.primitives);
    assert_eq!(report.skipped_objects, 0);

    let shown = PrintOptions {
        show_sheet_objects: true,
        ..options(MapKind::Sprint, None)
    };
    let (page, _) = render_page(&doc, &shown).unwrap();
    assert_eq!(page.primitives.len(), 1);
}

#[test]
fn north_lines_are_map_content() {
    // freemapper tags its north lines (601) as sheet objects; they belong to the map anyway.
    let mut north = line(symbol("601"), 25.0);
    north.tags = vec![("freemapper:source".into(), "sheet".into())];
    let doc = map(4000, vec![north]);
    let (page, _) = render_page(&doc, &options(MapKind::Sprint, None)).unwrap();
    assert!(!page.primitives.is_empty());
}

const LUEBECK: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/maps/luebeck.omap"
);

#[test]
fn luebeck_north_lines_drawn_info_block_only_on_request() {
    let mut doc =
        freemapper_print::omap::read::read_map_file(std::path::Path::new(LUEBECK)).unwrap();
    // Only the north lines and the information block. Since freemapper v0.5.0 the nine north
    // lines (601) lie in the default part with the tag `north-lines`; the part "peripheral
    // information" holds only the information block (991–994), tagged `sheet`.
    let info = doc
        .parts
        .iter()
        .find(|p| p.name == "peripheral information")
        .expect("part peripheral information");
    assert!(
        info.objects
            .iter()
            .all(|o| doc.symbol_set.code_of(o.symbol_id) != Some("601"))
    );
    let set = doc.symbol_set.clone();
    for part in &mut doc.parts {
        let keep_all = part.name == "peripheral information";
        part.objects
            .retain(|o| keep_all || set.code_of(o.symbol_id) == Some("601"));
    }
    let codes: Vec<&str> = doc
        .parts
        .iter()
        .flat_map(|p| &p.objects)
        .filter_map(|o| doc.symbol_set.code_of(o.symbol_id))
        .collect();
    assert_eq!(codes.iter().filter(|c| **c == "601").count(), 9);
    let texts = |page: &RenderedPage| {
        page.primitives
            .iter()
            .filter(|p| matches!(p.shape, Shape::Text { .. }))
            .count()
    };
    let north_colour = |page: &RenderedPage| {
        page.primitives
            .iter()
            .filter(|p| matches!(p.shape, Shape::Stroke { width, .. } if close(width, 0.14, 1e-9)))
            .count()
    };
    let (plain, _) = render_page(&doc, &options(MapKind::Sprint, None)).unwrap();
    assert!(north_colour(&plain) >= 9, "north lines drawn");
    assert_eq!(texts(&plain), 0, "no information block");
    let shown = PrintOptions {
        show_sheet_objects: true,
        ..options(MapKind::Sprint, None)
    };
    let (full, _) = render_page(&doc, &shown).unwrap();
    assert_eq!(north_colour(&full), north_colour(&plain));
    assert!(texts(&full) > 0, "information block drawn");
}

#[test]
fn skipped_objects_counted() {
    let doc = map(4000, vec![line(999_999, 25.0), line(symbol("501.1"), 25.0)]);
    let (page, report) = render_page(&doc, &options(MapKind::Sprint, None)).unwrap();
    assert_eq!(report.skipped_objects, 1);
    assert_eq!(report.truncated_objects, 0);
    assert_eq!(page.primitives.len(), 1);
}

#[test]
fn render_page_is_backend_neutral() {
    let doc = map(4000, vec![line(symbol("501.1"), 25.0)]);
    let (page, report) = render_page(&doc, &options(MapKind::Sprint, Some(5000))).unwrap();
    assert_eq!(page.page_mm, (210.0, 297.0));
    assert_eq!(report.page_mm, page.page_mm);
    assert!(report.overflow_mm.is_none());
    assert!(!page.colours.is_empty());
    let [primitive] = &page.primitives[..] else {
        panic!("{:?}", page.primitives)
    };
    let Shape::Stroke { path, .. } = &primitive.shape else {
        panic!("{:?}", primitive.shape)
    };
    let (first, last) = (path[0], *path.last().unwrap());
    assert!(close(last.0 - first.0, 20.0, 1e-9), "{path:?}");
    // Centred on the page: the default centre is the middle of what is drawn.
    assert!(
        close(first.0, 95.0, 1e-9) && close(last.0, 115.0, 1e-9),
        "{path:?}"
    );
    assert!(
        close(first.1, 148.5, 1e-9) && close(last.1, 148.5, 1e-9),
        "{path:?}"
    );
}

#[test]
fn creator_names_freemapper_print() {
    assert_eq!(
        freemapper_print::CREDIT,
        "freemapper-print – https://github.com/HenryJobst/freemapper-print"
    );
    let doc = map(4000, vec![line(symbol("501.1"), 25.0)]);
    let (pdf, _, _) = print_map(
        &doc,
        &options(MapKind::Sprint, None),
        "Credit",
        SheetLang::En,
    )
    .unwrap();
    // UTF-16BE (the en dash is not ASCII): "freemapper-print".
    assert!(
        find(
            &pdf,
            b"0066007200650065006D00610070007000650072002D007000720069006E0074"
        )
        .is_some(),
        "creator"
    );
}
