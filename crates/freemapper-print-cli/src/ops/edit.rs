//! The edits behind the MCP tools `set_layout`, `set_element` and `assign_mask`: each changes one table of the
//! project file through [`project::set_layout`], [`project::set_element`] or [`project::set_assignment`], which
//! keep the rest of the file as it was. The CLI has no commands for them.

use std::path::PathBuf;

use freemapper_print::courses::layout_for;
use freemapper_print::print::{Orientation, Paper};
use freemapper_print::project::{
    self, Assignment, ElementSpec, LayoutSpec, MapFrameSpec, NorthLines, ProjectError,
};
use serde::Serialize;

pub use freemapper_print::project::Change;

use super::masks::find_course;
use super::{OpError, load_project, read_courses, read_map};

/// What `set_layout` takes: the layout id and the keys to set. A key left out (`None`) keeps the value the layout
/// has, or the default for a new layout.
#[derive(Debug, Clone, Default)]
pub struct LayoutArgs {
    /// The project file or its folder.
    pub project: PathBuf,
    pub id: String,
    pub paper: Option<Paper>,
    pub orientation: Option<Orientation>,
    /// `Some(None)`: print at the map scale.
    pub print_scale: Option<Option<u32>>,
    /// `Some(None)`: centre the printed area.
    pub centre_map_mm: Option<Option<[f64; 2]>>,
    pub margin_mm: Option<f64>,
    pub border_mm: Option<f64>,
    pub crop: Option<bool>,
    pub north_lines: Option<NorthLines>,
    pub hide_parts: Option<Vec<String>>,
    pub hide_shared: Option<Vec<String>>,
}

/// The layout `set_layout` wrote.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LayoutResult {
    /// The project file.
    pub path: PathBuf,
    pub change: Change,
    /// The layout as the file reads back, with its elements.
    pub layout: LayoutSpec,
}

/// What `set_element` takes.
#[derive(Debug, Clone)]
pub struct ElementArgs {
    /// The project file or its folder.
    pub project: PathBuf,
    /// The layout id; `None` for the shared elements.
    pub layout: Option<String>,
    /// The element; it replaces the element of the same id.
    pub element: ElementSpec,
}

/// The element `set_element` wrote.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ElementResult {
    /// The project file.
    pub path: PathBuf,
    pub change: Change,
    /// The layout id; `None` for a shared element.
    pub layout: Option<String>,
    pub element: ElementSpec,
}

/// What `assign_mask` takes.
#[derive(Debug, Clone, Default)]
pub struct AssignArgs {
    /// The project file or its folder.
    pub project: PathBuf,
    pub course: String,
    /// The mask id (a `[[masks]]` entry or a white-out `whiteout-<n>` of the course file); `None` prints the
    /// course without a mask.
    pub mask: Option<String>,
    /// The layout; `None` keeps the course's layout.
    pub layout: Option<String>,
}

/// The assignment `assign_mask` wrote.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AssignResult {
    /// The project file.
    pub path: PathBuf,
    pub change: Change,
    pub assignment: Assignment,
}

/// Adds a layout or changes the keys of the layout of the same id; its elements stay.
pub fn set_layout(args: &LayoutArgs) -> Result<LayoutResult, OpError> {
    let loaded = load_project(&args.project)?;
    let mut layout = loaded
        .project
        .layouts
        .iter()
        .find(|l| l.id == args.id)
        .cloned()
        .unwrap_or_else(|| LayoutSpec {
            id: args.id.clone(),
            paper: Paper::A4,
            orientation: Orientation::Auto,
            print_scale: None,
            map_frame: MapFrameSpec::default(),
            north_lines: NorthLines::Auto,
            hide_parts: Vec::new(),
            elements: Vec::new(),
            hide_shared: Vec::new(),
        });
    let LayoutArgs {
        project: _,
        id: _,
        paper,
        orientation,
        print_scale,
        centre_map_mm,
        margin_mm,
        border_mm,
        crop,
        north_lines,
        hide_parts,
        hide_shared,
    } = args.clone();
    let frame = &mut layout.map_frame;
    set(&mut layout.paper, paper);
    set(&mut layout.orientation, orientation);
    set(&mut layout.print_scale, print_scale);
    set(&mut frame.centre_map_mm, centre_map_mm);
    set(&mut frame.margin_mm, margin_mm);
    set(&mut frame.border_mm, border_mm);
    set(&mut frame.crop, crop);
    set(&mut layout.north_lines, north_lines);
    set(&mut layout.hide_parts, hide_parts);
    set(&mut layout.hide_shared, hide_shared);
    let (lp, change) = project::set_layout(&loaded.path, &layout)
        .map_err(|source| edit_error(&loaded.path, source))?;
    let layout = lp
        .project
        .layouts
        .into_iter()
        .find(|l| l.id == args.id)
        .unwrap_or(layout);
    Ok(LayoutResult {
        path: loaded.path,
        change,
        layout,
    })
}

