//! The project file's course source, assignments and number corrections (M3 Task 5, ADR 0025).

use std::path::{Path, PathBuf};

use freemapper_print::course::fit::{FitError, SkipReason, YAxis};
use freemapper_print::course::{
    CorrectionScope, CourseLoadError, LayoutNote, NumberPlacement, Stale, base_changed, layout_for,
    load_courses,
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

fn load_all(path: &Path) -> Option<freemapper_print::course::LoadedCourses> {
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
    for gap in ["-1", "20.5", "nan"] {
        let msg = error_of(&format!(
            "[[numbers]]\ncontrol = \"31\"\nlayout = \"a4\"\ndirection = 1\ngap_mm = {gap}\n"
        ));
        assert!(msg.contains("numbers[0].gap_mm"), "{gap}: {msg}");
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
