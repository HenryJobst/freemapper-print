//! Runs the `freemapper-print` binary on the fixture maps.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use freemapper_print::omap::read::read_map_file;
use freemapper_print::print::{Orientation, Paper, PrintOptions, print_map};
use freemapper_print::scale::{MapKind, infer_kind};

const LUEBECK: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/maps/luebeck.omap"
);
const FOREST: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/maps/forest-sample.omap"
);

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_freemapper-print"))
        .args(args)
        .output()
        .expect("run binary")
}

fn render(map: &str, out: &Path, extra: &[&str]) -> Output {
    let mut args = vec!["render", map, "--out", out.to_str().unwrap()];
    args.extend_from_slice(extra);
    run(&args)
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn overflow_expected(
    map: &str,
    paper: Paper,
    orientation: Orientation,
    scale: Option<u32>,
) -> bool {
    let doc = read_map_file(Path::new(map)).unwrap();
    let kind = infer_kind(&doc, Path::new(map)).unwrap_or(MapKind::Sprint);
    let options = PrintOptions {
        print_scale: scale,
        kind,
        paper,
        orientation,
        centre: None,
        show_sheet_objects: false,
    };
    print_map(&doc, &options, "t")
        .unwrap()
        .1
        .overflow_mm
        .is_some()
}

#[test]
fn renders_luebeck_a4() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = dir.path().join("out.pdf");
    let o = render(LUEBECK, &pdf, &[]);
    let err = stderr(&o);
    assert!(o.status.success(), "{err}");
    assert!(o.stdout.is_empty());
    assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF"));
    assert!(err.contains("1:4000"), "{err}");
    assert!(err.contains("A4"), "{err}");
    assert!(err.contains("symbol factor"), "{err}");
    assert!(err.contains("actual size"), "{err}");
}

#[test]
fn refuses_sprint_at_10000() {
    let dir = tempfile::tempdir().unwrap();
    let o = render(LUEBECK, &dir.path().join("o.pdf"), &["--scale", "10000"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        stderr(&o).contains("1:3000, 1:4000, 1:5000"),
        "{}",
        stderr(&o)
    );
}

#[test]
fn asks_for_kind() {
    let dir = tempfile::tempdir().unwrap();
    let text = std::fs::read_to_string(LUEBECK).unwrap();
    assert!(text.contains("<georeferencing scale=\"4000\""));
    let copy: PathBuf = dir.path().join("copy.omap");
    std::fs::write(
        &copy,
        text.replace(
            "<georeferencing scale=\"4000\"",
            "<georeferencing scale=\"25000\"",
        ),
    )
    .unwrap();
    let o = render(copy.to_str().unwrap(), &dir.path().join("o.pdf"), &[]);
    let err = stderr(&o);
    assert_eq!(o.status.code(), Some(1));
    assert!(err.contains("--kind"), "{err}");
    assert!(
        err.contains("sprint") && err.contains("schoolyard") && err.contains("forest"),
        "{err}"
    );
}

#[test]
fn reports_overflow() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = dir.path().join("o.pdf");
    let cases = [
        (
            &["--paper", "a4", "--orientation", "portrait"][..],
            Orientation::Portrait,
            None,
        ),
        (
            &["--orientation", "landscape", "--scale", "5000"][..],
            Orientation::Landscape,
            Some(5000),
        ),
    ];
    let mut seen = [false, false];
    for (args, orientation, scale) in cases {
        let expected = overflow_expected(LUEBECK, Paper::A4, orientation, scale);
        let o = render(LUEBECK, &pdf, args);
        assert!(o.status.success(), "{}", stderr(&o));
        assert_eq!(
            stderr(&o).contains("does not fit"),
            expected,
            "{}",
            stderr(&o)
        );
        seen[usize::from(expected)] = true;
    }
    assert!(
        seen[0] && seen[1],
        "need one overflowing and one fitting case"
    );
}

#[test]
fn renders_forest_sample() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = dir.path().join("f.pdf");
    let o = render(FOREST, &pdf, &[]);
    let err = stderr(&o);
    assert!(o.status.success(), "{err}");
    assert!(pdf.exists());
    assert!(err.contains("actual size"), "{err}");
    eprintln!("{err}");
}

#[test]
fn missing_parent_directory_fails() {
    let dir = tempfile::tempdir().unwrap();
    let o = render(LUEBECK, &dir.path().join("nope/o.pdf"), &[]);
    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).contains("Error:"), "{}", stderr(&o));
}
