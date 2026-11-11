//! `masks window` and `masks corridor`: append a mask variant to the project file (ADR 0047).

use std::path::PathBuf;

use freemapper_print::course::model::{ControlKind, Course};
use freemapper_print::courses::{LoadedCourses, StopName, layout_for};
use freemapper_print::mask::Mask;
use freemapper_print::mask_build::{self, Selection, WindowOptions};
use freemapper_print::omap::{MapCoord, MapDocument};
use freemapper_print::project::{self, LayoutSpec, LoadedProject};
use serde::Serialize;
use serde::ser::SerializeStruct;

use super::{OpError, load_project, read_courses, read_map};

/// What `masks window` takes.
#[derive(Debug, Clone, Default)]
pub struct WindowArgs {
    /// The project file or its folder.
    pub project: PathBuf,
    pub course: String,
    /// The controls that get a window, as a list or range (`31,32` or `2-5`); `None` for all.
    pub controls: Option<String>,
    /// The radius of each window (print mm); `None` for the default.
    pub radius_mm: Option<f64>,
    /// `CODE=MM` items: a radius of its own for a control.
    pub radius_for: Vec<String>,
    /// No window around the start.
    pub no_start: bool,
    /// No window around the finish.
    pub no_finish: bool,
    /// The mask id; `None` for one made from the course name.
    pub id: Option<String>,
    /// Assign the mask to the course.
    pub assign: bool,
}

/// What `masks corridor` takes.
#[derive(Debug, Clone, Default)]
pub struct CorridorArgs {
    /// The project file or its folder.
    pub project: PathBuf,
    /// The polyline in map mm: `x,y;x,y;…`.
    pub points: String,
    /// The width of the corridor (print mm); `None` for the default.
    pub width_mm: Option<f64>,
    /// The mask id; `None` for `corridor-<n>`.
    pub id: Option<String>,
    /// Assign the mask to `course`.
    pub assign: bool,
    /// The course whose layout gives the print scale (and that `assign` assigns the mask to).
    pub course: Option<String>,
}

/// The course and layout a new mask was assigned to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Assigned {
    pub course: String,
    pub layout: String,
}

/// A window of its own radius.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OwnRadius {
    #[serde(serialize_with = "stop")]
    pub stop: StopName,
    pub radius_mm: f64,
}

/// Which stops of the course got no window although the course has them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StartFinish {
    Start,
    Finish,
    Both,
}

/// The window mask `masks window` wrote (or is about to write, when the comment is made).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WindowResult {
    /// The project file.
    pub path: PathBuf,
    /// The mask id.
    pub id: String,
    pub course: String,
    /// The stops with a window, in course order.
    #[serde(serialize_with = "stops")]
    pub windows: Vec<StopName>,
    /// The default radius (print mm).
    pub radius_mm: f64,
    /// The windows whose radius differs from the default.
    pub own_radius: Vec<OwnRadius>,
    /// The print scale of the course's layout, which turns print mm into map mm.
    pub print_scale: u32,
    pub assigned: Option<Assigned>,
    /// A start or finish of the course without a window: `check` will name it.
    pub without_window: Option<StartFinish>,
}

/// The corridor mask `masks corridor` wrote (or is about to write, when the comment is made).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CorridorResult {
    /// The project file.
    pub path: PathBuf,
    /// The mask id.
    pub id: String,
    /// The polyline as read (map mm).
    pub points: Vec<[f64; 2]>,
    /// The width (print mm).
    pub width_mm: f64,
    /// The print scale the width is measured at.
    pub print_scale: u32,
    pub assigned: Option<Assigned>,
}

