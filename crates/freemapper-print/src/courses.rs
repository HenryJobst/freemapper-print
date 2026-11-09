//! The project's courses: loading the course file the project names, fitting it to the map and comparing it
//! with the project file (ADR 0025). Assignments and number corrections that no longer match the course file
//! are reported, never dropped. This module sits above [`crate::course`], which knows nothing of the project
//! file, so that `course` can move into a crate of its own (ADR 0039).

use std::collections::BTreeMap;
use std::path::Path;

use crate::course::fit::{FitError, FitNote, FitOptions};
use crate::course::model::{Course, FittedEvent, MAX_COURSE_FILE_BYTES, NumberPlacement};
use crate::course::{CourseError, CourseNote, fit, read_iof_xml, read_ppen};
use crate::course::{CourseFilter, Special};
use crate::mask::{Budget, MAX_CHECK_STEPS, Mask, MaskPolygon, Problem};
use crate::omap::MapCoord;
use crate::omap::MapDocument;
use crate::overprint::applies;
use crate::project::{LayoutSpec, LoadedProject, NumberCorrection, Project, white_out_id};

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
    /// The white-outs of the course file as mask variants `whiteout-<n>`, in file order (ADR 0045).
    pub white_outs: Vec<WhiteOut>,
    /// Masks that no course uses, and white-outs that cannot make a mask.
    pub mask_notes: Vec<MaskNote>,
    /// What is wrong, if anything, with each mask made from white-outs, by id, so that [`mask_for`] checks a mask
    /// once.
    checked: BTreeMap<String, Option<Problem>>,
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
    /// The assignment of `course` names the white-out `mask` (`whiteout-<n>`), which the course file does not
    /// have; the course prints without a mask. Other unknown mask ids are refused when the project loads.
    Mask { course: String, mask: String },
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
            Self::Mask { course, mask } => write!(
                f,
                "the assignment for course {course} names the white-out {mask}, which the course file does not have"
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
    let white_outs = white_outs_of(&fitted.event);
    let mut stale = stale_entries(&project.project, &fitted.event, &white_outs);
    stale.extend(crate::numbers::changed_bases(project, &fitted.event));
    let (checked, mask_notes) = mask_checks(&project.project, &fitted.event, &white_outs);
    Ok(Some(LoadedCourses {
        notes: fitted.event.notes.clone(),
        event: fitted.event,
        fit_notes: fitted.notes,
        stale,
        white_outs,
        mask_notes,
        checked,
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

fn stale_entries(project: &Project, event: &FittedEvent, white_outs: &[WhiteOut]) -> Vec<Stale> {
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
    // The loader refused every other unknown id; a `whiteout-<n>` waits for the course file.
    for a in &project.assignments {
        if let Some(m) = &a.mask
            && !project.masks.iter().any(|own| &own.id == m)
            && !white_outs.iter().any(|w| &w.mask.id == m)
        {
            stale.push(Stale::Mask {
                course: a.course.clone(),
                mask: m.clone(),
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

/// A white-out of the course file as a mask variant, with the courses it applies to.
#[derive(Debug, Clone, PartialEq)]
pub struct WhiteOut {
    pub mask: Mask,
    pub courses: CourseFilter,
}

/// A remark about the mask variants.
#[derive(Debug, Clone, PartialEq)]
pub enum MaskNote {
    /// No assignment names the mask and no white-out of the course file has its id. Information only.
    MaskUnused { id: String },
    /// The white-outs of `course` (their ids joined by `+`) do not make a valid mask, for example because two
    /// of them overlap; the course prints without a mask. A warning.
    WhiteOutUnusable {
        course: String,
        mask: String,
        problem: Problem,
    },
}

impl MaskNote {
    pub fn is_warning(&self) -> bool {
        matches!(self, MaskNote::WhiteOutUnusable { .. })
    }
}

/// The mask a course prints with: a `[[masks]]` entry of the project file, or one made from the course file's
/// white-outs.
#[derive(Debug, Clone, PartialEq)]
pub enum MaskRef<'a> {
    Project(&'a Mask),
    WhiteOut(Mask),
}

impl MaskRef<'_> {
    pub fn mask(&self) -> &Mask {
        match self {
            MaskRef::Project(m) => m,
            MaskRef::WhiteOut(m) => m,
        }
    }
}

/// The mask `course` prints with (ADR 0045): the one its assignment names, otherwise the course file's white-outs
/// for the course (their filter read as for every special object: all courses, or the listed ones), otherwise
/// none. A white-out with a `[[masks]]` entry of its id (`whiteout-<n>`) takes that
/// entry's polygons. Several white-outs make one variant, their ids joined by `+`. `None` also when the named
/// white-out is missing ([`Stale::Mask`]) or the white-outs make no valid mask ([`MaskNote::WhiteOutUnusable`]).
pub fn mask_for<'a>(
    project: &'a Project,
    loaded: &LoadedCourses,
    course: &str,
) -> Option<MaskRef<'a>> {
    let mut check = |m: &Mask| match loaded.checked.get(&m.id) {
        Some(problem) => *problem,
        None => check_white_out(m, &mut Budget::new(MAX_CHECK_STEPS)),
    };
    match resolve(project, &loaded.white_outs, course, &mut check) {
        Resolved::Mask(m) => Some(m),
        _ => None,
    }
}

/// What [`mask_for`] finds for a course.
enum Resolved<'a> {
    None,
    Mask(MaskRef<'a>),
    /// The assignment names this white-out, which the course file lacks.
    Missing,
    /// The white-out mask with this id has this problem.
    Unusable(String, Problem),
}

/// The rule of [`mask_for`]. `check` tells what is wrong with a mask made from white-outs; masks of the project
/// file were checked when it was loaded.
fn resolve<'a>(
    project: &'a Project,
    white_outs: &[WhiteOut],
    course: &str,
    check: &mut dyn FnMut(&Mask) -> Option<Problem>,
) -> Resolved<'a> {
    let own = |id: &str| project.masks.iter().find(|m| m.id == id);
    let mut usable = |m: Mask| match check(&m) {
        None => Resolved::Mask(MaskRef::WhiteOut(m)),
        Some(problem) => Resolved::Unusable(m.id, problem),
    };
    let assigned = project
        .assignments
        .iter()
        .find(|a| a.course == course)
        .and_then(|a| a.mask.as_deref());
    if let Some(id) = assigned {
        if let Some(m) = own(id) {
            return Resolved::Mask(MaskRef::Project(m));
        }
        return match white_outs.iter().find(|w| w.mask.id == id) {
            Some(w) => usable(w.mask.clone()),
            None => Resolved::Missing,
        };
    }
    let mine: Vec<&Mask> = white_outs
        .iter()
        .filter(|w| applies(&w.courses, course))
        .map(|w| own(&w.mask.id).unwrap_or(&w.mask))
        .collect();
    match mine.as_slice() {
        [] => Resolved::None,
        [one] => match own(&one.id) {
            Some(m) => Resolved::Mask(MaskRef::Project(m)),
            None => usable((*one).clone()),
        },
        many => usable(Mask {
            id: many
                .iter()
                .map(|m| m.id.as_str())
                .collect::<Vec<_>>()
                .join("+"),
            polygons: many.iter().flat_map(|m| m.polygons.clone()).collect(),
        }),
    }
}

/// What is wrong with a mask made from white-outs, within `budget`.
fn check_white_out(m: &Mask, budget: &mut Budget) -> Option<Problem> {
    crate::mask::check(m, budget).err().map(|e| e.problem)
}

/// The white-outs of `event` as mask variants `whiteout-1`, `whiteout-2` … in file order. The first ring is the
/// outline, any further ones holes; points in map mm, y up, as in a `[[masks]]` table.
fn white_outs_of(event: &FittedEvent) -> Vec<WhiteOut> {
    let pt = |c: &MapCoord| [f64::from(c.x) / 1000.0, -f64::from(c.y) / 1000.0];
    event
        .specials
        .iter()
        .filter_map(|s| match s {
            Special::WhiteOut { rings, courses } => Some((rings, courses)),
            _ => None,
        })
        .enumerate()
        .map(|(i, (rings, courses))| {
            let mut rings = rings.iter().map(|r| r.iter().map(pt).collect::<Vec<_>>());
            let outline = rings.next().unwrap_or_default();
            WhiteOut {
                mask: Mask {
                    id: white_out_id(i + 1),
                    polygons: vec![MaskPolygon {
                        outline,
                        holes: rings.collect(),
                    }],
                },
                courses: courses.clone(),
            }
        })
        .collect()
}

/// The white-out masks of every course of `event` checked once, within one budget: the result for each id,
/// and the notes about masks.
fn mask_checks(
    project: &Project,
    event: &FittedEvent,
    white_outs: &[WhiteOut],
) -> (BTreeMap<String, Option<Problem>>, Vec<MaskNote>) {
    let mut checked: BTreeMap<String, Option<Problem>> = BTreeMap::new();
    let mut budget = Budget::new(MAX_CHECK_STEPS);
    let mut notes: Vec<MaskNote> = project
        .masks
        .iter()
        .filter(|m| {
            let assigned = project
                .assignments
                .iter()
                .any(|a| a.mask.as_deref() == Some(m.id.as_str()));
            !assigned && !white_outs.iter().any(|w| w.mask.id == m.id)
        })
        .map(|m| MaskNote::MaskUnused { id: m.id.clone() })
        .collect();
    for course in &event.courses {
        let mut check = |m: &Mask| {
            *checked
                .entry(m.id.clone())
                .or_insert_with(|| check_white_out(m, &mut budget))
        };
        if let Resolved::Unusable(mask, problem) =
            resolve(project, white_outs, &course.name, &mut check)
        {
            notes.push(MaskNote::WhiteOutUnusable {
                course: course.name.clone(),
                mask,
                problem,
            });
        }
    }
    (checked, notes)
}
