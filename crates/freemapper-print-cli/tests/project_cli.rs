//! Runs the project commands `init`, `check` and `export` of the `freemapper-print` binary on
//! projects built in temp dirs around the fixture maps.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use freemapper_print::export::{export_layouts, live_pages, select_layouts};
use freemapper_print::omap::read::read_map_file;
use freemapper_print::project::{self, FILE_NAME};
use freemapper_print::scale::MapKind;
use freemapper_print::sheet::{LogoCache, render_sheet};

const LUEBECK: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/maps/luebeck.omap"
);
const FOREST: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/maps/forest-sample.omap"
);
const CLUB_PNG: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/logos/club.png"
);

/// The binary with the program language `lang` forced, so the system locale cannot break the
/// assertions.
fn command(lang: &str) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_freemapper-print"));
    cmd.env("FREEMAPPER_LANG", lang)
        .env("LANG", "C")
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env_remove(freemapper_print::output::FAKE_FREE_SPACE_ENV);
    cmd
}

fn run(lang: &str, args: &[&str]) -> Output {
    command(lang).args(args).output().expect("run binary")
}

fn run_owned(lang: &str, args: &[String]) -> Output {
    command(lang).args(args).output().expect("run binary")
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// A temp dir with `maps/<name>` copied from `map` and an empty `event/` folder.
struct Workspace {
    dir: tempfile::TempDir,
    map: PathBuf,
}

impl Workspace {
    fn new(map: &str) -> Workspace {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("maps")).unwrap();
        std::fs::create_dir(dir.path().join("event")).unwrap();
        let target = dir
            .path()
            .join("maps")
            .join(Path::new(map).file_name().unwrap());
        std::fs::copy(map, &target).unwrap();
        Workspace { dir, map: target }
    }

    fn event(&self) -> PathBuf {
        self.dir.path().join("event")
    }

    fn project_file(&self) -> PathBuf {
        self.event().join(FILE_NAME)
    }

    /// Writes the project file: the head with the map and `kind`, then `rest`.
    fn write_project(&self, kind: &str, extra_map: &str, rest: &str) -> PathBuf {
        let name = self.map.file_name().unwrap().to_str().unwrap();
        let text = format!(
            "format = 1\n[event]\ntitle = \"Test\"\n[sources]\nmap = \"../maps/{name}\"\n\
             [map]\nkind = \"{kind}\"\n{extra_map}\n{rest}"
        );
        std::fs::write(self.project_file(), text).unwrap();
        self.project_file()
    }
}

/// A shared scale check bar in the bottom-right corner of the map frame.
const BAR: &str = "[[shared]]\nid = \"bar\"\nkind = \"scale_check_bar\"\n\
                   [shared.anchor]\nto = \"map_frame\"\ncorner = \"bottom_right\"\n";

fn layouts(ids: &[(&str, &str, &str)]) -> String {
    ids.iter()
        .map(|(id, paper, orientation)| {
            format!(
                "[[layouts]]\nid = \"{id}\"\npaper = \"{paper}\"\norientation = \"{orientation}\"\n"
            )
        })
        .collect()
}

/// The MediaBox of a one-page PDF in points.
fn media_box(pdf: &[u8]) -> [f64; 4] {
    let find = |hay: &[u8], needle: &[u8]| hay.windows(needle.len()).position(|w| w == needle);
    let start = find(pdf, b"/MediaBox [").expect("MediaBox") + 11;
    let end = start + find(&pdf[start..], b"]").expect("end of MediaBox");
    let text = std::str::from_utf8(&pdf[start..end]).unwrap();
    let v: Vec<f64> = text
        .split_whitespace()
        .map(|n| n.parse().unwrap())
        .collect();
    [v[0], v[1], v[2], v[3]]
}

