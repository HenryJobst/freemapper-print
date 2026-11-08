//! Sheets: one layout of the project applied to the map (design §4, §5.2).
//!
//! The map is drawn by [`render_page`] at the layout's print scale and clipped to the map frame,
//! the page minus the passe-partout. The sheet layer on top holds the frame line and the sheet
//! elements, each hung on an [`Anchor`](crate::project::Anchor) of the page or of the map frame.

pub mod layout;
pub mod placeholders;
mod text;

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
    /// No element of the layout shows the print scale or the contour interval, or the contour
    /// interval is not set. `missing` names what: `{print_scale}`, `{contour_interval}`,
    /// `map.contour_interval_m`.
    MissingScaleOrContour {
        layout: String,
        missing: Vec<String>,
    },
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
    page.sheet.extend(text::default_footer(&ctx));
    let missing = text::missing_scale_or_contour(&ctx);
    if !missing.is_empty() {
        notes.push(SheetNote::MissingScaleOrContour {
            layout: layout.id.clone(),
            missing,
        });
    }

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
    // Each arm after the text kinds is filled in by a later task of M2; until then the element
    // draws nothing.
    match spec.kind {
        ElementKind::TitleBlock
        | ElementKind::Text
        | ElementKind::CreditLine
        | ElementKind::CopyrightLine => text::place_text_element(ctx, spec, notes),
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
        let SheetItem::Shape { colour, shape } = &page.sheet[0] else {
            panic!("{:?}", page.sheet)
        };
        assert_eq!(*colour, DirectColour::BLACK);
        assert!(matches!(shape, Shape::Stroke { width, .. } if *width == 0.25));
        // Only the footer is left without the frame line.
        let mut layout = a4();
        layout.map_frame.border_mm = 0.0;
        let (page, _) = sheet(&map, &layout);
        assert!(
            page.sheet
                .iter()
                .all(|i| texts(std::slice::from_ref(i)).len() == 1)
        );
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
        assert_eq!(without_missing(&report.notes), []);
        let mut layout = a4();
        layout.hide_parts = vec!["peripheral information".into()];
        let (page, report) = sheet(&map, &layout);
        assert_eq!(page.primitives.len(), 1);
        assert_eq!(without_missing(&report.notes), []);
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
            without_missing(&report.notes),
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
        assert!(matches!(
            page.sheet[0],
            SheetItem::Shape {
                shape: Shape::Stroke { .. },
                ..
            }
        ));
        assert!(page.sheet.len() > 1, "the frame line and the footer");
        assert_eq!(
            report.notes,
            [SheetNote::MissingScaleOrContour {
                layout: "a4".into(),
                missing: vec![
                    "{print_scale}".into(),
                    "{contour_interval}".into(),
                    "map.contour_interval_m".into()
                ]
            }]
        );
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

    fn without_missing(notes: &[SheetNote]) -> Vec<SheetNote> {
        notes
            .iter()
            .filter(|n| !matches!(n, SheetNote::MissingScaleOrContour { .. }))
            .cloned()
            .collect()
    }

    type TextItem = (String, (f64, f64), f64, bool, DirectColour);

    /// The text items of a sheet layer: (text, origin, size mm, bold, colour).
    fn texts(items: &[SheetItem]) -> Vec<TextItem> {
        items
            .iter()
            .filter_map(|i| match i {
                SheetItem::Shape {
                    colour,
                    shape:
                        Shape::Text {
                            text,
                            origin,
                            size,
                            bold,
                            ..
                        },
                } => Some((text.clone(), *origin, *size, *bold, *colour)),
                _ => None,
            })
            .collect()
    }

    fn pt_mm(pt: f64) -> f64 {
        pt * 25.4 / 72.0
    }

    fn text_element(id: &str, kind: ElementKind, lines: &[&str], corner: Corner) -> ElementSpec {
        ElementSpec {
            kind,
            lines: lines.iter().map(|l| l.to_string()).collect(),
            anchor: Anchor {
                to: AnchorTarget::Page,
                corner,
                offset_mm: [2.0, 1.0],
            },
            ..element(id)
        }
    }

    fn sheet_of(lp: &LoadedProject, map: &MapDocument) -> (RenderedPage, SheetReport) {
        render_sheet(lp, map, &lp.project.layouts[0]).unwrap()
    }

    fn empty_map() -> MapDocument {
        doc(4000, vec![("default part", vec![line((50_000, 0))])])
    }

    #[test]
    fn title_block_layout() {
        use crate::render::fonts::{ASCENT, LINE_SPACING};
        let mut layout = a4();
        layout.elements = vec![text_element(
            "title",
            ElementKind::TitleBlock,
            &["{event}", "1:{print_scale}"],
            Corner::TopLeft,
        )];
        let lp = project(MapKind::Sprint, Vec::new(), &layout);
        let (page, report) = sheet_of(&lp, &empty_map());
        assert!(
            !report
                .notes
                .iter()
                .any(|n| matches!(n, SheetNote::UnknownPlaceholder { .. }))
        );
        // The frame line, a white box, two lines, then the footer.
        let SheetItem::Shape { colour, shape } = &page.sheet[1] else {
            panic!()
        };
        assert_eq!(*colour, DirectColour::WHITE);
        let Shape::Fill { rings } = shape else {
            panic!()
        };
        let (s1, s2) = (pt_mm(14.0), pt_mm(9.0));
        let h = s1 * LINE_SPACING + s2 * LINE_SPACING + 2.0;
        assert_eq!(rings[0][0], (2.0, 1.0));
        assert!((rings[0][2].1 - (1.0 + h)).abs() < 1e-9, "{rings:?}");
        let t = texts(&page.sheet);
        assert_eq!(t[0].0, "T");
        assert_eq!(t[1].0, "1:4000");
        assert!(t[0].3 && !t[1].3, "first line bold, others regular");
        assert_eq!((t[0].2, t[1].2), (s1, s2));
        assert!((t[0].1.0 - 3.0).abs() < 1e-9);
        assert!((t[0].1.1 - (2.0 + ASCENT * s1)).abs() < 1e-9);
        assert!((t[1].1.1 - (2.0 + s1 * LINE_SPACING + ASCENT * s2)).abs() < 1e-9);
        assert_eq!(t[0].4, DirectColour::BLACK);
    }

    #[test]
    fn text_element_and_box_white() {
        let mut spec = text_element("t", ElementKind::Text, &["x", "y"], Corner::BottomRight);
        spec.box_white = Some(false);
        spec.size_pt = Some(12.0);
        let mut layout = a4();
        layout.elements = vec![spec];
        let lp = project(MapKind::Sprint, Vec::new(), &layout);
        let (page, _) = sheet_of(&lp, &empty_map());
        let t = texts(&page.sheet);
        // No box: the frame line comes straight before the text; all lines regular at 12 pt.
        assert!(matches!(
            &page.sheet[1],
            SheetItem::Shape {
                shape: Shape::Text { .. },
                ..
            }
        ));
        assert!(!t[0].3 && !t[1].3);
        assert_eq!((t[0].2, t[1].2), (pt_mm(12.0), pt_mm(12.0)));
        // Right edge 2 mm in from the page edge.
        assert!(t[0].1.0 > 190.0 && t[0].1.0 < 208.0, "{:?}", t[0]);
    }

    #[test]
    fn unknown_placeholder_noted_and_kept() {
        let mut layout = a4();
        layout.elements = vec![text_element(
            "c",
            ElementKind::Text,
            &["Bahn {course}", "{layout}"],
            Corner::TopLeft,
        )];
        let lp = project(MapKind::Sprint, Vec::new(), &layout);
        let (page, report) = sheet_of(&lp, &empty_map());
        let t = texts(&page.sheet);
        assert_eq!(t[0].0, "Bahn {course}");
        assert_eq!(t[1].0, "a4");
        // (The element also covers part of the map: noted as well.)
        assert!(report.notes.contains(&SheetNote::UnknownPlaceholder {
            id: "c".into(),
            name: "course".into()
        }));
    }

    fn footer_texts(page: &RenderedPage) -> Vec<TextItem> {
        texts(&page.sheet)
    }

    #[test]
    fn credit_line_footer() {
        // No element in the project: the footer is there all the same.
        let lp = project(MapKind::Sprint, Vec::new(), &a4());
        let (page, _) = sheet_of(&lp, &empty_map());
        let t = footer_texts(&page);
        assert_eq!(t.len(), 1);
        let (text, origin, size, bold, colour) = &t[0];
        assert_eq!(
            text,
            "Erstellt mit freemapper-print – https://github.com/HenryJobst/freemapper-print"
        );
        assert!(text.contains('\u{2013}'));
        assert!(text.ends_with(env!("CARGO_PKG_REPOSITORY")));
        assert_eq!(*size, pt_mm(5.0));
        assert!(!bold);
        assert_eq!(*colour, DirectColour::BLACK);
        // Below the frame (bottom edge 289 mm), baseline 4 mm above the paper edge, left edge
        // at the frame's.
        assert_eq!(*origin, (8.0, 293.0));
        // English sheet.
        let mut en = lp.clone();
        en.project.event.lang = SheetLang::En;
        let (page, _) = sheet_of(&en, &empty_map());
        assert_eq!(
            footer_texts(&page)[0].0,
            "Created with freemapper-print – https://github.com/HenryJobst/freemapper-print"
        );
    }

    #[test]
    fn credit_line_inside_the_frame_when_the_margin_is_small() {
        use crate::render::fonts::{ASCENT, LINE_SPACING};
        let mut layout = a4();
        layout.map_frame.margin_mm = 6.0;
        let lp = project(MapKind::Sprint, Vec::new(), &layout);
        let (page, _) = sheet_of(&lp, &empty_map());
        let SheetItem::Shape {
            colour,
            shape: Shape::Fill { rings },
        } = &page.sheet[1]
        else {
            panic!("{:?}", page.sheet)
        };
        assert_eq!(*colour, DirectColour::WHITE);
        // Frame: 6..204 × 6..291. The box sits in its bottom left corner.
        assert_eq!(rings[0][0].0, 6.0);
        assert!((rings[0][2].1 - 291.0).abs() < 1e-9, "{rings:?}");
        let s = pt_mm(5.0);
        let top = 291.0 - (s * LINE_SPACING + 1.6);
        assert!((rings[0][0].1 - top).abs() < 1e-9);
        let t = footer_texts(&page);
        assert!((t[0].1.0 - 6.8).abs() < 1e-9);
        assert!((t[0].1.1 - (top + 0.8 + ASCENT * s)).abs() < 1e-9);
    }

    #[test]
    fn tall_footer_goes_inside_the_frame() {
        // Four lines do not fit into the 8 mm below the frame: the block moves onto a white box
        // inside the frame instead of overlapping its line.
        let mut map = empty_map();
        map.notes = "a\nb\nc".into();
        let lp = project(MapKind::Sprint, Vec::new(), &a4());
        let (page, _) = sheet_of(&lp, &map);
        let SheetItem::Shape {
            colour,
            shape: Shape::Fill { rings },
        } = &page.sheet[1]
        else {
            panic!("{:?}", page.sheet)
        };
        assert_eq!(*colour, DirectColour::WHITE);
        assert!((rings[0][2].1 - 289.0).abs() < 1e-9);
        assert!(footer_texts(&page).iter().all(|t| t.1.1 < 289.0));
    }

    #[test]
    fn credit_line_element_only_moves_it() {
        let mut spec = text_element("credit", ElementKind::CreditLine, &[], Corner::BottomRight);
        spec.size_pt = Some(7.0);
        spec.anchor.offset_mm = [3.0, 2.0];
        let mut layout = a4();
        layout.elements = vec![spec];
        let lp = project(MapKind::Sprint, Vec::new(), &layout);
        let (page, _) = sheet_of(&lp, &empty_map());
        let t = footer_texts(&page);
        // Exactly one credit line, at the element's place and size.
        assert_eq!(t.len(), 1);
        assert!(t[0].0.starts_with("Erstellt mit freemapper-print"));
        assert_eq!(t[0].2, pt_mm(7.0));
        assert!(t[0].1.0 > 100.0, "{:?}", t[0]);
    }

    #[test]
    fn copyright_from_notes() {
        let mut map = empty_map();
        map.notes = "© OpenStreetMap contributors, ODbL\n\n  \nCreated with freemapper – https://github.com/HenryJobst/freemapper\n".into();
        // A wide passe-partout, so that the whole block fits below the frame.
        let mut layout = a4();
        layout.map_frame.margin_mm = 15.0;
        let lp = project(MapKind::Sprint, Vec::new(), &layout);
        let (page, _) = sheet_of(&lp, &map);
        let t = footer_texts(&page);
        let names: Vec<&str> = t.iter().map(|x| x.0.as_str()).collect();
        assert_eq!(names[0], "© OpenStreetMap contributors, ODbL");
        assert_eq!(
            names[1],
            "Created with freemapper – https://github.com/HenryJobst/freemapper"
        );
        assert!(names[2].starts_with("Erstellt mit freemapper-print"));
        assert_eq!(names.len(), 3);
        // One stacked block: same size, same left edge, the credit line at the bottom.
        assert!(t.iter().all(|x| x.2 == pt_mm(5.0) && x.1.0 == 15.0));
        assert!((t[2].1.1 - 293.0).abs() < 1e-9);
        let step = pt_mm(5.0) * crate::render::fonts::LINE_SPACING;
        assert!((t[1].1.1 - (293.0 - step)).abs() < 1e-9);
        assert!((t[0].1.1 - (293.0 - 2.0 * step)).abs() < 1e-9);

        // `print.copyright` replaces the notes.
        let mut own = lp.clone();
        own.project.print.copyright = Some("Karte: Ich\nDaten: Du".into());
        let (page, _) = sheet_of(&own, &map);
        let names: Vec<String> = footer_texts(&page).into_iter().map(|x| x.0).collect();
        assert_eq!(&names[..2], ["Karte: Ich", "Daten: Du"]);
        assert_eq!(names.len(), 3);
    }

    #[test]
    fn copyright_line_element_moves_the_copyright_part() {
        let mut map = empty_map();
        map.notes = "Quelle".into();
        let mut layout = a4();
        layout.elements = vec![text_element(
            "src",
            ElementKind::CopyrightLine,
            &[],
            Corner::TopRight,
        )];
        let lp = project(MapKind::Sprint, Vec::new(), &layout);
        let (page, _) = sheet_of(&lp, &map);
        let t = footer_texts(&page);
        assert_eq!(t.len(), 2);
        let src = t.iter().find(|x| x.0 == "Quelle").unwrap();
        assert!(src.1.1 < 20.0 && src.1.0 > 100.0, "{src:?}");
        let credit = t.iter().find(|x| x.0.starts_with("Erstellt")).unwrap();
        assert_eq!(credit.1, (8.0, 293.0));
    }

    #[test]
    fn missing_scale_or_contour_noted() {
        let mut layout = a4();
        layout.elements = vec![text_element(
            "s",
            ElementKind::Text,
            &["1:{print_scale}"],
            Corner::TopLeft,
        )];
        let mut lp = project(MapKind::Sprint, Vec::new(), &layout);
        let (_, report) = sheet_of(&lp, &empty_map());
        let missing = |r: &SheetReport| {
            r.notes.iter().find_map(|n| match n {
                SheetNote::MissingScaleOrContour { missing, .. } => Some(missing.clone()),
                _ => None,
            })
        };
        assert_eq!(
            missing(&report).unwrap(),
            ["{contour_interval}", "map.contour_interval_m"]
        );
        // Both shown and the interval set: nothing to note.
        lp.project.layouts[0].elements[0]
            .lines
            .push("{contour_interval}".into());
        lp.project.map.contour_interval_m = Some(2.5);
        let (page, report) = sheet_of(&lp, &empty_map());
        assert_eq!(missing(&report), None);
        let t = texts(&page.sheet);
        assert_eq!((t[0].0.as_str(), t[1].0.as_str()), ("1:4000", "2,5 m"));
        // A shared element counts unless the layout hides it.
        lp.project.layouts[0].elements.clear();
        lp.project.shared = vec![text_element(
            "sh",
            ElementKind::Text,
            &["1:{print_scale} {contour_interval}"],
            Corner::TopLeft,
        )];
        let (_, report) = sheet_of(&lp, &empty_map());
        assert_eq!(missing(&report), None);
        lp.project.layouts[0].hide_shared = vec!["sh".into()];
        let (_, report) = sheet_of(&lp, &empty_map());
        assert_eq!(
            missing(&report).unwrap(),
            ["{print_scale}", "{contour_interval}"]
        );
    }
}
