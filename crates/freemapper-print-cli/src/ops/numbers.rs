//! `numbers accept`: appends a number correction to the project file (ADR 0025, addendum).

use std::path::PathBuf;

use freemapper_print::courses::{LoadedCourses, layout_for};
use freemapper_print::export::course_sheet_report;
use freemapper_print::omap::MapDocument;
use freemapper_print::project::{self, LoadedProject};
use freemapper_print::sheet::{LogoCache, SheetNote};
use serde::Serialize;

use super::{OpError, load_project, read_courses, read_map};

/// Where a correction applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptScope {
    /// The sheet of the course alone (`--sheet`).
    Sheet,
    /// Every course on the course's layout (`--layout-wide`).
    Layout,
}

/// What `numbers accept` takes.
#[derive(Debug, Clone)]
pub struct AcceptArgs {
    /// The project file or its folder.
    pub project: PathBuf,
    pub course: String,
    /// The control code.
    pub control: String,
    pub scope: AcceptScope,
    /// Direction (degrees) and gap (print mm) of the user's own; `None` takes the proposal of `check`.
    pub position: Option<(f64, f64)>,
}

/// The scope a correction was written for, by name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "name", rename_all = "snake_case")]
pub enum Scope {
    /// The layout id.
    Layout(String),
    /// The course name.
    Course(String),
}

/// The correction `numbers accept` wrote.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AcceptResult {
    /// The project file written.
    pub path: PathBuf,
    pub control: String,
    pub scope: Scope,
    /// Degrees, rounded to 0.1.
    pub direction: f64,
    /// Print mm, rounded to 0.01.
    pub gap_mm: f64,
    /// The position is the proposal of `check`, not the user's own.
    pub proposed: bool,
}

/// Appends the correction for the number of `args.control` on `args.course` to the project file. Without a
/// position of the user's own, the proposal of `check` for the sheet is taken; for the whole layout it must suit
/// every course there that prints the number with it (ruling R39).
pub fn accept(args: &AcceptArgs) -> Result<AcceptResult, OpError> {
    let loaded = load_project(&args.project)?;
    let map = read_map(&loaded.map_path())?;
    let Some(courses) = read_courses(&loaded, &map)? else {
        return Err(OpError::NumbersNeedCourseFile);
    };
    let course = courses
        .event
        .courses
        .iter()
        .find(|c| c.name == args.course)
        .ok_or_else(|| OpError::UnknownCourse {
            name: args.course.clone(),
            known: courses
                .event
                .courses
                .iter()
                .map(|c| c.name.clone())
                .collect(),
        })?;
    let mut known: Vec<String> = Vec::new();
    for stop in &course.stops {
        if !known.contains(&stop.control) {
            known.push(stop.control.clone());
        }
    }
    if !known.contains(&args.control) {
        return Err(OpError::UnknownControl {
            course: course.name.clone(),
            control: args.control.clone(),
            known,
        });
    }
    let layout = layout_for(&loaded.project, &course.name).0;
    let scope = match args.scope {
        AcceptScope::Sheet => Scope::Course(course.name.clone()),
        AcceptScope::Layout => Scope::Layout(layout.id.clone()),
    };
    let exists = loaded.project.numbers.iter().any(|n| {
        n.control == args.control
            && match &scope {
                Scope::Course(c) => n.course.as_ref() == Some(c),
                Scope::Layout(l) => n.course.is_none() && n.layout.as_ref() == Some(l),
            }
    });
    if exists {
        return Err(OpError::NumbersExists {
            control: args.control.clone(),
            scope,
        });
    }
    let (direction, gap) = match args.position {
        Some((d, g)) => {
            if !d.is_finite() || !g.is_finite() || g <= 0.0 || g > project::MAX_NUMBER_GAP_MM {
                return Err(OpError::NumbersValue);
            }
            (d, g)
        }
        None => {
            // The proposal of `check`: the same sheet, drawn the same way.
            let (_, report) =
                course_sheet_report(&loaded, &map, &courses, course, &LogoCache::new())
                    .map_err(OpError::Export)?;
            let proposal = report.notes.iter().find_map(|n| match n {
                SheetNote::NumberConflict(c)
                    if c.control == args.control && c.course == course.name && c.searched =>
                {
                    Some(c.proposal)
                }
                _ => None,
            });
            match proposal {
                Some(p) => (p.direction, p.gap_mm),
                None => {
                    return Err(OpError::NumbersNoProposal {
                        control: args.control.clone(),
                        course: course.name.clone(),
                    });
                }
            }
        }
    };
    // 0.1 degrees and 0.01 mm, as the placement rounds.
    let direction = (direction * 10.0).round() / 10.0;
    let gap = (gap * 100.0).round() / 100.0;
    // The course file's own position (level 2), so that `check` notices when the file moves the number.
    let base_seen = freemapper_print::numbers::resolve(course, &courses.event, &loaded, &layout.id)
        .into_iter()
        .find(|n| n.control == args.control)
        .and_then(|n| n.base)
        .map(freemapper_print::course::NumberPlacement::from);
    let (scope_layout, scope_course) = match &scope {
        Scope::Layout(l) => (Some(l.clone()), None),
        Scope::Course(c) => (None, Some(c.clone())),
    };
    let correction = project::NumberCorrection {
        control: args.control.clone(),
        layout: scope_layout,
        course: scope_course,
        direction,
        gap_mm: Some(gap),
        base_seen,
    };
    // A proposal for the whole layout must suit every course there that prints this number with it (ruling
    // R39): it was searched on one sheet only.
    if args.scope == AcceptScope::Layout && args.position.is_none() {
        let blocked = layout_conflicts(&loaded, &map, &courses, &layout.id, &correction)?;
        if !blocked.is_empty() {
            return Err(OpError::NumbersLayoutConflict {
                control: args.control.clone(),
                layout: layout.id.clone(),
                courses: blocked,
                course: course.name.clone(),
                direction,
                gap_mm: gap,
            });
        }
    }
    project::append_correction(&loaded.path, &correction).map_err(|source| {
        OpError::AppendCorrection {
            path: loaded.path.clone(),
            source,
        }
    })?;
    Ok(AcceptResult {
        path: loaded.path.clone(),
        control: args.control.clone(),
        scope,
        direction,
        gap_mm: gap,
        proposed: args.position.is_none(),
    })
}

