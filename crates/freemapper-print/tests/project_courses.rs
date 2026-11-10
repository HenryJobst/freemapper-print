//! The project file's course source, assignments and number corrections (M3 Task 5, ADR 0025).

use std::path::{Path, PathBuf};

use freemapper_print::course::NumberPlacement;
use freemapper_print::course::fit::{FitError, SkipReason, YAxis};
use freemapper_print::courses::{
    CorrectionScope, CourseLoadError, LayoutNote, LoadedCourses, MaskNote, MaskRef, Stale,
    base_changed, layout_for, load_courses, mask_for,
};
use freemapper_print::omap::read::read_map_file;
use freemapper_print::project::{self, FILE_NAME};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn luebeck_xml() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/courses/luebeck-georef.xml")
}

/// A project directory with the Lübeck map as `m.omap` and the course file as `course_name`.
fn project_dir(course_name: &str, body: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(
        fixtures().join("maps/luebeck.omap"),
        dir.path().join("m.omap"),
    )
    .unwrap();
    std::fs::copy(luebeck_xml(), dir.path().join(course_name)).unwrap();
    let path = dir.path().join(FILE_NAME);
    let head = format!(
        "format = 1\n[event]\ntitle = \"T\"\n[sources]\nmap = \"m.omap\"\ncourses = \"{course_name}\"\n[map]\nkind = \"sprint\"\n"
    );
    std::fs::write(&path, format!("{head}{body}")).unwrap();
    (dir, path)
}

const LAYOUTS: &str = "[[layouts]]\nid = \"a4\"\n[[layouts]]\nid = \"a3\"\n";

fn load_all(path: &Path) -> Option<freemapper_print::courses::LoadedCourses> {
    let lp = project::load(path).unwrap();
    let map = read_map_file(&lp.map_path()).unwrap();
    load_courses(&lp, &map).unwrap()
}

#[test]
fn courses_source_relative_to_project() {
    let (_d, path) = project_dir("c.xml", LAYOUTS);
    let loaded = load_all(&path).unwrap();
    assert_eq!(loaded.event.courses[0].name, "A");
    assert!(loaded.stale.is_empty());
    // Any case of the extension works; the project file's directory is the base.
    let (_d, path) = project_dir("C.XML", LAYOUTS);
    assert!(load_all(&path).is_some());
}

#[test]
fn no_course_source_loads_nothing() {
    let lp = project::load(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/projects/minimal")
            .join(FILE_NAME),
    )
    .unwrap();
    let map = read_map_file(&lp.map_path()).unwrap();
    assert!(load_courses(&lp, &map).unwrap().is_none());
}

#[test]
fn unknown_extension_names_the_file() {
    let (_d, path) = project_dir("c.txt", LAYOUTS);
    let lp = project::load(&path).unwrap();
    let map = read_map_file(&lp.map_path()).unwrap();
    let msg = load_courses(&lp, &map).unwrap_err().to_string();
    assert!(msg.contains("c.txt") && msg.contains(".ppen"), "{msg}");
}

#[test]
fn oversized_course_file_is_refused_before_reading() {
    let (_d, path) = project_dir("c.xml", LAYOUTS);
    let big = std::fs::File::create(path.parent().unwrap().join("c.xml")).unwrap();
    big.set_len(60 * 1024 * 1024).unwrap();
    let lp = project::load(&path).unwrap();
    let map = read_map_file(&lp.map_path()).unwrap();
    let err = load_courses(&lp, &map).unwrap_err();
    assert!(matches!(err, CourseLoadError::TooLarge { .. }), "{err}");
}

#[test]
fn fit_options_come_from_the_sources() {
    let (_d, path) = project_dir("c.xml", LAYOUTS);
    let text = std::fs::read_to_string(&path).unwrap().replace(
        "courses = \"c.xml\"\n",
        "courses = \"c.xml\"\ncourse_y_axis = \"down\"\ncourse_georef_check = false\n",
    );
    std::fs::write(&path, text).unwrap();
    let lp = project::load(&path).unwrap();
    assert_eq!(lp.project.sources.course_y_axis, Some(YAxis::Down));
    assert!(!lp.project.sources.course_georef_check);
    let map = read_map_file(&lp.map_path()).unwrap();
    // The Lübeck file has a control with only lat/lng; with the check off it cannot be placed.
    let err = load_courses(&lp, &map).unwrap_err();
    assert!(
        matches!(
            err,
            CourseLoadError::Fit {
                source: FitError::NoPositionNoGeoref {
                    reason: SkipReason::CheckOff,
                    ..
                },
                ..
            }
        ),
        "{err}"
    );
    // The fit's message comes once, as the source, not also in the message: a chain prints both.
    let mut chain = err.to_string();
    let mut source = std::error::Error::source(&err);
    while let Some(s) = source {
        chain = format!("{chain}: {s}");
        source = s.source();
    }
    assert_eq!(chain.matches("only lat/lng").count(), 1, "{chain}");
}

