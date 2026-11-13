//! The sheet preview: one sheet drawn by the same code as the PDF, as the renderer's own SVG, and the hit test on
//! it (ADR 0059, core ADR 0022).
//!
//! Everything here is plain and blocking; the server runs it on a blocking task.

use std::path::Path;

use freemapper_print::courses::layout_for;
use freemapper_print::export::ExportError;
use freemapper_print::project::{ElementKind, LayoutSpec, LoadedProject};
use freemapper_print::render::page::{MapClip, PaperBox, RenderedPage};
use freemapper_print::render::svg::{SvgError, SvgView, write_svg};
use freemapper_print::sheet::{
    FOOTER_ID, LogoCache, SheetReport, layout_elements, render_course_sheet, render_sheet,
};
use serde::Serialize;

use crate::ops::{OpError, Sheet, load_project, read_courses, read_map};

/// How near (mm on paper) a click must be to the centre of a control number to pick it.
const NUMBER_REACH_MM: f64 = 2.5;
/// How near (mm on paper) a click must be to a map line to pick it.
const MAP_REACH_MM: f64 = 0.5;

/// One drawn sheet: the page, its report and what the editor needs to answer a click.
pub struct Drawn {
    pub page: RenderedPage,
    pub report: SheetReport,
    /// The notes in the order the CLI prints them.
    pub sheet: Sheet,
    /// The kind of each drawn element, by id (the default footer is `footer`).
    kinds: Vec<(String, &'static str)>,
}

/// A sheet element on the page.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ElementRect {
    pub id: String,
    pub kind: &'static str,
    /// `[x_min, y_min, x_max, y_max]` in page mm, y down.
    pub rect_mm: [f64; 4],
}

/// What a click at a point of the page picks.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Pick {
    /// `element`, `number`, `map_object` or `mask`.
    pub kind: &'static str,
    /// The element id, the control code, the position of the object in the map the sheet was drawn from, or the
    /// mask id.
    pub id: String,
}

/// Why a preview could not be made.
#[derive(Debug)]
pub enum PreviewError {
    Op(OpError),
    Svg(SvgError),
}

impl From<OpError> for PreviewError {
    fn from(e: OpError) -> Self {
        PreviewError::Op(e)
    }
}

/// The element kind as the project file spells it.
pub(super) fn kind_name(kind: ElementKind) -> &'static str {
    match kind {
        ElementKind::TitleBlock => "title_block",
        ElementKind::Text => "text",
        ElementKind::Logo => "logo",
        ElementKind::ScaleCheckBar => "scale_check_bar",
        ElementKind::CreditLine => "credit_line",
        ElementKind::CopyrightLine => "copyright_line",
        ElementKind::ControlDescriptions => "control_descriptions",
    }
}

/// Draws one sheet of the project at `project` (a file or its folder).
///
/// - `course` names a course of the course file; its sheet is drawn on `layout`, or on the layout it is assigned
///   to. An unknown course is [`OpError::UnknownCourse`].
/// - Without `course`: the layout `layout` without a course; without either, the first course on its layout, and
///   in a project without a course file the first layout.
pub fn draw(project: &Path, course: Option<&str>, layout: Option<&str>) -> Result<Drawn, OpError> {
    let loaded = load_project(project)?;
    let map = read_map(&loaded.map_path())?;
    let courses = read_courses(&loaded, &map)?;
    let logos = LogoCache::new();
    let layouts = &loaded.project.layouts;
    let find_layout = |id: &str| -> Result<&LayoutSpec, OpError> {
        layouts
            .iter()
            .find(|l| l.id == id)
            .ok_or_else(|| OpError::UnknownLayout {
                id: id.to_owned(),
                known: layouts.iter().map(|l| l.id.clone()).collect(),
            })
    };
    let first_course = || {
        courses
            .as_ref()
            .and_then(|c| c.event.courses.first())
            .map(|c| c.name.as_str())
    };
    let course = match (course, layout) {
        (Some(name), _) => Some(name),
        (None, Some(_)) => None,
        (None, None) => first_course(),
    };
    let (page, report, shown_layout, shown_course) = match course {
        Some(name) => {
            let courses = courses.as_ref().ok_or_else(|| OpError::UnknownCourse {
                name: name.to_owned(),
                known: Vec::new(),
            })?;
            let found = courses
                .event
                .courses
                .iter()
                .find(|c| c.name == name)
                .ok_or_else(|| OpError::UnknownCourse {
                    name: name.to_owned(),
                    known: courses
                        .event
                        .courses
                        .iter()
                        .map(|c| c.name.clone())
                        .collect(),
                })?;
            let chosen = match layout {
                Some(id) => find_layout(id)?,
                None => layout_for(&loaded.project, name).0,
            };
            let (page, report) = render_course_sheet(&loaded, &map, courses, found, chosen, &logos)
                .map_err(|error| {
                    OpError::Export(ExportError::CourseSheet {
                        course: name.to_owned(),
                        error,
                    })
                })?;
            (page, report, chosen, Some(name))
        }
        None => {
            let chosen = match layout {
                Some(id) => find_layout(id)?,
                None => &layouts[0],
            };
            let (page, report) = render_sheet(&loaded, &map, chosen, &logos).map_err(|error| {
                OpError::Export(ExportError::Sheet {
                    layout: chosen.id.clone(),
                    error,
                })
            })?;
            (page, report, chosen, None)
        }
    };
    Ok(finish(
        &loaded,
        &map,
        page,
        report,
        shown_layout,
        shown_course,
    ))
}

