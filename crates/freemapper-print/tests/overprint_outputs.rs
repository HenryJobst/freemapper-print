//! Pages without a course have an empty overprint, so the M2 example exports stay byte for byte as
//! committed (M3 Task 6, ADR 0034).

use std::path::PathBuf;

use freemapper_print::export::sheet_pdf;
use freemapper_print::omap::read::read_map_file;
use freemapper_print::project;
use freemapper_print::sheet::LogoCache;

#[test]
fn existing_outputs_unchanged() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/luebeck-competition-layouts");
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