/// Appends a mask with circular windows around the controls of `args.course`. `comment` makes the comment
/// written above the mask, in the user's language, from what is about to be written.
pub fn window(
    args: &WindowArgs,
    comment: &dyn Fn(&WindowResult) -> String,
) -> Result<WindowResult, OpError> {
    let loaded = load_project(&args.project)?;
    let map = read_map(&loaded.map_path())?;
    let Some(courses) = read_courses(&loaded, &map)? else {
        return Err(OpError::MasksNeedCourseFile);
    };
    let course = find_course(&courses, &args.course)?;
    let build_error = |error| OpError::Build {
        error,
        course: course.name.clone(),
    };
    let selection = match &args.controls {
        Some(text) => Selection::parse(text).map_err(build_error)?,
        None => Selection::All,
    };
    let mut per_control = std::collections::BTreeMap::new();
    for item in &args.radius_for {
        let parsed = item.rsplit_once('=').and_then(|(code, mm)| {
            let mm: f64 = mm.trim().parse().ok()?;
            let code = code.trim();
            (!code.is_empty()).then(|| (code.to_string(), mm))
        });
        let Some((code, mm)) = parsed else {
            return Err(OpError::RadiusFor { text: item.clone() });
        };
        per_control.insert(code, mm);
    }
    let layout = layout_for(&loaded.project, &course.name).0;
    let print_scale = layout.print_scale.unwrap_or(map.georef.scale_denominator);
    let options = WindowOptions {
        selection,
        start: !args.no_start,
        finish: !args.no_finish,
        radius_mm: args.radius_mm.unwrap_or(mask_build::DEFAULT_RADIUS_MM),
        per_control,
        map_mm_per_print_mm: map_mm_per_print_mm(&map, print_scale)?,
    };
    let id = new_mask_id(&loaded, args.id.as_deref(), |taken| {
        mask_build::window_id(&course.name, taken)
    })?;
    if args.assign {
        check_assignable(&loaded, &course.name)?;
    }
    let (mask, windows) = mask_build::windows(
        &id,
        course,
        &courses.event,
        &options,
        mask_build::map_extent(&map),
    )
    .map_err(build_error)?;
    // A start or finish of the course whose code got no window: check names it.
    let without_window = |kind: ControlKind| {
        course.stops.iter().any(|s| {
            let of_kind = courses
                .event
                .controls
                .iter()
                .find(|c| c.code == s.control)
                .is_some_and(|c| s.kind.unwrap_or(c.kind) == kind);
            of_kind && !windows.iter().any(|w| w.stop.code() == s.control)
        })
    };
    let without_window = match (
        without_window(ControlKind::Start),
        without_window(ControlKind::Finish),
    ) {
        (true, true) => Some(StartFinish::Both),
        (true, false) => Some(StartFinish::Start),
        (false, true) => Some(StartFinish::Finish),
        (false, false) => None,
    };
    let result = WindowResult {
        path: loaded.path.clone(),
        id,
        course: course.name.clone(),
        windows: windows.iter().map(|w| w.stop.clone()).collect(),
        radius_mm: options.radius_mm,
        own_radius: windows
            .iter()
            .filter(|w| w.radius_mm != options.radius_mm)
            .map(|w| OwnRadius {
                stop: w.stop.clone(),
                radius_mm: w.radius_mm,
            })
            .collect(),
        print_scale,
        assigned: args.assign.then(|| Assigned {
            course: course.name.clone(),
            layout: layout.id.clone(),
        }),
        without_window,
    };
    write_mask(&loaded, &mask, result.assigned.as_ref(), &comment(&result))?;
    Ok(result)
}

/// Appends a mask with a corridor along the polyline `args.points`. `comment` makes the comment written above the
/// mask, in the user's language, from what is about to be written.
pub fn corridor(
    args: &CorridorArgs,
    comment: &dyn Fn(&CorridorResult) -> String,
) -> Result<CorridorResult, OpError> {
    let loaded = load_project(&args.project)?;
    let map = read_map(&loaded.map_path())?;
    let points = mask_build::parse_points(&args.points).map_err(|error| OpError::Build {
        error,
        course: String::new(),
    })?;
    let map_scale = map.georef.scale_denominator;
    let scale_of = |l: &LayoutSpec| l.print_scale.unwrap_or(map_scale);
    // The width is print mm, so it needs the print scale: the course's layout, or the one all layouts share.
    let (course, layout) = match &args.course {
        Some(name) => {
            let Some(courses) = read_courses(&loaded, &map)? else {
                return Err(OpError::MasksNeedCourseFile);
            };
            let course = find_course(&courses, name)?.name.clone();
            let layout = layout_for(&loaded.project, &course).0;
            (Some(course), layout)
        }
        None => {
            let mut scales: Vec<u32> = loaded.project.layouts.iter().map(scale_of).collect();
            scales.sort_unstable();
            scales.dedup();
            if scales.len() > 1 {
                return Err(OpError::MasksScales { scales });
            }
            (None, &loaded.project.layouts[0])
        }
    };
    let print_scale = scale_of(layout);
    let width = args.width_mm.unwrap_or(mask_build::DEFAULT_WIDTH_MM);
    let k = map_mm_per_print_mm(&map, print_scale)?;
    let id = new_mask_id(&loaded, args.id.as_deref(), mask_build::corridor_id)?;
    if args.assign
        && let Some(course) = &course
    {
        check_assignable(&loaded, course)?;
    }
    let polygon =
        mask_build::corridor(&points, width, k, mask_build::map_extent(&map)).map_err(|error| {
            OpError::Build {
                error,
                course: course.clone().unwrap_or_default(),
            }
        })?;
    let mask = Mask {
        id: id.clone(),
        polygons: vec![polygon],
    };
    let result = CorridorResult {
        path: loaded.path.clone(),
        id,
        points,
        width_mm: width,
        print_scale,
        assigned: match (course, args.assign) {
            (Some(course), true) => Some(Assigned {
                course,
                layout: layout.id.clone(),
            }),
            _ => None,
        },
    };
    write_mask(&loaded, &mask, result.assigned.as_ref(), &comment(&result))?;
    Ok(result)
}

