//! `project_info`: what a project holds, read without drawing a sheet. The CLI has no command for it; the MCP
//! server's `project_info` tool returns it.

use std::path::PathBuf;

use freemapper_print::courses::{MaskRef, layout_for, mask_for};
use freemapper_print::overprint::control_count;
use freemapper_print::print::{Orientation, Paper};
use freemapper_print::project::{Assignment, ElementKind, ElementSpec};
use freemapper_print::scale::MapKind;
use serde::Serialize;

use super::{OpError, load_project, read_courses, read_map};

/// Which project to describe.
#[derive(Debug, Clone)]
pub struct InfoArgs {
    /// The project file or its folder.
    pub project: PathBuf,
}

/// A project: its sources, layouts with their elements, courses, mask variants and assignments.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectInfo {
    /// The project file.
    pub path: PathBuf,
    /// The event title.
    pub title: String,
    pub kind: MapKind,
    /// The scale of the map file.
    pub map_scale: u32,
    /// The map and the course file as the project file names them (relative to it).
    pub map: PathBuf,
    pub courses_file: Option<PathBuf>,
    /// The elements every layout shows unless it hides them.
    pub shared_elements: Vec<ElementInfo>,
    pub layouts: Vec<LayoutInfo>,
    /// The courses of the course file in file order; empty without one.
    pub courses: Vec<CourseInfo>,
    /// The mask variants: the project file's `[[masks]]`, then the white-outs of the course file.
    pub masks: Vec<MaskInfo>,
    /// The `[[assignments]]` as written.
    pub assignments: Vec<Assignment>,
}

/// A sheet element by id and kind.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ElementInfo {
    pub id: String,
    pub kind: ElementKind,
}

/// A layout and its own elements.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LayoutInfo {
    pub id: String,
    pub paper: Paper,
    pub orientation: Orientation,
    /// `None`: the map scale.
    pub print_scale: Option<u32>,
    pub elements: Vec<ElementInfo>,
}

/// A course of the course file and how it prints.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CourseInfo {
    pub name: String,
    /// Its numbered controls: start, finish, crossing points and ends of marked routes do not count.
    pub controls: usize,
    /// The layout it prints on.
    pub layout: String,
    /// The ids of the mask variants its sheet applies.
    pub masks: Vec<String>,
}

/// Where a mask variant comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MaskSource {
    /// A `[[masks]]` entry of the project file.
    Project,
    /// A white-out of the course file (`whiteout-<n>`, ADR 0048).
    WhiteOut,
}

/// A mask variant.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MaskInfo {
    pub id: String,
    pub source: MaskSource,
    pub polygons: usize,
}

/// Reads the project, its map and its course file, and says what they hold.
pub fn info(args: &InfoArgs) -> Result<ProjectInfo, OpError> {
    let loaded = load_project(&args.project)?;
    let map = read_map(&loaded.map_path())?;
    let courses = read_courses(&loaded, &map)?;
    let project = &loaded.project;
    let elements = |list: &[ElementSpec]| {
        list.iter()
            .map(|e| ElementInfo {
                id: e.id.clone(),
                kind: e.kind,
            })
            .collect()
    };
    let mut masks: Vec<MaskInfo> = project
        .masks
        .iter()
        .map(|m| MaskInfo {
            id: m.id.clone(),
            source: MaskSource::Project,
            polygons: m.polygons.len(),
        })
        .collect();
    let mut course_infos = Vec::new();
    if let Some(courses) = &courses {
        masks.extend(courses.white_outs.iter().map(|w| MaskInfo {
            id: w.mask.id.clone(),
            source: MaskSource::WhiteOut,
            polygons: w.mask.polygons.len(),
        }));
        course_infos = courses
            .event
            .courses
            .iter()
            .map(|c| CourseInfo {
                name: c.name.clone(),
                controls: control_count(c, &courses.event),
                layout: layout_for(project, &c.name).0.id.clone(),
                masks: mask_for(project, courses, &c.name)
                    .iter()
                    .map(|m| match m {
                        MaskRef::Project(m) => m.id.clone(),
                        MaskRef::WhiteOut(m) => m.id.clone(),
                    })
                    .collect(),
            })
            .collect();
    }
    Ok(ProjectInfo {
        path: loaded.path.clone(),
        title: project.event.title.clone(),
        kind: project.map.kind,
        map_scale: map.georef.scale_denominator,
        map: project.sources.map.clone(),
        courses_file: project.sources.courses.clone(),
        shared_elements: elements(&project.shared),
        layouts: project
            .layouts
            .iter()
            .map(|l| LayoutInfo {
                id: l.id.clone(),
                paper: l.paper,
                orientation: l.orientation,
                print_scale: l.print_scale,
                elements: elements(&l.elements),
            })
            .collect(),
        courses: course_infos,
        masks,
        assignments: project.assignments.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::testing::viktoriapark;

    #[test]
    fn viktoriapark_has_one_course_on_one_layout() {
        let info = info(&InfoArgs {
            project: viktoriapark(),
        })
        .unwrap();
        assert_eq!(info.title, "Training Viktoriapark");
        assert_eq!(info.kind, MapKind::Sprint);
        assert_eq!(info.map_scale, 4000);
        assert_eq!(info.map, PathBuf::from("berlin-viktoriapark.omap"));
        assert_eq!(info.courses_file, Some(PathBuf::from("training.ppen")));
        let shared: Vec<_> = info.shared_elements.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(shared, ["title", "scale_check_bar"]);
        assert_eq!(info.layouts.len(), 1);
        let layout = &info.layouts[0];
        assert_eq!(
            (
                layout.id.as_str(),
                layout.paper,
                layout.orientation,
                layout.print_scale
            ),
            ("a4-4000", Paper::A4, Orientation::Landscape, Some(4000))
        );
        assert_eq!(layout.elements[0].kind, ElementKind::ControlDescriptions);
        assert_eq!(
            info.courses,
            [CourseInfo {
                name: "Training".into(),
                controls: 11,
                layout: "a4-4000".into(),
                masks: vec![],
            }]
        );
        assert!(info.masks.is_empty(), "{:?}", info.masks);
        assert_eq!(info.assignments.len(), 1);
        assert_eq!(info.assignments[0].course, "Training");
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["courses"][0]["controls"], 11);
        assert_eq!(
            json["layouts"][0]["elements"][0]["kind"],
            "control_descriptions"
        );
    }
}