#[test]
fn defaults_are_not_written_and_files_round_trip() {
    let (_d, path) = project_dir(
        "c.xml",
        &format!("{LAYOUTS}[[assignments]]\ncourse = \"A\"\nlayout = \"a3\"\n"),
    );
    let p = project::load(&path).unwrap().project;
    let written = toml::to_string(&p).unwrap();
    assert!(
        !written.contains("course_y_axis") && !written.contains("course_georef_check"),
        "{written}"
    );
    assert!(written.contains("[[assignments]]"), "{written}");
    let again: project::Project = toml::from_str(&written).unwrap();
    assert_eq!(again, p);
    // The committed examples have none of the new keys.
    let example = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/luebeck-competition-layouts")
        .join(FILE_NAME);
    let written = toml::to_string(&project::load(&example).unwrap().project).unwrap();
    for key in ["courses", "assignments", "numbers", "course_"] {
        assert!(!written.contains(key), "{key}: {written}");
    }
}

#[test]
fn renamed_course_reported_and_kept() {
    let (_d, path) = project_dir(
        "c.xml",
        &format!(
            "{LAYOUTS}[[assignments]]\ncourse = \"A\"\nlayout = \"a3\"\n[[assignments]]\ncourse = \"Old\"\nlayout = \"a4\"\n"
        ),
    );
    let lp = project::load(&path).unwrap();
    assert_eq!(lp.project.assignments.len(), 2);
    let loaded = load_all(&path).unwrap();
    assert_eq!(
        loaded.stale,
        [Stale::Assignment {
            course: "Old".into()
        }]
    );
    // The other courses still load, and the assignment is still in the project.
    assert_eq!(loaded.event.courses.len(), 1);
    assert_eq!(lp.project.assignments[1].course, "Old");
    assert_eq!(layout_for(&lp.project, "A").0.id, "a3");
}

#[test]
fn missing_control_correction_reported() {
    let (_d, path) = project_dir(
        "c.xml",
        &format!(
            "{LAYOUTS}[[numbers]]\ncontrol = \"31\"\nlayout = \"a4\"\ndirection = 90\n\
             [[numbers]]\ncontrol = \"99\"\nlayout = \"a4\"\ndirection = 90\n\
             [[numbers]]\ncontrol = \"32\"\ncourse = \"B\"\ndirection = 10\n"
        ),
    );
    let loaded = load_all(&path).unwrap();
    assert_eq!(
        loaded.stale,
        [
            Stale::Correction {
                control: "99".into(),
                scope: CorrectionScope::Layout("a4".into())
            },
            Stale::Correction {
                control: "32".into(),
                scope: CorrectionScope::Course("B".into())
            },
        ]
    );
}

fn error_of(body: &str) -> String {
    let (_d, path) = project_dir("c.xml", &format!("{LAYOUTS}{body}"));
    project::load(&path).unwrap_err().to_string()
}

#[test]
fn correction_needs_layout_or_course() {
    let msg = error_of("[[numbers]]\ncontrol = \"31\"\ndirection = 90\n");
    assert!(
        msg.contains("numbers[0]") && msg.contains("`layout`"),
        "{msg}"
    );
    let msg = error_of(
        "[[numbers]]\ncontrol = \"31\"\nlayout = \"a4\"\ncourse = \"A\"\ndirection = 90\n",
    );
    assert!(msg.contains("not both"), "{msg}");
}

#[test]
fn duplicate_assignment_is_error_with_line() {
    let msg = error_of(
        "[[assignments]]\ncourse = \"A\"\nlayout = \"a4\"\n[[assignments]]\ncourse = \"A\"\nlayout = \"a3\"\n",
    );
    // Head is 8 lines, layouts 4, the first assignment 3: the second entry's course is line 17.
    assert!(
        msg.contains(&format!("{FILE_NAME}:17:")) && msg.contains("assignments[1].course"),
        "{msg}"
    );
}