/// The course `name` of the course file, or a usage error that lists the courses.
fn find_course<'a>(
    courses: &'a LoadedCourses,
    name: &str,
) -> Result<&'a Course<MapCoord>, OpError> {
    courses
        .event
        .courses
        .iter()
        .find(|c| c.name == name)
        .ok_or_else(|| OpError::UnknownCourse {
            name: name.to_string(),
            known: courses
                .event
                .courses
                .iter()
                .map(|c| c.name.clone())
                .collect(),
        })
}

/// Map mm per print mm when the map is printed at 1:`print_scale`.
fn map_mm_per_print_mm(map: &MapDocument, print_scale: u32) -> Result<f64, OpError> {
    let map_scale = map.georef.scale_denominator;
    if map_scale == 0 {
        return Err(OpError::MasksMapScale);
    }
    Ok(f64::from(print_scale) / f64::from(map_scale))
}

/// The id a new mask gets: `given` after the checks of the project file, else `default` made unique. A taken id
/// is a usage error.
fn new_mask_id(
    loaded: &LoadedProject,
    given: Option<&str>,
    default: impl FnOnce(&[&str]) -> String,
) -> Result<String, OpError> {
    let taken: Vec<&str> = loaded.project.masks.iter().map(|m| m.id.as_str()).collect();
    let Some(id) = given else {
        return Ok(default(&taken));
    };
    let count = id.chars().count();
    if count == 0
        || count > freemapper_print::mask::MAX_ID_CHARS
        || id.chars().any(char::is_control)
    {
        return Err(OpError::MaskId);
    }
    if taken.contains(&id) {
        return Err(OpError::MaskIdTaken { id: id.to_string() });
    }
    Ok(id.to_string())
}

/// Refuses to assign a mask to a course whose assignment names another mask.
fn check_assignable(loaded: &LoadedProject, course: &str) -> Result<(), OpError> {
    let other = loaded
        .project
        .assignments
        .iter()
        .find(|a| a.course == course)
        .and_then(|a| a.mask.as_deref());
    match other {
        Some(mask) => Err(OpError::CourseHasMask {
            course: course.to_string(),
            mask: mask.to_string(),
        }),
        None => Ok(()),
    }
}

/// Writes `mask` with [`project::append_mask`]; a refusal of the project file becomes its [`OpError`].
fn write_mask(
    loaded: &LoadedProject,
    mask: &Mask,
    assign: Option<&Assigned>,
    comment: &str,
) -> Result<(), OpError> {
    let assign = assign.map(|a| (a.course.as_str(), a.layout.as_str()));
    project::append_mask(&loaded.path, mask, assign, comment).map_err(|err| {
        let path = loaded.path.clone();
        match err {
            project::ProjectError::DuplicateId { id, .. } => OpError::MaskIdTaken { id },
            project::ProjectError::CourseHasMask { course, mask, .. } => {
                OpError::CourseHasMask { course, mask }
            }
            project::ProjectError::AssignmentNotEditable {
                course, mask, new, ..
            } => OpError::AssignmentInline { course, mask, new },
            err @ project::ProjectError::Write { .. } => OpError::WriteFile {
                path,
                source: anyhow::Error::new(err),
            },
            // A file that no longer loads has been put back as it was.
            err => OpError::LoadChangedMask {
                path,
                source: anyhow::Error::new(err),
            },
        }
    })?;
    Ok(())
}

