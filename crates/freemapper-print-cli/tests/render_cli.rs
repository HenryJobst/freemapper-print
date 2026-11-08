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

/// The binary with English forced, so a German system locale cannot break the assertions.
fn command() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_freemapper-print"));
    cmd.env("FREEMAPPER_LANG", "en")
        .env("LANG", "C")
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES");
    cmd
}

fn run(args: &[&str]) -> Output {
    command().args(args).output().expect("run binary")
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
    let kind: MapKind =
        infer_kind(&doc, Path::new(map)).expect("the fixture's kind must be inferable");
    let options = PrintOptions {
        print_scale: scale,
        kind,
        paper,
        orientation,
        centre: None,
        show_sheet_objects: false,
        frame_margin_mm: None,
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
    assert!(!dir.path().join("o.pdf").exists());
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
    assert!(!dir.path().join("o.pdf").exists());
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
}

#[test]
fn missing_parent_directory_fails() {
    let dir = tempfile::tempdir().unwrap();
    let o = render(LUEBECK, &dir.path().join("nope/o.pdf"), &[]);
    assert_eq!(o.status.code(), Some(1));
    let err = stderr(&o);
    assert!(
        err.contains("Error:") && err.contains("does not exist"),
        "{err}"
    );
    assert!(!dir.path().join("nope").exists());
}

#[test]
fn overflow_amount_matches_the_library() {
    // Lübeck on A4 portrait at 1:4000 is about 6 cm too wide (62.7 mm on painted bounds when this
    // test was written); the CLI prints the library's number rounded to 0.1 mm.
    let doc = read_map_file(Path::new(LUEBECK)).unwrap();
    let options = PrintOptions {
        print_scale: None,
        kind: MapKind::Sprint,
        paper: Paper::A4,
        orientation: Orientation::Portrait,
        centre: None,
        show_sheet_objects: false,
        frame_margin_mm: None,
    };
    let (x, y) = print_map(&doc, &options, "t")
        .unwrap()
        .1
        .overflow_mm
        .unwrap();
    assert!((55.0..70.0).contains(&x) && y == 0.0, "({x}, {y})");
    let dir = tempfile::tempdir().unwrap();
    let o = render(
        LUEBECK,
        &dir.path().join("o.pdf"),
        &["--orientation", "portrait"],
    );
    let err = stderr(&o);
    assert!(err.contains(&format!("{x:.1} mm too wide")), "{err}");
    assert!(!err.contains("too tall"), "{err}");
}

#[test]
fn renders_on_a3() {
    let dir = tempfile::tempdir().unwrap();
    let o = render(LUEBECK, &dir.path().join("o.pdf"), &["--paper", "a3"]);
    let err = stderr(&o);
    assert!(o.status.success(), "{err}");
    assert!(err.contains("A3"), "{err}");
    assert!(!err.contains("does not fit"), "{err}");
}

#[test]
fn kind_overrides_inference() {
    let dir = tempfile::tempdir().unwrap();
    let o = render(
        LUEBECK,
        &dir.path().join("o.pdf"),
        &["--kind", "forest", "--scale", "4000"],
    );
    let err = stderr(&o);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        err.contains("1:5000, 1:7500"),
        "forest scales expected: {err}"
    );
    assert!(!dir.path().join("o.pdf").exists());
}

#[test]
fn kind_makes_an_uninferable_map_printable() {
    let dir = tempfile::tempdir().unwrap();
    let text = std::fs::read_to_string(LUEBECK).unwrap();
    let copy = dir.path().join("copy.omap");
    std::fs::write(
        &copy,
        text.replace(
            "<georeferencing scale=\"4000\"",
            "<georeferencing scale=\"15000\"",
        ),
    )
    .unwrap();
    let o = render(
        copy.to_str().unwrap(),
        &dir.path().join("o.pdf"),
        &["--kind", "forest"],
    );
    assert!(o.status.success(), "{}", stderr(&o));
}

#[test]
fn show_sheet_objects_adds_content() {
    let dir = tempfile::tempdir().unwrap();
    let plain = dir.path().join("a.pdf");
    let sheet = dir.path().join("b.pdf");
    assert!(render(LUEBECK, &plain, &[]).status.success());
    let o = render(LUEBECK, &sheet, &["--show-sheet-objects"]);
    assert!(o.status.success(), "{}", stderr(&o));
    // The sheet objects add painting operations, so the PDF changes and grows.
    let (plain, sheet) = (
        std::fs::read(&plain).unwrap(),
        std::fs::read(&sheet).unwrap(),
    );
    assert_ne!(plain, sheet);
    assert!(
        sheet.len() > plain.len(),
        "{} vs {}",
        sheet.len(),
        plain.len()
    );
}

#[test]
fn rejects_a_text_file() {
    let dir = tempfile::tempdir().unwrap();
    let txt = dir.path().join("notes.omap");
    std::fs::write(&txt, "this is not a map").unwrap();
    let pdf = dir.path().join("o.pdf");
    let o = render(txt.to_str().unwrap(), &pdf, &[]);
    let err = stderr(&o);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        err.contains("Error:") && err.contains("could not read the map"),
        "{err}"
    );
    assert!(!pdf.exists());
}

#[test]
fn relative_out_resolves_against_the_working_directory() {
    let dir = tempfile::tempdir().unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_freemapper-print"))
        .current_dir(dir.path())
        .args(["render", LUEBECK, "--out", "rel.pdf"])
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", stderr(&o));
    assert!(dir.path().join("rel.pdf").exists());
}