#[test]
fn unknown_layout_and_bad_numbers_are_errors() {
    let msg = error_of("[[assignments]]\ncourse = \"A\"\nlayout = \"zz\"\n");
    assert!(
        msg.contains("assignments[0].layout") && msg.contains("a3, a4"),
        "{msg}"
    );
    let msg = error_of("[[numbers]]\ncontrol = \"31\"\nlayout = \"zz\"\ndirection = 1\n");
    assert!(msg.contains("numbers[0].layout"), "{msg}");
    // 0 < gap <= 10 mm, as freemapper-courseplan accepts (ruling R29).
    for gap in ["-1", "0", "10.5", "nan"] {
        let msg = error_of(&format!(
            "[[numbers]]\ncontrol = \"31\"\nlayout = \"a4\"\ndirection = 1\ngap_mm = {gap}\n"
        ));
        assert!(msg.contains("numbers[0].gap_mm"), "{gap}: {msg}");
    }
    for gap in ["0.1", "10"] {
        let (_d, path) = project_dir(
            "c.xml",
            &format!(
                "{LAYOUTS}[[numbers]]\ncontrol = \"31\"\nlayout = \"a4\"\ndirection = 1\ngap_mm = {gap}\n"
            ),
        );
        assert!(project::load(&path).is_ok(), "{gap}");
    }
    let msg = error_of("[[numbers]]\ncontrol = \"31\"\nlayout = \"a4\"\ndirection = inf\n");
    assert!(
        msg.contains("numbers[0].direction") && msg.contains("finite"),
        "{msg}"
    );
}

#[test]
fn direction_is_normalised() {
    let (_d, path) = project_dir(
        "c.xml",
        &format!(
            "{LAYOUTS}[[numbers]]\ncontrol = \"31\"\nlayout = \"a4\"\ndirection = -90\n[[numbers]]\ncontrol = \"32\"\nlayout = \"a4\"\ndirection = 725\n"
        ),
    );
    let n = project::load(&path).unwrap().project.numbers;
    assert_eq!((n[0].direction, n[1].direction), (270.0, 5.0));
}

#[test]
fn course_without_assignment_takes_first_layout() {
    let (_d, path) = project_dir("c.xml", LAYOUTS);
    let lp = project::load(&path).unwrap();
    let (layout, note) = layout_for(&lp.project, "A");
    assert_eq!(layout.id, "a4");
    let note = note.unwrap();
    assert_eq!(
        note,
        LayoutNote::DefaultLayout {
            course: "A".into(),
            layout: "a4".into()
        }
    );
    assert!(!note.is_warning());
}

#[test]
fn base_change_under_correction_reported() {
    let seen = NumberPlacement {
        direction: 90.0,
        gap_mm: 1.0,
    };
    let at = |direction, gap_mm| NumberPlacement { direction, gap_mm };
    assert!(!base_changed(&seen, &at(90.4, 1.04)));
    assert!(base_changed(&seen, &at(90.6, 1.0)));
    assert!(base_changed(&seen, &at(90.0, 1.06)));
    // Across the 0/360 seam.
    assert!(!base_changed(&at(359.8, 1.0), &at(0.2, 1.0)));
    assert!(base_changed(&at(359.0, 1.0), &at(1.0, 1.0)));
    // The stored base survives a save and load.
    let (_d, path) = project_dir(
        "c.xml",
        &format!(
            "{LAYOUTS}[[numbers]]\ncontrol = \"31\"\nlayout = \"a4\"\ndirection = 10\nbase_seen = {{ direction = 90, gap_mm = 1.0 }}\n"
        ),
    );
    let n = project::load(&path).unwrap().project.numbers;
    assert_eq!(n[0].base_seen, Some(seen));
    assert!(
        Stale::CorrectionOverridesChangedBase {
            control: "31".into()
        }
        .is_warning()
    );
}

#[test]
fn base_seen_takes_any_finite_gap() {
    // A course file number that touches its circle has a negative gap; the record of it must load.
    let (_d, path) = project_dir(
        "c.xml",
        &format!(
            "{LAYOUTS}[[numbers]]\ncontrol = \"31\"\nlayout = \"a4\"\ndirection = 10\nbase_seen = {{ direction = 36.9, gap_mm = -0.4 }}\n"
        ),
    );
    let n = project::load(&path).unwrap().project.numbers;
    assert_eq!(n[0].base_seen.unwrap().gap_mm, -0.4);
    let msg = error_of(
        "[[numbers]]\ncontrol = \"31\"\nlayout = \"a4\"\ndirection = 10\nbase_seen = { direction = 36.9, gap_mm = nan }\n",
    );
    assert!(msg.contains("base_seen.gap_mm"), "{msg}");
}