/// A stop as `{kind, code}`: `kind` is `start`, `control` or `finish`.
fn stop<S: serde::Serializer>(stop: &StopName, serializer: S) -> Result<S::Ok, S::Error> {
    let kind = match stop {
        StopName::Start(_) => "start",
        StopName::Control(_) => "control",
        StopName::Finish(_) => "finish",
    };
    let mut s = serializer.serialize_struct("Stop", 2)?;
    s.serialize_field("kind", kind)?;
    s.serialize_field("code", stop.code())?;
    s.end()
}

/// Stops as a list of `{kind, code}`.
fn stops<S: serde::Serializer>(stops: &[StopName], serializer: S) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeSeq;
    struct One<'a>(&'a StopName);
    impl Serialize for One<'_> {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            stop(self.0, serializer)
        }
    }
    let mut seq = serializer.serialize_seq(Some(stops.len()))?;
    for s in stops {
        seq.serialize_element(&One(s))?;
    }
    seq.end()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::testing::viktoriapark_copy;

    #[test]
    fn window_mask_on_viktoriapark_names_its_windows_and_is_written_with_the_comment() {
        let dir = viktoriapark_copy();
        let result = window(
            &WindowArgs {
                project: dir.path().to_path_buf(),
                course: "Training".into(),
                controls: Some("1-3".into()),
                radius_for: vec!["52=9".into()],
                no_finish: true,
                assign: true,
                ..WindowArgs::default()
            },
            &|r| format!("{} windows", r.windows.len()),
        )
        .unwrap();
        assert_eq!(result.course, "Training");
        assert_eq!(result.id, "window-Training");
        assert_eq!(result.print_scale, 4000);
        // The start, controls 1 to 3 and no finish.
        let codes: Vec<&str> = result.windows.iter().map(StopName::code).collect();
        assert_eq!(codes.len(), 4, "{codes:?}");
        assert!(matches!(result.windows[0], StopName::Start(_)));
        assert_eq!(&codes[1..], ["34", "52", "45"]);
        assert_eq!(result.own_radius.len(), 1);
        assert_eq!(result.own_radius[0].radius_mm, 9.0);
        assert_eq!(result.without_window, Some(StartFinish::Finish));
        assert_eq!(
            result.assigned,
            Some(Assigned {
                course: "Training".into(),
                layout: "a4-4000".into()
            })
        );
        let text = std::fs::read_to_string(&result.path).unwrap();
        assert!(text.contains("# 4 windows\n"), "{text}");
        let loaded = project::load(&result.path).unwrap();
        assert_eq!(loaded.project.masks[0].id, "window-Training");
        assert_eq!(
            loaded.project.assignments[0].mask.as_deref(),
            Some("window-Training")
        );
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["windows"][0]["kind"], "start");
        assert_eq!(json["windows"][1]["code"], "34");
        assert_eq!(json["without_window"], "finish");
    }

    #[test]
    fn corridor_on_viktoriapark_takes_the_scale_of_its_one_layout() {
        let dir = viktoriapark_copy();
        let result = corridor(
            &CorridorArgs {
                project: dir.path().to_path_buf(),
                points: "10,10;40,20;70,15".into(),
                width_mm: Some(4.0),
                ..CorridorArgs::default()
            },
            &|r| format!("{} points", r.points.len()),
        )
        .unwrap();
        assert_eq!(result.id, "corridor-1");
        assert_eq!(result.points, [[10.0, 10.0], [40.0, 20.0], [70.0, 15.0]]);
        assert_eq!(result.width_mm, 4.0);
        assert_eq!(result.print_scale, 4000);
        assert_eq!(result.assigned, None);
        // A second mask with the same id is refused before anything is written.
        let before = std::fs::read_to_string(&result.path).unwrap();
        let taken = corridor(
            &CorridorArgs {
                project: dir.path().to_path_buf(),
                points: "10,10;40,20".into(),
                id: Some("corridor-1".into()),
                ..CorridorArgs::default()
            },
            &|_| String::new(),
        )
        .unwrap_err();
        assert!(
            matches!(&taken, OpError::MaskIdTaken { id } if id == "corridor-1"),
            "{taken:?}"
        );
        assert_eq!(std::fs::read_to_string(&result.path).unwrap(), before);
    }
}