#[test]
fn refuses_to_overwrite_the_map() {
    let dir = tempfile::tempdir().unwrap();
    let copy = dir.path().join("m.omap");
    std::fs::copy(LUEBECK, &copy).unwrap();
    let before = std::fs::read(&copy).unwrap();
    // Same file through a different spelling.
    let alias = dir.path().join(".").join("m.omap");
    let o = render(copy.to_str().unwrap(), &alias, &[]);
    let err = stderr(&o);
    assert_eq!(o.status.code(), Some(1));
    assert!(err.contains("--out points at the map file"), "{err}");
    assert_eq!(std::fs::read(&copy).unwrap(), before);
}

#[test]
fn usage_errors_exit_with_2() {
    assert_eq!(run(&["render"]).status.code(), Some(2));
}

#[test]
fn reduction_warns_enlargement_does_not() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = dir.path().join("o.pdf");
    let o = render(LUEBECK, &pdf, &["--scale", "5000"]);
    let err = stderr(&o);
    assert!(o.status.success(), "{err}");
    assert!(
        err.contains("1:5000 is not a standard scale for sprint maps")
            && err.contains("training only"),
        "{err}"
    );
    let o = render(LUEBECK, &pdf, &["--scale", "3000", "--paper", "a3"]);
    let err = stderr(&o);
    assert!(o.status.success(), "{err}");
    assert!(err.contains("symbol factor 1.333"), "{err}");
    assert!(!err.contains("not a standard scale"), "{err}");
}

#[test]
fn broken_sidecar_gets_a_note() {
    let dir = tempfile::tempdir().unwrap();
    let copy = dir.path().join("m.omap");
    std::fs::copy(LUEBECK, &copy).unwrap();
    std::fs::write(dir.path().join("freemapper.toml"), "map_kind = 3\n").unwrap();
    let o = render(copy.to_str().unwrap(), &dir.path().join("o.pdf"), &[]);
    let err = stderr(&o);
    assert!(o.status.success(), "{err}");
    assert!(
        err.contains("freemapper.toml is not valid") && err.contains("comes from the map file"),
        "{err}"
    );
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn render_messages_in_german() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("o.pdf");
    let o = render(LUEBECK, &out, &["--paper", "a4", "--lang", "de"]);
    assert!(o.status.success(), "{}", stderr(&o));
    let err = stderr(&o);
    assert!(err.contains("geschrieben: A4 Querformat"), "{err}");
    assert!(err.contains("Druckmaßstab 1:4000"), "{err}");
    assert!(err.contains("Symbolfaktor 1,000"), "{err}");
    assert!(err.contains("In Originalgröße drucken"), "{err}");
    assert!(!err.contains("actual size"), "{err}");
}

#[test]
fn lang_works_before_the_subcommand_and_from_the_environment() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("o.pdf");
    let o = run(&[
        "--lang=de",
        "render",
        LUEBECK,
        "--out",
        out.to_str().unwrap(),
    ]);
    assert!(
        stderr(&o).contains("In Originalgröße drucken"),
        "{}",
        stderr(&o)
    );
    // --lang beats the environment; an unknown FREEMAPPER_LANG falls through to LANG.
    let o = command()
        .args(["render", LUEBECK, "--out", out.to_str().unwrap()])
        .env("FREEMAPPER_LANG", "fr")
        .env("LANG", "de_DE.UTF-8")
        .output()
        .unwrap();
    assert!(
        stderr(&o).contains("In Originalgröße drucken"),
        "{}",
        stderr(&o)
    );
    let o = command()
        .args([
            "--lang",
            "en",
            "render",
            LUEBECK,
            "--out",
            out.to_str().unwrap(),
        ])
        .env("FREEMAPPER_LANG", "de")
        .output()
        .unwrap();
    assert!(stderr(&o).contains("actual size"), "{}", stderr(&o));
}

#[test]
fn unknown_lang_is_a_usage_error() {
    let o = run(&["--lang", "fr", "render", LUEBECK, "--out", "x.pdf"]);
    assert_eq!(o.status.code(), Some(2));
    let err = stderr(&o);
    assert!(
        err.contains("unknown language \"fr\"") && err.contains("en, de"),
        "{err}"
    );
}

#[test]
fn errors_come_in_german_too() {
    let o = run(&[
        "--lang",
        "de",
        "render",
        LUEBECK,
        "--out",
        "/no/such/dir/o.pdf",
    ]);
    assert_eq!(o.status.code(), Some(1));
    let err = stderr(&o);
    assert!(
        err.starts_with("Fehler: ") && err.contains("existiert nicht"),
        "{err}"
    );
}

#[test]
fn help_follows_the_language() {
    let o = run(&["render", "--help"]);
    let en = stdout(&o);
    assert!(
        en.contains("Render a map to a PDF at an exact print scale."),
        "{en}"
    );
    assert!(en.contains("Exit codes: 0 on success"), "{en}");
    assert!(en.contains("--lang"), "{en}");
    let o = run(&["render", "--help", "--lang", "de"]);
    let de = stdout(&o);
    assert!(de.contains("Schreibt eine Karte als PDF"), "{de}");
    assert!(de.contains("Zu schreibende PDF-Datei"), "{de}");
    assert!(de.contains("Exit-Codes: 0 bei Erfolg"), "{de}");
    assert!(!de.contains("Paper size"), "{de}");
    let top = stdout(&run(&["--help", "--lang", "de"]));
    assert!(
        top.contains("Druckt fertige Orientierungslaufkarten"),
        "{top}"
    );
}