#[test]
fn course_file_must_be_a_regular_file() {
    let (_d, path) = project_dir("c.xml", LAYOUTS);
    let dir = path.parent().unwrap();
    std::fs::remove_file(dir.join("c.xml")).unwrap();
    std::fs::create_dir(dir.join("c.xml")).unwrap();
    let lp = project::load(&path).unwrap();
    let map = read_map_file(&lp.map_path()).unwrap();
    let err = load_courses(&lp, &map).unwrap_err();
    assert!(matches!(err, CourseLoadError::NotAFile { .. }), "{err}");
    assert!(err.to_string().contains("c.xml"));
}

#[cfg(unix)]
#[test]
fn course_fifo_is_refused() {
    let (_d, path) = project_dir("c.xml", LAYOUTS);
    let fifo = path.parent().unwrap().join("c.xml");
    std::fs::remove_file(&fifo).unwrap();
    let status = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap();
    assert!(status.success());
    let lp = project::load(&path).unwrap();
    let map = read_map_file(&lp.map_path()).unwrap();
    let err = load_courses(&lp, &map).unwrap_err();
    assert!(matches!(err, CourseLoadError::NotAFile { .. }), "{err}");
}

#[test]
fn duplicate_correction_is_error_with_line() {
    let one = "[[numbers]]\ncontrol = \"31\"\nlayout = \"a4\"\ndirection = 1\n";
    let msg = error_of(&format!("{one}{one}"));
    // Head 8 lines, layouts 4, the first correction 4: the second control key is on line 18.
    assert!(
        msg.contains(&format!("{FILE_NAME}:18:")) && msg.contains("numbers[1].control"),
        "{msg}"
    );
    // The same control in another scope is fine.
    let (_d, path) = project_dir(
        "c.xml",
        &format!(
            "{LAYOUTS}{one}[[numbers]]\ncontrol = \"31\"\nlayout = \"a3\"\ndirection = 1\n[[numbers]]\ncontrol = \"31\"\ncourse = \"A\"\ndirection = 1\n"
        ),
    );
    assert_eq!(project::load(&path).unwrap().project.numbers.len(), 3);
}

#[test]
fn course_names_match_exactly() {
    let (_d, path) = project_dir(
        "c.xml",
        &format!("{LAYOUTS}[[assignments]]\ncourse = \"a\"\nlayout = \"a3\"\n"),
    );
    let loaded = load_all(&path).unwrap();
    assert_eq!(loaded.stale, [Stale::Assignment { course: "a".into() }]);
}

/// The Lübeck example's `.ppen` through `load_courses`: the reader, the fit and the stable ids of the course
/// crate meet the project here, now that `tests/fit_courses.rs` lives in freemapper-core (ADR 0043).
#[test]
fn the_luebeck_example_courses_load_through_the_course_crate() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/luebeck-competition-courses")
        .join(FILE_NAME);
    let loaded = load_all(&path).unwrap();
    let names: Vec<&str> = loaded
        .event
        .courses
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(names, ["A", "B", "C"]);
    let c51 = loaded
        .event
        .controls
        .iter()
        .find(|c| c.code == "51")
        .unwrap();
    assert_eq!(c51.id, Some(3));
    // `<location x="-91.189" y="36.718">` in map µm, y turned down.
    let at = c51.position.unwrap();
    assert_eq!((at.x, at.y), (-91189, -36718));
}

const SQUARE_MASK: &str = "polygons = [[[[-100, -100], [100, -100], [100, 100], [-100, 100]], [[0, 0], [10, 0], [10, 10]]]]\n";

#[test]
fn unused_mask_is_noted() {
    let (_d, path) = project_dir(
        "c.xml",
        &format!(
            "{LAYOUTS}[[masks]]\nid = \"used\"\n{SQUARE_MASK}[[masks]]\nid = \"spare\"\n{SQUARE_MASK}\
             [[assignments]]\ncourse = \"A\"\nlayout = \"a3\"\nmask = \"used\"\n"
        ),
    );
    let lp = project::load(&path).unwrap();
    let map = read_map_file(&lp.map_path()).unwrap();
    let loaded = load_courses(&lp, &map).unwrap().unwrap();
    assert_eq!(
        loaded.mask_notes,
        [MaskNote::MaskUnused { id: "spare".into() }]
    );
    assert!(!loaded.mask_notes[0].is_warning());
    assert!(loaded.stale.is_empty(), "{:?}", loaded.stale);
    // The assigned course gets its mask, the others none (the XML file has no white-out).
    let a = mask_for(&lp.project, &loaded, "A");
    assert!(
        matches!(a.as_slice(), [MaskRef::Project(m)] if m.id == "used"),
        "{a:?}"
    );
    assert!(mask_for(&lp.project, &loaded, "B").is_empty());
    assert!(loaded.white_outs.is_empty());
}

