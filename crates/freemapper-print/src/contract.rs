//! The reader side of the suite contract (ADR 0050): what freemapper-print derives from the fixtures of
//! `freemapper-contract` equals their `expected.toml`. The fixtures are maps and sidecars exactly as
//! freemapper wrote them. Every test name starts with `contract_`; freemapper-core's CI will run them by
//! that prefix.

use std::path::PathBuf;

use freemapper_contract::{Expected, Fixture, fixtures};

use crate::omap::MapDocument;
use crate::omap::read::read_map_file;
use crate::print::{
    Orientation, Paper, PrintOptions, is_base_object, is_sheet_object, render_page,
};
use crate::render::bounds::painted_bounds;
use crate::scale::{MapKind, infer_kind, read_sidecar, read_sidecar_title, symbol_factor};

/// Map coordinates per millimetre.
const UM_PER_MM: f64 = 1000.0;
/// How far the painted box may differ from the box of the object coordinates (mm). In today's fixtures
/// north lines are the outermost objects and add half their width, less than 0.1 mm.
const SYMBOL_MARGIN_MM: f64 = 1.0;

/// A fixture in a folder of its own, read with freemapper-print's reader.
struct Read {
    name: &'static str,
    /// Keeps the folder until the test ends.
    _dir: tempfile::TempDir,
    path: PathBuf,
    map: MapDocument,
    expected: Expected,
}

fn read(fixture: &Fixture) -> Read {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture.write_into(dir.path()).unwrap();
    let map = read_map_file(&path).unwrap();
    Read {
        name: fixture.name,
        _dir: dir,
        path,
        map,
        expected: fixture.expected(),
    }
}

fn each(test: impl Fn(&Read)) {
    assert!(!fixtures().is_empty());
    for fixture in fixtures() {
        test(&read(fixture));
    }
}

fn kind(read: &Read) -> MapKind {
    *MapKind::ALL
        .iter()
        .find(|k| k.name() == read.expected.map_kind)
        .unwrap_or_else(|| panic!("{}: unknown kind {}", read.name, read.expected.map_kind))
}

#[test]
fn contract_kind() {
    each(|read| {
        // The sidecar names the kind; nothing is inferred from scale or symbol set.
        assert_eq!(
            read_sidecar(&read.path).unwrap(),
            Some(kind(read)),
            "{}",
            read.name
        );
        assert_eq!(
            infer_kind(&read.map, &read.path),
            Some(kind(read)),
            "{}",
            read.name
        );
    });
}

#[test]
fn contract_title() {
    each(|read| {
        assert_eq!(
            read_sidecar_title(&read.path).unwrap().as_deref(),
            Some(read.expected.title.as_str()),
            "{}",
            read.name
        );
    });
}

#[test]
fn contract_classes() {
    each(|read| {
        let objects = || read.map.parts.iter().flat_map(|p| &p.objects);
        assert_eq!(
            objects().filter(|o| is_sheet_object(o)).count(),
            read.expected.objects.sheet,
            "{}: sheet objects",
            read.name
        );
        assert_eq!(
            objects().filter(|o| is_base_object(o)).count(),
            read.expected.objects.base,
            "{}: base area",
            read.name
        );
        assert_eq!(
            objects().count(),
            read.expected.objects.sheet
                + read.expected.objects.base
                + read.expected.objects.north_lines
                + read.expected.objects.content,
            "{}: all objects",
            read.name
        );
    });
}

#[test]
fn contract_scale() {
    each(|read| {
        let scale = read.expected.scale_denominator;
        assert_eq!(read.map.georef.scale_denominator, scale, "{}", read.name);
        assert_eq!(
            symbol_factor(kind(read), scale, scale).unwrap(),
            1.0,
            "{}",
            read.name
        );
    });
}

#[test]
fn contract_bounds() {
    each(|read| {
        let options = PrintOptions {
            print_scale: None,
            kind: kind(read),
            paper: Paper::A4,
            orientation: Orientation::Portrait,
            centre: None,
            show_sheet_objects: false,
            frame_margin_mm: None,
        };
        let (page, report) = render_page(&read.map, &options).unwrap();
        assert_eq!(report.skipped_objects, 0, "{}", read.name);
        assert_eq!(report.unknown_type_objects, 0, "{}", read.name);
        // Back from page millimetres to millimetres of the map.
        let offset = (
            page.page_mm.0 / 2.0 - page.centre_mm.0,
            page.page_mm.1 / 2.0 - page.centre_mm.1,
        );
        let (min, max) = painted_bounds(&page.primitives).unwrap();
        let painted = [
            min.0 - offset.0,
            min.1 - offset.1,
            max.0 - offset.0,
            max.1 - offset.1,
        ];
        let mm = |b: [i32; 4]| b.map(|v| f64::from(v) / UM_PER_MM);
        let content = mm(read.expected.bbox.content.unwrap());
        let sheet = mm(read.expected.bbox.sheet.unwrap());
        // Without sheet objects the page ends north of the title block and the scale bar (y points south).
        assert!(
            painted[3] < sheet[1],
            "{}: painted {painted:?}, sheet objects {sheet:?}",
            read.name
        );
        // Everything painted is map content, and all of it is painted.
        for (edge, (p, c)) in painted.iter().zip(content).enumerate() {
            assert!(
                (p - c).abs() <= SYMBOL_MARGIN_MM,
                "{}: edge {edge}, painted {painted:?}, content {content:?}",
                read.name
            );
        }
    });
}
