//! The edits the page can ask for: `POST api/op/{name}` looks the name up in [`EDITOR_OPS`]. Each entry takes the
//! arguments as JSON and calls an `ops::*` function (ADR 0059); the project file is never named by the page.
//! Adding an op is one entry in the table and its argument type.

use std::path::{Path, PathBuf};

use freemapper_print::print::{Orientation, Paper};
use freemapper_print::project::{Anchor, Corner, ElementKind, ElementSpec, LayoutSpec, NorthLines};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::preview;
use crate::ops::{self, OpError, edit, masks, numbers};
use crate::text::{Lang, Texts};

/// Why an edit did not happen.
#[derive(Debug)]
pub(super) enum EditError {
    /// The arguments do not have the form of this op.
    Form,
    /// The change was refused; the file is as it was.
    Op(OpError),
    /// Refused with a sentence of the editor itself: the Fluent id and the value it names.
    Say(&'static str, String),
}

impl From<OpError> for EditError {
    fn from(error: OpError) -> Self {
        EditError::Op(error)
    }
}

/// One op: the project file and the JSON arguments in, the result as JSON out.
/// The language is the request's; it is the one of the comment an op writes into the file.
pub(super) type Run = fn(&Path, Value, &Lang, Sheets<'_>) -> Result<Value, EditError>;

/// The sheet on screen for a course and a layout: the cached one when the file has not changed, so an edit that
/// needs the sheet's geometry does not draw it again while the history is locked.
pub(super) type Sheets<'a> =
    &'a dyn Fn(Option<&str>, Option<&str>) -> Result<std::sync::Arc<preview::Drawn>, OpError>;

/// The ops of the editor by name.
pub(super) const EDITOR_OPS: &[(&str, Run)] = &[
    ("set_layout", set_layout),
    ("set_element", set_element),
    ("assign_mask", assign_mask),
    ("move_element", move_element),
    ("mask_window", mask_window),
    ("mask_corridor", mask_corridor),
    ("numbers_accept", numbers_accept),
];

/// The op `name`, if the editor has it.
pub(super) fn find(name: &str) -> Option<Run> {
    EDITOR_OPS
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, run)| *run)
}

/// Reads the arguments as `A`, runs `f` and writes the result as JSON without the project file's path.
fn apply<A, R>(args: Value, f: impl FnOnce(A) -> Result<R, EditError>) -> Result<Value, EditError>
where
    A: DeserializeOwned,
    R: serde::Serialize,
{
    let args = serde_json::from_value(args).map_err(|_| EditError::Form)?;
    let mut value = serde_json::to_value(f(args)?).expect("an op result serialises");
    if let Some(object) = value.as_object_mut() {
        object.remove("path");
    }
    Ok(value)
}

/// A field that can be left out (keep the value) or null (clear it).
fn nullable<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LayoutBody {
    id: String,
    paper: Option<Paper>,
    #[serde(default, deserialize_with = "nullable")]
    page_mm: Option<Option<[f64; 2]>>,
    orientation: Option<Orientation>,
    #[serde(default, deserialize_with = "nullable")]
    print_scale: Option<Option<u32>>,
    #[serde(default, deserialize_with = "nullable")]
    centre_map_mm: Option<Option<[f64; 2]>>,
    margin_mm: Option<f64>,
    border_mm: Option<f64>,
    crop: Option<bool>,
    north_lines: Option<NorthLines>,
    hide_parts: Option<Vec<String>>,
    hide_shared: Option<Vec<String>>,
}