fn set<T>(field: &mut T, value: Option<T>) {
    if let Some(value) = value {
        *field = value;
    }
}

/// Adds a sheet element to a layout or to the shared elements, or replaces the element of the same id there.
pub fn set_element(args: &ElementArgs) -> Result<ElementResult, OpError> {
    let loaded = load_project(&args.project)?;
    if let Some(id) = &args.layout
        && !loaded.project.layouts.iter().any(|l| &l.id == id)
    {
        return Err(OpError::UnknownLayout {
            id: id.clone(),
            known: loaded
                .project
                .layouts
                .iter()
                .map(|l| l.id.clone())
                .collect(),
        });
    }
    let (_, change) = project::set_element(&loaded.path, args.layout.as_deref(), &args.element)
        .map_err(|source| edit_error(&loaded.path, source))?;
    Ok(ElementResult {
        path: loaded.path,
        change,
        layout: args.layout.clone(),
        element: args.element.clone(),
    })
}

/// Sets the mask (and, when given, the layout) a course prints with. The mask must be one of the project file's
/// `[[masks]]` or a white-out of the course file.
pub fn assign_mask(args: &AssignArgs) -> Result<AssignResult, OpError> {
    let loaded = load_project(&args.project)?;
    let map = read_map(&loaded.map_path())?;
    let Some(courses) = read_courses(&loaded, &map)? else {
        return Err(OpError::AssignNeedsCourseFile);
    };
    let course = find_course(&courses, &args.course)?.name.clone();
    if let Some(mask) = &args.mask {
        let mut known: Vec<String> = loaded.project.masks.iter().map(|m| m.id.clone()).collect();
        known.extend(courses.white_outs.iter().map(|w| w.mask.id.clone()));
        if !known.contains(mask) {
            return Err(OpError::UnknownMask {
                id: mask.clone(),
                known,
            });
        }
    }
    let layout = match &args.layout {
        Some(id) => {
            if !loaded.project.layouts.iter().any(|l| &l.id == id) {
                return Err(OpError::UnknownLayout {
                    id: id.clone(),
                    known: loaded
                        .project
                        .layouts
                        .iter()
                        .map(|l| l.id.clone())
                        .collect(),
                });
            }
            id.clone()
        }
        // The layout the course prints on now: its assignment's, else the first.
        None => layout_for(&loaded.project, &course).0.id.clone(),
    };
    let assignment = Assignment {
        course,
        layout,
        mask: args.mask.clone(),
    };
    let (_, change) = project::set_assignment(&loaded.path, &assignment)
        .map_err(|source| edit_error(&loaded.path, source))?;
    Ok(AssignResult {
        path: loaded.path,
        change,
        assignment,
    })
}

