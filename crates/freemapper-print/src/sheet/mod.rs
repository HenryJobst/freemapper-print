//! Sheets: one layout of the project applied to the map (design §4, §5.2).
//!
//! The map is drawn by [`render_page`] at the layout's print scale and clipped to the map frame,
//! the page minus the passe-partout. The sheet layer on top holds the frame line and the sheet
//! elements, each hung on an [`Anchor`](crate::project::Anchor) of the page or of the map frame.

pub(crate) mod course_sheet;
pub mod layout;
mod logo;
mod north_lines;
pub mod placeholders;
mod reasons;
mod scale_bar;
mod text;

use std::borrow::Cow;
use std::path::PathBuf;

use crate::course::{CourseNote, FitNote, Stale};
use crate::omap::{MapCoord, MapDocument, OmapError};
use crate::print::Orientation;
use crate::print::{PrintError, PrintOptions, PrintReport, render_page};
use crate::project::{
    AnchorTarget, ElementKind, ElementSpec, LayoutSpec, LoadedProject, NorthLines, SheetLang,
};
use crate::render::page::{DirectColour, PaperBox, RenderedPage, Shape, SheetItem};
use crate::render::symbols::{Cap, Join};

pub use course_sheet::{
    course_centre, course_event_notes, course_file_stem, course_length_m, render_course_sheet,
};
pub use layout::anchor_point;
pub use logo::{LogoBudget, LogoCache, MAX_PAGE_LOGO_PIXELS, MAX_PAGE_SVG_UNITS};
pub use reasons::{LogoSimplification, LogoSkipReason, MissingItem};
pub use text::FOOTER_ID;

/// What a sheet shows and what the user should know about it.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetReport {
    /// The map on the sheet: scale, factors, overflow against the map frame, skipped objects.
    pub print: PrintReport,
    pub notes: Vec<SheetNote>,
}

impl SheetReport {
    /// Whether the user should look at the sheet: any note of the print report, or any sheet
    /// note that is not [`SheetNote::is_info`]. `check` fails on this.
    pub fn needs_attention(&self) -> bool {
        self.print.has_notes() || self.notes.iter().any(|n| !n.is_info())
    }
}

/// What the default footer holds: an element can move either part out of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FooterContent {
    CopyrightAndCredit,
    Copyright,
    Credit,
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
    /// interval is not set. `missing` names what.
    MissingScaleOrContour {
        layout: String,
        missing: Vec<MissingItem>,
    },
    /// The default footer, holding `content`, covers part of the map frame: the passe-partout
    /// is too narrow for it, so it sits on its white box inside the frame.
    FooterOverlapsMap { content: FooterContent },
    /// The default footer, holding `content`, reaches past the page edge.
    FooterOutsidePage { content: FooterContent },
    /// The default footer, holding `content`, covers part of the element `id`.
    FooterCoversElement { id: String, content: FooterContent },
    /// The layout shows no `scale_check_bar` element. The design puts one on every sheet by
    /// default (§5.4); `init` adds it, and the note makes a missing one visible.
    MissingScaleCheckBar { layout: String },
    /// The layout crops the map on purpose (`map_frame.crop`): the map reaches this far (mm, in x
    /// and in y) past the map frame. Informs only; the print report then has no overflow.
    MapCropped { overflow_mm: (f64, f64) },
    /// The map had no north lines; this many were added (ADR 0023, ADR 0027). They run
    /// unbroken across the frame: breaking them at small features such as boulders and path ends
    /// is the mapper's job in OOM, and the program cannot know where.
    NorthLinesAdded { count: usize },
    /// The map had no north lines and its symbol set has no visible line symbol 601 or 601.x either, so this
    /// many plain black lines of 0.14 mm were drawn over the map, clipped to the map frame. Lines
    /// on the sheet layer lie over the map objects instead of under them (ADR 0027).
    NorthLinesWithoutSymbol { count: usize },
    /// The map notes (or `print.copyright`) have more lines than the footer shows: `shown`
    /// sources of `total` are printed, the rest counted in a last line.
    CopyrightTruncated {
        layout: String,
        total: usize,
        shown: usize,
    },
    /// A copyright line is wider than the footer's white box inside the map frame and is cut,
    /// ending in "…". `line` is the line as written.
    CopyrightLineCut { line: String },
    /// A logo was left out, and why. `file` is the logo file, resolved against the project.
    LogoSkipped {
        id: String,
        file: Option<PathBuf>,
        reason: LogoSkipReason,
    },
    /// An SVG logo uses something the sheet cannot draw as written: a gradient, transparency,
    /// dashes or a clip path. `reason` says what and how it is drawn instead. One note per kind
    /// and logo.
    LogoSimplified {
        id: String,
        reason: LogoSimplification,
    },
    /// A text names a placeholder that does not exist; it is printed as written.
    UnknownPlaceholder { id: String, name: String },
    /// `hide_parts` of the layout names a map part the map does not have.
    UnknownPart { layout: String, name: String },
    /// The `credit_line` element would not lie wholly on the page, so the credit line stays in
    /// the default footer (ADR 0029).
    CreditLineMoved { id: String },
    /// These controls of the course (codes, each once, in course order) lie outside the map frame, wholly or in
    /// part: their circles are cut off or missing.
    CourseOutsideFrame {
        course: String,
        controls: Vec<String>,
    },
    /// The course has no assignment in the project file and prints on the first layout. Informs only.
    DefaultLayout { course: String, layout: String },
    /// The course sheet has no control description box.
    NoDescriptionBox { course: String },
    /// A limit ([`crate::overprint::MAX_REPEATS`]) left out hatch lines, dashes or crosses of the course
    /// overprint.
    OverprintCut { course: String },
    /// The text `id` uses the placeholder `name` (`climb_m` or `length_km`), but the course file gives no value
    /// for the course, so it prints empty. Informs only.
    CourseValueUnknown {
        course: String,
        id: String,
        name: String,
    },
    /// What the course reader found but does not support.
    Course(CourseNote),
    /// What fitting the course file to the map found out.
    Fit(FitNote),
    /// An entry of the project file that no longer matches the course file.
    Stale(Stale),
}

