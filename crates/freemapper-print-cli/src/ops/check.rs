//! `check`: every sheet the project prints, drawn in memory, with its notes.

use std::path::PathBuf;

use freemapper_print::courses::{LoadedCourses, layout_for};
use freemapper_print::export::{course_sheet_report, sheet_report};
use freemapper_print::omap::MapDocument;
use freemapper_print::project::{LayoutSpec, LoadedProject};
use freemapper_print::sheet::{LogoCache, course_event_notes};
use serde::Serialize;

use super::{Notice, OpError, Sheet, load_project, read_courses, read_map};

/// What to check.
#[derive(Debug, Clone)]
pub struct CheckArgs {
    /// The project file or its folder.
    pub project: PathBuf,
}

/// The sheets of a project and their notes.
#[derive(Debug, Serialize)]
pub struct CheckResult {
    /// The scale of the map file (the `1:n` the sheets are measured against).
    pub map_scale: u32,
    /// The notes of the course file as a whole; `None` when the project names no course file.
    pub course_file: Option<Vec<Notice>>,
    /// One sheet per course of the course file, in file order; empty without a course file.
    pub courses: Vec<SheetCheck>,
    /// One sheet per layout without a course, as `export --layout` writes it, in file order.
    pub layouts: Vec<SheetCheck>,
}

impl CheckResult {
    /// A note of the course file asks the user to look.
    pub fn course_file_needs_attention(&self) -> bool {
        self.course_file
            .iter()
            .flatten()
            .any(|n| n.severity() != super::note::Severity::Info)
    }
}

/// One sheet of `check`: drawn with its notes, or the error that stopped it.
#[derive(Debug, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum SheetCheck {
    Drawn(Sheet),
    Failed {
        course: Option<String>,
        layout: String,
        /// A note of kind `sheet_failed` that says why.
        error: Notice,
    },
}

impl SheetCheck {
    /// A sheet that cannot be drawn needs a look too.
    pub fn needs_attention(&self) -> bool {
        match self {
            SheetCheck::Drawn(sheet) => sheet.needs_attention,
            SheetCheck::Failed { .. } => true,
        }
    }
}

/// Checks the project: with a course file every course on its layout, then every layout sheet without a
/// course; without one, every layout. A sheet that cannot be drawn does not stop the others.
pub fn check(args: &CheckArgs) -> Result<CheckResult, OpError> {
    let loaded = load_project(&args.project)?;
    let map = read_map(&loaded.map_path())?;
    let courses = read_courses(&loaded, &map)?;
    // One logo cache for all sheets: each logo file is decoded once.
    let logos = LogoCache::new();
    let mut result = CheckResult {
        map_scale: map.georef.scale_denominator,
        course_file: None,
        courses: Vec::new(),
        layouts: Vec::new(),
    };
    if let Some(courses) = &courses {
        result.course_file = Some(course_file_notes(courses));
        result.courses = courses
            .event
            .courses
            .iter()
            .map(|course| check_course(&loaded, &map, courses, course, &logos))
            .collect();
    }
    result.layouts = loaded
        .project
        .layouts
        .iter()
        .map(|layout| check_layout(&loaded, &map, layout, &logos))
        .collect();
    Ok(result)
}

/// The notes of the course file as a whole.
pub fn course_file_notes(courses: &LoadedCourses) -> Vec<Notice> {
    course_event_notes(courses)
        .into_iter()
        .map(|n| Notice::sheet(None, n))
        .collect()
}

/// The sheet of `layout` without a course. The PDF is made in memory only: the font check needs it.
pub fn check_layout(
    loaded: &LoadedProject,
    map: &MapDocument,
    layout: &LayoutSpec,
    logos: &LogoCache,
) -> SheetCheck {
    match sheet_report(loaded, map, layout, logos) {
        Ok(report) => SheetCheck::Drawn(Sheet::new(None, layout, report, loaded.project.map.kind)),
        Err(error) => SheetCheck::Failed {
            course: None,
            layout: layout.id.clone(),
            error: Notice::failed(None, error),
        },
    }
}

/// The sheet of `course` on its layout.
fn check_course(
    loaded: &LoadedProject,
    map: &MapDocument,
    courses: &LoadedCourses,
    course: &freemapper_print::course::model::Course<freemapper_print::omap::MapCoord>,
    logos: &LogoCache,
) -> SheetCheck {
    match course_sheet_report(loaded, map, courses, course, logos) {
        Ok((layout, report)) => SheetCheck::Drawn(Sheet::new(
            Some(&course.name),
            layout,
            report,
            loaded.project.map.kind,
        )),
        Err(error) => SheetCheck::Failed {
            course: Some(course.name.clone()),
            layout: layout_for(&loaded.project, &course.name).0.id.clone(),
            error: Notice::failed(Some(&course.name), error),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::testing::viktoriapark;

    fn kinds(sheet: &SheetCheck) -> Vec<&'static str> {
        match sheet {
            SheetCheck::Drawn(s) => s.notes.iter().map(Notice::kind).collect(),
            SheetCheck::Failed { .. } => vec!["failed"],
        }
    }

    #[test]
    fn viktoriapark_has_one_course_and_one_layout_sheet_without_notes_that_count() {
        let result = check(&CheckArgs {
            project: viktoriapark(),
        })
        .unwrap();
        assert_eq!(result.map_scale, 4000);
        let file: Vec<_> = result
            .course_file
            .as_ref()
            .unwrap()
            .iter()
            .map(Notice::kind)
            .collect();
        assert_eq!(file, ["fit_georef_skipped"]);
        assert!(!result.course_file_needs_attention());
        assert_eq!(result.courses.len(), 1);
        let SheetCheck::Drawn(course) = &result.courses[0] else {
            panic!("the course sheet is drawn");
        };
        assert_eq!(course.course.as_deref(), Some("Training"));
        assert_eq!(course.layout, "a4-4000");
        assert_eq!(kinds(&result.courses[0]), ["map_cropped"]);
        assert_eq!(
            kinds(&result.layouts[0]),
            [
                "map_cropped",
                "course_placeholder_not_filled",
                "course_placeholder_not_filled"
            ]
        );
        assert!(
            result
                .courses
                .iter()
                .chain(&result.layouts)
                .all(|s| !s.needs_attention())
        );
        // Serialised: the outcome as a tag, the notes as kind, severity and course.
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["courses"][0]["outcome"], "drawn");
        assert_eq!(json["courses"][0]["notes"][0]["kind"], "map_cropped");
        assert_eq!(json["courses"][0]["notes"][0]["severity"], "info");
        assert_eq!(json["courses"][0]["notes"][0]["course"], "Training");
        assert_eq!(
            json["layouts"][0]["notes"][0]["course"],
            serde_json::Value::Null
        );
    }
}