/// A refusal of the project file as an [`OpError`]: a failed write, or a change that was not made.
fn edit_error(path: &std::path::Path, source: ProjectError) -> OpError {
    match source {
        err @ ProjectError::Write { .. } => OpError::WriteFile {
            path: path.to_path_buf(),
            source: anyhow::Error::new(err),
        },
        ProjectError::NotEditable { target, .. } => OpError::NotEditable {
            path: path.to_path_buf(),
            target,
        },
        source => OpError::EditProject {
            path: path.to_path_buf(),
            source,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::testing::viktoriapark_copy;
    use freemapper_print::project::{Anchor, AnchorTarget, Corner, ElementKind};

    #[test]
    fn viktoriapark_gets_a_layout_an_element_and_an_assignment() {
        let dir = viktoriapark_copy();
        let project = dir.path().to_path_buf();
        // The one layout at 1:5000: the other keys and the element stay.
        let result = set_layout(&LayoutArgs {
            project: project.clone(),
            id: "a4-4000".into(),
            print_scale: Some(Some(5000)),
            ..LayoutArgs::default()
        })
        .unwrap();
        assert_eq!(result.change, Change::Changed);
        assert_eq!(result.layout.print_scale, Some(5000));
        assert_eq!(result.layout.map_frame.margin_mm, 12.0);
        assert!(result.layout.map_frame.crop);
        assert_eq!(result.layout.elements[0].id, "descriptions");
        let text = std::fs::read_to_string(&result.path).unwrap();
        assert!(text.contains("print_scale = 5000\n"), "{text}");
        assert!(text.contains("# Vorschlag für Sprintkarten."), "{text}");
        // A new A3 layout with a text.
        let result = set_layout(&LayoutArgs {
            project: project.clone(),
            id: "a3".into(),
            paper: Some(Paper::A3),
            ..LayoutArgs::default()
        })
        .unwrap();
        assert_eq!(result.change, Change::Added);
        let note = ElementSpec {
            id: "note".into(),
            kind: ElementKind::Text,
            anchor: Anchor {
                to: AnchorTarget::Page,
                corner: Corner::BottomLeft,
                offset_mm: [5.0, 5.0],
            },
            lines: vec!["Start 10:00".into()],
            size_pt: Some(10.0),
            box_white: None,
            file: None,
            width_mm: None,
            cell_mm: None,
        };
        let element = set_element(&ElementArgs {
            project: project.clone(),
            layout: Some("a3".into()),
            element: note.clone(),
        })
        .unwrap();
        assert_eq!(element.change, Change::Added);
        let unknown = set_element(&ElementArgs {
            project: project.clone(),
            layout: Some("a5".into()),
            element: note,
        })
        .unwrap_err();
        assert!(
            matches!(&unknown, OpError::UnknownLayout { id, .. } if id == "a5"),
            "{unknown:?}"
        );
        // The course moves to the new layout, without a mask.
        let assigned = assign_mask(&AssignArgs {
            project: project.clone(),
            course: "Training".into(),
            mask: None,
            layout: Some("a3".into()),
        })
        .unwrap();
        assert_eq!(assigned.change, Change::Changed);
        assert_eq!(assigned.assignment.layout, "a3");
        let loaded = project::load(&assigned.path).unwrap();
        assert_eq!(loaded.project.layouts.len(), 2);
        assert_eq!(loaded.project.layouts[1].elements.len(), 1);
        assert_eq!(loaded.project.assignments[0].layout, "a3");
        // A mask the project does not have, and a course the course file does not have.
        let before = std::fs::read_to_string(&assigned.path).unwrap();
        let err = assign_mask(&AssignArgs {
            project: project.clone(),
            course: "Training".into(),
            mask: Some("window-x".into()),
            layout: None,
        })
        .unwrap_err();
        assert!(
            matches!(&err, OpError::UnknownMask { id, known } if id == "window-x" && known.is_empty()),
            "{err:?}"
        );
        let err = assign_mask(&AssignArgs {
            project,
            course: "Lang".into(),
            ..AssignArgs::default()
        })
        .unwrap_err();
        assert!(matches!(err, OpError::UnknownCourse { .. }), "{err:?}");
        assert_eq!(std::fs::read_to_string(&assigned.path).unwrap(), before);
    }

    #[test]
    fn a_layout_value_the_project_file_refuses_is_not_written() {
        let dir = viktoriapark_copy();
        let file = dir.path().join(project::FILE_NAME);
        let before = std::fs::read_to_string(&file).unwrap();
        let err = set_layout(&LayoutArgs {
            project: dir.path().to_path_buf(),
            id: "a4-4000".into(),
            margin_mm: Some(-1.0),
            ..LayoutArgs::default()
        })
        .unwrap_err();
        let OpError::EditProject { source, .. } = &err else {
            panic!("{err:?}");
        };
        assert!(source.to_string().contains("margin_mm"), "{source}");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
    }
}