fn finish(
    loaded: &LoadedProject,
    map: &freemapper_print::omap::MapDocument,
    page: RenderedPage,
    report: SheetReport,
    layout: &LayoutSpec,
    course: Option<&str>,
) -> Drawn {
    let mut kinds: Vec<(String, &'static str)> = layout_elements(loaded, layout)
        .map(|e| (e.id.clone(), kind_name(e.kind)))
        .collect();
    kinds.push((FOOTER_ID.to_owned(), "footer"));
    let sheet = Sheet::new(
        course,
        layout,
        report.clone(),
        loaded.project.map.kind,
        map.georef.scale_denominator,
    );
    Drawn {
        page,
        report,
        sheet,
        kinds,
    }
}

impl Drawn {
    /// The sheet as the renderer's SVG: the whole page, `viewBox` in page mm (1 mm is 1 user unit).
    pub fn svg(&self) -> Result<String, SvgError> {
        write_svg(&self.page, &SvgView::default())
    }

    /// The page point `(x_mm, y_mm)` (mm, y down) as a point of the map in the units of a mask: map mm with y up,
    /// to 0.001 mm.
    pub fn map_mm(&self, x_mm: f64, y_mm: f64) -> [f64; 2] {
        let (mx, my) = freemapper_print::sheet::page_to_map_mm(
            self.page.page_mm,
            self.page.centre_mm,
            self.report.print.geometry_factor,
            (x_mm, y_mm),
        );
        let round = |v: f64| (v * 1000.0).round() / 1000.0 + 0.0;
        [round(mx), round(-my)]
    }

    /// Direction (degrees, 0 = east, counter-clockwise) and gap (print mm) that put the number of `control` with
    /// its box centred at the page point `(x_mm, y_mm)`. `None` when the sheet has no number for the control.
    pub fn number_placement_at(&self, control: &str, x_mm: f64, y_mm: f64) -> Option<(f64, f64)> {
        let at = self
            .report
            .numbers
            .iter()
            .position(|(code, _)| code == control)?;
        let anchor = self.report.number_anchors.get(at)?;
        let placement = anchor.placement_at((x_mm, y_mm));
        Some((placement.direction, placement.gap_mm))
    }

    /// The elements on the page, in paint order.
    pub fn elements(&self) -> Vec<ElementRect> {
        self.report
            .elements
            .iter()
            .map(|(id, rect)| ElementRect {
                kind: self
                    .kinds
                    .iter()
                    .find(|(known, _)| known == id)
                    .map_or("element", |(_, kind)| kind),
                id: id.clone(),
                rect_mm: [rect.min.0, rect.min.1, rect.max.0, rect.max.1],
            })
            .collect()
    }

    /// What a click at `(x_mm, y_mm)` (page mm, y down) picks, topmost first: a sheet element, a control number,
    /// a map object, a mask. `None` where nothing is.
    pub fn pick(&self, x_mm: f64, y_mm: f64) -> Option<Pick> {
        let p = (x_mm, y_mm);
        if !p.0.is_finite() || !p.1.is_finite() {
            return None;
        }
        if let Some((id, _)) = self
            .report
            .elements
            .iter()
            .rev()
            .find(|(_, r)| inside(*r, p))
        {
            return Some(Pick {
                kind: "element",
                id: id.clone(),
            });
        }
        let reach2 = NUMBER_REACH_MM * NUMBER_REACH_MM;
        if let Some((code, _)) = self
            .report
            .numbers
            .iter()
            .filter(|(_, c)| (c.0 - p.0).powi(2) + (c.1 - p.1).powi(2) <= reach2)
            .min_by(|(_, a), (_, b)| {
                let (da, db) = (
                    (a.0 - p.0).powi(2) + (a.1 - p.1).powi(2),
                    (b.0 - p.0).powi(2) + (b.1 - p.1).powi(2),
                );
                da.total_cmp(&db)
            })
        {
            return Some(Pick {
                kind: "number",
                id: code.clone(),
            });
        }
        if let Some(hit) = self.page.hits_at(p, MAP_REACH_MM).first()
            && let Some(position) = self.report.print.source_objects.get(hit.object as usize)
        {
            return Some(Pick {
                kind: "map_object",
                id: position.to_string(),
            });
        }
        let frame = self.page.clip;
        if inside(frame, p) {
            for (clip, id) in self.page.map_clips.iter().zip(&self.report.mask_ids) {
                if winding(clip, p) == 0 {
                    return Some(Pick {
                        kind: "mask",
                        id: id.clone(),
                    });
                }
            }
        }
        None
    }
}

fn inside(r: PaperBox, p: (f64, f64)) -> bool {
    r.min.0 <= p.0 && p.0 <= r.max.0 && r.min.1 <= p.1 && p.1 <= r.max.1
}

/// The winding number of the rings of `clip` around `p`; 0 where the mask hides the map.
fn winding(clip: &MapClip, p: (f64, f64)) -> i64 {
    let mut total = 0;
    for ring in clip.rings() {
        let n = ring.len();
        for i in 0..n {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            let side = (b.0 - a.0) * (p.1 - a.1) - (p.0 - a.0) * (b.1 - a.1);
            if a.1 <= p.1 {
                if b.1 > p.1 && side > 0.0 {
                    total += 1;
                }
            } else if b.1 <= p.1 && side < 0.0 {
                total -= 1;
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::testing::viktoriapark;

    fn luebeck() -> std::path::PathBuf {
        std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/examples/luebeck-competition-layouts"
        ))
    }

    #[test]
    fn a_layout_sheet_is_the_renderers_exact_svg() {
        let drawn = draw(&luebeck(), None, Some("a4-4000")).unwrap();
        let svg = drawn.svg().unwrap();
        assert_eq!(svg, write_svg(&drawn.page, &SvgView::default()).unwrap());
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        // A4 landscape: the viewBox is the page in mm.
        assert!(svg.contains("viewBox=\"0 0 297 210\""), "{}", &svg[..200]);
        assert_eq!(drawn.page.page_mm, (297.0, 210.0));
    }

    #[test]
    fn the_elements_come_with_kind_and_rectangle() {
        let drawn = draw(&luebeck(), None, Some("a4-4000")).unwrap();
        let elements = drawn.elements();
        let title = elements.iter().find(|e| e.id == "title").unwrap();
        assert_eq!(title.kind, "title_block");
        let [x0, y0, x1, y1] = title.rect_mm;
        assert!(x0 < x1 && y0 < y1 && x1 <= 297.0 && y1 <= 210.0);
        assert!(elements.iter().any(|e| e.kind == "footer"));
        // A click in the middle of the title picks the title.
        let pick = drawn.pick((x0 + x1) / 2.0, (y0 + y1) / 2.0).unwrap();
        assert_eq!((pick.kind, pick.id.as_str()), ("element", "title"));
    }

    #[test]
    fn a_click_on_the_map_picks_an_object_of_the_map() {
        let drawn = draw(&luebeck(), None, Some("a4-4000")).unwrap();
        let frame = drawn.page.clip;
        let mut found = None;
        // Probe the frame on a grid until something of the map answers.
        'scan: for i in 0..60 {
            for j in 0..40 {
                let x = frame.min.0 + (frame.max.0 - frame.min.0) * f64::from(i) / 60.0;
                let y = frame.min.1 + (frame.max.1 - frame.min.1) * f64::from(j) / 40.0;
                if let Some(pick) = drawn.pick(x, y)
                    && pick.kind == "map_object"
                {
                    found = Some(pick);
                    break 'scan;
                }
            }
        }
        let pick = found.expect("some map object lies in the frame");
        let position: usize = pick.id.parse().unwrap();
        assert!(drawn.report.print.source_objects.contains(&position));
        assert_eq!(drawn.pick(f64::NAN, 1.0), None);
        assert_eq!(drawn.pick(-5.0, -5.0), None);
    }

    #[test]
    fn a_course_sheet_has_numbers_and_the_assigned_layout() {
        let drawn = draw(&viktoriapark(), Some("Training"), None).unwrap();
        assert_eq!(drawn.sheet.course.as_deref(), Some("Training"));
        // A number that no sheet element covers is picked as a number, by its control code.
        let free = drawn
            .report
            .numbers
            .iter()
            .find(|(_, c)| {
                !drawn.report.elements.iter().any(|(_, r)| {
                    r.min.0 <= c.0 && c.0 <= r.max.0 && r.min.1 <= c.1 && c.1 <= r.max.1
                })
            })
            .expect("a control number lies clear of the elements");
        let pick = drawn.pick((free.1).0, (free.1).1).unwrap();
        assert_eq!((pick.kind, pick.id.as_str()), ("number", free.0.as_str()));
        // Without a course or layout the first course is drawn.
        let first = draw(&viktoriapark(), None, None).unwrap();
        assert!(first.sheet.course.is_some());
    }

    #[test]
    fn unknown_names_are_usage_errors() {
        assert!(matches!(
            draw(&viktoriapark(), Some("Nope"), None),
            Err(OpError::UnknownCourse { .. })
        ));
        assert!(matches!(
            draw(&viktoriapark(), None, Some("nope")),
            Err(OpError::UnknownLayout { .. })
        ));
    }

    #[test]
    fn a_project_without_courses_previews_its_first_layout() {
        let drawn = draw(&luebeck(), None, None).unwrap();
        assert_eq!(drawn.sheet.course, None);
        assert_eq!(drawn.sheet.layout, "a4-4000");
        // A course on a project without a course file is unknown.
        assert!(matches!(
            draw(&luebeck(), Some("Training"), None),
            Err(OpError::UnknownCourse { .. })
        ));
    }

    #[test]
    fn a_free_page_previews_at_its_size() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["luebeck.omap", "freemapper.toml"] {
            std::fs::copy(luebeck().join(name), dir.path().join(name)).unwrap();
        }
        std::fs::create_dir(dir.path().join("logos")).unwrap();
        for entry in std::fs::read_dir(luebeck().join("logos")).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_file() {
                std::fs::copy(
                    entry.path(),
                    dir.path().join("logos").join(entry.file_name()),
                )
                .unwrap();
            }
        }
        let text = std::fs::read_to_string(luebeck().join("freemapper-print.toml")).unwrap();
        let text = text.replacen(
            "paper = \"A4\"\norientation = \"landscape\"",
            "page_mm = [250.0, 400.0]\norientation = \"portrait\"",
            1,
        );
        assert!(text.contains("page_mm"));
        std::fs::write(dir.path().join("freemapper-print.toml"), text).unwrap();
        let drawn = draw(dir.path(), None, Some("a4-4000")).unwrap();
        assert_eq!(drawn.page.page_mm, (250.0, 400.0));
        assert!(drawn.svg().unwrap().contains("viewBox=\"0 0 250 400\""));
    }

    #[test]
    fn a_click_where_a_mask_hides_the_map_picks_the_mask() {
        let project = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/examples/luebeck-training-window"
        ));
        let drawn = draw(&project, Some("Fenster"), None).unwrap();
        assert_eq!(drawn.report.mask_ids, ["window-Fenster"]);
        let frame = drawn.page.clip;
        // Somewhere in the frame the window leaves the map hidden: that point picks the mask (or, over a
        // sheet element or a number, that one).
        let mut kinds = std::collections::BTreeSet::new();
        for i in 0..80 {
            for j in 0..50 {
                let x = frame.min.0 + (frame.max.0 - frame.min.0) * f64::from(i) / 80.0;
                let y = frame.min.1 + (frame.max.1 - frame.min.1) * f64::from(j) / 50.0;
                if let Some(pick) = drawn.pick(x, y) {
                    if pick.kind == "mask" {
                        assert_eq!(pick.id, "window-Fenster");
                    }
                    kinds.insert(pick.kind);
                }
            }
        }
        assert!(kinds.contains("mask"), "{kinds:?}");
        assert!(kinds.contains("map_object"), "{kinds:?}");
    }
}