/// A project folder with the Lübeck example's map and its `.ppen`, `specials` added to the course file.
fn ppen_dir(specials: &str, body: &str) -> (tempfile::TempDir, PathBuf) {
    let example = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/luebeck-competition-courses");
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(example.join("luebeck.omap"), dir.path().join("m.omap")).unwrap();
    let ppen = std::fs::read_to_string(example.join("courses.ppen")).unwrap();
    let ppen = ppen.replace(
        "</course-scribe-event>",
        &format!("{specials}</course-scribe-event>"),
    );
    std::fs::write(dir.path().join("c.ppen"), ppen).unwrap();
    let path = dir.path().join(FILE_NAME);
    let head = "format = 1\n[event]\ntitle = \"T\"\n[sources]\nmap = \"m.omap\"\ncourses = \"c.ppen\"\n\
                [map]\nkind = \"sprint\"\n[[layouts]]\nid = \"a4\"\n";
    std::fs::write(&path, format!("{head}{body}")).unwrap();
    (dir, path)
}

/// A Purple Pen white-out: a square of side `s` at (`x`, `y`) in file mm (y up) for `courses`.
fn white_out(id: u32, x: f64, y: f64, s: f64, courses: &str) -> String {
    let loc = |x: f64, y: f64| format!("<location x=\"{x}\" y=\"{y}\" />");
    format!(
        "<special-object id=\"{}\" kind=\"white-out\">{}{}{}{}{courses}</special-object>\n",
        100 + id,
        loc(x, y),
        loc(x + s, y),
        loc(x + s, y + s),
        loc(x, y + s),
    )
}

/// The ids of the masks `mask_for` gives `course`.
fn mask_ids(project: &project::Project, loaded: &LoadedCourses, course: &str) -> Vec<String> {
    mask_for(project, loaded, course)
        .iter()
        .map(|m| m.mask().id.clone())
        .collect()
}

