//! Sheets: one layout of the project applied to the map (design §4, §5.2).
//!
//! The map is drawn by [`render_page`] at the layout's print scale and clipped to the map frame,
//! the page minus the passe-partout. The sheet layer on top holds the frame line and the sheet
//! elements, each hung on an [`Anchor`](crate::project::Anchor) of the page or of the map frame.

pub mod layout;

use std::borrow::Cow;

use crate::omap::{MapCoord, MapDocument, OmapError};
use crate::print::{PrintError, PrintOptions, PrintReport, render_page};
use crate::project::{ElementKind, ElementSpec, LayoutSpec, LoadedProject};
use crate::render::page::{PaperBox, RenderedPage, SheetItem};

pub use layout::anchor_point;

/// What a sheet shows and what the user should know about it.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetReport {
    /// The map on the sheet: scale, factors, overflow against the map frame, skipped objects.
    pub print: PrintReport,
    pub notes: Vec<SheetNote>,
}

/// Something about a sheet the user should know. None of them stops the export.
#[derive(Debug, Clone, PartialEq)]
pub enum SheetNote {
    /// Part of the element lies outside the page and is cut off.
    ElementOutsidePage { id: String },
    /// An element hung on the page covers part of the map frame. Allowed; the element hides the
    /// map beneath it.
    ElementOverlapsMap { id: String },
    /// The paper is larger than A3, which most office printers do not take.
    LargerThanA3,
    /// No element of the layout shows the print scale or the contour interval.
    MissingScaleOrContour,
    /// The map had no north lines; this many were added (ADR 0023).
    NorthLinesAdded { count: usize },
    /// A logo was left out, and why.
    LogoSkipped { id: String, reason: String },
    /// A text names a placeholder that does not exist; it is printed as written.
    UnknownPlaceholder { id: String, name: String },
    /// `hide_parts` of the layout names a map part the map does not have.
    UnknownPart { layout: String, name: String },
}

#[derive(Debug, thiserror::Error)]
pub enum SheetError {
    #[error(transparent)]
    Print(#[from] PrintError),
    #[error("could not prepare the map for the sheet: {0}")]
    Map(#[from] OmapError),
}

/// What an element kind needs to draw itself.
#[derive(Debug, Clone, Copy)]
pub struct SheetContext<'a> {
    pub project: &'a LoadedProject,
    /// The map as drawn: hidden parts left out, generated north lines added.
    pub map: &'a MapDocument,
    pub layout: &'a LayoutSpec,
    /// The map on the page: print scale and factors.
    pub print: &'a PrintReport,
    /// The whole page (mm, origin top left).
    pub page: PaperBox,
    /// The map frame on the page.
    pub frame: PaperBox,
}

/// An element as drawn: its sheet items in paint order and the box they cover on the page.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedElement {
    pub items: Vec<SheetItem>,
    pub rect: PaperBox,
}

/// The page for `layout`: the map at the layout's print scale (the map scale when it has none),
/// clipped to the map frame, with the frame line and the sheet elements on top. `map` is the map
/// of `project`, read once by the caller for all layouts.
///
/// The map kind comes from the project, the symbols scale as ADR 0018 sets. Map parts named in
/// `hide_parts` are left out; sheet objects (`freemapper:source=sheet`) are not drawn, the sheet
/// elements take their place, while north lines always are (ADR 0023).
pub fn render_sheet(
    project: &LoadedProject,
    map: &MapDocument,
    layout: &LayoutSpec,
) -> Result<(RenderedPage, SheetReport), SheetError> {
    let mut notes = Vec::new();
    if layout::larger_than_a3(layout.paper) {
        notes.push(SheetNote::LargerThanA3);
    }
    let map = without_hidden_parts(map, layout, &mut notes);
    let map = with_north_lines(map, project, layout, &mut notes)?;

    let options = PrintOptions {
        print_scale: layout.print_scale,
        kind: project.project.map.kind,
        paper: layout.paper,
        orientation: layout.orientation,
        centre: layout.map_frame.centre_map_mm.map(map_coord),
        show_sheet_objects: false,
        frame_margin_mm: Some(layout.map_frame.margin_mm),
    };
    let (mut page, print) = render_page(&map, &options)?;
    let page_box = PaperBox {
        min: (0.0, 0.0),
        max: page.page_mm,
    };
    let frame = page.clip;
    page.sheet
        .extend(layout::frame_border(frame, layout.map_frame.border_mm));

    let ctx = SheetContext {
        project,
        map: &map,
        layout,
        print: &print,
        page: page_box,
        frame,
    };
    for spec in layout_elements(project, layout) {
        if let Some(placed) = place_element(&ctx, spec, &mut notes) {
            notes.extend(layout::bounds_notes(
                &spec.id,
                spec.anchor.to,
                placed.rect,
                page_box,
                frame,
            ));
            page.sheet.extend(placed.items);
        }
    }
    // Task 6 adds here what every sheet carries without an element: the credit line in its
    // default footer place when no `credit_line` element moved it, and `MissingScaleOrContour`
    // when no element shows `{print_scale}` or `{contour_interval}`.

    Ok((page, SheetReport { print, notes }))
}

