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
    assert_eq!(p.map.contour_interval_m, Some(2.5));
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
        err.contains("2.5 m") && err.contains("confirm") && err.contains("sprint"),
        "{err}"
    );
    assert!(
        err.contains(&ws.project_file().display().to_string()),
        "{err}"
    );
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