#[test]
fn white_outs_become_mask_variants() {
    // In file order: 1 for A (course id 1), 2 for all courses, 3 for C (id 3), overlapping 2.
    let specials = format!(
        "{}{}{}",
        white_out(
            1,
            -80.0,
            40.0,
            10.0,
            "<courses><course course=\"1\" /></courses>"
        ),
        white_out(2, 50.0, 40.0, 10.0, "<courses all=\"true\" />"),
        white_out(
            3,
            55.0,
            45.0,
            10.0,
            "<courses><course course=\"3\" /></courses>"
        ),
    );
    let (_d, path) = ppen_dir(
        &specials,
        "[[assignments]]\ncourse = \"B\"\nlayout = \"a4\"\nmask = \"whiteout-103\"\n\
         [[assignments]]\ncourse = \"A\"\nlayout = \"a4\"\n",
    );
    let lp = project::load(&path).unwrap();
    let map = read_map_file(&lp.map_path()).unwrap();
    let loaded = load_courses(&lp, &map).unwrap().unwrap();
    let ids: Vec<&str> = loaded
        .white_outs
        .iter()
        .map(|w| w.mask.id.as_str())
        .collect();
    assert_eq!(ids, ["whiteout-101", "whiteout-102", "whiteout-103"]);
    // The coordinates are the file's, y up, as in a `[[masks]]` table.
    assert_eq!(
        loaded.white_outs[0].mask.polygons[0].outline,
        [[-80.0, 40.0], [-70.0, 40.0], [-70.0, 50.0], [-80.0, 50.0]]
    );
    // A has white-outs 1 and 2, each its own mask (ruling R8), from the course file.
    let a = mask_for(&lp.project, &loaded, "A");
    assert!(a.iter().all(|m| matches!(m, MaskRef::WhiteOut(_))));
    assert_eq!(
        mask_ids(&lp.project, &loaded, "A"),
        ["whiteout-101", "whiteout-102"]
    );
    // B names white-out 3 itself, which wins over white-out 2 of all courses: only that one.
    assert_eq!(mask_ids(&lp.project, &loaded, "B"), ["whiteout-103"]);
    // C has 2 and 3, which overlap: both, unchecked; the clips of Task 2 unite them. No note.
    assert_eq!(
        mask_ids(&lp.project, &loaded, "C"),
        ["whiteout-102", "whiteout-103"]
    );
    // B names a white-out by its number: an Info line says where that area lies (ruling R16).
    assert_eq!(
        loaded.mask_notes,
        [MaskNote::WhiteOutByNumber {
            course: "B".into(),
            id: "whiteout-103".into(),
            centre: [60.0, 50.0],
            stand_in: false,
        }]
    );
    assert!(!loaded.mask_notes[0].is_warning());

    // A `[[masks]]` entry named like a white-out stands in for it, for the white-out's courses, and counts as
    // used. A white-out the course file lacks is stale data, and the course gets no mask.
    let (_d, path) = ppen_dir(
        &specials,
        "[[masks]]\nid = \"whiteout-103\"\n\
         polygons = [[[[150, 40], [160, 40], [160, 50], [150, 50]]]]\n\
         [[assignments]]\ncourse = \"B\"\nlayout = \"a4\"\nmask = \"whiteout-9\"\n",
    );
    let lp = project::load(&path).unwrap();
    let loaded = load_courses(&lp, &map).unwrap().unwrap();
    // The stand-in replaces white-out 3 on C, its only course: the Info line names where the replaced area lies.
    assert_eq!(
        loaded.mask_notes,
        [MaskNote::WhiteOutByNumber {
            course: "C".into(),
            id: "whiteout-103".into(),
            centre: [60.0, 50.0],
            stand_in: true,
        }]
    );
    assert_eq!(
        loaded.stale,
        [Stale::Mask {
            course: "B".into(),
            mask: "whiteout-9".into()
        }]
    );
    assert!(mask_for(&lp.project, &loaded, "B").is_empty());
    let c = mask_for(&lp.project, &loaded, "C");
    assert!(
        matches!(c.as_slice(), [MaskRef::WhiteOut(w), MaskRef::Project(p)]
        if w.id == "whiteout-102" && p.polygons[0].outline[0] == [150.0, 40.0]),
        "{c:?}"
    );
}

/// The mask ids of courses A, B and C for a project with `specials` in the course file and no assignments.
fn white_out_masks(specials: &str) -> Vec<Vec<String>> {
    let (_d, path) = ppen_dir(specials, "");
    let lp = project::load(&path).unwrap();
    let map = read_map_file(&lp.map_path()).unwrap();
    let loaded = load_courses(&lp, &map).unwrap().unwrap();
    assert!(loaded.mask_notes.is_empty(), "{:?}", loaded.mask_notes);
    ["A", "B", "C"]
        .iter()
        .map(|c| mask_ids(&lp.project, &loaded, c))
        .collect()
}

#[test]
fn whiteout_only_applies_to_its_courses() {
    // White-out 1 lists course B (id 2); white-out 2 lists no course, so it applies to none (ruling R6), as other
    // specials and Purple Pen do.
    let specials = format!(
        "{}{}",
        white_out(
            1,
            -80.0,
            40.0,
            10.0,
            "<courses><course course=\"2\" /></courses>"
        ),
        white_out(2, 50.0, 40.0, 10.0, "<courses />"),
    );
    let none: Vec<String> = Vec::new();
    assert_eq!(
        white_out_masks(&specials),
        [none.clone(), vec!["whiteout-101".to_string()], none]
    );
}

#[test]
fn whiteout_with_all_applies_everywhere() {
    let specials = white_out(1, -80.0, 40.0, 10.0, "<courses all=\"true\" />");
    let all = vec!["whiteout-101".to_string()];
    assert_eq!(white_out_masks(&specials), [all.clone(), all.clone(), all]);
}

#[test]
fn overlapping_white_outs_give_both_masks() {
    // Ruling R8: overlapping white-outs of one course are two masks, not none; nothing is left without its
    // white-out.
    let specials = format!(
        "{}{}",
        white_out(1, 50.0, 40.0, 10.0, "<courses all=\"true\" />"),
        white_out(2, 55.0, 45.0, 10.0, "<courses all=\"true\" />"),
    );
    let both = vec!["whiteout-101".to_string(), "whiteout-102".to_string()];
    assert_eq!(
        white_out_masks(&specials),
        [both.clone(), both.clone(), both]
    );
}