/// The elements `layout` shows, in paint order: the shared ones it does not hide, then its own.
pub fn layout_elements<'a>(
    project: &'a LoadedProject,
    layout: &'a LayoutSpec,
) -> impl Iterator<Item = &'a ElementSpec> {
    project
        .project
        .shared
        .iter()
        .filter(|e| !layout.hide_shared.contains(&e.id))
        .chain(&layout.elements)
}

/// Draws one element. Each kind computes its size, asks [`anchor_point`] where its top-left
/// corner goes, and returns its items with the box they cover; `render_sheet` notes the box.
/// A kind that cannot draw (a logo file that will not load) adds its own note and returns `None`.
fn place_element(
    ctx: &SheetContext<'_>,
    spec: &ElementSpec,
    notes: &mut Vec<SheetNote>,
) -> Option<PlacedElement> {
    // Each arm is filled in by a later task of M2; until then the element draws nothing.
    let _ = (ctx, notes);
    match spec.kind {
        // Task 6: text, title block (placeholders, `UnknownPlaceholder`), credit and copyright
        // lines.
        ElementKind::TitleBlock
        | ElementKind::Text
        | ElementKind::CreditLine
        | ElementKind::CopyrightLine => None,
        // Task 7: the scale check bar.
        ElementKind::ScaleCheckBar => None,
        // Task 8: logos (`LogoSkipped`).
        ElementKind::Logo => None,
    }
}

/// `map` without the parts the layout hides, by exact name (case counts). A name that matches no
/// part gives [`SheetNote::UnknownPart`].
fn without_hidden_parts<'a>(
    map: &'a MapDocument,
    layout: &LayoutSpec,
    notes: &mut Vec<SheetNote>,
) -> Cow<'a, MapDocument> {
    for name in &layout.hide_parts {
        if !map.parts.iter().any(|p| &p.name == name) {
            notes.push(SheetNote::UnknownPart {
                layout: layout.id.clone(),
                name: name.clone(),
            });
        }
    }
    let hidden = |name: &String| layout.hide_parts.contains(name);
    if !map.parts.iter().any(|p| hidden(&p.name)) {
        return Cow::Borrowed(map);
    }
    let mut kept = map.clone();
    kept.parts.retain(|p| !hidden(&p.name));
    Cow::Owned(kept)
}

/// Task 9 adds generated north lines here when the map has none and the layout allows them
/// (`north_lines = "auto"`, ADR 0023): map objects of the map's 601 symbol, so that they sort by
/// its colour priority like drawn ones, and [`SheetNote::NorthLinesAdded`]. They are added
/// before drawing, after the hidden parts are gone. Without `centre_map_mm` the frame's place on
/// the map follows from what is drawn, so the lines have to span at least the drawn map plus
/// the frame size.
fn with_north_lines<'a>(
    map: Cow<'a, MapDocument>,
    project: &LoadedProject,
    layout: &LayoutSpec,
    notes: &mut Vec<SheetNote>,
) -> Result<Cow<'a, MapDocument>, SheetError> {
    let _ = (project, layout, notes);
    Ok(map)
}