fn set_layout(
    project: &Path,
    args: Value,
    _lang: &Lang,
    _sheets: Sheets<'_>,
) -> Result<Value, EditError> {
    apply(args, |a: LayoutBody| {
        Ok(edit::set_layout(&edit::LayoutArgs {
            project: project.to_path_buf(),
            id: a.id,
            paper: a.paper,
            page_mm: a.page_mm,
            orientation: a.orientation,
            print_scale: a.print_scale,
            centre_map_mm: a.centre_map_mm,
            margin_mm: a.margin_mm,
            border_mm: a.border_mm,
            crop: a.crop,
            north_lines: a.north_lines,
            hide_parts: a.hide_parts,
            hide_shared: a.hide_shared,
        })?)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ElementBody {
    /// The layout; left out for a shared element.
    layout: Option<String>,
    id: String,
    kind: ElementKind,
    anchor: Anchor,
    #[serde(default)]
    lines: Vec<String>,
    size_pt: Option<f64>,
    box_white: Option<bool>,
    /// A logo's image, relative to the project file; the page cannot name a file elsewhere.
    file: Option<PathBuf>,
    width_mm: Option<f64>,
    cell_mm: Option<f64>,
}

/// A logo file the page names must lie inside the project folder: relative, no `..`.
fn inside_project(file: &Path) -> bool {
    use std::path::Component;
    file.components()
        .all(|part| matches!(part, Component::Normal(_) | Component::CurDir))
}

fn set_element(
    project: &Path,
    args: Value,
    _lang: &Lang,
    _sheets: Sheets<'_>,
) -> Result<Value, EditError> {
    apply(args, |a: ElementBody| {
        if let Some(file) = &a.file
            && !inside_project(file)
        {
            return Err(EditError::Say("edit-logo-outside", String::new()));
        }
        Ok(edit::set_element(&edit::ElementArgs {
            project: project.to_path_buf(),
            layout: a.layout,
            element: ElementSpec {
                id: a.id,
                kind: a.kind,
                anchor: a.anchor,
                lines: a.lines,
                size_pt: a.size_pt,
                box_white: a.box_white,
                file: a.file,
                width_mm: a.width_mm,
                cell_mm: a.cell_mm,
            },
        })?)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AssignBody {
    course: String,
    mask: Option<String>,
    layout: Option<String>,
}

fn assign_mask(
    project: &Path,
    args: Value,
    _lang: &Lang,
    _sheets: Sheets<'_>,
) -> Result<Value, EditError> {
    apply(args, |a: AssignBody| {
        Ok(edit::assign_mask(&edit::AssignArgs {
            project: project.to_path_buf(),
            course: a.course,
            mask: a.mask,
            layout: a.layout,
        })?)
    })
}

/// Moving an element on the sheet: the shift on the page in mm (x right, y down).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MoveBody {
    /// The layout of the sheet on screen; the element is the layout's own, else the shared one of this id.
    layout: Option<String>,
    id: String,
    dx_mm: f64,
    dy_mm: f64,
}

/// The anchor moved by (`dx`, `dy`) on the page. The offset is measured inward from the corner, so from a right
/// or bottom edge it counts against the shift; from a left or top edge and from the middle it counts with it.
pub(super) fn moved(anchor: Anchor, dx: f64, dy: f64) -> Anchor {
    let sign = |towards_end: bool| if towards_end { -1.0 } else { 1.0 };
    let (right, bottom) = (
        matches!(
            anchor.corner,
            Corner::TopRight | Corner::Right | Corner::BottomRight
        ),
        matches!(
            anchor.corner,
            Corner::BottomLeft | Corner::Bottom | Corner::BottomRight
        ),
    );
    // 0.01 mm: a dragged value never shows float noise in the file.
    let round = |v: f64| (v * 100.0).round() / 100.0;
    let [ox, oy] = anchor.offset_mm;
    Anchor {
        offset_mm: [round(ox + sign(right) * dx), round(oy + sign(bottom) * dy)],
        ..anchor
    }
}

fn find_element<'a>(
    layouts: &'a [LayoutSpec],
    shared: &'a [ElementSpec],
    layout: Option<&str>,
    id: &str,
) -> Option<(Option<String>, &'a ElementSpec)> {
    if let Some(layout) = layout
        && let Some(found) = layouts
            .iter()
            .find(|l| l.id == layout)
            .and_then(|l| l.elements.iter().find(|e| e.id == id))
    {
        return Some((Some(layout.to_owned()), found));
    }
    shared.iter().find(|e| e.id == id).map(|e| (None, e))
}

fn move_element(
    project: &Path,
    args: Value,
    _lang: &Lang,
    _sheets: Sheets<'_>,
) -> Result<Value, EditError> {
    apply(args, |a: MoveBody| {
        if !(a.dx_mm.is_finite()
            && a.dy_mm.is_finite()
            && a.dx_mm.abs() < 1e4
            && a.dy_mm.abs() < 1e4)
        {
            return Err(EditError::Form);
        }
        let loaded = ops::load_project(project)?;
        let Some((layout, element)) = find_element(
            &loaded.project.layouts,
            &loaded.project.shared,
            a.layout.as_deref(),
            &a.id,
        ) else {
            return Err(EditError::Say("edit-no-element", a.id));
        };
        let mut element = element.clone();
        element.anchor = moved(element.anchor, a.dx_mm, a.dy_mm);
        Ok(edit::set_element(&edit::ElementArgs {
            project: project.to_path_buf(),
            layout,
            element,
        })?)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WindowBody {
    course: String,
    /// Codes (`31,32`) or places (`2-5`); left out for every control.
    controls: Option<String>,
    /// Print mm.
    radius: Option<f64>,
    #[serde(default)]
    radius_for: Vec<String>,
    #[serde(default)]
    no_start: bool,
    #[serde(default)]
    no_finish: bool,
    id: Option<String>,
    #[serde(default)]
    assign: bool,
}

fn mask_window(
    project: &Path,
    args: Value,
    lang: &Lang,
    _sheets: Sheets<'_>,
) -> Result<Value, EditError> {
    apply(args, |a: WindowBody| {
        let texts = Texts::new(lang);
        Ok(masks::window(
            &masks::WindowArgs {
                project: project.to_path_buf(),
                course: a.course,
                controls: a.controls,
                radius_mm: a.radius,
                radius_for: a.radius_for,
                no_start: a.no_start,
                no_finish: a.no_finish,
                id: a.id,
                assign: a.assign,
            },
            &|r| crate::window_comment(&texts, r),
        )?)
    })
}

/// A corridor along a line: `points` as the op takes them (map mm, y up), or `page_points_mm` the page drew
/// (page mm, y down) on the sheet of `course` and `layout`, which the server turns into map mm.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CorridorBody {
    points: Option<String>,
    page_points_mm: Option<Vec<[f64; 2]>>,
    layout: Option<String>,
    /// Print mm.
    width: Option<f64>,
    course: Option<String>,
    id: Option<String>,
    #[serde(default)]
    assign: bool,
}

fn mask_corridor(
    project: &Path,
    args: Value,
    lang: &Lang,
    sheets: Sheets<'_>,
) -> Result<Value, EditError> {
    apply(args, |a: CorridorBody| {
        let texts = Texts::new(lang);
        if a.assign && a.course.is_none() {
            return Err(EditError::Say("mcp-error-corridor-assign", String::new()));
        }
        let points = match (a.points, a.page_points_mm) {
            (Some(text), None) => text,
            (None, Some(drawn)) => {
                let sheet = sheets(a.course.as_deref(), a.layout.as_deref())?;
                drawn
                    .iter()
                    .map(|[x, y]| {
                        let [mx, my] = sheet.map_mm(*x, *y);
                        format!("{mx},{my}")
                    })
                    .collect::<Vec<_>>()
                    .join(";")
            }
            _ => return Err(EditError::Form),
        };
        Ok(masks::corridor(
            &masks::CorridorArgs {
                project: project.to_path_buf(),
                points,
                width_mm: a.width,
                id: a.id,
                assign: a.assign,
                course: a.course,
            },
            &|r| crate::corridor_comment(&texts, r),
        )?)
    })
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum ScopeBody {
    LayoutWide,
    Sheet,
}

/// A number's correction. The position is the proposal of `check` (nothing given), a direction with a gap, or the
/// page point `to_page_mm` the number was dropped at on the sheet of `course` and `layout`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NumbersBody {
    course: String,
    control: String,
    scope: ScopeBody,
    direction: Option<f64>,
    gap: Option<f64>,
    to_page_mm: Option<[f64; 2]>,
    layout: Option<String>,
}

fn numbers_accept(
    project: &Path,
    args: Value,
    _lang: &Lang,
    sheets: Sheets<'_>,
) -> Result<Value, EditError> {
    apply(args, |a: NumbersBody| {
        let position = match (a.direction, a.gap, a.to_page_mm) {
            (None, None, None) => None,
            (Some(direction), Some(gap), None) => Some((direction, gap)),
            (None, None, Some([x, y])) => {
                if !(x.is_finite() && y.is_finite()) {
                    return Err(EditError::Form);
                }
                let sheet = sheets(Some(&a.course), a.layout.as_deref())?;
                match sheet.number_placement_at(&a.control, x, y) {
                    Some(position) => Some(position),
                    None => return Err(EditError::Say("edit-no-number", a.control)),
                }
            }
            _ => return Err(EditError::Form),
        };
        Ok(numbers::accept(&numbers::AcceptArgs {
            project: project.to_path_buf(),
            course: a.course,
            control: a.control,
            scope: match a.scope {
                ScopeBody::LayoutWide => numbers::AcceptScope::Layout,
                ScopeBody::Sheet => numbers::AcceptScope::Sheet,
            },
            position,
        })?)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use freemapper_print::project::AnchorTarget;

    #[test]
    fn a_logo_file_must_stay_inside_the_project_folder() {
        for good in ["logo.png", "logos/a.svg", "./logo.png"] {
            assert!(inside_project(Path::new(good)), "{good}");
        }
        for bad in [
            "../x.png",
            "a/../../x",
            "a/../x",
            "/etc/x.png",
            "C:\\x.png",
            "\\\\host\\s\\x.png",
        ] {
            // On Windows these carry a prefix or root; elsewhere the absolute and `..` forms are caught.
            let refused = !inside_project(Path::new(bad));
            if cfg!(windows) || !bad.contains('\\') {
                assert!(refused, "{bad}");
            }
        }
    }

    #[test]
    fn a_shift_counts_inward_from_the_corner() {
        let at = |corner| Anchor {
            to: AnchorTarget::Page,
            corner,
            offset_mm: [10.0, 10.0],
        };
        // Moving 3 right and 2 down.
        assert_eq!(moved(at(Corner::TopLeft), 3.0, 2.0).offset_mm, [13.0, 12.0]);
        assert_eq!(moved(at(Corner::TopRight), 3.0, 2.0).offset_mm, [7.0, 12.0]);
        assert_eq!(
            moved(at(Corner::BottomLeft), 3.0, 2.0).offset_mm,
            [13.0, 8.0]
        );
        assert_eq!(
            moved(at(Corner::BottomRight), 3.0, 2.0).offset_mm,
            [7.0, 8.0]
        );
        assert_eq!(moved(at(Corner::Centre), 3.0, 2.0).offset_mm, [13.0, 12.0]);
        assert_eq!(moved(at(Corner::Top), 0.1, 0.2).offset_mm, [10.1, 10.2]);
    }
}
