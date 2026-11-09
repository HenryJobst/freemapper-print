//! The project's courses: loading the course file the project names, fitting it to the map and comparing it
//! with the project file (ADR 0025). Assignments and number corrections that no longer match the course file
//! are reported, never dropped. This module sits above [`crate::course`], which knows nothing of the project
//! file, so that `course` can move into a crate of its own (ADR 0039).

use std::path::Path;

use crate::course::fit::{FitError, FitNote, FitOptions};
use crate::course::model::{Course, FittedEvent, MAX_COURSE_FILE_BYTES, NumberPlacement};
use crate::course::{CourseError, CourseNote, fit, read_iof_xml, read_ppen};
use crate::omap::MapDocument;
use crate::project::{LayoutSpec, LoadedProject, NumberCorrection, Project};

/// Largest difference in direction (degrees) after which a course file's number placement counts as moved.
pub const BASE_TOLERANCE_DEG: f64 = 0.5;
/// Largest difference in gap (print mm) after which a course file's number placement counts as moved.
pub const BASE_TOLERANCE_MM: f64 = 0.05;

/// The course file, read and fitted to the map, with what the project file says that does not match.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedCourses {
    pub event: FittedEvent,
    /// What the reader found but does not support.
    pub notes: Vec<CourseNote>,
    /// What the fit found out.
    pub fit_notes: Vec<FitNote>,
    /// Entries of the project file that name courses or controls the course file does not have.
    pub stale: Vec<Stale>,
}

/// Where a number correction applies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorrectionScope {
    Layout(String),
    Course(String),
}

impl std::fmt::Display for CorrectionScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Layout(l) => write!(f, "layout {l}"),
            Self::Course(c) => write!(f, "course {c}"),
        }
    }
}

/// An entry of the project file that no longer matches the course file. The entry stays in the file.
#[derive(Debug, Clone, PartialEq)]
pub enum Stale {
    /// The assignment's course is not in the course file (renamed or removed).
    Assignment { course: String },
    /// The correction's control is not in the course file, or not in the course it names.
    Correction {
        control: String,
        scope: CorrectionScope,
    },
    /// A correction overrides a placement that the course file has changed since the correction was made.
    CorrectionOverridesChangedBase { control: String },
}

impl std::fmt::Display for Stale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Assignment { course } => write!(
                f,
                "the assignment for course {course} names a course the course file does not have"
            ),
            Self::Correction { control, scope } => write!(
                f,
                "the number correction for control {control} ({scope}) names a control the course file does not have there"
            ),
            Self::CorrectionOverridesChangedBase { control } => write!(
                f,
                "the number correction for control {control} overrides a placement that has changed in the course file since"
            ),
        }
    }
}

impl Stale {
    /// Stale data is always a warning: `check` exits with 1.
    pub fn is_warning(&self) -> bool {
        true
    }
}

/// Whether the course file's placement `now` differs from the `seen` one a correction was made against.
pub fn base_changed(seen: &NumberPlacement, now: &NumberPlacement) -> bool {
    let turn = (seen.direction - now.direction).rem_euclid(360.0);
    let turn = turn.min(360.0 - turn);
    turn > BASE_TOLERANCE_DEG || (seen.gap_mm - now.gap_mm).abs() > BASE_TOLERANCE_MM
}

/// Why the course file could not be loaded. Every message names the file.
#[derive(Debug, thiserror::Error)]
pub enum CourseLoadError {
    #[error("cannot read course file {file}")]
    Io {
        file: String,
        source: std::io::Error,
    },
    #[error("course file {file} is not a regular file")]
    NotAFile { file: String },
    #[error("course file {file} is {bytes} bytes; at most {max} are read")]
    TooLarge {
        file: String,
        bytes: u64,
        max: usize,
    },
    #[error(
        "course file {file}: unknown file type; use `.ppen` (Purple Pen) or `.xml` (IOF XML 3.0)"
    )]
    UnknownType { file: String },
    /// The reader's error is the `source`, not part of the message, so that a chain prints it once.
    #[error("course file {file}")]
    Read { file: String, source: CourseError },
    /// The fit's error is the `source`, not part of the message.
    #[error("course file {file}")]
    Fit { file: String, source: FitError },
}