fn assert_page_mm(pdf_path: &Path, w_mm: f64, h_mm: f64) {
    let pdf = std::fs::read(pdf_path).unwrap();
    let b = media_box(&pdf);
    let pt = |mm: f64| mm * 72.0 / 25.4;
    assert!(
        (b[2] - b[0] - pt(w_mm)).abs() < 0.01 && (b[3] - b[1] - pt(h_mm)).abs() < 0.01,
        "{}: {b:?}",
        pdf_path.display()
    );
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|r| {
            r.map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

// ---- init ----

#[test]
fn init_writes_project() {
    let ws = Workspace::new(LUEBECK);
    let o = run(
        "en",
        &[
            "init",
            ws.event().to_str().unwrap(),
            "--map",
            ws.map.to_str().unwrap(),
            "--kind",
            "sprint",
            "--title",
            "Stadtlauf",
        ],
    );
    let err = stderr(&o);
    assert!(o.status.success(), "{err}");
    let text = std::fs::read_to_string(ws.project_file()).unwrap();
    let loaded = project::load(&ws.project_file()).expect("the new file loads");
    let p = &loaded.project;
    assert_eq!(p.sources.map, Path::new("../maps/luebeck.omap"), "{text}");
    assert_eq!(p.event.title, "Stadtlauf");
    assert_eq!(p.map.kind, MapKind::Sprint);
    // ISSprOM 2019 and freemapper both use 2 m.
    assert_eq!(p.map.contour_interval_m, Some(2.0));
    assert_eq!(p.layouts.len(), 1);
    // Lübeck does not fit A4 at 1:4000; A3 with room for the footer does.
    assert_eq!(p.layouts[0].id, "a3");
    assert_eq!(p.layouts[0].paper, freemapper_print::print::Paper::A3);
    assert_eq!(p.layouts[0].print_scale, Some(4000));
    let title = p.shared.iter().find(|e| e.id == "title").unwrap();
    assert_eq!(
        title.lines,
        ["{event}", "1:{print_scale} · {contour_interval}"]
    );
    assert!(p.shared.iter().any(|e| e.id == "scale_check_bar"));
    // The proposal is marked in the file, right above the value, and on stderr.
    let above = text
        .lines()
        .take_while(|l| !l.starts_with("contour_interval_m"))
        .last()
        .unwrap();
    assert!(
        above.starts_with('#') && above.to_lowercase().contains("confirm"),
        "{text}"
    );
    assert!(
        err.contains("2 m") && err.contains("confirm") && err.contains("sprint"),
        "{err}"
    );
    assert!(
        err.contains(&ws.project_file().display().to_string()),
        "{err}"
    );
    // Where the kind came from, a line before the fit search, and the plain next step.
    assert!(
        err.contains("Map kind: sprint maps, as --kind says."),
        "{err}"
    );
    assert!(err.contains("Looking for an A4 or A3 sheet"), "{err}");
    assert!(err.contains("check") && !err.contains("Warning"), "{err}");
}

#[test]
fn init_takes_kind_from_freemapper_toml_in_german() {
    let ws = Workspace::new(FOREST);
    std::fs::write(
        ws.dir.path().join("maps/freemapper.toml"),
        "map_kind = \"forest\"\n",
    )
    .unwrap();
    let o = run(
        "de",
        &[
            "init",
            ws.event().to_str().unwrap(),
            "--map",
            ws.map.to_str().unwrap(),
        ],
    );
    let err = stderr(&o);
    assert!(o.status.success(), "{err}");
    let p = project::load(&ws.project_file()).unwrap().project;
    assert_eq!(p.map.kind, MapKind::Forest);
    assert_eq!(p.map.contour_interval_m, Some(5.0));
    assert_eq!(p.event.lang, project::SheetLang::De);
    assert_eq!(p.event.title, "forest-sample");
    assert!(err.contains("Äquidistanz") && err.contains("5 m"), "{err}");
    assert!(
        err.contains("Kartenart: Waldkarten, wie in der freemapper.toml"),
        "{err}"
    );
    // The generated project passes the check: the forest sample fits on A4.
    let o = run("de", &["check", ws.project_file().to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
}

#[test]
fn init_schoolyard_proposes_no_contour_interval() {
    let ws = Workspace::new(LUEBECK);
    let o = run(
        "en",
        &[
            "init",
            ws.event().to_str().unwrap(),
            "--map",
            ws.map.to_str().unwrap(),
            "--kind",
            "schoolyard",
        ],
    );
    assert!(o.status.success(), "{}", stderr(&o));
    let text = std::fs::read_to_string(ws.project_file()).unwrap();
    let p = project::load(&ws.project_file()).unwrap().project;
    assert_eq!(p.map.contour_interval_m, None);
    assert!(
        text.lines()
            .any(|l| l.starts_with('#') && l.contains("contour_interval_m")),
        "{text}"
    );
    assert!(stderr(&o).contains("contour_interval_m"), "{}", stderr(&o));
}

/// A copy of Lübeck at 1:25000, whose kind the program cannot infer.
fn uninferable_map(ws: &Workspace) -> PathBuf {
    let text = std::fs::read_to_string(LUEBECK).unwrap();
    let copy = ws.dir.path().join("maps/odd.omap");
    std::fs::write(
        &copy,
        text.replace(
            "<georeferencing scale=\"4000\"",
            "<georeferencing scale=\"25000\"",
        ),
    )
    .unwrap();
    copy
}

#[test]
fn init_on_luebeck_passes_check() {
    let ws = Workspace::new(LUEBECK);
    // No --kind and no freemapper.toml: the kind comes from the map file, as for render.
    let o = run(
        "en",
        &[
            "init",
            ws.event().to_str().unwrap(),
            "--map",
            ws.map.to_str().unwrap(),
        ],
    );
    assert!(o.status.success(), "{}", stderr(&o));
    let p = project::load(&ws.project_file()).unwrap().project;
    assert_eq!(p.map.kind, MapKind::Sprint);
    assert!(stderr(&o).contains("A3"), "{}", stderr(&o));
    let o = run("en", &["check", ws.event().to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
}

#[test]
fn init_refuses_control_characters_in_the_title() {
    let ws = Workspace::new(LUEBECK);
    for (lang, word) in [("en", "line breaks"), ("de", "Zeilenumbrüche")] {
        let o = run(
            lang,
            &[
                "init",
                ws.event().to_str().unwrap(),
                "--map",
                ws.map.to_str().unwrap(),
                "--kind",
                "sprint",
                "--title",
                "Lauf\n[map]\ncontour_interval_m = 9",
            ],
        );
        assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
        assert!(stderr(&o).contains(word), "{lang}: {}", stderr(&o));
        assert!(!ws.project_file().exists());
    }
}

#[test]
fn init_names_a_broken_freemapper_toml() {
    let ws = Workspace::new(LUEBECK);
    let map = uninferable_map(&ws);
    std::fs::write(
        ws.dir.path().join("maps/freemapper.toml"),
        "map_kind = 12\n",
    )
    .unwrap();
    let o = run(
        "en",
        &[
            "init",
            ws.event().to_str().unwrap(),
            "--map",
            map.to_str().unwrap(),
        ],
    );
    let err = stderr(&o);
    assert_eq!(o.status.code(), Some(1), "{err}");
    assert!(
        err.contains("freemapper.toml is not valid") && err.contains("--kind"),
        "{err}"
    );
    assert!(!ws.project_file().exists());
}

#[test]
fn init_asks_for_kind() {
    let ws = Workspace::new(LUEBECK);
    let map = uninferable_map(&ws);
    for (lang, word) in [("en", "cannot tell the map kind"), ("de", "Kartenart")] {
        let o = run(
            lang,
            &[
                "init",
                ws.event().to_str().unwrap(),
                "--map",
                map.to_str().unwrap(),
            ],
        );
        assert!(stderr(&o).contains("--kind"), "{}", stderr(&o));
        assert_eq!(o.status.code(), Some(1), "{}", stderr(&o));
        assert!(stderr(&o).contains(word), "{}", stderr(&o));
        assert!(!ws.project_file().exists());
    }
}

#[test]
fn init_refuses_overwrite() {
    let ws = Workspace::new(LUEBECK);
    std::fs::write(ws.project_file(), "keep").unwrap();
    for (lang, word) in [("en", "exists already"), ("de", "gibt es schon")] {
        let o = run(
            lang,
            &[
                "init",
                ws.event().to_str().unwrap(),
                "--map",
                ws.map.to_str().unwrap(),
                "--kind",
                "sprint",
            ],
        );
        assert_eq!(o.status.code(), Some(1), "{}", stderr(&o));
        assert!(stderr(&o).contains(word), "{lang}: {}", stderr(&o));
        assert_eq!(std::fs::read_to_string(ws.project_file()).unwrap(), "keep");
    }
}

// ---- check ----

#[test]
fn check_reports_notes() {
    let ws = Workspace::new(LUEBECK);
    std::fs::copy(CLUB_PNG, ws.event().join("club.png")).unwrap();
    let path = ws.write_project(
        "sprint",
        "",
        &format!(
            "{}[[layouts.elements]]\nid = \"club\"\nkind = \"logo\"\nfile = \"club.png\"\n\
             width_mm = 400.0\n[layouts.elements.anchor]\nto = \"page\"\ncorner = \"top_left\"\n",
            layouts(&[("a4", "A4", "landscape")])
        ),
    );
    let o = run("en", &["check", path.to_str().unwrap()]);
    let err = stderr(&o);
    assert_eq!(o.status.code(), Some(1), "{err}");
    assert!(
        err.contains("club") && err.contains("past the page edge"),
        "{err}"
    );
    assert!(
        err.contains("map.contour_interval_m") && err.contains("{contour_interval}"),
        "{err}"
    );
    assert!(err.contains("a4"), "{err}");
    let o = run("de", &["check", path.to_str().unwrap()]);
    let err = stderr(&o);
    assert_eq!(o.status.code(), Some(1), "{err}");
    assert!(
        err.contains("club") && err.contains("über den Blattrand"),
        "{err}"
    );
    assert!(err.contains("Äquidistanz"), "{err}");
    // check writes nothing.
    assert_eq!(files_in(&ws.event()), ["club.png", FILE_NAME]);
}

#[test]
fn check_fails_on_a_broken_project() {
    let ws = Workspace::new(LUEBECK);
    let path = ws.write_project("sprint", "", "[[layouts]]\nid = \"a\"\npaperr = \"A4\"\n");
    let o = run("en", &["check", path.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).contains("paperr"), "{}", stderr(&o));
}

#[test]
fn check_goes_on_after_a_layout_that_cannot_be_drawn() {
    let ws = Workspace::new(LUEBECK);
    let rest = "[[layouts]]\nid = \"bad\"\nprint_scale = 10000\n[[layouts]]\nid = \"good\"\n";
    let path = ws.write_project("sprint", "contour_interval_m = 2.5", rest);
    let o = run("en", &["check", path.to_str().unwrap()]);
    let err = stderr(&o);
    assert_eq!(o.status.code(), Some(1), "{err}");
    let bad = err
        .find("could not draw layout bad")
        .expect("error for bad");
    let good = err.find("Layout good:").expect("good is checked too");
    assert!(bad < good, "{err}");
    assert!(err.contains("Layouts with notes: 2 of 2."), "{err}");
}

#[test]
fn check_passes_with_added_north_lines_only() {
    let ws = Workspace::new(FOREST);
    let rest = format!(
        "{}[[shared]]\nid = \"t\"\nkind = \"title_block\"\nlines = [\"1:{{print_scale}} {{contour_interval}}\"]\n\
         [shared.anchor]\nto = \"map_frame\"\ncorner = \"top_left\"\n{BAR}",
        layouts(&[("a4", "A4", "auto")])
    );
    let path = ws.write_project("forest", "contour_interval_m = 5.0", &rest);
    let o = run("en", &["check", path.to_str().unwrap()]);
    let err = stderr(&o);
    assert_eq!(o.status.code(), Some(0), "{err}");
    assert!(err.contains("north lines"), "{err}");
    assert!(err.contains("mapper's job in OOM"), "{err}");
}

// ---- export ----

#[test]
fn export_all_layouts() {
    let ws = Workspace::new(LUEBECK);
    let path = ws.write_project(
        "sprint",
        "contour_interval_m = 2.5",
        &layouts(&[("a4", "A4", "portrait"), ("big", "A3", "landscape")]),
    );
    let o = run("en", &["export", path.to_str().unwrap()]);
    let err = stderr(&o);
    assert!(o.status.success(), "{err}");
    let out = ws.event().join("out");
    assert_eq!(files_in(&out), ["a4.pdf", "big.pdf"]);
    assert_page_mm(&out.join("a4.pdf"), 210.0, 297.0);
    assert_page_mm(&out.join("big.pdf"), 420.0, 297.0);
    // Notes per layout, the reminder once at the end.
    let a4 = err.find("a4.pdf").expect("a4 summary");
    let big = err.find("big.pdf").expect("big summary");
    assert!(a4 < big, "{err}");
    assert_eq!(err.matches("actual size").count(), 1, "{err}");
    assert!(err.trim_end().ends_with("breaks the scale."), "{err}");
    // The MissingScaleOrContour note of each layout.
    assert_eq!(err.matches("{print_scale}").count(), 2, "{err}");
}

#[test]
fn export_twenty_layouts() {
    let ws = Workspace::new(FOREST);
    let ids: Vec<String> = (0..20).map(|i| format!("l{i:02}")).collect();
    let specs: Vec<(&str, &str, &str)> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), if i % 2 == 0 { "A4" } else { "A3" }, "auto"))
        .collect();
    let path = ws.write_project("forest", "contour_interval_m = 5.0", &layouts(&specs));
    let out_dir = ws.dir.path().join("pdfs");
    std::fs::create_dir(&out_dir).unwrap();
    let o = run(
        "en",
        &[
            "export",
            path.to_str().unwrap(),
            "--out-dir",
            out_dir.to_str().unwrap(),
        ],
    );
    assert!(o.status.success(), "{}", stderr(&o));
    let expected: Vec<String> = ids.iter().map(|id| format!("{id}.pdf")).collect();
    assert_eq!(files_in(&out_dir), expected);

    // The CLI runs this loop. In-process, the live-page counter shows one page at a time.
    let loaded = project::load(&path).unwrap();
    let map = read_map_file(&loaded.map_path()).unwrap();
    let biggest = loaded
        .project
        .layouts
        .iter()
        .map(|l| {
            let (page, _) = render_sheet(&loaded, &map, l, &LogoCache::new()).unwrap();
            page.primitives.len() + page.sheet.len()
        })
        .max()
        .unwrap();
    let selected = select_layouts(&loaded, &[]).unwrap();
    let lib_dir = tempfile::tempdir().unwrap();
    live_pages::reset();
    let mut seen = 0;
    export_layouts(&loaded, &map, &selected, lib_dir.path(), |done| {
        assert_eq!(
            live_pages::stats().live,
            0,
            "page of {} still alive",
            done.layout.id
        );
        assert!(done.path.is_file());
        seen += 1;
    })
    .unwrap();
    let stats = live_pages::stats();
    assert_eq!(seen, 20);
    assert_eq!(stats.peak_live, 1, "{stats:?}");
    assert_eq!(stats.peak_items, biggest, "{stats:?}");
    assert_eq!(stats.live, 0, "{stats:?}");
}

#[test]
fn export_one_layout() {
    let ws = Workspace::new(FOREST);
    let path = ws.write_project(
        "forest",
        "contour_interval_m = 5.0",
        &layouts(&[("a", "A4", "auto"), ("b", "A3", "portrait")]),
    );
    let o = run("de", &["export", path.to_str().unwrap(), "--layout", "b"]);
    let err = stderr(&o);
    assert!(o.status.success(), "{err}");
    assert!(
        err.contains("b.pdf geschrieben") && err.contains("„Tatsächliche Größe“"),
        "{err}"
    );
    let out = ws.event().join("out");
    assert_eq!(files_in(&out), ["b.pdf"]);
    assert_page_mm(&out.join("b.pdf"), 297.0, 420.0);

    for (lang, word) in [("en", "no layout"), ("de", "kein Layout")] {
        let o = run(lang, &["export", path.to_str().unwrap(), "--layout", "zz"]);
        let err = stderr(&o);
        assert_eq!(o.status.code(), Some(2), "{err}");
        assert!(
            err.contains(word) && err.contains("zz") && err.contains("a, b"),
            "{lang}: {err}"
        );
    }
    assert_eq!(files_in(&out), ["b.pdf"]);
}

#[test]
fn runs_from_other_cwd() {
    let ws = Workspace::new(FOREST);
    let rest = format!(
        "{}[[shared]]\nid = \"t\"\nkind = \"title_block\"\nlines = [\"1:{{print_scale}} {{contour_interval}}\"]\n\
         [shared.anchor]\nto = \"map_frame\"\ncorner = \"top_left\"\n{BAR}",
        layouts(&[("a4", "A4", "auto")])
    );
    let path = ws.write_project("forest", "contour_interval_m = 5.0", &rest);
    let elsewhere = tempfile::tempdir().unwrap();
    // An absolute project path from an unrelated folder.
    let o = command("en")
        .current_dir(elsewhere.path())
        .args(["export", path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", stderr(&o));
    assert_eq!(files_in(&ws.event().join("out")), ["a4.pdf"]);
    assert!(files_in(elsewhere.path()).is_empty());
    // A relative project folder and a relative out dir from the workspace root.
    let o = command("en")
        .current_dir(ws.dir.path())
        .args(["export", "event", "--out-dir", "maps"])
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", stderr(&o));
    assert!(ws.dir.path().join("maps/a4.pdf").is_file());
    let o = command("en")
        .current_dir(ws.dir.path())
        .args(["check", "event"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
}

#[test]
fn export_disk_full() {
    let ws = Workspace::new(FOREST);
    let path = ws.write_project(
        "forest",
        "contour_interval_m = 5.0",
        &layouts(&[("a4", "A4", "auto")]),
    );
    for (lang, word) in [
        ("en", "not enough disk space"),
        ("de", "nicht genug Speicherplatz"),
    ] {
        let o = command(lang)
            .env(freemapper_print::output::FAKE_FREE_SPACE_ENV, "1000")
            .args(["export", path.to_str().unwrap()])
            .output()
            .unwrap();
        let err = stderr(&o);
        assert_eq!(o.status.code(), Some(1), "{err}");
        assert!(
            err.contains(word) && err.contains("a4.pdf"),
            "{lang}: {err}"
        );
        assert!(
            files_in(&ws.event().join("out")).is_empty(),
            "no PDF, no .part"
        );
    }
}

#[test]
fn export_needs_an_existing_out_dir() {
    let ws = Workspace::new(FOREST);
    let path = ws.write_project(
        "forest",
        "contour_interval_m = 5.0",
        &layouts(&[("a4", "A4", "auto")]),
    );
    let missing = ws.dir.path().join("nope");
    let o = run(
        "en",
        &[
            "export",
            path.to_str().unwrap(),
            "--out-dir",
            missing.to_str().unwrap(),
        ],
    );
    assert_eq!(o.status.code(), Some(1), "{}", stderr(&o));
    assert!(stderr(&o).contains("does not exist"), "{}", stderr(&o));
    assert!(!missing.exists());
}

#[test]
fn missing_project_file_names_the_os_error_once() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("nothere.toml");
    for lang in ["en", "de"] {
        let o = run(lang, &["check", missing.to_str().unwrap()]);
        assert_eq!(o.status.code(), Some(1));
        let err = stderr(&o);
        assert!(err.contains("nothere.toml"), "{err}");
        assert_eq!(err.matches("os error 2").count(), 1, "{err}");
    }
}

#[test]
fn unwritable_output_names_the_os_error_once() {
    let ws = Workspace::new(LUEBECK);
    let o = run(
        "en",
        &[
            "init",
            ws.event().to_str().unwrap(),
            "--map",
            ws.map.to_str().unwrap(),
        ],
    );
    assert!(o.status.success(), "{}", stderr(&o));
    // A folder where the PDF should go: renaming the finished file onto it fails.
    for id in ["a4", "a3"] {
        let blocked = ws.event().join("out").join(format!("{id}.pdf"));
        std::fs::create_dir_all(&blocked).unwrap();
        std::fs::write(blocked.join("x"), "x").unwrap();
    }
    let o = run("en", &["export", ws.event().to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(1));
    let err = stderr(&o);
    assert!(err.contains(".pdf"), "{err}");
    assert_eq!(err.matches("os error").count(), 1, "{err}");
}

#[test]
fn render_into_a_folder_names_the_os_error_once() {
    let ws = Workspace::new(LUEBECK);
    let blocked = ws.event().join("map.pdf");
    std::fs::create_dir_all(&blocked).unwrap();
    std::fs::write(blocked.join("x"), "x").unwrap();
    let args = [
        "render",
        ws.map.to_str().unwrap(),
        "--out",
        blocked.to_str().unwrap(),
    ];
    let o = run("en", &args);
    assert_eq!(o.status.code(), Some(1));
    let err = stderr(&o);
    assert!(err.contains("map.pdf"), "{err}");
    assert_eq!(err.matches("os error").count(), 1, "{err}");
}

#[test]
fn init_without_fit_warns_and_says_check_will_complain() {
    let ws = Workspace::new(LUEBECK);
    // Lübeck at 1:5000, no standard sprint scale: every sheet has a note.
    let text = std::fs::read_to_string(&ws.map).unwrap();
    let text = text.replacen(
        "<georeferencing scale=\"4000\"",
        "<georeferencing scale=\"5000\"",
        1,
    );
    std::fs::write(&ws.map, text).unwrap();
    for (lang, warning, next) in [
        (
            "en",
            "Warning: no A4 or A3 sheet",
            "until the layout changes",
        ),
        ("de", "Warnung: Weder auf A4", "bis das Layout geändert ist"),
    ] {
        let folder = ws.event().join(lang);
        let o = run(
            lang,
            &[
                "init",
                folder.to_str().unwrap(),
                "--map",
                ws.map.to_str().unwrap(),
                "--kind",
                "sprint",
            ],
        );
        let err = stderr(&o);
        assert!(o.status.success(), "{err}");
        assert!(err.contains(warning) && err.contains(next), "{lang}: {err}");
    }
}

// ---- courses ----

const LUEBECK_COURSES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../freemapper-print/tests/fixtures/courses/luebeck-sheets.xml"
);

/// A project on the Lübeck map with the two courses of `luebeck-sheets.xml`: `Kurz` on the first layout (no
/// assignment), `D/H 12` assigned to an A3 layout.
fn course_workspace() -> (Workspace, PathBuf) {
    let ws = Workspace::new(LUEBECK);
    std::fs::copy(LUEBECK_COURSES, ws.event().join("c.xml")).unwrap();
    let text = format!(
        "format = 1\n[event]\ntitle = \"Lübeck: Training\"\n[sources]\nmap = \"../maps/luebeck.omap\"\n\
         courses = \"c.xml\"\ncourse_y_axis = \"up\"\n[map]\nkind = \"sprint\"\ncontour_interval_m = 2.0\n{}\
         {}[[assignments]]\ncourse = \"D/H 12\"\nlayout = \"a3\"\n",
        BAR,
        layouts(&[("a4", "A4", "portrait"), ("a3", "A3", "landscape")])
    );
    std::fs::write(ws.project_file(), text).unwrap();
    let path = ws.project_file();
    (ws, path)
}

#[test]
fn export_course_filter_and_single_file() {
    let (ws, path) = course_workspace();
    let p = path.to_str().unwrap();
    let o = run("en", &["export", p]);
    let err = stderr(&o);
    assert_eq!(o.status.code(), Some(0), "{err}");
    let out = ws.event().join("out");
    assert_eq!(files_in(&out), ["D-H 12.pdf", "Kurz.pdf"]);
    assert_page_mm(&out.join("Kurz.pdf"), 210.0, 297.0);
    assert_page_mm(&out.join("D-H 12.pdf"), 420.0, 297.0);
    assert!(err.contains("for course \"Kurz\" on layout a4"), "{err}");
    assert!(err.contains("course \"Kurz\" has no assignment"), "{err}");
    // The notes of the course file come once, before the courses.
    assert_eq!(err.matches("not checked").count(), 1, "{err}");
    // --course picks courses; the rest stays as written.
    std::fs::remove_dir_all(&out).unwrap();
    let o = run("de", &["export", p, "--course", "D/H 12"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(files_in(&out), ["D-H 12.pdf"]);
    assert!(
        stderr(&o).contains("für Bahn „D/H 12“ auf Layout a3"),
        "{}",
        stderr(&o)
    );
    // One PDF with a page per course, named after the event title.
    let o = run("en", &["export", p, "--single-file"]);
    let err = stderr(&o);
    assert_eq!(o.status.code(), Some(0), "{err}");
    let single = "Lübeck- Training - alle Bahnen.pdf";
    assert_eq!(files_in(&out), ["D-H 12.pdf", single]);
    let pdf = std::fs::read(out.join(single)).unwrap();
    assert_eq!(pdf.windows(9).filter(|w| w == b"/MediaBox").count(), 2);
    assert!(pdf.windows(8).any(|w| w == b"/Count 2"));
    assert!(
        err.contains("Page 2, course \"D/H 12\"") && err.contains("with 2 pages"),
        "{err}"
    );
    // An unknown course is a usage error that lists the known ones.
    let o = run("en", &["export", p, "--course", "Lang"]);
    let err = stderr(&o);
    assert_eq!(o.status.code(), Some(2), "{err}");
    assert!(
        err.contains("\"Lang\"") && err.contains("\"Kurz\", \"D/H 12\""),
        "{err}"
    );
    // --layout exports the layout sheets without courses and cannot be combined with --course.
    std::fs::remove_dir_all(&out).unwrap();
    let o = run("en", &["export", p, "--layout", "a3"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(files_in(&out), ["a3.pdf"]);
    let o = run("en", &["export", p, "--layout", "a3", "--course", "Kurz"]);
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
}

#[test]
fn check_runs_per_course() {
    let (ws, path) = course_workspace();
    let o = run("en", &["check", path.to_str().unwrap()]);
    let err = stderr(&o);
    // No description box yet: every course has a note.
    assert_eq!(o.status.code(), Some(1), "{err}");
    assert!(err.contains("Course \"Kurz\" on layout a4"), "{err}");
    assert!(err.contains("Course \"D/H 12\" on layout a3"), "{err}");
    assert!(err.contains("Courses with notes: 2 of 2."), "{err}");
    assert!(!err.contains("Layout a4:"), "{err}");
    let o = run("de", &["check", path.to_str().unwrap()]);
    assert!(
        stderr(&o).contains("Bahnen mit Hinweisen: 2 von 2."),
        "{}",
        stderr(&o)
    );
    assert_eq!(files_in(&ws.event()), ["c.xml", FILE_NAME]);
}

#[test]
fn course_flags_need_a_course_file() {
    let ws = Workspace::new(LUEBECK);
    let path = ws.write_project("sprint", "", &layouts(&[("a4", "A4", "portrait")]));
    let o = run("en", &["export", path.to_str().unwrap(), "--single-file"]);
    let err = stderr(&o);
    assert_eq!(o.status.code(), Some(2), "{err}");
    assert!(err.contains("[sources] courses"), "{err}");
}

#[test]
fn nothing_to_export_writes_no_pdf() {
    // The only course offers alternative controls, so the reader leaves it out.
    let (ws, path) = course_workspace();
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<CourseData xmlns="http://www.orienteering.org/datastandard/3.0" iofVersion="3.0">
  <RaceCourseData>
    <Map><Scale>4000</Scale></Map>
    <Control type="Start"><Id>S1</Id><MapPosition x="-50" y="60" unit="mm"/></Control>
    <Control type="Control"><Id>31</Id><MapPosition x="20" y="-30" unit="mm"/></Control>
    <Control type="Control"><Id>32</Id><MapPosition x="100" y="-120" unit="mm"/></Control>
    <Control type="Finish"><Id>F1</Id><MapPosition x="60" y="20" unit="mm"/></Control>
    <Course>
      <Name>Wahl</Name>
      <CourseControl type="Start"><Control>S1</Control></CourseControl>
      <CourseControl type="Control"><Control>31</Control><Control>32</Control></CourseControl>
      <CourseControl type="Finish"><Control>F1</Control></CourseControl>
    </Course>
  </RaceCourseData>
</CourseData>
"#;
    std::fs::write(ws.event().join("c.xml"), xml).unwrap();
    for single in [false, true] {
        let mut args = vec!["export", path.to_str().unwrap()];
        if single {
            args.push("--single-file");
        }
        let o = run("en", &args);
        let err = stderr(&o);
        assert_eq!(o.status.code(), Some(0), "{err}");
        assert!(
            err.contains("\"Wahl\"") && err.contains("no PDF was written"),
            "{err}"
        );
        assert_eq!(files_in(&ws.event().join("out")), Vec::<String>::new());
    }
}

// ---- courses in init, numbers accept ----

const LUEBECK_PPEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../freemapper-print/tests/fixtures/courses/luebeck-sheets.ppen"
);
const LUEBECK_NUMBERS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../freemapper-print/tests/fixtures/courses/luebeck-numbers.xml"
);

fn init_args(ws: &Workspace, courses: &str) -> Vec<String> {
    [
        "init",
        ws.event().to_str().unwrap(),
        "--map",
        ws.map.to_str().unwrap(),
        "--kind",
        "sprint",
        "--courses",
        courses,
    ]
    .map(String::from)
    .to_vec()
}

#[test]
fn init_with_ppen_proposes_layouts_and_assignments() {
    use freemapper_print::project::ElementKind;
    for (lang, scale_line, layout_word) in [
        ("en", "Print scales come from the course file", "Layout"),
        (
            "de",
            "Die Druckmaßstäbe stammen aus der Bahndatei",
            "Layout",
        ),
    ] {
        let ws = Workspace::new(LUEBECK);
        let ppen = ws.dir.path().join("luebeck.ppen");
        std::fs::copy(LUEBECK_PPEN, &ppen).unwrap();
        let o = run_owned(lang, &init_args(&ws, ppen.to_str().unwrap()));
        let err = stderr(&o);
        assert!(o.status.success(), "{err}");
        let loaded = project::load(&ws.project_file()).expect("the new file loads");
        let p = &loaded.project;
        assert_eq!(
            p.sources.courses.as_deref(),
            Some(Path::new("../luebeck.ppen"))
        );
        // Print scales 4000 (Kurz, and the course without an option) and 5000 (Lang).
        assert_eq!(p.layouts.len(), 2, "{err}");
        let at = |s: u32| p.layouts.iter().find(|l| l.print_scale == Some(s)).unwrap();
        let assigned = |c: &str| {
            p.assignments
                .iter()
                .find(|a| a.course == c)
                .map(|a| a.layout.as_str())
        };
        assert_eq!(assigned("Kurz"), Some(at(4000).id.as_str()));
        assert_eq!(assigned("Ohne Maßstab"), Some(at(4000).id.as_str()));
        assert_eq!(assigned("Lang"), Some(at(5000).id.as_str()));
        assert_ne!(at(4000).id, at(5000).id);
        for l in &p.layouts {
            assert!(l.map_frame.crop, "{}", l.id);
            let boxes: Vec<_> = l
                .elements
                .iter()
                .filter(|e| e.kind == ElementKind::ControlDescriptions)
                .collect();
            assert_eq!(boxes.len(), 1, "{}", l.id);
            assert_eq!(boxes[0].cell_mm, None);
        }
        assert!(err.contains(scale_line), "{lang}: {err}");
        assert!(
            err.contains(layout_word) && err.contains(&at(5000).id),
            "{err}"
        );
        assert!(err.contains("\"Lang\"") || err.contains("„Lang“"), "{err}");
    }
}

#[test]
fn init_with_iof_xml_makes_one_layout_without_description_box() {
    let ws = Workspace::new(LUEBECK);
    let xml = ws.event().join("c.xml");
    std::fs::copy(LUEBECK_COURSES, &xml).unwrap();
    let o = run_owned("en", &init_args(&ws, xml.to_str().unwrap()));
    let err = stderr(&o);
    assert!(o.status.success(), "{err}");
    let p = project::load(&ws.project_file()).unwrap().project;
    assert_eq!(p.sources.courses.as_deref(), Some(Path::new("c.xml")));
    assert_eq!(p.layouts.len(), 1);
    assert_eq!(p.layouts[0].print_scale, Some(4000));
    assert!(p.layouts[0].elements.is_empty());
    assert!(err.contains("IOF XML"), "{err}");
}

#[test]
fn init_with_a_course_file_that_does_not_fit_writes_nothing() {
    let ws = Workspace::new(LUEBECK);
    let xml = ws.event().join("c.xml");
    let text = std::fs::read_to_string(LUEBECK_COURSES)
        .unwrap()
        .replace("<Scale>4000</Scale>", "<Scale>5000</Scale>");
    std::fs::write(&xml, text).unwrap();
    let folder = ws.dir.path().join("new-event");
    let o = run(
        "en",
        &[
            "init",
            folder.to_str().unwrap(),
            "--map",
            ws.map.to_str().unwrap(),
            "--kind",
            "sprint",
            "--courses",
            xml.to_str().unwrap(),
        ],
    );
    let err = stderr(&o);
    assert_eq!(o.status.code(), Some(1), "{err}");
    assert!(
        err.contains("1:5000") && err.contains("course file"),
        "{err}"
    );
    assert!(!folder.exists(), "{err}");
}

/// A project with the conflicting course of `luebeck-numbers.xml`, comments around the tables, and a
/// layout of the same name nowhere else.
fn numbers_workspace() -> (Workspace, PathBuf, String) {
    let ws = Workspace::new(LUEBECK);
    std::fs::copy(LUEBECK_NUMBERS, ws.event().join("c.xml")).unwrap();
    let text = "# My event, kept by hand\nformat = 1\n[event]\ntitle = \"Eng\" # the title\n\
                [sources]\nmap = \"../maps/luebeck.omap\"\ncourses = \"c.xml\"\ncourse_y_axis = \"up\"\n\
                [map]\nkind = \"sprint\"\ncontour_interval_m = 2.0\n\n# The only layout\n\
                [[layouts]]\nid = \"a3\"\npaper = \"A3\"\n# end of file comment\n"
        .to_string();
    std::fs::write(ws.project_file(), &text).unwrap();
    let path = ws.project_file();
    (ws, path, text)
}

#[test]
fn check_lists_number_conflicts_with_proposals() {
    let (_ws, path, _) = numbers_workspace();
    let p = path.to_str().unwrap();
    let o = run("en", &["check", p]);
    let err = stderr(&o);
    assert_eq!(o.status.code(), Some(1), "{err}");
    assert!(
        err.contains("the number of control 31 covers the symbol of control 32"),
        "{err}"
    );
    assert!(
        err.contains("A free position is direction 307.9° with a gap of 0.5 mm"),
        "{err}"
    );
    assert!(err.contains("numbers accept"), "{err}");
    let o = run("de", &["check", p]);
    let err = stderr(&o);
    assert!(
        err.contains("Die Nummer von Posten 31 überdeckt das Symbol von Posten 32")
            || err.contains("überdeckt die Nummer von Posten 31 das Symbol von Posten 32"),
        "{err}"
    );
    assert!(
        err.contains("Richtung 307,9° mit 0,5 mm Abstand") && err.contains("numbers accept"),
        "{err}"
    );
}

#[test]
fn numbers_accept_appends_and_keeps_comments() {
    let (_ws, path, old) = numbers_workspace();
    let p = path.to_str().unwrap();
    // The proposal of `check`, for the sheet.
    let o = run(
        "en",
        &[
            "numbers",
            "accept",
            p,
            "--course",
            "Eng",
            "--control",
            "31",
            "--sheet",
        ],
    );
    let err = stderr(&o);
    assert_eq!(o.status.code(), Some(0), "{err}");
    let new = std::fs::read_to_string(&path).unwrap();
    assert!(
        new.starts_with(&old),
        "everything before is unchanged:\n{new}"
    );
    let loaded = project::load(&path).unwrap();
    let n = &loaded.project.numbers;
    assert_eq!(n.len(), 1, "{new}");
    assert_eq!(n[0].control, "31");
    assert_eq!(n[0].course.as_deref(), Some("Eng"));
    assert_eq!(n[0].layout, None);
    assert_eq!(n[0].direction, 307.9);
    assert_eq!(n[0].gap_mm, Some(0.5));
    // Control 31 has a position in the course file, so the correction remembers it.
    assert!(n[0].base_seen.is_some(), "{new}");
    assert!(err.contains("\"Eng\"") && err.contains("31"), "{err}");
    // The conflict is gone.
    let o = run("en", &["check", p]);
    assert!(!stderr(&o).contains("covers the symbol"), "{}", stderr(&o));
    // A layout-wide correction with values of its own is rounded: 0.1° and 0.01 mm. Control 32 has no
    // position in the file, so there is no base to remember.
    let before = std::fs::read_to_string(&path).unwrap();
    let o = run(
        "de",
        &[
            "numbers",
            "accept",
            p,
            "--course",
            "Eng",
            "--control",
            "32",
            "--layout-wide",
            "--direction",
            "90.04",
            "--gap",
            "1.236",
        ],
    );
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let new = std::fs::read_to_string(&path).unwrap();
    assert!(new.starts_with(&before));
    let loaded = project::load(&path).unwrap();
    let n = &loaded.project.numbers[1];
    assert_eq!(n.control, "32");
    assert_eq!(n.layout.as_deref(), Some("a3"));
    assert_eq!(n.course, None);
    assert_eq!(n.direction, 90.0);
    assert_eq!(n.gap_mm, Some(1.24));
    assert_eq!(n.base_seen, None);
    assert!(stderr(&o).contains("Layout a3"), "{}", stderr(&o));
}

#[test]
fn numbers_accept_refuses_duplicate() {
    let (_ws, path, _) = numbers_workspace();
    let p = path.to_str().unwrap();
    let args = [
        "numbers",
        "accept",
        p,
        "--course",
        "Eng",
        "--control",
        "31",
        "--sheet",
    ];
    assert_eq!(run("en", &args).status.code(), Some(0));
    let done = std::fs::read_to_string(&path).unwrap();
    for (lang, hint) in [
        ("en", "Edit the file"),
        ("de", "Ändern Sie sie in der Datei"),
    ] {
        let o = run(lang, &args);
        let err = stderr(&o);
        assert_eq!(o.status.code(), Some(2), "{err}");
        assert!(err.contains(hint) && err.contains("31"), "{lang}: {err}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), done);
    }
    // No conflict at control 32, so no proposal to take.
    let o = run(
        "en",
        &[
            "numbers",
            "accept",
            p,
            "--course",
            "Eng",
            "--control",
            "32",
            "--sheet",
        ],
    );
    let err = stderr(&o);
    assert_eq!(o.status.code(), Some(2), "{err}");
    assert!(err.contains("no proposal"), "{err}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), done);
    // An unknown course and a gap outside the limit are usage errors too.
    let o = run(
        "en",
        &[
            "numbers",
            "accept",
            p,
            "--course",
            "Nope",
            "--control",
            "31",
            "--sheet",
        ],
    );
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
    let o = run(
        "en",
        &[
            "numbers",
            "accept",
            p,
            "--course",
            "Eng",
            "--control",
            "32",
            "--sheet",
            "--direction",
            "10",
            "--gap",
            "11",
        ],
    );
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), done);
}

#[test]
fn numbers_accept_restores_the_file_when_it_no_longer_loads() {
    let (_ws, path, old) = numbers_workspace();
    // An inline `numbers` array makes the appended table a duplicate key.
    let broken = format!("numbers = []\n{old}");
    // `numbers = []` must sit in the top-level table, which the head already is.
    std::fs::write(&path, &broken).unwrap();
    let o = run(
        "en",
        &[
            "numbers",
            "accept",
            path.to_str().unwrap(),
            "--course",
            "Eng",
            "--control",
            "31",
            "--sheet",
        ],
    );
    let err = stderr(&o);
    assert_eq!(o.status.code(), Some(1), "{err}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), broken);
}

#[test]
fn check_reports_y_up_and_stale() {
    let ws = Workspace::new(LUEBECK);
    std::fs::copy(LUEBECK_COURSES, ws.event().join("c.xml")).unwrap();
    let text = format!(
        "format = 1\n[event]\ntitle = \"T\"\n[sources]\nmap = \"../maps/luebeck.omap\"\ncourses = \"c.xml\"\n\
         [map]\nkind = \"sprint\"\ncontour_interval_m = 2.0\n{BAR}{}\
         [[assignments]]\ncourse = \"Weg\"\nlayout = \"a4\"\n\
         [[numbers]]\ncontrol = \"99\"\ncourse = \"Kurz\"\ndirection = 10.0\n",
        layouts(&[("a4", "A4", "portrait")])
    );
    std::fs::write(ws.project_file(), text).unwrap();
    let p = ws.project_file();
    let p = p.to_str().unwrap();
    let o = run("en", &["check", p]);
    let err = stderr(&o);
    assert_eq!(o.status.code(), Some(1), "{err}");
    assert!(err.contains("y axis points up"), "{err}");
    assert!(err.contains("assigns course \"Weg\""), "{err}");
    assert!(
        err.contains("number of control 99 for course \"Kurz\""),
        "{err}"
    );
    let o = run("de", &["check", p]);
    let err = stderr(&o);
    assert!(
        err.contains("y-Achse der Bahndatei zeigt nach oben"),
        "{err}"
    );
    assert!(err.contains("Bahn „Weg“"), "{err}");
    assert!(err.contains("Posten 99 für Bahn „Kurz“"), "{err}");
}