/// The courses printed with layout `layout_id` whose sheet, with the layout-wide `correction` added, has a
/// conflict at its control. Courses with a correction of their own for the control are left out: theirs wins.
fn layout_conflicts(
    loaded: &LoadedProject,
    map: &MapDocument,
    courses: &LoadedCourses,
    layout_id: &str,
    correction: &project::NumberCorrection,
) -> Result<Vec<String>, OpError> {
    let mut trial = loaded.clone();
    trial.project.numbers.push(correction.clone());
    let logos = LogoCache::new();
    let mut blocked = Vec::new();
    for c in &courses.event.courses {
        let on_layout = layout_for(&loaded.project, &c.name).0.id == layout_id;
        let visits = c.stops.iter().any(|s| s.control == correction.control);
        let own = loaded.project.numbers.iter().any(|n| {
            n.control == correction.control && n.course.as_deref() == Some(c.name.as_str())
        });
        if !on_layout || !visits || own {
            continue;
        }
        let (_, report) =
            course_sheet_report(&trial, map, courses, c, &logos).map_err(OpError::Export)?;
        let conflict = report
            .notes
            .iter()
            .any(|n| matches!(n, SheetNote::NumberConflict(k) if k.control == correction.control));
        if conflict {
            blocked.push(c.name.clone());
        }
    }
    Ok(blocked)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::testing::viktoriapark_copy;

    fn args(project: &std::path::Path, control: &str) -> AcceptArgs {
        AcceptArgs {
            project: project.to_path_buf(),
            course: "Training".into(),
            control: control.into(),
            scope: AcceptScope::Sheet,
            position: None,
        }
    }

    #[test]
    fn viktoriapark_takes_a_position_of_the_users_own_once() {
        let dir = viktoriapark_copy();
        // Control 34 has no conflict on the sheet, so there is nothing to propose.
        let none = accept(&args(dir.path(), "34")).unwrap_err();
        assert!(
            matches!(&none, OpError::NumbersNoProposal { control, course } if control == "34" && course == "Training"),
            "{none:?}"
        );
        let unknown = accept(&args(dir.path(), "99")).unwrap_err();
        let OpError::UnknownControl { known, .. } = &unknown else {
            panic!("{unknown:?}");
        };
        // The start, the eleven controls of the course file and the finish.
        assert_eq!(known.len(), 13, "{known:?}");
        assert_eq!(known[1], "34");
        let result = accept(&AcceptArgs {
            position: Some((44.96, 0.744)),
            ..args(dir.path(), "34")
        })
        .unwrap();
        assert_eq!(result.scope, Scope::Course("Training".into()));
        assert_eq!((result.direction, result.gap_mm), (45.0, 0.74));
        assert!(!result.proposed);
        let loaded = project::load(&result.path).unwrap();
        assert_eq!(loaded.project.numbers.len(), 1);
        assert_eq!(
            loaded.project.numbers[0].course.as_deref(),
            Some("Training")
        );
        let again = accept(&AcceptArgs {
            position: Some((10.0, 1.0)),
            ..args(dir.path(), "34")
        })
        .unwrap_err();
        assert!(matches!(again, OpError::NumbersExists { .. }), "{again:?}");
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["scope"]["kind"], "course");
        assert_eq!(json["scope"]["name"], "Training");
    }
}