/// Reads the course file `[sources] courses` names, chooses the reader by extension (`.ppen`, `.xml`,
/// any case), fits it to `map` and lists the stale entries of the project file, corrections over a changed
/// course file placement included ([`crate::numbers::changed_bases`]). `None` without a
/// course source.
pub fn load_courses(
    project: &LoadedProject,
    map: &MapDocument,
) -> Result<Option<LoadedCourses>, CourseLoadError> {
    let Some(rel) = &project.project.sources.courses else {
        return Ok(None);
    };
    let path = project.resolve(rel);
    let file = path.display().to_string();
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase());
    let reader: fn(&[u8]) -> Result<_, CourseError> = match ext.as_deref() {
        Some("ppen") => read_ppen,
        Some("xml") => read_iof_xml,
        _ => return Err(CourseLoadError::UnknownType { file }),
    };
    let bytes = read_capped(&path, &file)?;
    let raw = reader(&bytes).map_err(|source| CourseLoadError::Read {
        file: file.clone(),
        source,
    })?;
    let s = &project.project.sources;
    let options = FitOptions {
        y_axis: s.course_y_axis,
        check_georef: s.course_georef_check,
    };
    let fitted = fit(raw, map, &options).map_err(|source| CourseLoadError::Fit { file, source })?;
    let mut stale = stale_entries(&project.project, &fitted.event);
    stale.extend(crate::numbers::changed_bases(project, &fitted.event));
    Ok(Some(LoadedCourses {
        notes: fitted.event.notes.clone(),
        event: fitted.event,
        fit_notes: fitted.notes,
        stale,
    }))
}

/// The file's bytes, after it proved to be a regular file within [`MAX_COURSE_FILE_BYTES`]. The read is
/// capped again, because a file can grow after the size check.
fn read_capped(path: &Path, file: &str) -> Result<Vec<u8>, CourseLoadError> {
    use std::io::Read;
    let io = |source| CourseLoadError::Io {
        file: file.to_string(),
        source,
    };
    let meta = std::fs::metadata(path).map_err(io)?;
    if !meta.is_file() {
        return Err(CourseLoadError::NotAFile {
            file: file.to_string(),
        });
    }
    let too_large = |bytes| CourseLoadError::TooLarge {
        file: file.to_string(),
        bytes,
        max: MAX_COURSE_FILE_BYTES,
    };
    if meta.len() > MAX_COURSE_FILE_BYTES as u64 {
        return Err(too_large(meta.len()));
    }
    let limit = MAX_COURSE_FILE_BYTES as u64 + 1;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(io)?
        .take(limit)
        .read_to_end(&mut bytes)
        .map_err(io)?;
    if bytes.len() as u64 > MAX_COURSE_FILE_BYTES as u64 {
        return Err(too_large(bytes.len() as u64));
    }
    Ok(bytes)
}

fn stale_entries(project: &Project, event: &FittedEvent) -> Vec<Stale> {
    let has_course = |name: &str| event.courses.iter().any(|c| c.name == name);
    let stops = |c: &Course<_>, control: &str| c.stops.iter().any(|s| s.control == control);
    let mut stale = Vec::new();
    for a in &project.assignments {
        if !has_course(&a.course) {
            stale.push(Stale::Assignment {
                course: a.course.clone(),
            });
        }
    }
    for n in &project.numbers {
        let found = match (&n.layout, &n.course) {
            (_, Some(course)) => event
                .courses
                .iter()
                .any(|c| &c.name == course && stops(c, &n.control)),
            _ => event.courses.iter().any(|c| stops(c, &n.control)),
        };
        if !found {
            stale.push(Stale::Correction {
                control: n.control.clone(),
                scope: scope_of(n),
            });
        }
    }
    stale
}

fn scope_of(n: &NumberCorrection) -> CorrectionScope {
    match (&n.course, &n.layout) {
        (Some(c), _) => CorrectionScope::Course(c.clone()),
        (None, l) => CorrectionScope::Layout(l.clone().unwrap_or_default()),
    }
}

/// A remark about a layout choice. Information only: it does not count for `check`'s exit code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutNote {
    /// The course has no assignment; it prints on the first layout.
    DefaultLayout { course: String, layout: String },
}

impl LayoutNote {
    pub fn is_warning(&self) -> bool {
        false
    }
}

/// The layout a course prints on: the assignment's, otherwise the first layout (with the note).
/// Requires at least one layout in `project`; `project::load` guarantees that, and that assignments name
/// known layouts.
pub fn layout_for<'a>(
    project: &'a Project,
    course_name: &str,
) -> (&'a LayoutSpec, Option<LayoutNote>) {
    let assigned = project
        .assignments
        .iter()
        .find(|a| a.course == course_name)
        .and_then(|a| project.layouts.iter().find(|l| l.id == a.layout));
    match assigned {
        Some(l) => (l, None),
        None => {
            let first = &project.layouts[0];
            (
                first,
                Some(LayoutNote::DefaultLayout {
                    course: course_name.to_string(),
                    layout: first.id.clone(),
                }),
            )
        }
    }
}
