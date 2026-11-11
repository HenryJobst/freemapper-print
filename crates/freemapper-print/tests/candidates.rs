//! freemapper's candidates stay off the printed page.
//!
//! freemapper v0.8.0 keeps its candidates (objects a mapper may still take into the map) in a separate
//! map, `templates/candidates.omap`, which the map file names as an OOM template. Since v0.8.0 they
//! include control sites for course setters (531 with `freemapper:control`, ADR 0074 there) and
//! passages nobody can reach (522 with `freemapper:hidden=unreachable`, ADR 0076 there). Their
//! symbols are ordinary, visible map symbols, so only the file they live in keeps them off the sheet:
//! freemapper-print draws the map file and never its templates.

use freemapper_print::omap::read::read_map_file;
use freemapper_print::print::{Orientation, Paper, PrintOptions, render_page};
use freemapper_print::render::page::RenderedPage;
use freemapper_print::scale::MapKind;
use std::path::{Path, PathBuf};

fn fixture() -> PathBuf {
    PathBuf::from(format!(
        "{}/../../tests/fixtures/maps/luebeck.omap",
        env!("CARGO_MANIFEST_DIR")
    ))
}

/// Two candidates copied unchanged from `examples/schleswig-holstein-luebeck/templates/candidates.omap`
/// of freemapper v0.8.0 (a2b41b4): a waste basket as a control site (symbol 156 = 531) and a building
/// passage hidden as unreachable (symbol 145 = 522). The symbol ids are those of the fixture map.
const CANDIDATES: &str = r#"<part name="candidates"><objects count="2">
<object type="0" symbol="156" rotation="0"><tags><t k="freemapper:source">osm</t><t k="freemapper:source_id">node/339972984</t><t k="freemapper:osm">amenity=waste_basket</t><t k="freemapper:currency">2026-11-07</t><t k="freemapper:control">waste_basket</t></tags><coords count="1">48810 -46864;</coords></object>
<object type="1" symbol="145"><tags><t k="freemapper:source">osm</t><t k="freemapper:source_id">way/39315137</t><t k="freemapper:osm">highway=footway; tunnel=building_passage</t><t k="freemapper:currency">2026-11-07</t><t k="freemapper:hidden">unreachable</t></tags><coords count="5">-119404 1857;-121711 2096;-121634 2842;-119327 2603;-119404 1857 18;</coords><pattern rotation="0"><coord x="0" y="0"/></pattern></object>
</objects></part>"#;

/// The fixture map with its parts replaced by the two candidates: a candidates map as freemapper
/// writes it, with the same colours and symbols.
fn candidates_map(map_xml: &str) -> String {
    let start = map_xml.find("<parts ").expect("map has <parts>");
    let end = map_xml.rfind("</parts>").expect("map has </parts>");
    format!(
        "{}<parts count=\"1\" current=\"0\">\n{CANDIDATES}\n{}",
        &map_xml[..start],
        &map_xml[end..]
    )
}

fn page(path: &Path) -> RenderedPage {
    let map = read_map_file(path).unwrap();
    let options = PrintOptions {
        print_scale: None,
        kind: MapKind::Sprint,
        paper: Paper::A3,
        page_mm: None,
        orientation: Orientation::Auto,
        centre: None,
        show_sheet_objects: false,
        frame_margin_mm: None,
    };
    render_page(&map, &options).unwrap().0
}

fn has_tag(tags: &[(String, String)], key: &str) -> bool {
    tags.iter().any(|(k, _)| k == key)
}

#[test]
fn the_map_names_its_candidates_as_a_template_and_holds_none() {
    let xml = std::fs::read_to_string(fixture()).unwrap();
    assert!(xml.contains(
        r#"<template type="TemplateMap" open="true" name="candidates.omap" path="templates/candidates.omap""#
    ));
    let map = read_map_file(&fixture()).unwrap();
    let objects: Vec<_> = map.parts.iter().flat_map(|p| &p.objects).collect();
    assert!(
        objects
            .iter()
            .all(|o| !has_tag(&o.tags, "freemapper:control")
                && !has_tag(&o.tags, "freemapper:hidden")),
        "no candidate tag in the map file"
    );
    let x = map.symbol_set.id_for_code("531").expect("531 in ISSprOM");
    assert!(
        objects.iter().all(|o| o.symbol_id != x),
        "no 531 map object"
    );
}

#[test]
fn candidates_next_to_the_map_do_not_reach_the_page() {
    let map_xml = std::fs::read_to_string(fixture()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let map_path = dir.path().join("luebeck.omap");
    std::fs::write(&map_path, &map_xml).unwrap();
    std::fs::create_dir(dir.path().join("templates")).unwrap();
    let candidates_path = dir.path().join("templates/candidates.omap");
    std::fs::write(&candidates_path, candidates_map(&map_xml)).unwrap();

    // The two candidates are drawable: printed as a map of their own, both appear.
    let alone = page(&candidates_path);
    let drawn: std::collections::BTreeSet<u32> =
        alone.primitives.iter().map(|p| p.symbol).collect();
    assert_eq!(drawn.into_iter().collect::<Vec<_>>(), [145, 156]);

    // Next to the map they change nothing: the page is the page of the map alone, with no 531.
    let with_template = page(&map_path);
    assert_eq!(with_template, page(&fixture()));
    assert!(with_template.primitives.iter().all(|p| p.symbol != 156));
}