/// A point in mm on the map sheet as map coordinates (µm). Far outside the `i32` range it
/// saturates at ±2147 m, kilometres off any page.
fn map_coord([x, y]: [f64; 2]) -> MapCoord {
    MapCoord::new((x * 1000.0).round() as i32, (y * 1000.0).round() as i32)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::omap::{MapGeoref, MapObject, MapObjectKind, MapPart, SymbolSet};
    use crate::print::{Orientation, Paper};
    use crate::project::{
        Anchor, AnchorTarget, Corner, Event, MapFrameSpec, MapSettings, NorthLines, PrintSettings,
        Project, SheetLang, Sources,
    };
    use crate::render::page::{DirectColour, Shape};
    use crate::scale::MapKind;

    fn doc(scale: u32, parts: Vec<(&str, Vec<MapObject>)>) -> MapDocument {
        MapDocument {
            notes: String::new(),
            georef: MapGeoref {
                scale_denominator: scale,
                declination_deg: 0.0,
                grivation_deg: 0.0,
                ref_projected: None,
                crs_spec: None,
            },
            symbol_set: SymbolSet::issprom_2019(),
            parts: parts
                .into_iter()
                .map(|(name, objects)| MapPart {
                    name: name.into(),
                    objects,
                })
                .collect(),
            templates: Vec::new(),
            unknown_objects: 0,
        }
    }

    /// An edge line (501.1, 0.1 mm wide) from the origin to `to` (µm).
    fn line(to: (i32, i32)) -> MapObject {
        MapObject {
            symbol_id: SymbolSet::issprom_2019().id_for_code("501.1").unwrap(),
            kind: MapObjectKind::Path,
            coords: vec![MapCoord::new(0, 0), MapCoord::new(to.0, to.1)],
            tags: Vec::new(),
            pattern: None,
        }
    }

    fn a4() -> LayoutSpec {
        LayoutSpec {
            id: "a4".into(),
            paper: Paper::A4,
            orientation: Orientation::Portrait,
            print_scale: None,
            map_frame: MapFrameSpec::default(),
            north_lines: NorthLines::Auto,
            hide_parts: Vec::new(),
            elements: Vec::new(),
            hide_shared: Vec::new(),
        }
    }

    fn project(kind: MapKind, shared: Vec<ElementSpec>, layout: &LayoutSpec) -> LoadedProject {
        LoadedProject {
            path: PathBuf::from("freemapper-print.toml"),
            dir: PathBuf::from("/nowhere"),
            project: Project {
                format: 1,
                event: Event {
                    title: "T".into(),
                    date: None,
                    organiser: None,
                    lang: SheetLang::De,
                },
                sources: Sources {
                    map: PathBuf::from("m.omap"),
                },
                map: MapSettings {
                    kind,
                    contour_interval_m: None,
                },
                print: PrintSettings::default(),
                shared,
                layouts: vec![layout.clone()],
            },
        }
    }

    fn sheet(map: &MapDocument, layout: &LayoutSpec) -> (RenderedPage, SheetReport) {
        render_sheet(&project(MapKind::Sprint, Vec::new(), layout), map, layout).unwrap()
    }

    #[test]
    fn map_frame_is_page_minus_margin() {
        // A 198 mm line fits in the 200 mm of the `render` command but not in a 194 mm frame.
        let map = doc(4000, vec![("default part", vec![line((198_000, 0))])]);
        let (page, report) = sheet(&map, &a4());
        let frame = PaperBox {
            min: (8.0, 8.0),
            max: (202.0, 289.0),
        };
        assert_eq!(page.clip, frame);
        assert_eq!(page.page_mm, (210.0, 297.0));
        // 198 mm plus half the 0.1 mm line width at each end, on 194 mm of frame.
        let (dx, dy) = report.print.overflow_mm.expect("overflows the frame");
        assert!((dx - 4.1).abs() < 1e-9 && dy == 0.0, "{dx} {dy}");
        // Centred in the frame.
        let Shape::Stroke { path, .. } = &page.primitives[0].shape else {
            panic!()
        };
        assert!((path[0].0 - 6.0).abs() < 1e-9, "{path:?}");
        assert!((path[0].1 - 148.5).abs() < 1e-9, "{path:?}");
    }

    #[test]
    fn frame_border_drawn_on_the_sheet_layer() {
        let map = doc(4000, vec![("default part", vec![line((50_000, 0))])]);
        let (page, _) = sheet(&map, &a4());
        let [SheetItem::Shape { colour, shape }] = &page.sheet[..] else {
            panic!("{:?}", page.sheet)
        };
        assert_eq!(*colour, DirectColour::BLACK);
        assert!(matches!(shape, Shape::Stroke { width, .. } if *width == 0.25));
        let mut layout = a4();
        layout.map_frame.border_mm = 0.0;
        let (page, _) = sheet(&map, &layout);
        assert!(page.sheet.is_empty());
    }

    #[test]
    fn centre_map_mm_lands_in_the_frame_centre() {
        let map = doc(4000, vec![("default part", vec![line((25_000, 0))])]);
        let mut layout = a4();
        layout.map_frame.centre_map_mm = Some([10.0, 0.0]);
        layout.map_frame.margin_mm = 20.0;
        let (page, _) = sheet(&map, &layout);
        let Shape::Stroke { path, .. } = &page.primitives[0].shape else {
            panic!()
        };
        // Map x = 10 mm sits at the frame centre, x = 105 mm on the page.
        assert!((path[0].0 - 95.0).abs() < 1e-9, "{path:?}");
        assert!((path[0].1 - 148.5).abs() < 1e-9, "{path:?}");
    }

    #[test]
    fn hide_parts_removes_part() {
        let map = doc(
            4000,
            vec![
                ("default part", vec![line((50_000, 0))]),
                (
                    "peripheral information",
                    vec![line((0, 50_000)), line((10_000, 10_000))],
                ),
            ],
        );
        let (page, report) = sheet(&map, &a4());
        assert_eq!(page.primitives.len(), 3);
        assert!(report.notes.is_empty(), "{:?}", report.notes);
        let mut layout = a4();
        layout.hide_parts = vec!["peripheral information".into()];
        let (page, report) = sheet(&map, &layout);
        assert_eq!(page.primitives.len(), 1);
        assert!(report.notes.is_empty(), "{:?}", report.notes);
    }

    #[test]
    fn unknown_part_noted() {
        let map = doc(4000, vec![("default part", vec![line((50_000, 0))])]);
        let mut layout = a4();
        // Part names match exactly, case included.
        layout.hide_parts = vec!["Default Part".into()];
        let (page, report) = sheet(&map, &layout);
        assert_eq!(page.primitives.len(), 1);
        assert_eq!(
            report.notes,
            [SheetNote::UnknownPart {
                layout: "a4".into(),
                name: "Default Part".into()
            }]
        );
    }

    #[test]
    fn layout_print_scale_and_kind() {
        // A forest map drawn at 1:10000, printed at 1:7500: geometry 10000/7500, symbols as the
        // 1:7500 size over the 1:10000 size, 2.0 / 1.5 (ADR 0018).
        let map = doc(10000, vec![("default part", vec![line((50_000, 0))])]);
        let mut layout = a4();
        layout.print_scale = Some(7500);
        let lp = project(MapKind::Forest, Vec::new(), &layout);
        let (_, report) = render_sheet(&lp, &map, &layout).unwrap();
        assert_eq!(report.print.print_scale, 7500);
        assert!((report.print.geometry_factor - 4.0 / 3.0).abs() < 1e-12);
        assert!((report.print.symbol_factor - 4.0 / 3.0).abs() < 1e-12);
        assert!(!report.print.non_standard_scale);
        // Without a print scale the map scale applies.
        let (_, report) = render_sheet(&lp, &map, &a4()).unwrap();
        assert_eq!(report.print.print_scale, 10000);
        assert_eq!(report.print.symbol_factor, 1.0);
        // The kind comes from the project: 1:7500 is no sprint scale.
        let sprint = project(MapKind::Sprint, Vec::new(), &layout);
        let err = render_sheet(&sprint, &map, &layout).unwrap_err();
        assert!(
            matches!(err, SheetError::Print(PrintError::Scale(_))),
            "{err}"
        );
    }

    #[test]
    fn luebeck_sheet_from_the_minimal_project() {
        let dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/projects/minimal");
        let lp = crate::project::load(&dir.join(crate::project::FILE_NAME)).unwrap();
        let map = crate::omap::read::read_map_file(&lp.map_path()).unwrap();
        let layout = &lp.project.layouts[0];
        let (page, report) = render_sheet(&lp, &map, layout).unwrap();
        assert_eq!(page.clip, crate::print::frame_box(page.page_mm, 8.0));
        assert_eq!(report.print.print_scale, 4000);
        assert_eq!(page.sheet.len(), 1, "the frame line");
        assert!(report.notes.is_empty(), "{:?}", report.notes);
        let (pdf, _) =
            crate::render::pdf::write_pdf(&page, &crate::render::pdf::PdfMeta::new("t")).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
    }

    fn element(id: &str) -> ElementSpec {
        ElementSpec {
            id: id.into(),
            kind: ElementKind::ScaleCheckBar,
            anchor: Anchor {
                to: AnchorTarget::Page,
                corner: Corner::BottomRight,
                offset_mm: [0.0, 0.0],
            },
            lines: Vec::new(),
            size_pt: None,
            box_white: None,
            file: None,
            width_mm: None,
        }
    }

    #[test]
    fn hide_shared_leaves_out_shared_elements() {
        let mut layout = a4();
        layout.elements = vec![element("own")];
        layout.hide_shared = vec!["b".into()];
        let lp = project(
            MapKind::Sprint,
            vec![element("a"), element("b"), element("c")],
            &layout,
        );
        let ids: Vec<&str> = layout_elements(&lp, &layout)
            .map(|e| e.id.as_str())
            .collect();
        assert_eq!(ids, ["a", "c", "own"]);
    }
}
