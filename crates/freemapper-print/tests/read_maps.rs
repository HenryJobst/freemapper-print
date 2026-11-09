//! Reads the two real maps in `tests/fixtures/maps`.

use freemapper_print::omap::read::read_map_file;
use std::path::Path;

fn fixture(name: &str) -> String {
    format!(
        "{}/../../tests/fixtures/maps/{name}",
        env!("CARGO_MANIFEST_DIR")
    )
}

/// Number of `<object ` elements inside `<parts>`. Symbol definitions (point symbols) contain
/// `<object ` elements of their own, which are not map objects.
fn objects_in_parts(path: &str) -> usize {
    let xml = std::fs::read_to_string(path).unwrap();
    let start = xml.find("<parts ").expect("file has <parts>");
    let end = xml.rfind("</parts>").expect("file has </parts>");
    xml[start..end].matches("<object ").count()
}

#[test]
fn luebeck_object_count() {
    let path = fixture("luebeck.omap");
    let doc = read_map_file(Path::new(&path)).unwrap();
    let read: usize = doc.parts.iter().map(|p| p.objects.len()).sum();
    assert_eq!(read, objects_in_parts(&path));
    // freemapper v0.5.0 (e8b6db9): 2160 objects in the default part (with the nine north
    // lines), 13 in "peripheral information" (the information block).
    assert_eq!(read, 2173);
    assert_eq!(doc.georef.scale_denominator, 4000);
    assert_eq!(doc.georef.crs_spec.as_deref(), Some("+init=epsg:25832"));
    assert!(doc.georef.ref_projected.is_some());
    let sheet = doc
        .parts
        .iter()
        .find(|p| p.name == "peripheral information")
        .expect("part peripheral information");
    assert_eq!(sheet.objects.len(), 13);
    assert!(sheet.objects.iter().all(|o| {
        o.tags
            .iter()
            .any(|(k, v)| k == "freemapper:source" && v == "sheet")
    }));
}

#[test]
fn forest_sample_reads() {
    let path = fixture("forest-sample.omap");
    let doc = read_map_file(Path::new(&path)).unwrap();
    let read: usize = doc.parts.iter().map(|p| p.objects.len()).sum();
    assert_eq!(read, objects_in_parts(&path));
    assert_eq!(read, 539);
    assert_eq!(doc.georef.scale_denominator, 10000);
}