impl SheetNote {
    /// Whether the note only informs: it does not count toward `check`'s exit code, and the CLI
    /// marks it "Info:" instead of "Note:".
    pub fn is_info(&self) -> bool {
        match self {
            SheetNote::NorthLinesAdded { .. }
            | SheetNote::MapCropped { .. }
            | SheetNote::DefaultLayout { .. }
            | SheetNote::CourseValueUnknown { .. } => true,
            // A course note informs unless something of the course file is left out of the print.
            SheetNote::Course(note) => !matches!(
                note,
                CourseNote::WhiteOutLaterInM4
                    | CourseNote::AlternativeControls(_)
                    | CourseNote::Variations(_)
                    | CourseNote::UnknownElement(_)
            ),
            SheetNote::Fit(note) => !note.is_warning(),
            SheetNote::Stale(stale) => !stale.is_warning(),
            _ => false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SheetError {
    #[error(transparent)]
    Print(#[from] PrintError),
    #[error("could not prepare the map for the sheet")]
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
    /// The logos of the export or `check` run, decoded once.
    pub logos: &'a LogoCache,
    /// What the logos of this sheet have used of its budget.
    pub logo_budget: &'a LogoBudget,
    /// The course placeholders of a course sheet; `None` on a layout sheet.
    pub course: Option<&'a placeholders::CourseValues>,
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
/// elements take their place, while north lines always are (ADR 0023). Logos come from `logos`,
/// which the sheets of one run share.
pub fn render_sheet(
    project: &LoadedProject,
    map: &MapDocument,
    layout: &LayoutSpec,
    logos: &LogoCache,
) -> Result<(RenderedPage, SheetReport), SheetError> {
    render_body(project, map, layout, logos, None, None)
}

/// The sheet of [`render_sheet`], also for a course sheet: `course` fills the course placeholders, and `centre`
/// (map coordinates) goes to the centre of the map frame when the layout sets no `centre_map_mm`.
fn render_body(
    project: &LoadedProject,
    map: &MapDocument,
    layout: &LayoutSpec,
    logos: &LogoCache,
    course: Option<&placeholders::CourseValues>,
    centre: Option<MapCoord>,
) -> Result<(RenderedPage, SheetReport), SheetError> {
    let mut notes = Vec::new();
    if layout::larger_than_a3(layout.paper) {
        notes.push(SheetNote::LargerThanA3);
    }
    let original = map;
    let map = without_hidden_parts(map, layout, &mut notes);

    let mut options = PrintOptions {
        print_scale: layout.print_scale,
        kind: project.project.map.kind,
        paper: layout.paper,
        orientation: layout.orientation,
        centre: layout
            .map_frame
            .centre_map_mm
            .map(oom_display_to_map)
            .or(centre),
        show_sheet_objects: false,
        frame_margin_mm: Some(layout.map_frame.margin_mm),
    };
    let (mut page, mut print) = render_page(&map, &options)?;
    if layout.north_lines == NorthLines::Auto && !north_lines::has_north_lines(original) {
        let spacing_m = project.rules.north_line_spacing_m(project.project.map.kind);
        add_north_lines(
            &map,
            spacing_m,
            &mut options,
            &mut page,
            &mut print,
            &mut notes,
        )?;
    }
    if layout.map_frame.crop
        && let Some(overflow_mm) = print.overflow_mm.take()
    {
        notes.push(SheetNote::MapCropped { overflow_mm });
    }
    let page_box = PaperBox {
        min: (0.0, 0.0),
        max: page.page_mm,
    };
    let frame = page.clip;
    page.sheet
        .extend(layout::frame_border(frame, layout.map_frame.border_mm));

    let logo_budget = LogoBudget::default();
    let ctx = SheetContext {
        project,
        map: &map,
        layout,
        print: &print,
        page: page_box,
        frame,
        logos,
        logo_budget: &logo_budget,
        course,
    };
    // The credit line goes on top of everything else, so no element can hide it.
    let mut credit_lines = Vec::new();
    // Where each element lies, for the footer's overlap check.
    let mut placed_rects: Vec<(&str, PaperBox)> = Vec::new();
    for spec in layout_elements(project, layout) {
        let Some(placed) = place_element(&ctx, spec, &mut notes) else {
            continue;
        };
        if spec.kind == ElementKind::CreditLine {
            if layout::inside(placed.rect, page_box) {
                placed_rects.push((&spec.id, placed.rect));
                credit_lines.push((spec, placed));
            } else {
                notes.push(SheetNote::CreditLineMoved {
                    id: spec.id.clone(),
                });
            }
            continue;
        }
        notes.extend(layout::bounds_notes(
            &spec.id,
            spec.anchor.to,
            placed.rect,
            page_box,
            frame,
        ));
        placed_rects.push((&spec.id, placed.rect));
        page.sheet.extend(placed.items);
    }
    let credit_in_footer = credit_lines.is_empty();
    if let Some((footer, content)) = text::default_footer(&ctx, credit_in_footer, &mut notes) {
        // Hung on the page, so that a footer inside the frame counts as covering the map.
        let bounds = layout::bounds_notes(
            text::FOOTER_ID,
            AnchorTarget::Page,
            footer.rect,
            page_box,
            frame,
        );
        notes.extend(bounds.into_iter().map(|n| match n {
            SheetNote::ElementOverlapsMap { .. } => SheetNote::FooterOverlapsMap { content },
            SheetNote::ElementOutsidePage { .. } => SheetNote::FooterOutsidePage { content },
            other => other,
        }));
        for (id, rect) in &placed_rects {
            if layout::overlap(footer.rect, *rect) {
                notes.push(SheetNote::FooterCoversElement {
                    id: (*id).to_string(),
                    content,
                });
            }
        }
        page.sheet.extend(footer.items);
    }
    for (spec, credit) in credit_lines {
        // Wholly on the page; over the map is the user's choice, noted as for any element.
        notes.extend(layout::bounds_notes(
            &spec.id,
            spec.anchor.to,
            credit.rect,
            page_box,
            frame,
        ));
        page.sheet.extend(credit.items);
    }
    if !layout_elements(project, layout).any(|e| e.kind == ElementKind::ScaleCheckBar) {
        notes.push(SheetNote::MissingScaleCheckBar {
            layout: layout.id.clone(),
        });
    }
    let missing = text::missing_scale_or_contour(&ctx);
    if !missing.is_empty() {
        notes.push(SheetNote::MissingScaleOrContour {
            layout: layout.id.clone(),
            missing,
        });
    }

    Ok((page, SheetReport { print, notes }))
}

/// Adds the default footer to `page`, a page drawn without a project (the `render` command): the
/// copyright lines from `map.notes` and the credit line in `lang`, placed against the page's frame
/// as on a sheet (ADR 0029). With `render`'s 5 mm margin it sits on a white box in the bottom-left
/// corner of the frame. The map does not move. Notes: [`SheetNote::CopyrightTruncated`] (with the
/// layout `""`), and [`SheetNote::FooterOverlapsMap`] when the box covers part of what the map
/// draws.
pub fn add_map_footer(
    page: &mut RenderedPage,
    map: &MapDocument,
    lang: SheetLang,
) -> Vec<SheetNote> {
    let mut notes = Vec::new();
    let page_box = PaperBox {
        min: (0.0, 0.0),
        max: page.page_mm,
    };
    if let Some((footer, content)) =
        text::map_footer(&map.notes, lang, page_box, page.clip, &mut notes)
    {
        let drawn = crate::render::bounds::painted_bounds(&page.primitives);
        let covers = drawn.is_some_and(|(min, max)| {
            footer.rect.min.0 < max.0
                && min.0 < footer.rect.max.0
                && footer.rect.min.1 < max.1
                && min.1 < footer.rect.max.1
        });
        if covers {
            notes.push(SheetNote::FooterOverlapsMap { content });
        }
        page.sheet.extend(footer.items);
    }
    notes
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
    match spec.kind {
        ElementKind::TitleBlock
        | ElementKind::Text
        | ElementKind::CreditLine
        | ElementKind::CopyrightLine => text::place_text_element(ctx, spec, notes),
        ElementKind::ScaleCheckBar => Some(scale_bar::place_scale_bar(ctx, spec)),
        ElementKind::Logo => logo::place_logo(ctx, spec, notes),
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

/// North lines for a map that has none (ADR 0023, ADR 0027), over the frame of `page`, the
/// first drawing of `map`. `page` and `print` are replaced by the redrawn ones or extended.
///
/// With a symbol 601 (or 601.1, or the lowest 601.x) the lines become objects of that symbol in
/// a part of their own, so they sort by its colour priority like drawn ones; the map is then
/// drawn again with the orientation and the centre of the first drawing fixed, so that the lines
/// change neither the page nor the overflow report. Without any such symbol, plain 0.14 mm black
/// lines go on the sheet layer of `page`, within the frame.
fn add_north_lines(
    map: &MapDocument,
    spacing_m: f64,
    options: &mut PrintOptions,
    page: &mut RenderedPage,
    print: &mut PrintReport,
    notes: &mut Vec<SheetNote>,
) -> Result<(), SheetError> {
    let gf = print.geometry_factor;
    // A symbol that paints a line (visible, type line or combined), else the plain fallback.
    let symbol = north_lines::symbol_id(map.symbol_set.codes(), |id| {
        north_lines::draws_a_line(&map.symbol_set, id)
    });
    let half_width = match symbol {
        Some(id) => north_lines::half_width_mm(&map.symbol_set, id, print.symbol_factor),
        None => north_lines::FALLBACK_WIDTH_MM * print.symbol_factor / 2.0,
    };
    let inset = north_lines::inset_mm(half_width);
    let frame = north_lines::frame_in_map_mm(page.clip, page.page_mm, page.centre_mm, gf, inset);
    let spacing = north_lines::spacing_mm(spacing_m, map.georef.scale_denominator);
    let xs = north_lines::line_xs_mm(spacing, frame.min.0, frame.max.0);
    if xs.is_empty() {
        return Ok(());
    }
    let Some(symbol) = symbol else {
        // Page x of map x: the page centre shows `centre_mm`.
        let page_x = |x: f64| page.page_mm.0 / 2.0 + x * gf - page.centre_mm.0;
        let (top, bottom) = (page.clip.min.1 + inset, page.clip.max.1 - inset);
        // Symbols scale by the symbol factor (ADR 0018), and so does the plain line.
        let width = north_lines::FALLBACK_WIDTH_MM * print.symbol_factor;
        let lines = xs.iter().map(|x| SheetItem::Shape {
            colour: DirectColour::BLACK,
            shape: Shape::Stroke {
                path: vec![(page_x(*x), top), (page_x(*x), bottom)],
                closed: false,
                width,
                cap: Cap::Flat,
                join: Join::Miter,
            },
        });
        let lines: Vec<SheetItem> = lines.collect();
        page.sheet.extend(lines);
        notes.push(SheetNote::NorthLinesWithoutSymbol { count: xs.len() });
        return Ok(());
    };
    let mut with = map.clone();
    with.parts
        .push(north_lines::part(symbol, &xs, frame.min.1, frame.max.1));
    options.orientation = if page.page_mm.0 > page.page_mm.1 {
        Orientation::Landscape
    } else {
        Orientation::Portrait
    };
    let centre = map_coord([page.centre_mm.0 / gf, page.centre_mm.1 / gf]);
    options.centre = Some(centre);
    notes.push(SheetNote::NorthLinesAdded { count: xs.len() });
    (*page, *print) = render_page(&with, options)?;
    Ok(())
}

/// `centre_map_mm` as OOM's status bar shows map coordinates (mm, y pointing up) to the file's
/// map coordinates (y pointing down): OOM's `MapWidget::updateCursorposLabel` prints `x` and
/// `-y` (ADR 0028).
fn oom_display_to_map([x, y]: [f64; 2]) -> MapCoord {
    map_coord([x, -y])
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
                combined_scale_factor: 1.0,
                map_ref_mm: (0.0, 0.0),
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

    pub(super) fn a4() -> LayoutSpec {
        LayoutSpec {
            id: "a4".into(),
            paper: Paper::A4,
            orientation: Orientation::Portrait,
            print_scale: None,
            map_frame: MapFrameSpec::default(),
            // Off here, so that tests of other things see only the objects they set up.
            north_lines: NorthLines::Never,
            hide_parts: Vec::new(),
            elements: Vec::new(),
            hide_shared: Vec::new(),
        }
    }

    pub(super) fn project(
        kind: MapKind,
        shared: Vec<ElementSpec>,
        layout: &LayoutSpec,
    ) -> LoadedProject {
        LoadedProject {
            path: PathBuf::from("freemapper-print.toml"),
            dir: PathBuf::from("/nowhere"),
            rules: crate::rules::base().clone(),
            project: Project {
                format: 1,
                event: Event {
                    title: "T".into(),
                    date: None,
                    organiser: None,
                    lang: SheetLang::De,
                },
                sources: Sources::new(PathBuf::from("m.omap")),
                map: MapSettings {
                    kind,
                    contour_interval_m: None,
                },
                print: PrintSettings::default(),
                shared,
                layouts: vec![layout.clone()],
                assignments: Vec::new(),
                numbers: Vec::new(),
            },
        }
    }

    fn sheet(map: &MapDocument, layout: &LayoutSpec) -> (RenderedPage, SheetReport) {
        render_sheet(
            &project(MapKind::Sprint, Vec::new(), layout),
            map,
            layout,
            &LogoCache::new(),
        )
        .unwrap()
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
        // Only the footer, its white box and its line, is left without the frame line.
        let mut layout = a4();
        layout.map_frame.border_mm = 0.0;
        let (page, _) = sheet(&map, &layout);
        assert_eq!(page.sheet.len(), 2, "{:?}", page.sheet);
        assert!(matches!(
            page.sheet[0],
            SheetItem::Shape {
                colour: DirectColour::WHITE,
                shape: Shape::Fill { .. }
            }
        ));
        assert_eq!(texts(&page.sheet[1..]).len(), 1);
    }

    #[test]
    fn intended_crop_turns_overflow_into_information() {
        // 198 mm of line on a 194 mm frame: 4.1 mm too wide.
        let map = doc(4000, vec![("default part", vec![line((198_000, 0))])]);
        let mut layout = a4();
        layout.elements = vec![element("bar")];
        layout.map_frame.crop = true;
        let mut lp = project(MapKind::Sprint, Vec::new(), &layout);
        lp.project.map.contour_interval_m = Some(2.0);
        lp.project.layouts[0].elements.push(text_element(
            "t",
            ElementKind::Text,
            &["1:{print_scale} {contour_interval}"],
            Corner::TopLeft,
        ));
        let (page, report) = sheet_of(&lp, &map);
        assert_eq!(report.print.overflow_mm, None);
        let Some(SheetNote::MapCropped {
            overflow_mm: (dx, dy),
        }) = report
            .notes
            .iter()
            .find(|n| matches!(n, SheetNote::MapCropped { .. }))
        else {
            panic!("{:?}", report.notes)
        };
        assert!((dx - 4.1).abs() < 1e-9 && *dy == 0.0);
        assert!(!report.needs_attention(), "{:?}", report.notes);
        // The page is the same as without crop.
        lp.project.layouts[0].map_frame.crop = false;
        let (plain, uncropped) = sheet_of(&lp, &map);
        assert_eq!(page.primitives, plain.primitives);
        assert!(uncropped.needs_attention());
        assert!(uncropped.print.overflow_mm.is_some());
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
    fn centre_map_mm_takes_oom_coordinates_with_y_up() {
        // The file stores y pointing down: the line runs from the origin 40 mm down. OOM's
        // status bar shows that end as y = -40 (y up), and so does `centre_map_mm`.
        let map = doc(4000, vec![("default part", vec![line((0, 40_000))])]);
        let mut layout = a4();
        layout.map_frame.centre_map_mm = Some([0.0, -40.0]);
        let (page, _) = sheet(&map, &layout);
        let Shape::Stroke { path, .. } = &page.primitives[0].shape else {
            panic!()
        };
        // The lower end sits at the frame centre (105, 148.5), the origin 40 mm above it.
        assert!((path[1].0 - 105.0).abs() < 1e-9, "{path:?}");
        assert!((path[1].1 - 148.5).abs() < 1e-9, "{path:?}");
        assert!((path[0].1 - 108.5).abs() < 1e-9, "{path:?}");
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
        let (_, report) = render_sheet(&lp, &map, &layout, &LogoCache::new()).unwrap();
        assert_eq!(report.print.print_scale, 7500);
        assert!((report.print.geometry_factor - 4.0 / 3.0).abs() < 1e-12);
        assert!((report.print.symbol_factor - 4.0 / 3.0).abs() < 1e-12);
        assert!(!report.print.non_standard_scale);
        // Without a print scale the map scale applies.
        let (_, report) = render_sheet(&lp, &map, &a4(), &LogoCache::new()).unwrap();
        assert_eq!(report.print.print_scale, 10000);
        assert_eq!(report.print.symbol_factor, 1.0);
        // A print scale of 0, which the loader refuses, fails here too instead of drawing.
        let mut zero = layout.clone();
        zero.print_scale = Some(0);
        let err = render_sheet(&lp, &map, &zero, &LogoCache::new()).unwrap_err();
        assert!(
            matches!(err, SheetError::Print(PrintError::Scale(_))),
            "{err}"
        );
        // The kind comes from the project: 1:7500 is no sprint scale.
        let sprint = project(MapKind::Sprint, Vec::new(), &layout);
        let err = render_sheet(&sprint, &map, &layout, &LogoCache::new()).unwrap_err();
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
        let (page, report) = render_sheet(&lp, &map, layout, &LogoCache::new()).unwrap();
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
        // Luebeck's notes have several lines: the footer sits on the map.
        assert_eq!(
            report.notes,
            [
                SheetNote::FooterOverlapsMap {
                    content: FooterContent::CopyrightAndCredit
                },
                SheetNote::MissingScaleCheckBar {
                    layout: "a4".into()
                },
                SheetNote::MissingScaleOrContour {
                    layout: "a4".into(),
                    missing: vec![
                        MissingItem::PrintScaleText,
                        MissingItem::ContourText,
                        MissingItem::ContourValue
                    ]
                }
            ]
        );
        let (pdf, _) =
            crate::render::pdf::write_pdf(&page, &crate::render::pdf::PdfMeta::new("t")).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
    }

    pub(super) fn element(id: &str) -> ElementSpec {
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
            .filter(|n| {
                !matches!(
                    n,
                    SheetNote::MissingScaleOrContour { .. }
                        | SheetNote::MissingScaleCheckBar { .. }
                )
            })
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

    pub(super) fn sheet_of(lp: &LoadedProject, map: &MapDocument) -> (RenderedPage, SheetReport) {
        render_sheet(lp, map, &lp.project.layouts[0], &LogoCache::new()).unwrap()
    }

    pub(super) fn empty_map() -> MapDocument {
        doc(4000, vec![("default part", vec![line((50_000, 0))])])
    }

    #[test]
    fn scale_bar_uses_print_scale_and_anchor() {
        let mut layout = a4();
        layout.print_scale = Some(7500);
        layout.elements = vec![element("bar")];
        let lp = project(MapKind::Forest, Vec::new(), &layout);
        // Map scale 1:15000, print scale 1:7500: the bar follows the print scale.
        let map = doc(15000, vec![("default part", vec![line((50_000, 0))])]);
        let (page, report) = sheet_of(&lp, &map);
        assert_eq!(texts(&page.sheet)[0].0, "250 m = 33,3 mm");
        assert_eq!(report.print.print_scale, 7500);
        // Bottom right of the page, 0 offset: the white box ends at the paper corner.
        let SheetItem::Shape {
            shape: Shape::Fill { rings, .. },
            ..
        } = &page.sheet[1]
        else {
            panic!("white box")
        };
        let (w, h) = page.page_mm;
        assert!((rings[0][2].0 - w).abs() < 1e-9 && (rings[0][2].1 - h).abs() < 1e-9);
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
        let Shape::Fill { rings, .. } = shape else {
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
        // On a white box that keeps off the frame line's outer half (289.125 mm).
        let Some(SheetItem::Shape {
            colour: DirectColour::WHITE,
            shape: Shape::Fill { rings, .. },
        }) = page.sheet.iter().rev().nth(1)
        else {
            panic!("{:?}", page.sheet)
        };
        assert!(rings[0].iter().all(|p| p.1 >= 289.125), "{rings:?}");
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
            shape: Shape::Fill { rings, .. },
        } = &page.sheet[1]
        else {
            panic!("{:?}", page.sheet)
        };
        assert_eq!(*colour, DirectColour::WHITE);
        // Frame: 6..204 × 6..291. The box sits in its bottom left corner.
        assert_eq!(rings[0][0].0, 6.125);
        assert!((rings[0][2].1 - 290.875).abs() < 1e-9, "{rings:?}");
        let s = pt_mm(5.0);
        let top = 290.875 - (s * LINE_SPACING + 1.6);
        assert!((rings[0][0].1 - top).abs() < 1e-9);
        let t = footer_texts(&page);
        assert!((t[0].1.0 - 6.925).abs() < 1e-9);
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
            shape: Shape::Fill { rings, .. },
        } = &page.sheet[1]
        else {
            panic!("{:?}", page.sheet)
        };
        assert_eq!(*colour, DirectColour::WHITE);
        assert!((rings[0][2].1 - 288.875).abs() < 1e-9);
        assert!(footer_texts(&page).iter().all(|t| t.1.1 < 289.0));
    }

    #[test]
    fn copyright_lines_capped() {
        let mut map = empty_map();
        map.notes = (1..=200).map(|i| format!("Quelle {i}\n")).collect();
        let lp = project(MapKind::Sprint, Vec::new(), &a4());
        let (page, report) = sheet_of(&lp, &map);
        let t = footer_texts(&page);
        // Five sources, the line for the rest, the credit line.
        assert_eq!(t.len(), 7);
        assert_eq!(t[4].0, "Quelle 5");
        assert_eq!(t[5].0, "… und 195 weitere Quellen");
        assert!(report.notes.contains(&SheetNote::CopyrightTruncated {
            layout: "a4".into(),
            total: 200,
            shown: 5
        }));
        let mut en = lp.clone();
        en.project.event.lang = SheetLang::En;
        let (page, _) = sheet_of(&en, &map);
        assert_eq!(footer_texts(&page)[5].0, "… and 195 more sources");
        // Exactly six lines are shown as they are.
        map.notes = (1..=6).map(|i| format!("Q{i}\n")).collect();
        let (page, report) = sheet_of(&lp, &map);
        assert_eq!(footer_texts(&page).len(), 7);
        assert!(
            !report
                .notes
                .iter()
                .any(|n| matches!(n, SheetNote::CopyrightTruncated { .. }))
        );
        // The box stays inside the page.
        assert!(
            !report
                .notes
                .iter()
                .any(|n| matches!(n, SheetNote::ElementOutsidePage { .. }))
        );
    }

    #[test]
    fn long_copyright_line_cut_to_the_frame() {
        use crate::render::fonts::{FontFace, text_width};
        let mut map = empty_map();
        map.notes = "x".repeat(300);
        let lp = project(MapKind::Sprint, Vec::new(), &a4());
        let (page, _) = sheet_of(&lp, &map);
        let t = footer_texts(&page);
        assert!(t[0].0.ends_with('…') && t[0].0.len() < 300, "{}", t[0].0);
        let w = text_width(&t[0].0, FontFace::Regular) * pt_mm(5.0);
        // The frame (194 mm) less the box padding and the frame line (0.25 mm).
        assert!(w <= 194.0 - 1.6 - 0.25 && w > 180.0, "{w}");
        // The credit line stays whole.
        assert!(t[1].0.ends_with("freemapper-print"));
        let (_, report) = sheet_of(&lp, &map);
        assert!(report.notes.contains(&SheetNote::CopyrightLineCut {
            line: "x".repeat(300)
        }));
        // A line that fits gives no note.
        map.notes = "x".repeat(30);
        let (_, report) = sheet_of(&lp, &map);
        assert!(
            !report
                .notes
                .iter()
                .any(|n| matches!(n, SheetNote::CopyrightLineCut { .. }))
        );
    }

    #[test]
    fn footer_over_the_map_noted() {
        let mut map = empty_map();
        map.notes = "a\nb\nc".into();
        let lp = project(MapKind::Sprint, Vec::new(), &a4());
        let (_, report) = sheet_of(&lp, &map);
        assert!(report.notes.contains(&SheetNote::FooterOverlapsMap {
            content: FooterContent::CopyrightAndCredit
        }));
        // Below the frame: nothing to note.
        let (_, report) = sheet_of(&lp, &empty_map());
        assert!(
            !report
                .notes
                .iter()
                .any(|n| matches!(n, SheetNote::ElementOverlapsMap { .. }))
        );
    }

    #[test]
    fn footer_box_keeps_off_the_frame_line() {
        let mut map = empty_map();
        map.notes = "a\nb\nc".into();
        let lp = project(MapKind::Sprint, Vec::new(), &a4());
        let (page, _) = sheet_of(&lp, &map);
        let SheetItem::Shape {
            shape: Shape::Fill { rings, .. },
            ..
        } = &page.sheet[1]
        else {
            panic!()
        };
        // Frame 8..202 × 8..289, line 0.25 mm wide.
        assert!((rings[0][0].0 - 8.125).abs() < 1e-9);
        assert!((rings[0][2].1 - 288.875).abs() < 1e-9);
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
    fn credit_line_off_the_page_falls_back_to_the_footer() {
        let mut spec = text_element("credit", ElementKind::CreditLine, &[], Corner::BottomRight);
        spec.anchor.offset_mm = [5000.0, 5000.0];
        let mut layout = a4();
        layout.elements = vec![spec];
        let lp = project(MapKind::Sprint, Vec::new(), &layout);
        let (page, report) = sheet_of(&lp, &empty_map());
        let t = footer_texts(&page);
        assert_eq!(t.len(), 1);
        assert!(t[0].0.starts_with("Erstellt mit freemapper-print"));
        // The default place: below the frame, left-aligned with it.
        assert_eq!(t[0].1, (8.0, 293.0));
        assert!(report.notes.contains(&SheetNote::CreditLineMoved {
            id: "credit".into()
        }));
        assert!(
            !report
                .notes
                .iter()
                .any(|n| matches!(n, SheetNote::ElementOutsidePage { .. })),
            "{:?}",
            report.notes
        );
    }

    #[test]
    fn credit_line_partly_off_the_page_falls_back_too() {
        let mut spec = text_element("credit", ElementKind::CreditLine, &[], Corner::TopLeft);
        spec.anchor.offset_mm = [-3.0, 2.0];
        let mut layout = a4();
        layout.elements = vec![spec];
        let lp = project(MapKind::Sprint, Vec::new(), &layout);
        let (page, report) = sheet_of(&lp, &empty_map());
        assert_eq!(footer_texts(&page)[0].1, (8.0, 293.0));
        assert!(report.notes.contains(&SheetNote::CreditLineMoved {
            id: "credit".into()
        }));
    }

    #[test]
    fn tiny_credit_line_drawn_at_five_pt() {
        // The project loader refuses less than 5 pt; a spec built by hand is drawn at 5 pt.
        let mut spec = text_element("credit", ElementKind::CreditLine, &[], Corner::BottomRight);
        spec.size_pt = Some(0.001);
        let mut layout = a4();
        layout.elements = vec![spec];
        let lp = project(MapKind::Sprint, Vec::new(), &layout);
        let (page, _) = sheet_of(&lp, &empty_map());
        assert_eq!(footer_texts(&page)[0].2, pt_mm(5.0));
    }

    /// A layout with a 15 mm passe-partout, one copyright source and a black 150 mm logo in the
    /// page's bottom-left corner, listed before `extra`; and the logo file's folder.
    fn black_logo_sheet(extra: Vec<ElementSpec>) -> (RenderedPage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let svg = dir.path().join("black.svg");
        std::fs::write(
            &svg,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="20"><rect width="100" height="20" fill="#000"/></svg>"##,
        )
        .unwrap();
        let logo = ElementSpec {
            kind: ElementKind::Logo,
            anchor: Anchor {
                to: AnchorTarget::Page,
                corner: Corner::BottomLeft,
                offset_mm: [0.0, 0.0],
            },
            file: Some(svg),
            width_mm: Some(150.0),
            ..element("blk")
        };
        let mut layout = a4();
        layout.map_frame.margin_mm = 15.0;
        layout.elements = std::iter::once(logo).chain(extra).collect();
        let mut lp = project(MapKind::Sprint, Vec::new(), &layout);
        lp.project.print.copyright = Some("© OSM".into());
        let (page, _) = sheet_of(&lp, &empty_map());
        (page, dir)
    }

    /// Indices in `page.sheet` of: the last black fill (the logo), the white fill under the credit
    /// line's origin, and the credit line's text.
    fn credit_paint_order(page: &RenderedPage) -> (usize, usize, usize) {
        let credit = page
            .sheet
            .iter()
            .position(|i| {
                matches!(i, SheetItem::Shape { shape: Shape::Text { text, .. }, .. }
                    if text.starts_with("Erstellt mit freemapper-print"))
            })
            .expect("credit line");
        let SheetItem::Shape {
            shape: Shape::Text { origin, .. },
            ..
        } = &page.sheet[credit]
        else {
            unreachable!()
        };
        let under = |i: &SheetItem| match i {
            SheetItem::Shape {
                colour: DirectColour::WHITE,
                shape: Shape::Fill { rings, .. },
            } => {
                let xs = rings[0].iter().map(|p| p.0);
                let ys = rings[0].iter().map(|p| p.1);
                let (x0, x1) = (
                    xs.clone().fold(f64::MAX, f64::min),
                    xs.fold(f64::MIN, f64::max),
                );
                let (y0, y1) = (
                    ys.clone().fold(f64::MAX, f64::min),
                    ys.fold(f64::MIN, f64::max),
                );
                (x0..=x1).contains(&origin.0) && (y0..=y1).contains(&origin.1)
            }
            _ => false,
        };
        let white = page.sheet[..credit]
            .iter()
            .rposition(under)
            .expect("a white box under the credit line");
        let logo = page
            .sheet
            .iter()
            .rposition(|i| {
                matches!(i, SheetItem::Shape { colour, shape: Shape::Fill { .. } }
                    if *colour == DirectColour::BLACK)
            })
            .expect("the black logo");
        (logo, white, credit)
    }

    #[test]
    fn credit_line_on_white_over_a_dark_element() {
        // The default footer below the frame, over a black logo listed before it.
        let (page, _dir) = black_logo_sheet(Vec::new());
        let (logo, white, credit) = credit_paint_order(&page);
        assert!(logo < white && white < credit, "{logo} {white} {credit}");
        // A moved credit line over the logo gets its own white box.
        let mut moved = text_element("credit", ElementKind::CreditLine, &[], Corner::BottomLeft);
        moved.anchor.offset_mm = [20.0, 5.0];
        let (page, _dir) = black_logo_sheet(vec![moved]);
        let (logo, white, credit) = credit_paint_order(&page);
        assert!(logo < white && white < credit, "{logo} {white} {credit}");
    }

    #[test]
    fn credit_line_painted_after_every_other_item() {
        // A white-box text listed after the credit line and lying over it: the credit line still
        // comes last in the paint order.
        let credit = text_element("credit", ElementKind::CreditLine, &[], Corner::BottomLeft);
        let mut cover = text_element(
            "cover",
            ElementKind::Text,
            &["xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"],
            Corner::BottomLeft,
        );
        cover.size_pt = Some(20.0);
        let mut layout = a4();
        layout.elements = vec![credit, cover];
        let lp = project(MapKind::Sprint, Vec::new(), &layout);
        let (page, _) = sheet_of(&lp, &empty_map());
        let last = page.sheet.last().unwrap();
        let SheetItem::Shape {
            shape: Shape::Text { text, .. },
            ..
        } = last
        else {
            panic!("{last:?}")
        };
        assert!(text.starts_with("Erstellt mit freemapper-print"), "{text}");
        // The default footer is painted last as well, after a covering element.
        let lp = project(MapKind::Sprint, Vec::new(), &a4());
        let (page, _) = sheet_of(&lp, &empty_map());
        assert!(matches!(
            page.sheet.last(),
            Some(SheetItem::Shape {
                shape: Shape::Text { text, .. },
                ..
            }) if text.starts_with("Erstellt mit")
        ));
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
            [MissingItem::ContourText, MissingItem::ContourValue]
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
            [MissingItem::PrintScaleText, MissingItem::ContourText]
        );
    }

    #[test]
    fn footer_notes_name_what_the_footer_holds() {
        // Three sources do not fit below the 8 mm frame: the footer covers the map.
        let mut map = empty_map();
        map.notes = "a\nb\nc".into();
        let overlaps = |lp: &LoadedProject, map: &MapDocument| {
            let (_, report) = sheet_of(lp, map);
            report.notes.into_iter().find_map(|n| match n {
                SheetNote::FooterOverlapsMap { content } => Some(content),
                _ => None,
            })
        };
        let lp = project(MapKind::Sprint, Vec::new(), &a4());
        assert_eq!(overlaps(&lp, &map), Some(FooterContent::CopyrightAndCredit));
        // The credit line moved away: only the copyright lines are left.
        let mut credit = text_element("credit", ElementKind::CreditLine, &[], Corner::TopRight);
        credit.anchor.offset_mm = [2.0, 2.0];
        let mut moved = a4();
        moved.elements = vec![credit];
        let lp = project(MapKind::Sprint, Vec::new(), &moved);
        assert_eq!(overlaps(&lp, &map), Some(FooterContent::Copyright));
        // The copyright lines moved away: only the credit line, inside a 6 mm margin.
        let mut layout = a4();
        layout.map_frame.margin_mm = 6.0;
        layout.elements = vec![text_element(
            "src",
            ElementKind::CopyrightLine,
            &[],
            Corner::TopRight,
        )];
        let lp = project(MapKind::Sprint, Vec::new(), &layout);
        assert_eq!(overlaps(&lp, &map), Some(FooterContent::Credit));
        // No old-style footer notes.
        let (_, report) = sheet_of(&lp, &map);
        assert!(!report.notes.iter().any(|n| matches!(n,
            SheetNote::ElementOverlapsMap { id } | SheetNote::ElementOutsidePage { id }
                if id == FOOTER_ID)));
    }

    #[test]
    fn footer_covering_an_element_noted() {
        // Three source lines do not fit below the frame: the footer goes into its bottom-left
        // corner, where the bar hangs too.
        let mut map = empty_map();
        map.notes = "a\nb\nc".into();
        let mut bar = element("bar");
        bar.anchor = Anchor {
            to: AnchorTarget::MapFrame,
            corner: Corner::BottomLeft,
            offset_mm: [2.0, 2.0],
        };
        let mut layout = a4();
        layout.elements = vec![bar.clone()];
        let lp = project(MapKind::Sprint, Vec::new(), &layout);
        let (_, report) = sheet_of(&lp, &map);
        let note = SheetNote::FooterCoversElement {
            id: "bar".into(),
            content: FooterContent::CopyrightAndCredit,
        };
        assert!(report.notes.contains(&note), "{:?}", report.notes);
        // In the other corner, or with the footer below the frame: nothing to note.
        let mut right = lp.clone();
        right.project.layouts[0].elements[0].anchor.corner = Corner::BottomRight;
        let (_, report) = sheet_of(&right, &map);
        assert!(!report.notes.contains(&note));
        let (_, report) = sheet_of(&lp, &empty_map());
        assert!(!report.notes.contains(&note));
    }

    #[test]
    fn missing_scale_check_bar_noted() {
        let note = SheetNote::MissingScaleCheckBar {
            layout: "a4".into(),
        };
        let lp = project(MapKind::Sprint, Vec::new(), &a4());
        let (_, report) = sheet_of(&lp, &empty_map());
        assert!(report.notes.contains(&note), "{:?}", report.notes);
        assert!(report.needs_attention());
        // A shared bar counts, unless the layout hides it; so does the layout's own.
        let mut lp = project(MapKind::Sprint, vec![element("bar")], &a4());
        let (_, report) = sheet_of(&lp, &empty_map());
        assert!(!report.notes.contains(&note));
        lp.project.layouts[0].hide_shared = vec!["bar".into()];
        let (_, report) = sheet_of(&lp, &empty_map());
        assert!(report.notes.contains(&note));
        lp.project.layouts[0].elements = vec![element("own")];
        let (_, report) = sheet_of(&lp, &empty_map());
        assert!(!report.notes.contains(&note));
    }

    // --- North lines (Task 9, ADR 0027) ---

    fn auto() -> LayoutSpec {
        LayoutSpec {
            north_lines: NorthLines::Auto,
            ..a4()
        }
    }

    /// An object of the symbol with `code` from (0, 0) to `to` (µm).
    fn object_of(code: &str, to: (i32, i32)) -> MapObject {
        MapObject {
            symbol_id: SymbolSet::issprom_2019().id_for_code(code).unwrap(),
            ..line(to)
        }
    }

    /// x of every vertical two-point stroke, ascending.
    fn vertical_xs(shapes: impl Iterator<Item = Shape>) -> Vec<f64> {
        let mut xs: Vec<f64> = shapes
            .filter_map(|s| match s {
                Shape::Stroke { path, .. }
                    if path.len() == 2
                        && (path[0].0 - path[1].0).abs() < 1e-9
                        && path[0].1 != path[1].1 =>
                {
                    Some(path[0].0)
                }
                _ => None,
            })
            .collect();
        xs.sort_by(f64::total_cmp);
        xs
    }

    fn map_xs(page: &RenderedPage) -> Vec<f64> {
        vertical_xs(page.primitives.iter().map(|p| p.shape.clone()))
    }

    fn sheet_xs(page: &RenderedPage) -> Vec<f64> {
        vertical_xs(page.sheet.iter().filter_map(|i| match i {
            SheetItem::Shape { shape, .. } => Some(shape.clone()),
            _ => None,
        }))
    }

    fn gaps(xs: &[f64]) -> Vec<f64> {
        xs.windows(2).map(|w| w[1] - w[0]).collect()
    }

    fn added(report: &SheetReport) -> Option<usize> {
        report.notes.iter().find_map(|n| match n {
            SheetNote::NorthLinesAdded { count } => Some(*count),
            _ => None,
        })
    }

    fn no_lines_map() -> MapDocument {
        doc(4000, vec![("default part", vec![line((50_000, 0))])])
    }

    #[test]
    fn adds_lines_when_missing() {
        let (page, report) = sheet(&no_lines_map(), &auto());
        let xs = map_xs(&page);
        assert!(xs.len() >= 5, "{xs:?}");
        assert_eq!(added(&report), Some(xs.len()));
        // 120 m at 1:4000 is 30 mm on paper, and the lines sit on the map grid from x = 0.
        assert!(gaps(&xs).iter().all(|g| (g - 30.0).abs() < 1e-6), "{xs:?}");
        // Map x = 0 is at page x = 105 - 25 (the map is centred on its drawn extent, 0 to 50 mm;
        // the centre comes from the first pass, which has no lines), so the lines are 30 mm apart from x = 0 at page x 80.
        assert!(xs.iter().any(|x| (x - 80.0).abs() < 1e-6), "{xs:?}");
        // Every line lies in the frame.
        assert!(xs.iter().all(|x| (8.0..=202.0).contains(x)), "{xs:?}");
        // The lines span the frame in height and add no overflow or other page change.
        let (_, never) = sheet(&no_lines_map(), &a4());
        assert_eq!(report.print.overflow_mm, never.print.overflow_mm);
        assert_eq!(report.print.page_mm, never.print.page_mm);
    }

    #[test]
    fn lines_keep_the_ground_spacing_at_another_print_scale() {
        let mut layout = auto();
        layout.print_scale = Some(3000);
        let (page, _) = sheet(&no_lines_map(), &layout);
        let xs = map_xs(&page);
        assert!(xs.len() >= 4, "{xs:?}");
        assert!(gaps(&xs).iter().all(|g| (g - 40.0).abs() < 1e-6), "{xs:?}");
    }

    #[test]
    fn lines_follow_the_projects_rule_set() {
        // A test-only overlay sets the sprint spacing to 60 m: 15 mm at 1:4000.
        let overlay = "format = 1\n[profile]\nid = \"test-north\"\nkind = \"overlay\"\n\
                       base = \"iof-print\"\nname = { en = \"Test\" }\n[sources.T]\ntitle = \"T\"\n\
                       [[rules]]\nid = \"iofprint.north_lines.sprint\"\nkey = \"north_lines\"\n\
                       severity = \"hint\"\nmap_kinds = [\"sprint\"]\n\
                       values = { spacing_metres = 60.0 }\nsource = { doc = \"T\", section = \"1\" }\n";
        let files = [
            ("iof-print", crate::rules::embedded_text("iof-print")),
            ("test-north", overlay),
        ];
        let layout = auto();
        let mut lp = project(MapKind::Sprint, Vec::new(), &layout);
        lp.rules =
            crate::rules::load_for_tests(&files, "iof-print", &["test-north".to_string()]).unwrap();
        let (page, _) = render_sheet(&lp, &no_lines_map(), &layout, &LogoCache::new()).unwrap();
        let xs = map_xs(&page);
        assert!(xs.len() >= 5, "{xs:?}");
        assert!(gaps(&xs).iter().all(|g| (g - 15.0).abs() < 1e-6), "{xs:?}");
    }

    #[test]
    fn lines_span_the_frame_around_centre_map_mm() {
        let mut layout = auto();
        layout.map_frame.centre_map_mm = Some([400.0, 0.0]);
        let (page, report) = sheet(&no_lines_map(), &layout);
        let xs = map_xs(&page);
        // Map x = 400 mm is at page x = 105; 390 / 30 = 13: a line at map x = 390 mm.
        assert!(xs.iter().any(|x| (x - 95.0).abs() < 1e-6), "{xs:?}");
        assert!(xs.len() >= 5 && xs.iter().all(|x| (8.0..=202.0).contains(x)));
        assert_eq!(added(&report), Some(xs.len()));
    }

    #[test]
    fn hostile_centre_keeps_the_count_small() {
        let mut layout = auto();
        layout.map_frame.centre_map_mm = Some([1e15, -1e15]);
        let (page, report) = sheet(&no_lines_map(), &layout);
        // Coordinates saturate far out; only distinct, drawn lines are counted.
        let xs = map_xs(&page);
        assert_eq!(added(&report).unwrap_or(0), xs.len());
        assert!(xs.len() < 100);
        assert!(xs.windows(2).all(|w| w[1] > w[0]), "{xs:?}");
    }

    #[test]
    fn keeps_existing_lines() {
        let map = doc(
            4000,
            vec![("default part", vec![object_of("601", (0, 50_000))])],
        );
        let (page, report) = sheet(&map, &auto());
        assert_eq!(added(&report), None);
        assert_eq!(map_xs(&page).len(), 1);
    }

    #[test]
    fn older_maps_keep_their_sheet_tagged_north_lines() {
        // freemapper before v0.5.0 put its north lines in "peripheral information", tagged as
        // sheet objects. They still count as the map's own lines and are drawn.
        let tagged = |x: i32| MapObject {
            coords: vec![MapCoord::new(x, 0), MapCoord::new(x, 50_000)],
            tags: vec![("freemapper:source".into(), "sheet".into())],
            ..object_of("601", (0, 50_000))
        };
        let map = doc(
            4000,
            vec![
                ("default part", vec![line((50_000, 0))]),
                ("peripheral information", vec![tagged(0), tagged(30_000)]),
            ],
        );
        let (page, report) = sheet(&map, &auto());
        assert_eq!(added(&report), None);
        assert!(
            !report
                .notes
                .iter()
                .any(|n| matches!(n, SheetNote::NorthLinesWithoutSymbol { .. }))
        );
        assert_eq!(map_xs(&page).len(), 2);
        assert!(sheet_xs(&page).is_empty());
    }

    #[test]
    fn variant_of_601_counts_as_north_line() {
        let map = doc(
            4000,
            vec![("default part", vec![object_of("601.5", (0, 50_000))])],
        );
        let (_, report) = sheet(&map, &auto());
        assert_eq!(added(&report), None);
    }

    #[test]
    fn hidden_north_line_part_not_replaced() {
        let map = doc(
            4000,
            vec![
                ("default part", vec![line((50_000, 0))]),
                (
                    "peripheral information",
                    vec![object_of("601", (0, 50_000))],
                ),
            ],
        );
        let mut layout = auto();
        layout.hide_parts = vec!["peripheral information".into()];
        let (page, report) = sheet(&map, &layout);
        assert_eq!(added(&report), None);
        assert!(map_xs(&page).is_empty());
        assert!(sheet_xs(&page).is_empty());
    }

    #[test]
    fn never_setting_adds_none() {
        let (page, report) = sheet(&no_lines_map(), &a4());
        assert_eq!(added(&report), None);
        assert!(map_xs(&page).is_empty() && sheet_xs(&page).is_empty());
    }

    #[test]
    fn uses_map_symbol_priority() {
        // A drawn 601 gives the priority the generated lines must have.
        let reference = doc(
            4000,
            vec![("default part", vec![object_of("601", (0, 50_000))])],
        );
        let (ref_page, _) = sheet(&reference, &a4());
        let want: Vec<i32> = ref_page.primitives.iter().map(|p| p.priority).collect();
        let (page, _) = sheet(&no_lines_map(), &auto());
        let vertical: Vec<i32> = page
            .primitives
            .iter()
            .filter(|p| !vertical_xs(std::iter::once(p.shape.clone())).is_empty())
            .map(|p| p.priority)
            .collect();
        assert!(!vertical.is_empty());
        assert!(
            vertical.iter().all(|p| *p == want[0]),
            "{vertical:?} {want:?}"
        );
    }

    #[test]
    fn falls_back_to_601_1_then_the_lowest_variant() {
        let pick = |codes: &[&str], usable: fn(u32) -> bool| {
            north_lines::symbol_id(
                codes.iter().enumerate().map(|(i, c)| (*c, i as u32)),
                usable,
            )
        };
        let all = |_: u32| true;
        assert_eq!(pick(&["601.4", "601", "601.1"], all), Some(1));
        assert_eq!(pick(&["601.4", "601.1"], all), Some(1));
        assert_eq!(pick(&["601.10", "601.4", "601.9"], all), Some(1));
        assert_eq!(pick(&["501", "602"], all), None);
        // Unusable candidates are passed over, in the same order.
        assert_eq!(pick(&["601", "601.1", "601.4"], |i| i != 0), Some(1));
        assert_eq!(pick(&["601", "601.1", "601.4"], |i| i == 2), Some(2));
        assert_eq!(pick(&["601", "601.1"], |_| false), None);
    }

    fn luebeck() -> MapDocument {
        crate::omap::read::read_map_file(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/maps/luebeck.omap"
        )))
        .unwrap()
    }

    fn vertical_count(page: &RenderedPage) -> usize {
        map_xs(page).len()
    }

    #[test]
    fn luebeck_keeps_its_tagged_north_lines() {
        // Since freemapper v0.5.0 its nine 601 objects lie in the default part, tagged
        // `freemapper:source=north-lines`, and count as the map's own lines.
        let map = luebeck();
        let tagged = |part: &str| {
            map.parts
                .iter()
                .filter(|p| p.name == part)
                .flat_map(|p| &p.objects)
                .filter(|o| {
                    o.tags
                        .iter()
                        .any(|(k, v)| k == "freemapper:source" && v == "north-lines")
                        && map.symbol_set.code_of(o.symbol_id) == Some("601")
                })
                .count()
        };
        assert_eq!(tagged("default part"), 9);
        assert_eq!(tagged("peripheral information"), 0);
        let (_, report) = sheet(&map, &auto());
        assert_eq!(added(&report), None);
        assert!(
            !report
                .notes
                .iter()
                .any(|n| matches!(n, SheetNote::NorthLinesWithoutSymbol { .. }))
        );
    }

    #[test]
    fn luebeck_without_north_objects_gets_visible_lines() {
        let mut map = luebeck();
        let set = map.symbol_set.clone();
        for part in &mut map.parts {
            part.objects.retain(|o| {
                !set.code_of(o.symbol_id)
                    .is_some_and(|c| c.starts_with("601"))
            });
        }
        let (never, _) = sheet(&map, &a4());
        let (page, report) = sheet(&map, &auto());
        let n = added(&report).expect("lines added");
        assert!(n > 0);
        // Each is drawn: the 601 line symbol of the set, not the area symbol 601.1.
        assert_eq!(vertical_count(&page), vertical_count(&never) + n);
    }

    #[test]
    fn area_and_hidden_variants_are_not_taken() {
        // Luebeck: 601.1 and 601.6 are area symbols, 601.4 and 601.9 are hidden, 601.5 is a
        // visible line. Without 601 the lowest qualifying variant is 601.5.
        let mut map = luebeck();
        map.parts.clear();
        map.symbol_set = map.symbol_set.without_codes(|c| c == "601");
        let (page, report) = sheet(&map, &auto());
        assert!(added(&report).is_some_and(|n| n > 0));
        assert!(vertical_count(&page) > 0);
        // Without 601 and 601.5 only area and hidden symbols remain: the plain fallback.
        map.symbol_set = map.symbol_set.without_codes(|c| c == "601.5");
        let (page, report) = sheet(&map, &auto());
        assert_eq!(added(&report), None);
        assert!(
            report
                .notes
                .iter()
                .any(|n| matches!(n, SheetNote::NorthLinesWithoutSymbol { .. }))
        );
        assert_eq!(vertical_count(&page), 0);
        assert!(!sheet_xs(&page).is_empty());
    }

    #[test]
    fn fallback_without_symbol_noted() {
        let mut map = no_lines_map();
        map.symbol_set = map.symbol_set.without_codes(|c| c.starts_with("601"));
        let (page, report) = sheet(&map, &auto());
        assert!(map_xs(&page).is_empty());
        let xs = sheet_xs(&page);
        assert!(xs.len() >= 5, "{xs:?}");
        assert!(gaps(&xs).iter().all(|g| (g - 30.0).abs() < 1e-6), "{xs:?}");
        assert!(
            report
                .notes
                .contains(&SheetNote::NorthLinesWithoutSymbol { count: xs.len() })
        );
        assert_eq!(added(&report), None);
        // Black, 0.14 mm, inside the frame.
        for item in &page.sheet {
            if let SheetItem::Shape {
                colour,
                shape: Shape::Stroke { path, width, .. },
            } = item
                && path.len() == 2
                && path[0].0 == path[1].0
            {
                assert_eq!(*colour, DirectColour::BLACK);
                assert!((width - 0.14).abs() < 1e-12);
                assert!(path[0].1 >= 8.0 && path[1].1 <= 289.0);
                assert!((8.0..=202.0).contains(&path[0].0));
            }
        }
    }

    #[test]
    fn fallback_width_follows_the_symbol_factor() {
        let mut map = no_lines_map();
        map.symbol_set = map.symbol_set.without_codes(|c| c.starts_with("601"));
        let mut layout = auto();
        layout.print_scale = Some(3000);
        let (page, _) = sheet(&map, &layout);
        let widths: Vec<f64> = page
            .sheet
            .iter()
            .filter_map(|i| match i {
                SheetItem::Shape {
                    shape: Shape::Stroke { path, width, .. },
                    ..
                } if path.len() == 2 && path[0].0 == path[1].0 => Some(*width),
                _ => None,
            })
            .collect();
        assert!(!widths.is_empty());
        assert!(widths.iter().all(|w| (w - 0.14 * 4.0 / 3.0).abs() < 1e-9));
    }
}
