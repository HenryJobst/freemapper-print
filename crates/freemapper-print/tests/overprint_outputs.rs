//! Pages without a course have an empty overprint, so the M2 example exports stay byte for byte as
//! committed (M3 Task 6, ADR 0034). Course sheets without a mask stay byte for byte as committed too (M4 Task 2,
//! ADR 0045), and so do the `render` outputs of `tests/fixtures/examples/luebeck-scales`, which never have a mask.

use std::path::PathBuf;

use freemapper_print::courses::{load_courses, mask_for};
use freemapper_print::export::{course_pdf_paths, course_sheet_pdf, sheet_pdf};
use freemapper_print::omap::read::read_map_file;
use freemapper_print::print::{Orientation, Paper, PrintOptions, print_map};
use freemapper_print::project;
use freemapper_print::project::SheetLang;
use freemapper_print::scale::infer_kind;
use freemapper_print::sheet::LogoCache;

#[test]
fn existing_outputs_unchanged() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/examples/luebeck-competition-layouts");
    let loaded = project::load(&dir.join(project::FILE_NAME)).unwrap();
    let map = read_map_file(&loaded.map_path()).unwrap();
    let logos = LogoCache::new();
    let mut compared = 0;
    for layout in &loaded.project.layouts {
        let (pdf, _) = sheet_pdf(&loaded, &map, layout, &logos).unwrap();
        let committed = std::fs::read(dir.join("out").join(format!("{}.pdf", layout.id))).unwrap();
        assert!(
            pdf == committed,
            "{} differs from the committed export",
            layout.id
        );
        compared += 1;
    }
    assert_eq!(compared, 2);
}

#[test]
fn no_mask_output_unchanged() {
    // The course examples have no mask and no white-out: every course sheet is the committed PDF.
    let examples = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/examples");
    let logos = LogoCache::new();
    let mut compared = 0;
    for dir in [
        "luebeck-competition-courses",
        "luebeck-competition-courses/iof",
        "berlin-viktoriapark-training",
    ] {
        let dir = examples.join(dir);
        let loaded = project::load(&dir.join(project::FILE_NAME)).unwrap();
        let map = read_map_file(&loaded.map_path()).unwrap();
        let courses = load_courses(&loaded, &map).unwrap().unwrap();
        let paths = course_pdf_paths(&courses, &dir.join("out"));
        for (course, path) in courses.event.courses.iter().zip(&paths) {
            assert!(mask_for(&loaded.project, &courses, &course.name).is_empty());
            let (pdf, _, _) = course_sheet_pdf(&loaded, &map, &courses, course, &logos).unwrap();
            let committed = std::fs::read(path).unwrap();
            assert!(
                pdf == committed,
                "{} differs from the committed export",
                path.display()
            );
            compared += 1;
        }
    }
    assert_eq!(compared, 7);
}

#[test]
fn render_outputs_unchanged() {
    // The commands that made the PDFs: `render luebeck.omap --paper a4` and `--paper a3 --scale 3000`, in German.
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/examples/luebeck-scales");
    let path = dir.join("luebeck.omap");
    let map = read_map_file(&path).unwrap();
    let kind = infer_kind(&map, &path).unwrap();
    for (file, paper, scale) in [
        ("luebeck-4000-a4.pdf", Paper::A4, None),
        ("luebeck-3000-a3.pdf", Paper::A3, Some(3000)),
    ] {
        let options = PrintOptions {
            print_scale: scale,
            kind,
            paper,
            page_mm: None,
            orientation: Orientation::Auto,
            centre: None,
            show_sheet_objects: false,
            frame_margin_mm: None,
        };
        let (pdf, _, _) = print_map(&map, &options, "luebeck", SheetLang::De).unwrap();
        let committed = std::fs::read(dir.join(file)).unwrap();
        assert!(pdf == committed, "{file} differs from the committed render");
    }
}
