//! Course sheets: one course on its layout (design §5.7). The sheet is the layout's sheet with the frame centred
//! on the course unless the layout sets `centre_map_mm`, the course placeholders filled, the course overprint
//! on the map (ADR 0034) and the control description box (ADR 0037).

use std::collections::HashSet;

use crate::course::{Course, CourseKind, FittedEvent, SourceFormat, Special};
use crate::courses::{LoadedCourses, MaskRef, layout_for, mask_for};
use crate::describe::{DescriptionHeader, description_box, has_descriptions, moved};
use crate::mask::{Mask, to_internal};
use crate::numbers::{place_on_sheet, resolve};
use crate::omap::{MapCoord, MapDocument};
use crate::overprint::{applies, control_count, on_map, overprint};
use crate::print::Orientation;
use crate::project::{ElementKind, ElementSpec, LayoutSpec, LoadedProject};
use crate::render::page::{MapClip, PaperBox, Pt, RenderedPage};
use crate::scale::geometry_factor;

use super::layout::{anchor_point, inside};
use super::mask_check::Applied;
use super::placeholders::CourseValues;
use super::{
    LogoCache, NoBoxReason, PlacedElement, SheetContext, SheetError, SheetNote, SheetReport,
    layout_elements, render_body,
};

/// Most masks one course sheet applies, in the order of [`mask_for`]. Each is a clip of the map; a course file
/// with thousands of white-outs would otherwise make a page that viewers take minutes to draw. The rest are left
/// out with [`SheetNote::MasksCapped`].
pub const MAX_APPLIED_MASKS: usize = 1000;

/// What the description box of a course sheet is drawn from.
#[derive(Debug, Clone)]
pub struct DescriptionSource<'a> {
    pub course: &'a Course<MapCoord>,
    pub event: &'a FittedEvent,
    pub header: DescriptionHeader,
}

/// The sheet of `course` (one of `courses`) on `layout`: the map as [`super::render_sheet`] draws it, centred on
/// [`course_centre`] when the layout has no `centre_map_mm`, with the course overprint and the course
/// placeholders. Numbers sit where their level puts them ([`resolve`]); those the course file gives no position
/// for are placed automatically, and every number is checked ([`place_on_sheet`]).
///
/// Notes beyond the layout sheet's: [`SheetNote::CourseOutsideFrame`], [`SheetNote::DefaultLayout`] when
/// `layout` is the first layout taken for a course without assignment, [`SheetNote::NoDescriptionBox`] (also
/// when the layout has no `control_descriptions` element), [`SheetNote::UnknownDescriptionSymbol`],
/// [`SheetNote::OverprintCut`], [`SheetNote::MasksCapped`], [`SheetNote::CourseValueUnknown`], [`SheetNote::NumberConflict`],
/// [`SheetNote::NumberGapOutOfRange`] and the notes of the check against the masks (ADR 0046):
/// [`SheetNote::ElementCoversWindow`], [`SheetNote::MaskCoversNothing`], [`SheetNote::WhiteOutsCoverNothing`],
/// [`SheetNote::MaskLeavesNoWindow`], [`SheetNote::ControlInMaskedArea`],
/// [`SheetNote::OverprintInMaskedArea`] and [`SheetNote::MaskCheckLimited`]. The notes of the course file as a whole
/// come from [`course_event_notes`].
pub fn render_course_sheet(
    project: &LoadedProject,
    map: &MapDocument,
    courses: &LoadedCourses,
    course: &Course<MapCoord>,
    layout: &LayoutSpec,
    logos: &LogoCache,
) -> Result<(RenderedPage, SheetReport), SheetError> {
    let event = &courses.event;
    let lang = project.project.event.lang;
    let values = CourseValues::new(
        &course.name,
        course_length_m(course, event, map.georef.scale_denominator),
        course.climb_m,
        control_count(course, event),
        lang,
    );
    let map_scale = map.georef.scale_denominator;
    let centre = course_centre(course, event, |w, h| fits_frame(layout, map_scale, w, h));
    let source = DescriptionSource {
        course,
        event,
        header: DescriptionHeader {
            event: project.project.event.title.clone(),
            course: course.name.clone(),
            length_km: values.length_km.clone(),
            // Rounded to 5 m, as Purple Pen prints it.
            climb_m: course
                .climb_m
                .filter(|m| m.is_finite())
                .map(|m| format!("{}", ((m / 5.0).round() * 5.0).max(0.0) as u64))
                .unwrap_or_default(),
        },
    };
    let (mut page, mut report, rects) = render_body(
        project,
        map,
        layout,
        logos,
        Some(&values),
        Some(&source),
        centre,
    )?;

    let gf = report.print.geometry_factor;
    let (page_mm, centre_mm) = (page.page_mm, page.centre_mm);
    // Page x of map x: the page centre shows `centre_mm` (as `render_page` places the map).
    let to_page = move |c: MapCoord| -> Pt {
        (
            page_mm.0 / 2.0 + f64::from(c.x) / 1000.0 * gf - centre_mm.0,
            page_mm.1 / 2.0 + f64::from(c.y) / 1000.0 * gf - centre_mm.1,
        )
    };
    let kind = project.project.map.kind;
    let sizes = project.rules.course_symbol(kind);
    // Course symbols take the standard's size at the print scale (ruling R35): the profile gives them at the base
    // scale, while the map's own symbols come enlarged for the scale the map is drawn at.
    let cf = project.rules.course_factor(kind, report.print.print_scale);
    // Numbers sit where their level puts them (spec §5.7); a conflict is reported, never moved away.
    let placed = place_on_sheet(
        course,
        event,
        resolve(course, event, project, &layout.id),
        &page,
        to_page,
        &rects,
        &sizes,
        cf,
        &project.rules.placement_weights(),
        crate::numbers::number_gap(&project.rules),
        &|id| {
            map.symbol_set
                .code_of(id)
                .map_or_else(|| format!("#{id}"), str::to_string)
        },
        crate::numbers::sheet_budget(courses.event.courses.len()),
    );
    let centres: Vec<(String, Pt)> = placed
        .numbers
        .iter()
        .map(|n| (n.resolved.control.clone(), n.centre))
        .collect();
    page.overprint = on_map(
        overprint(course, event, &centres, to_page, &sizes, cf),
        project.rules.purple_cmyk(),
        &page.colours,
        &page.primitives,
    );
    // Each mask of the course clips the map on its own (ADR 0045, ruling R8): page x of a map point in mm, as
    // `to_page` places a course coordinate.
    let map_to_page = move |p: Pt| map_to_page_mm(page_mm, centre_mm, gf, p);
    let masks = mask_for(&project.project, courses, &course.name);
    page.map_clips = masks
        .iter()
        .take(MAX_APPLIED_MASKS)
        .map(|m| map_clip(m.mask(), page.clip, map_to_page))
        .collect();

    report.numbers = centres.clone();
    report.number_anchors = placed.numbers.iter().map(|n| n.anchor).collect();
    debug_assert_eq!(report.numbers.len(), report.number_anchors.len());
    report.mask_ids = masks
        .iter()
        .take(MAX_APPLIED_MASKS)
        .map(|m| match m {
            MaskRef::Project(m) => m.id.clone(),
            MaskRef::WhiteOut(m) => m.id.clone(),
        })
        .collect();
    let notes = &mut report.notes;
    notes.extend(placed.conflicts.into_iter().map(SheetNote::NumberConflict));
    if placed.search_limited {
        notes.push(SheetNote::NumberSearchLimited {
            course: course.name.clone(),
        });
    }
    notes.extend(
        placed
            .gaps_out_of_range
            .into_iter()
            .map(|g| SheetNote::NumberGapOutOfRange {
                course: g.course,
                control: g.control,
                gap_mm: g.gap_mm,
                min: g.min,
                max: g.max,
            }),
    );
    let r = (sizes.circle_diameter_mm / 2.0 + sizes.line_width_mm / 2.0) * cf;
    let frame = page.clip;
    let mut outside: Vec<String> = Vec::new();
    for stop in &course.stops {
        let Some(at) = event
            .controls
            .iter()
            .find(|c| c.code == stop.control)
            .and_then(|c| c.position)
        else {
            continue;
        };
        let (x, y) = to_page(at);
        let cut = x - r < frame.min.0
            || x + r > frame.max.0
            || y - r < frame.min.1
            || y + r > frame.max.1;
        if cut && !outside.contains(&stop.control) {
            outside.push(stop.control.clone());
        }
    }
    if !outside.is_empty() {
        notes.push(SheetNote::CourseOutsideFrame {
            course: course.name.clone(),
            controls: outside,
        });
    }
    notes.extend(elements_over_course(
        &rects, course, event, &to_page, &sizes, cf,
    ));
    // What the masks the sheet applies hide of the course (ADR 0046). Page mm back to the internal frame, the
    // inverse of `map_to_page`.
    let page_to_map = move |p: Pt| page_to_map_mm(page_mm, centre_mm, gf, p);
    let clipped: Vec<&Mask> = masks
        .iter()
        .take(MAX_APPLIED_MASKS)
        .map(|m| m.mask())
        .collect();
    notes.extend(super::mask_check::elements_over_windows(
        &course.name,
        &rects,
        &clipped,
        frame,
        &map_to_page,
    ));
    let applied: Vec<Applied<'_>> = masks
        .iter()
        .take(MAX_APPLIED_MASKS)
        .map(|m| Applied {
            mask: m.mask(),
            white_out: matches!(m, MaskRef::WhiteOut(_)),
        })
        .collect();
    notes.extend(super::mask_check::mask_notes(
        course,
        event,
        &applied,
        frame,
        &to_page,
        &page_to_map,
        &sizes,
        cf,
    ));
    if masks.len() > MAX_APPLIED_MASKS {
        notes.push(SheetNote::MasksCapped {
            course: course.name.clone(),
            applied: MAX_APPLIED_MASKS,
            total: masks.len(),
        });
    }
    if page.overprint.cut {
        notes.push(SheetNote::OverprintCut {
            course: course.name.clone(),
        });
    }
    // A box element that could not draw has noted why already.
    if !layout_elements(project, layout).any(|e| e.kind == ElementKind::ControlDescriptions) {
        notes.push(SheetNote::NoDescriptionBox {
            course: course.name.clone(),
            reason: if event.source.format == SourceFormat::IofXml {
                NoBoxReason::IofXml
            } else {
                NoBoxReason::NoElement
            },
        });
    }
    if let (default, Some(_)) = layout_for(&project.project, &course.name)
        && default.id == layout.id
    {
        notes.push(SheetNote::DefaultLayout {
            course: course.name.clone(),
            layout: layout.id.clone(),
        });
    }
    Ok((page, report))
}

/// The clip of `mask` in `frame`: its outlines and holes, written in map mm with y up, on the page by `to_page`
/// (internal map mm, y down, to page mm). [`MapClip::new`] sets the orientation of the rings on the page.
pub(crate) fn map_clip(mask: &Mask, frame: PaperBox, to_page: impl Fn(Pt) -> Pt) -> MapClip {
    let on_page = |ring: &Vec<[f64; 2]>| -> Vec<Pt> {
        ring.iter().map(|&p| to_page(to_internal(p))).collect()
    };
    MapClip::new(
        frame,
        mask.polygons.iter().map(|p| on_page(&p.outline)),
        mask.polygons
            .iter()
            .flat_map(|p| p.holes.iter().map(on_page)),
    )
}

/// The description box a `control_descriptions` element draws on a course sheet, hung on its anchor; `None`
/// on a layout sheet (silently), and with a [`SheetNote::NoDescriptionBox`] for a course from IOF XML, a
/// course without any descriptions, or a box that would reach past the page edge. The cell size is the
/// element's `cell_mm`, else the rule profile's for the map kind; it does not follow the print scale. Each
/// symbol id the standard does not know gives a [`SheetNote::UnknownDescriptionSymbol`].
pub(super) fn place_descriptions(
    ctx: &SheetContext<'_>,
    spec: &ElementSpec,
    notes: &mut Vec<SheetNote>,
) -> Option<PlacedElement> {
    let source = ctx.descriptions?;
    let course = source.course;
    let note = |notes: &mut Vec<SheetNote>, reason| {
        let n = SheetNote::NoDescriptionBox {
            course: course.name.clone(),
            reason,
        };
        if !notes.contains(&n) {
            notes.push(n);
        }
    };
    if source.event.source.format == SourceFormat::IofXml {
        note(notes, NoBoxReason::IofXml);
        return None;
    }
    if !has_descriptions(course, source.event) {
        note(notes, NoBoxReason::NoDescriptions);
        return None;
    }
    let project = ctx.project;
    let cell = spec
        .cell_mm
        .unwrap_or_else(|| project.rules.description_cell_mm(project.project.map.kind));
    let drawn = description_box(
        course,
        source.event,
        cell,
        project.project.event.lang,
        &source.header,
        ctx.map.georef.scale_denominator,
    );
    for id in drawn.unknown_symbols {
        let n = SheetNote::UnknownDescriptionSymbol {
            course: course.name.clone(),
            id,
        };
        if !notes.contains(&n) {
            notes.push(n);
        }
    }
    let size = drawn.element.rect.max;
    let at = anchor_point(&spec.anchor, ctx.page, ctx.frame, size);
    let placed = moved(drawn.element, at);
    if !inside(placed.rect, ctx.page) {
        note(notes, NoBoxReason::OutsidePage);
        return None;
    }
    Some(placed)
}

/// [`SheetNote::ElementCoversCourse`] for each element in `rects` (id and page box) that intersects the control
/// circle, start triangle or finish circles of a stop of `course` (ruling R27). A symbol counts as the circle
/// around it out to the outer edge of its line at the size factor `sf`: the control circle, the outer finish
/// circle, and the circle through the corners of the start (or map exchange) triangle, so a box may count just
/// before it touches a triangle's side. Crossing points and ends of marked routes do not count. `sf` is the course
/// factor ([`crate::rules::RuleSet::course_factor`]).
fn elements_over_course(
    rects: &[(String, crate::render::page::PaperBox)],
    course: &Course<MapCoord>,
    event: &FittedEvent,
    to_page: &impl Fn(MapCoord) -> Pt,
    sizes: &crate::rules::CourseSymbolSizes,
    sf: f64,
) -> Vec<SheetNote> {
    use crate::course::ControlKind;
    let half_line = sizes.line_width_mm / 2.0;
    let symbols: Vec<(&str, Pt, f64)> = course
        .stops
        .iter()
        .filter_map(|stop| {
            let control = event.controls.iter().find(|c| c.code == stop.control)?;
            let at = to_page(control.position?);
            let kind = match stop.kind {
                None | Some(ControlKind::MapExchange) => control.kind,
                Some(k) => k,
            };
            let r = match kind {
                ControlKind::Control => sizes.circle_diameter_mm / 2.0,
                ControlKind::Finish => sizes.finish_outer_mm / 2.0,
                ControlKind::Start | ControlKind::MapExchange => sizes.start_side_mm / 3f64.sqrt(),
                ControlKind::CrossingPoint | ControlKind::EndOfMarkedRoute => return None,
            };
            Some((stop.control.as_str(), at, (r + half_line) * sf))
        })
        .collect();
    let mut notes = Vec::new();
    for (id, rect) in rects {
        let mut controls: Vec<String> = Vec::new();
        for (code, (x, y), r) in &symbols {
            let dx = x - x.clamp(rect.min.0, rect.max.0);
            let dy = y - y.clamp(rect.min.1, rect.max.1);
            if dx.hypot(dy) < *r && !controls.iter().any(|c| c == code) {
                controls.push((*code).to_string());
            }
        }
        if !controls.is_empty() {
            notes.push(SheetNote::ElementCoversCourse {
                element: id.clone(),
                controls,
            });
        }
    }
    notes
}

/// Page mm of a point `(x, y)` of the map's internal frame (mm, y down): the page centre shows `centre_mm`, and
/// `factor` is the geometry factor of the print.
pub fn map_to_page_mm(page_mm: Pt, centre_mm: Pt, factor: f64, (x, y): Pt) -> Pt {
    (
        page_mm.0 / 2.0 + x * factor - centre_mm.0,
        page_mm.1 / 2.0 + y * factor - centre_mm.1,
    )
}

/// The inverse of [`map_to_page_mm`]: the point of the map's internal frame (mm, y down) under a page point.
pub fn page_to_map_mm(page_mm: Pt, centre_mm: Pt, factor: f64, (x, y): Pt) -> Pt {
    (
        (x - page_mm.0 / 2.0 + centre_mm.0) / factor,
        (y - page_mm.1 / 2.0 + centre_mm.1) / factor,
    )
}

/// The notes of the course file as a whole, once per run: what the reader left out or found, what the fit
/// found, the entries of the project file that no longer match the course file, and the masks no course uses.
pub fn course_event_notes(courses: &LoadedCourses) -> Vec<SheetNote> {
    let course = courses.notes.iter().cloned().map(SheetNote::Course);
    let fit = courses.fit_notes.iter().cloned().map(SheetNote::Fit);
    let stale = courses.stale.iter().cloned().map(SheetNote::Stale);
    let masks = courses.mask_notes.iter().cloned().map(SheetNote::Mask);
    course.chain(fit).chain(stale).chain(masks).collect()
}

/// The map positions of the stops of `course` and the bends of its legs.
fn stops_and_bends<'a>(
    course: &'a Course<MapCoord>,
    event: &'a FittedEvent,
) -> impl Iterator<Item = MapCoord> + 'a {
    let stops = course.stops.iter().filter_map(|s| {
        event
            .controls
            .iter()
            .find(|c| c.code == s.control)
            .and_then(|c| c.position)
    });
    let bends = course.legs.iter().flat_map(|l| l.bends.iter().copied());
    stops.chain(bends)
}

/// The points of `special` when it applies to the course named `course`. A white-out does not count: it is no
/// overprint shape but a mask, which hides the map and never widens the frame (ADR 0045).
fn special_points(special: &Special<MapCoord>, course: &str) -> Vec<MapCoord> {
    match special {
        Special::OutOfBounds { rings, courses, .. } | Special::Dangerous { rings, courses, .. }
            if applies(courses, course) =>
        {
            rings.iter().flatten().copied().collect()
        }
        Special::ForbiddenRoute { path, courses, .. }
        | Special::Boundary { path, courses, .. }
        | Special::MarkedRoute { path, courses, .. }
        | Special::Line { path, courses, .. }
            if applies(courses, course) =>
        {
            path.clone()
        }
        Special::FirstAid { pt, courses, .. }
        | Special::Water { pt, courses, .. }
        | Special::Text { pt, courses, .. }
            if applies(courses, course) =>
        {
            vec![*pt]
        }
        _ => Vec::new(),
    }
}

/// A box in map coordinates (µm).
#[derive(Debug, Clone, Copy)]
struct MapBox {
    min: (i32, i32),
    max: (i32, i32),
}

impl MapBox {
    fn around(points: impl IntoIterator<Item = MapCoord>, start: Option<MapBox>) -> Option<MapBox> {
        points.into_iter().fold(start, |b, p| {
            Some(match b {
                None => MapBox {
                    min: (p.x, p.y),
                    max: (p.x, p.y),
                },
                Some(b) => MapBox {
                    min: (b.min.0.min(p.x), b.min.1.min(p.y)),
                    max: (b.max.0.max(p.x), b.max.1.max(p.y)),
                },
            })
        })
    }

    /// Width and height in map mm.
    fn size_mm(self) -> (f64, f64) {
        let len = |a: i32, b: i32| (f64::from(b) - f64::from(a)) / 1000.0;
        (len(self.min.0, self.max.0), len(self.min.1, self.max.1))
    }

    fn centre(self) -> MapCoord {
        let mid = |a: i32, b: i32| ((i64::from(a) + i64::from(b)) / 2) as i32;
        MapCoord::new(mid(self.min.0, self.max.0), mid(self.min.1, self.max.1))
    }
}

/// The map point the frame centres on for `course`: the centre of the box around its stops and leg bends. Each
/// special object that applies to the course widens the box, in file order, as long as `fits` (width and
/// height in map mm) accepts the widened box, so an object far away, such as an out-of-bounds area for all
/// courses, does not pull the course off the frame. `None` for a course without any stop on the map.
pub fn course_centre(
    course: &Course<MapCoord>,
    event: &FittedEvent,
    fits: impl Fn(f64, f64) -> bool,
) -> Option<MapCoord> {
    let mut b = MapBox::around(stops_and_bends(course, event), None)?;
    for special in &event.specials {
        if let Some(wider) = MapBox::around(special_points(special, &course.name), Some(b)) {
            let (w, h) = wider.size_mm();
            if fits(w, h) {
                b = wider;
            }
        }
    }
    Some(b.centre())
}

/// Whether a box of `w` × `h` map mm fits in the map frame of `layout` at its print scale: in its orientation,
/// or in either for `auto`.
fn fits_frame(layout: &LayoutSpec, map_scale: u32, w: f64, h: f64) -> bool {
    let gf = geometry_factor(map_scale, layout.print_scale.unwrap_or(map_scale));
    let (pw, ph) = layout.paper.size_mm();
    let m = 2.0 * layout.map_frame.margin_mm;
    let fit = |fw: f64, fh: f64| w * gf <= fw - m && h * gf <= fh - m;
    match layout.orientation {
        Orientation::Portrait => fit(pw, ph),
        Orientation::Landscape => fit(ph, pw),
        Orientation::Auto => fit(pw, ph) || fit(ph, pw),
    }
}

/// The length of `course` in metres: the file's when it gives one, else the legs through their bends (a marked
/// route counts like any leg), map mm times `map_scale` ÷ 1000. `None` for a score course or a course with a
/// stop that has no map position.
pub fn course_length_m(
    course: &Course<MapCoord>,
    event: &FittedEvent,
    map_scale: u32,
) -> Option<f64> {
    if let Some(m) = course.length_m {
        return Some(m);
    }
    if course.kind == CourseKind::Score || course.legs.is_empty() {
        return None;
    }
    let position = |i: usize| {
        let code = &course.stops.get(i)?.control;
        event
            .controls
            .iter()
            .find(|c| &c.code == code)
            .and_then(|c| c.position)
    };
    let mut mm = 0.0;
    for (i, leg) in course.legs.iter().enumerate() {
        let mut points = vec![position(i)?];
        points.extend(&leg.bends);
        points.push(position(i + 1)?);
        mm += points
            .windows(2)
            .map(|w| {
                let dx = f64::from(w[1].x) - f64::from(w[0].x);
                let dy = f64::from(w[1].y) - f64::from(w[0].y);
                dx.hypot(dy) / 1000.0
            })
            .sum::<f64>();
    }
    Some(mm * f64::from(map_scale) / 1000.0)
}

/// Longest file stem, in bytes, before a number is added to make it unique: well below the 255 bytes most file
/// systems allow for a name.
const MAX_STEM_BYTES: usize = 150;

/// Combining marks and the precomposed Latin-1 letters they form with a base letter: base letters, then the
/// composed letters in the same order.
const COMPOSE: [(char, &str, &str); 7] = [
    ('\u{300}', "AEIOUaeiou", "ÀÈÌÒÙàèìòù"),
    ('\u{301}', "AEIOUYaeiouy", "ÁÉÍÓÚÝáéíóúý"),
    ('\u{302}', "AEIOUaeiou", "ÂÊÎÔÛâêîôû"),
    ('\u{303}', "ANOano", "ÃÑÕãñõ"),
    ('\u{308}', "AEIOUaeiouy", "ÄËÏÖÜäëïöüÿ"),
    ('\u{30A}', "Aa", "Åå"),
    ('\u{327}', "Cc", "Çç"),
];

/// `base` followed by the combining `mark` as one precomposed Latin-1 letter, if there is one.
fn compose(base: char, mark: char) -> Option<char> {
    let (_, bases, composed) = COMPOSE.iter().find(|(m, _, _)| *m == mark)?;
    let i = bases.chars().position(|b| b == base)?;
    composed.chars().nth(i)
}

/// The precomposed form of `name` for the Latin-1 letters (German umlauts among them): a base letter followed
/// by its combining mark becomes one character. Other text stays as it is.
fn nfc_latin1(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if let Some(prev) = out.chars().next_back()
            && let Some(composed) = compose(prev, c)
        {
            out.pop();
            out.push(composed);
        } else {
            out.push(c);
        }
    }
    out
}

/// Names Windows reserves for devices; a file of such a name (with any extension) cannot be created there.
fn reserved_on_windows(stem: &str) -> bool {
    let upper = stem.to_ascii_uppercase();
    matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper.len() == 4
            && upper.as_bytes()[3].is_ascii_digit())
}

/// A file stem for the course `name`, unique among `taken` (lower-case stems, which this adds to):
/// 1. the Latin-1 letters precomposed (NFC), so `Lübeck` in NFD and NFC give one name;
/// 2. each of `/ \ : * ? " < > |` and every control character replaced by `-`;
/// 3. dots and spaces trimmed at both ends (and the stem cut to 150 bytes); an empty stem becomes `course`, and a
///    device name Windows reserves (the part before the first dot, without trailing spaces) gets a `-`;
/// 4. ` (2)`, ` (3)` … added until no stem in `taken` matches it ignoring case.
pub fn course_file_stem(name: &str, taken: &mut HashSet<String>) -> String {
    let replaced: String = nfc_latin1(name)
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '-'
            } else {
                c
            }
        })
        .collect();
    let mut stem = replaced.trim_matches(['.', ' ']).to_string();
    if stem.len() > MAX_STEM_BYTES {
        let mut end = MAX_STEM_BYTES;
        while !stem.is_char_boundary(end) {
            end -= 1;
        }
        stem.truncate(end);
        stem = stem.trim_end_matches(['.', ' ']).to_string();
    }
    if stem.is_empty() {
        stem = "course".to_string();
    }
    // Windows takes `con.1` and `con .txt` for the device too: the part before the first dot counts, without
    // its trailing spaces.
    let base = stem.split('.').next().unwrap_or("").trim_end_matches(' ');
    if reserved_on_windows(base) {
        stem.insert(base.len(), '-');
    }
    let mut unique = stem.clone();
    let mut n = 2;
    while taken.contains(&unique.to_lowercase()) {
        unique = format!("{stem} ({n})");
        n += 1;
    }
    taken.insert(unique.to_lowercase());
    unique
}

/// A project in a temporary folder with the Lübeck map and `tests/fixtures/courses/luebeck-sheets.xml`: the
/// event "Lübeck Training", y up confirmed, layouts and more from `body`. For tests here and in `export`.
#[cfg(test)]
pub(crate) fn course_project(
    body: &str,
) -> (tempfile::TempDir, LoadedProject, MapDocument, LoadedCourses) {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(
        manifest.join("../../tests/fixtures/maps/luebeck.omap"),
        dir.path().join("m.omap"),
    )
    .unwrap();
    std::fs::copy(
        manifest.join("tests/fixtures/courses/luebeck-sheets.xml"),
        dir.path().join("c.xml"),
    )
    .unwrap();
    let path = dir.path().join(crate::project::FILE_NAME);
    let head = "format = 1\n[event]\ntitle = \"Lübeck Training\"\nlang = \"de\"\n[sources]\nmap = \"m.omap\"\n\
        courses = \"c.xml\"\ncourse_y_axis = \"up\"\n[map]\nkind = \"sprint\"\ncontour_interval_m = 2.0\n";
    std::fs::write(&path, format!("{head}{body}")).unwrap();
    let project = crate::project::load(&path).unwrap();
    let map = crate::omap::read::read_map_file(&project.map_path()).unwrap();
    let courses = crate::courses::load_courses(&project, &map)
        .unwrap()
        .unwrap();
    (dir, project, map, courses)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::page::{Shape, SheetItem};

    #[test]
    fn page_and_map_points_round_trip() {
        let (page, centre, factor) = ((297.0, 210.0), (12.5, -40.25), 1.6);
        for p in [(0.0, 0.0), (-33.3, 80.1), (150.0, -2.0)] {
            let on_page = map_to_page_mm(page, centre, factor, p);
            let back = page_to_map_mm(page, centre, factor, on_page);
            assert!(
                (back.0 - p.0).abs() < 1e-9 && (back.1 - p.1).abs() < 1e-9,
                "{p:?} {back:?}"
            );
        }
        // The map point under the page centre is the one the page centre shows.
        let centre_point = page_to_map_mm(page, centre, factor, (148.5, 105.0));
        assert!((centre_point.0 - centre.0 / factor).abs() < 1e-9);
    }

    const A4: &str =
        "[[layouts]]\nid = \"a4\"\norientation = \"portrait\"\nnorth_lines = \"never\"\n";

    fn course<'a>(courses: &'a LoadedCourses, name: &str) -> &'a Course<MapCoord> {
        courses
            .event
            .courses
            .iter()
            .find(|c| c.name == name)
            .unwrap()
    }

    fn sheet(body: &str, name: &str) -> (RenderedPage, SheetReport, LoadedCourses) {
        let (_dir, project, map, courses) = course_project(body);
        let layout = layout_for(&project.project, name).0;
        let (page, report) = render_course_sheet(
            &project,
            &map,
            &courses,
            course(&courses, name),
            layout,
            &LogoCache::new(),
        )
        .unwrap();
        (page, report, courses)
    }

    fn sheet_texts(page: &RenderedPage) -> Vec<String> {
        page.sheet
            .iter()
            .filter_map(|i| match i {
                SheetItem::Shape {
                    shape: Shape::Text { text, .. },
                    ..
                } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    /// The page point of map point `c` on `page` at geometry factor `gf`.
    fn on_page(page: &RenderedPage, gf: f64, c: MapCoord) -> Pt {
        (
            page.page_mm.0 / 2.0 + f64::from(c.x) / 1000.0 * gf - page.centre_mm.0,
            page.page_mm.1 / 2.0 + f64::from(c.y) / 1000.0 * gf - page.centre_mm.1,
        )
    }

    #[test]
    fn course_sheet_has_overprint_and_credit() {
        let (page, report, _) = sheet(A4, "Kurz");
        // Start triangle, three circles... in lower purple: start, 2 controls, 2 finish circles, 3 legs.
        assert!(
            page.overprint.lower.len() >= 8,
            "{:?}",
            page.overprint.lower.len()
        );
        // The numbers 1 and 2 in upper purple.
        assert_eq!(page.overprint.upper.len(), 2);
        assert!(page.overprint.below_priority.is_some());
        assert_eq!(page.overprint.colour.cmyk, crate::overprint::IOF_PURPLE);
        assert!(!page.overprint.cut);
        let texts = sheet_texts(&page);
        assert!(
            texts
                .iter()
                .any(|t| t.contains(crate::credit_line(crate::project::SheetLang::De))),
            "{texts:?}"
        );
        // IOF XML carries no descriptions: the sheet says so and only informs.
        let no_box = SheetNote::NoDescriptionBox {
            course: "Kurz".into(),
            reason: NoBoxReason::IofXml,
        };
        assert!(report.notes.contains(&no_box));
        assert!(no_box.is_info());
        assert!(report.notes.contains(&SheetNote::DefaultLayout {
            course: "Kurz".into(),
            layout: "a4".into()
        }));
        assert!(
            !report
                .notes
                .iter()
                .any(|n| matches!(n, SheetNote::CourseOutsideFrame { .. }))
        );
        // An assigned course gets no default-layout note.
        let body = format!(
            "{A4}[[layouts]]\nid = \"a3\"\npaper = \"A3\"\norientation = \"portrait\"\nnorth_lines = \"never\"\n[[assignments]]\ncourse = \"Kurz\"\nlayout = \"a3\"\n"
        );
        let (page, report, _) = sheet(&body, "Kurz");
        assert_eq!(page.page_mm, (297.0, 420.0));
        assert!(
            !report
                .notes
                .iter()
                .any(|n| matches!(n, SheetNote::DefaultLayout { .. }))
        );
    }

    #[test]
    fn walkable_area_keeps_the_lower_purple_in_place() {
        // Rule R19 (ADR 0034) places the lower purple by the full-strength colours alone. The walkable area of a
        // freemapper sprint map is a new colour behind all others (priority 32 on Lübeck's 32 colours) and a
        // rectangle far beyond the map: neither may move the purple, the numbers or the frame.
        use crate::print::base_fixture::{base_part, with_walkable_area};
        let (_dir, project, map, courses) = course_project(A4);
        let mut based = map.clone();
        based.symbol_set = with_walkable_area(&map.symbol_set);
        let part = base_part(
            &based.symbol_set,
            (-1_000_000, -1_000_000),
            (1_200_000, 900_000),
        );
        based.parts.insert(0, part);
        let layout = layout_for(&project.project, "Kurz").0;
        let draw = |m: &MapDocument| {
            render_course_sheet(
                &project,
                m,
                &courses,
                course(&courses, "Kurz"),
                layout,
                &LogoCache::new(),
            )
            .unwrap()
        };
        let (plain, plain_report) = draw(&map);
        let (page, report) = draw(&based);
        let walkable = page
            .colours
            .iter()
            .find(|c| c.name.contains("walkable area"))
            .unwrap();
        assert_eq!(walkable.priority, 32);
        assert_eq!(page.centre_mm, plain.centre_mm);
        assert_eq!(page.overprint, plain.overprint);
        assert!(page.overprint.below_priority.is_some());
        assert_eq!(report.notes, plain_report.notes);
        // The map is the same, with the base area added under it.
        let key =
            |p: &crate::render::draw::Primitive| (p.priority, p.shape.clone(), p.class, p.symbol);
        let map_of = |page: &RenderedPage| -> Vec<_> {
            page.primitives
                .iter()
                .filter(|p| !p.base)
                .map(key)
                .collect()
        };
        assert_eq!(map_of(&page), map_of(&plain));
        // The PDF writer paints the lower purple before the first primitive at or above the deepest full-strength
        // colour: the base area lies before it, so the course stays visible over it.
        let below = page.overprint.below_priority.unwrap();
        let split = page
            .primitives
            .iter()
            .position(|p| p.priority <= below)
            .unwrap();
        assert!(page.primitives[..split].iter().any(|p| p.base));
        assert!(page.primitives[split..].iter().all(|p| !p.base));
    }

    #[test]
    fn frame_centred_on_course() {
        let (page, report, courses) = sheet(A4, "Kurz");
        let gf = report.print.geometry_factor;
        // Kurz spans x from S1 (-50 mm) to 32 (100 mm) and y from S1 (-60 mm) to 32 (120 mm), y down: its centre,
        // (25, 30) mm, lies at the centre of the frame.
        let centre = course_centre(course(&courses, "Kurz"), &courses.event, |_, _| true).unwrap();
        assert_eq!(centre, MapCoord::new(25_000, 30_000));
        let (x, y) = on_page(&page, gf, centre);
        let frame = page.clip;
        assert!((x - (frame.min.0 + frame.max.0) / 2.0).abs() < 1e-6, "{x}");
        assert!((y - (frame.min.1 + frame.max.1) / 2.0).abs() < 1e-6, "{y}");
        // With centre_map_mm the layout decides: OOM coordinates, y up.
        let body = "[[layouts]]\nid = \"a4\"\norientation = \"portrait\"\nnorth_lines = \"never\"\nmap_frame = { centre_map_mm = [0.0, 0.0] }\n";
        let (page, report, _) = sheet(body, "Kurz");
        let (x, y) = on_page(&page, report.print.geometry_factor, MapCoord::new(0, 0));
        assert!(
            (x - 105.0).abs() < 1e-6 && (y - 148.5).abs() < 1e-6,
            "{x} {y}"
        );
    }

    #[test]
    fn specials_widen_the_frame_only_while_it_fits() {
        use crate::course::{CourseFilter, Special};
        let (_dir, project, map, mut courses) = course_project(A4);
        let water = |x: i32, y: i32| Special::Water {
            pt: MapCoord::new(x, y),
            courses: CourseFilter::All,
            id: None,
        };
        // A water point 2 km east of the course, for all courses, and one just west of it.
        courses.event.specials = vec![water(500_000, 30_000), water(-80_000, 30_000)];
        let kurz = course(&courses, "Kurz");
        // Stops span x -50..100 mm; the near point widens that to -80..100 mm, the far one would not fit.
        let layout = &project.project.layouts[0];
        let fits = |w, h| fits_frame(layout, map.georef.scale_denominator, w, h);
        assert_eq!(
            course_centre(kurz, &courses.event, fits),
            Some(MapCoord::new(10_000, 30_000))
        );
        // Without a frame to fit, the far point would pull the centre away.
        let all = course_centre(kurz, &courses.event, |_, _| true).unwrap();
        assert_eq!(all, MapCoord::new(210_000, 30_000));
        // The sheet centres on the near box.
        let (page, report) =
            render_course_sheet(&project, &map, &courses, kurz, layout, &LogoCache::new()).unwrap();
        let (x, y) = on_page(
            &page,
            report.print.geometry_factor,
            MapCoord::new(10_000, 30_000),
        );
        assert!(
            (x - 105.0).abs() < 1e-6 && (y - 148.5).abs() < 1e-6,
            "{x} {y}"
        );
    }

    #[test]
    fn map_beyond_the_frame_is_expected_on_course_sheets() {
        // The Lübeck map is wider than an A4 frame at 1:4000.
        let (_, report, _) = sheet(A4, "Kurz");
        assert_eq!(report.print.overflow_mm, None);
        assert!(
            report
                .notes
                .iter()
                .any(|n| matches!(n, SheetNote::MapCroppedToCourse { .. }) && n.is_info()),
            "{:?}",
            report.notes
        );
        // With crop = true the layout's own note stays.
        let body = "[[layouts]]\nid = \"a4\"\norientation = \"portrait\"\nnorth_lines = \"never\"\nmap_frame = { crop = true }\n";
        let (_, report, _) = sheet(body, "Kurz");
        assert!(
            report
                .notes
                .iter()
                .any(|n| matches!(n, SheetNote::MapCropped { .. }))
        );
        // Controls outside the frame still count.
        let outside = SheetNote::CourseOutsideFrame {
            course: "K".into(),
            controls: vec!["31".into()],
        };
        assert!(!outside.is_info());
    }

    #[test]
    fn north_lines_second_pass_keeps_the_course_centre() {
        // Without its north lines the map gets generated ones, and is drawn again around the same centre.
        let (_dir, project, mut map, courses) =
            course_project("[[layouts]]\nid = \"a4\"\norientation = \"portrait\"\n");
        let set = map.symbol_set.clone();
        for part in &mut map.parts {
            part.objects.retain(|o| {
                !set.code_of(o.symbol_id)
                    .is_some_and(|c| c.starts_with("601"))
            });
        }
        let kurz = course(&courses, "Kurz");
        let layout = &project.project.layouts[0];
        let (page, report) =
            render_course_sheet(&project, &map, &courses, kurz, layout, &LogoCache::new()).unwrap();
        assert!(
            report
                .notes
                .iter()
                .any(|n| matches!(n, SheetNote::NorthLinesAdded { .. })),
            "{:?}",
            report.notes
        );
        let centre = course_centre(kurz, &courses.event, |_, _| true).unwrap();
        let (x, y) = on_page(&page, report.print.geometry_factor, centre);
        assert!(
            (x - 105.0).abs() < 1e-6 && (y - 148.5).abs() < 1e-6,
            "{x} {y}"
        );
    }

    #[test]
    fn course_outside_frame_noted() {
        // A frame centred far east of the course: S1 and 33 lie beyond its left edge, 31 is cut by it.
        let body = "[[layouts]]\nid = \"a4\"\norientation = \"portrait\"\nnorth_lines = \"never\"\nmap_frame = { centre_map_mm = [117.0, 0.0] }\n";
        let (_, report, _) = sheet(body, "D/H 12");
        let outside: Vec<&SheetNote> = report
            .notes
            .iter()
            .filter(|n| matches!(n, SheetNote::CourseOutsideFrame { .. }))
            .collect();
        assert_eq!(
            outside,
            [&SheetNote::CourseOutsideFrame {
                course: "D/H 12".into(),
                controls: vec!["S1".into(), "33".into(), "31".into()],
            }]
        );
        assert!(report.needs_attention());
    }

    #[test]
    fn course_placeholders_filled() {
        let body = format!(
            "{A4}[[shared]]\nid = \"t\"\nkind = \"text\"\nlines = [\"{{course}}: {{length_km}} km, {{climb_m}} m, {{controls}} Posten\"]\n\
             anchor = {{ to = \"map_frame\", corner = \"top_left\" }}\n"
        );
        // Kurz has no length in the file: its legs are √13000 + √14500 + √21200 mm = 380.04 mm at 1:4000.
        let (_dir, project, map, courses) = course_project(&body);
        let kurz = course(&courses, "Kurz");
        let hand = (13000f64.sqrt() + 14500f64.sqrt() + 21200f64.sqrt()) * 4.0;
        let length = course_length_m(kurz, &courses.event, map.georef.scale_denominator).unwrap();
        assert!((length - hand).abs() < 0.01, "{length} {hand}");
        assert!((length - 1520.14).abs() < 0.01, "{length}");
        let layout = &project.project.layouts[0];
        let logos = LogoCache::new();
        let (page, report) =
            render_course_sheet(&project, &map, &courses, kurz, layout, &logos).unwrap();
        assert!(sheet_texts(&page).contains(&"Kurz: 1,5 km,  m, 2 Posten".to_string()));
        assert!(report.notes.contains(&SheetNote::CourseValueUnknown {
            course: "Kurz".into(),
            id: "t".into(),
            name: "climb_m".into()
        }));
        // D/H 12 gives length and climb; three controls.
        let dh = course(&courses, "D/H 12");
        let (page, report) =
            render_course_sheet(&project, &map, &courses, dh, layout, &logos).unwrap();
        assert!(sheet_texts(&page).contains(&"D/H 12: 2,1 km, 25 m, 2 Posten".to_string()));
        assert!(
            !report
                .notes
                .iter()
                .any(|n| matches!(n, SheetNote::CourseValueUnknown { .. }))
        );
        // A layout sheet fills the course placeholders with nothing and says so; they are no unknown ones.
        let (page, report) = super::super::render_sheet(&project, &map, layout, &logos).unwrap();
        assert!(sheet_texts(&page).contains(&":  km,  m,  Posten".to_string()));
        assert!(
            !report
                .notes
                .iter()
                .any(|n| matches!(n, SheetNote::UnknownPlaceholder { .. }))
        );
        let not_filled: Vec<&SheetNote> = report
            .notes
            .iter()
            .filter(|n| matches!(n, SheetNote::CoursePlaceholderNotFilled { .. }))
            .collect();
        assert_eq!(not_filled.len(), 4, "{:?}", report.notes);
        assert!(not_filled.iter().all(|n| n.is_info()));
        assert!(
            not_filled.contains(&&SheetNote::CoursePlaceholderNotFilled {
                element: "t".into(),
                name: "course".into()
            })
        );
    }

    #[test]
    fn length_includes_bends_and_marked_routes() {
        let (_dir, _project, map, courses) = course_project(A4);
        let scale = map.georef.scale_denominator;
        let mut kurz = course(&courses, "Kurz").clone();
        let plain = course_length_m(&kurz, &courses.event, scale).unwrap();
        // A bend at (80, 0) mm off the straight line from 32 (100, 120) to F1 (60, -20), marked as a taped route.
        let leg = &mut kurz.legs[2];
        leg.bends.push(MapCoord::new(80_000, 0));
        leg.marked_route = true;
        let bent = course_length_m(&kurz, &courses.event, scale).unwrap();
        let expected = plain - 21200f64.sqrt() * 4.0 + (14800f64.sqrt() + 800f64.sqrt()) * 4.0;
        assert!((bent - expected).abs() < 0.01, "{bent} {expected}");
        assert_eq!(kurz.legs.len(), 3);
    }

    #[test]
    fn course_file_notes_once_per_run() {
        let (_dir, _project, _map, courses) = course_project(A4);
        let notes = course_event_notes(&courses);
        // No lat/lng: the georeference check is skipped; the reader found no descriptions in IOF XML.
        assert!(
            notes.iter().any(|n| matches!(n, SheetNote::Fit(_))),
            "{notes:?}"
        );
        assert!(notes.iter().all(SheetNote::is_info), "{notes:?}");
    }

    #[test]
    fn course_file_names_are_safe_and_unique() {
        let mut taken = HashSet::new();
        let mut stem = |n: &str| course_file_stem(n, &mut taken);
        assert_eq!(stem("D/H 12"), "D-H 12");
        assert_eq!(stem("Bahn 1: lang"), "Bahn 1- lang");
        // NFD (u + combining diaeresis) and NFC give the same precomposed name, the second made unique.
        assert_eq!(stem("Lu\u{308}beck"), "Lübeck");
        assert_eq!(stem("Lübeck"), "Lübeck (2)");
        // Names that differ only in case get different files on case-insensitive file systems too.
        assert_eq!(stem("A"), "A");
        assert_eq!(stem("a"), "a (2)");
        assert_eq!(stem("a"), "a (3)");
        assert_eq!(stem(" ..Bahn *?\"<>|\\ ."), "Bahn -------");
        assert_eq!(stem("Tab\there\n"), "Tab-here-");
        assert_eq!(stem(".."), "course");
        assert_eq!(stem(""), "course (2)");
        assert_eq!(stem("con"), "con-");
        assert_eq!(stem("con.1"), "con-.1");
        assert_eq!(stem("Nul .txt"), "Nul- .txt");
        assert_eq!(stem("lpt1.a.b"), "lpt1-.a.b");
        assert_eq!(stem("Console.1"), "Console.1");
        assert_eq!(stem("Ça va? Øre"), "Ça va- Øre");
        let long = stem(&"ü".repeat(200));
        assert_eq!(long.len(), MAX_STEM_BYTES);
        assert_eq!(
            nfc_latin1("A\u{30A}c\u{327}y\u{308}x\u{308}"),
            "Åçÿx\u{308}"
        );
    }

    /// The layout `a4` with a description box hung on `anchor` (TOML inline table) and `extra` fields.
    fn with_box(anchor: &str, extra: &str) -> String {
        format!(
            "{A4}[[layouts.elements]]\nid = \"descriptions\"\nkind = \"control_descriptions\"\nanchor = {anchor}\n{extra}"
        )
    }

    const TOP_RIGHT: &str =
        "{ to = \"map_frame\", corner = \"top_right\", offset_mm = [2.0, 2.0] }";

    /// The course file's controls as a `.ppen` would give them: descriptions in columns D and G, 2018.
    fn as_ppen(courses: &mut LoadedCourses) {
        use crate::course::Description;
        courses.event.source.format = SourceFormat::PurplePen;
        for c in &mut courses.event.controls {
            let d = |cols: [Option<&str>; 6]| Description {
                columns: cols.map(|c| c.map(str::to_string)),
                standard: "2018".into(),
                column_f_text: None,
            };
            c.description = Some(match c.code.as_str() {
                "F1" => d([Some("14.3"), None, None, None, None, None]),
                "S1" => d([None, Some("2.6"), None, None, None, None]),
                _ => d([None, Some("1.1"), None, None, Some("11.1N"), None]),
            });
        }
    }

    /// The course sheet of `name` for a project with `body`, the course file changed by `change`.
    fn ppen_sheet(
        body: &str,
        name: &str,
        change: impl FnOnce(&mut LoadedCourses),
    ) -> (RenderedPage, SheetReport) {
        let (_dir, project, map, mut courses) = course_project(body);
        change(&mut courses);
        let layout = layout_for(&project.project, name).0;
        render_course_sheet(
            &project,
            &map,
            &courses,
            course(&courses, name),
            layout,
            &LogoCache::new(),
        )
        .unwrap()
    }

    /// The white rectangles of the sheet layer as boxes.
    fn white_boxes(page: &RenderedPage) -> Vec<crate::render::page::PaperBox> {
        use crate::render::page::{DirectColour, PaperBox};
        page.sheet
            .iter()
            .filter_map(|i| match i {
                SheetItem::Shape {
                    colour,
                    shape: Shape::Fill { rings, .. },
                } if *colour == DirectColour::WHITE && rings.len() == 1 && rings[0].len() == 4 => {
                    let r = &rings[0];
                    Some(PaperBox {
                        min: r[0],
                        max: r[2],
                    })
                }
                _ => None,
            })
            .collect()
    }

    fn box_notes(report: &SheetReport) -> Vec<&SheetNote> {
        report
            .notes
            .iter()
            .filter(|n| {
                matches!(
                    n,
                    SheetNote::NoDescriptionBox { .. } | SheetNote::UnknownDescriptionSymbol { .. }
                )
            })
            .collect()
    }

    #[test]
    fn iof_course_gets_note_not_box() {
        let body = with_box(TOP_RIGHT, "");
        let (page, report, _) = sheet(&body, "Kurz");
        let expected = SheetNote::NoDescriptionBox {
            course: "Kurz".into(),
            reason: NoBoxReason::IofXml,
        };
        assert_eq!(box_notes(&report), vec![&expected]);
        assert!(expected.is_info());
        assert!(
            !sheet_texts(&page).iter().any(|t| t == "Kurz"),
            "{:?}",
            sheet_texts(&page)
        );
        // Without the element a .ppen course informs that the layout has no box.
        let (_, report) = ppen_sheet(A4, "Kurz", as_ppen);
        let none = SheetNote::NoDescriptionBox {
            course: "Kurz".into(),
            reason: NoBoxReason::NoElement,
        };
        assert_eq!(box_notes(&report), vec![&none]);
        assert!(none.is_info());
        // A .ppen course without any descriptions cannot fill the box: a warning.
        let (_, report) = ppen_sheet(&body, "Kurz", |c| {
            as_ppen(c);
            for control in &mut c.event.controls {
                control.description = None;
            }
        });
        let empty = SheetNote::NoDescriptionBox {
            course: "Kurz".into(),
            reason: NoBoxReason::NoDescriptions,
        };
        assert_eq!(box_notes(&report), vec![&empty]);
        assert!(!empty.is_info() && report.needs_attention());
        // A layout sheet leaves the element out without a note.
        let (_dir, project, map, _) = course_project(&body);
        let (page, report) = super::super::render_sheet(
            &project,
            &map,
            &project.project.layouts[0],
            &LogoCache::new(),
        )
        .unwrap();
        assert!(box_notes(&report).is_empty());
        assert!(
            !white_boxes(&page)
                .iter()
                .any(|b| (b.max.0 - b.min.0 - 48.3).abs() < 1e-9)
        );
    }

    #[test]
    fn box_respects_anchor_and_stays_on_page() {
        let (page, report) = ppen_sheet(&with_box(TOP_RIGHT, ""), "Kurz", as_ppen);
        assert!(box_notes(&report).is_empty(), "{:?}", report.notes);
        // Title, course row, start, two controls, finish: six rows of the profile's 6 mm sprint cells, plus half a
        // 0.3 mm line on each side. Hung 2 mm inside the frame's top-right corner.
        let frame = page.clip;
        let boxes = white_boxes(&page);
        let b = boxes
            .iter()
            .find(|b| (b.max.0 - b.min.0 - 48.3).abs() < 1e-9)
            .unwrap_or_else(|| panic!("{boxes:?}"));
        assert!((b.max.1 - b.min.1 - 36.3).abs() < 1e-9, "{b:?}");
        assert!((b.max.0 - (frame.max.0 - 2.0)).abs() < 1e-9, "{b:?}");
        assert!((b.min.1 - (frame.min.1 + 2.0)).abs() < 1e-9, "{b:?}");
        // The finish row: 32 to F1 is √21200 mm = 145.6 mm, 582.4 m at 1:4000.
        let texts = sheet_texts(&page);
        for t in [
            "Lübeck Training",
            "Kurz",
            "1,5 km",
            "1",
            "31",
            "2",
            "32",
            "580 m",
        ] {
            assert!(texts.contains(&t.to_string()), "{t}: {texts:?}");
        }
        // Hung on the map frame, the box may cover the map without a note.
        assert!(
            !report
                .notes
                .iter()
                .any(|n| matches!(n, SheetNote::ElementOverlapsMap { .. }))
        );
        // cell_mm sets the size.
        let (page, _) = ppen_sheet(&with_box(TOP_RIGHT, "cell_mm = 5.0\n"), "Kurz", as_ppen);
        assert!(
            white_boxes(&page)
                .iter()
                .any(|b| (b.max.0 - b.min.0 - 40.25).abs() < 1e-9)
        );
        // Hung on the page over the map frame: the M2 note for elements over the map.
        let page_anchor = "{ to = \"page\", corner = \"top_left\", offset_mm = [20.0, 20.0] }";
        let (_, report) = ppen_sheet(&with_box(page_anchor, ""), "Kurz", as_ppen);
        assert!(report.notes.contains(&SheetNote::ElementOverlapsMap {
            id: "descriptions".into()
        }));
        // Pushed past the page edge, the box is left out instead of cut, with a warning.
        let off = "{ to = \"page\", corner = \"top_right\", offset_mm = [-30.0, 20.0] }";
        let (page, report) = ppen_sheet(&with_box(off, ""), "Kurz", as_ppen);
        let outside = SheetNote::NoDescriptionBox {
            course: "Kurz".into(),
            reason: NoBoxReason::OutsidePage,
        };
        assert_eq!(box_notes(&report), vec![&outside]);
        assert!(!outside.is_info() && report.needs_attention());
        assert!(!sheet_texts(&page).contains(&"580 m".to_string()));
        assert!(
            !report
                .notes
                .iter()
                .any(|n| matches!(n, SheetNote::ElementOutsidePage { .. }))
        );
    }

    #[test]
    fn unknown_symbol_id_noted() {
        let (page, report) = ppen_sheet(&with_box(TOP_RIGHT, ""), "Kurz", |c| {
            as_ppen(c);
            let d = c.event.controls[1].description.as_mut().unwrap();
            d.columns[1] = Some("99.9".into());
        });
        let note = SheetNote::UnknownDescriptionSymbol {
            course: "Kurz".into(),
            id: "99.9".into(),
        };
        assert_eq!(box_notes(&report), vec![&note]);
        assert!(!note.is_info());
        // The box is drawn all the same.
        assert!(sheet_texts(&page).contains(&"31".to_string()));
    }

    #[test]
    fn element_over_the_course_is_noted() {
        // In the frame's top-right corner the box covers no part of Kurz.
        let (_, report) = ppen_sheet(&with_box(TOP_RIGHT, ""), "Kurz", as_ppen);
        assert!(
            !report
                .notes
                .iter()
                .any(|n| matches!(n, SheetNote::ElementCoversCourse { .. })),
            "{:?}",
            report.notes
        );
        // In the middle of the frame it lies over control 31, which sits at the frame centre.
        let centre = "{ to = \"map_frame\", corner = \"centre\" }";
        let (_, report) = ppen_sheet(&with_box(centre, ""), "Kurz", as_ppen);
        let note = SheetNote::ElementCoversCourse {
            element: "descriptions".into(),
            controls: vec!["31".into()],
        };
        assert!(report.notes.contains(&note), "{:?}", report.notes);
        assert!(!note.is_info() && report.needs_attention());
        // A text block over the start triangle counts as well: S1 is at (30, 58.5) mm on the page, and the
        // circle through the triangle's corners reaches 7 / √3 + 0.175 = 4.22 mm from it.
        let body = format!(
            "{A4}[[layouts.elements]]\nid = \"t\"\nkind = \"text\"\nlines = [\"X\"]\n\
             anchor = {{ to = \"page\", corner = \"top_left\", offset_mm = [32.0, 50.0] }}\n"
        );
        let (_, report, _) = sheet(&body, "Kurz");
        assert!(
            report.notes.contains(&SheetNote::ElementCoversCourse {
                element: "t".into(),
                controls: vec!["S1".into()],
            }),
            "{:?}",
            report.notes
        );
    }

    /// A project on the map fixture `map` (with `alter` applied to the map after reading it) of `kind`, with
    /// the IOF XML `xml` as course file read y down, the course "K" at 1:`print_scale` on an A4 portrait layout,
    /// and its loaded courses.
    fn project_on(
        map: &str,
        alter: impl FnOnce(&mut MapDocument),
        kind: &str,
        xml: &str,
        print_scale: u32,
    ) -> (tempfile::TempDir, LoadedProject, MapDocument, LoadedCourses) {
        let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let dir = tempfile::tempdir().unwrap();
        std::fs::copy(
            manifest.join("../../tests/fixtures/maps").join(map),
            dir.path().join("m.omap"),
        )
        .unwrap();
        std::fs::write(dir.path().join("c.xml"), xml).unwrap();
        let path = dir.path().join(crate::project::FILE_NAME);
        std::fs::write(
            &path,
            format!(
                "format = 1\n[event]\ntitle = \"T\"\nlang = \"en\"\n[sources]\nmap = \"m.omap\"\ncourses = \"c.xml\"\n\
                 course_y_axis = \"down\"\n[map]\nkind = \"{kind}\"\ncontour_interval_m = 5.0\n[[layouts]]\nid = \"p\"\n\
                 paper = \"A4\"\norientation = \"portrait\"\nprint_scale = {print_scale}\nnorth_lines = \"never\"\n"
            ),
        )
        .unwrap();
        let project = crate::project::load(&path).unwrap();
        let mut map = crate::omap::read::read_map_file(&project.map_path()).unwrap();
        alter(&mut map);
        let courses = crate::courses::load_courses(&project, &map)
            .unwrap()
            .unwrap();
        (dir, project, map, courses)
    }

    /// Start at (80, 40), control 31 at (100, 50), a crossing point at (105, 60) and the finish at (110, 70) map
    /// mm, y down; `scale` is the map scale the file states.
    fn probe_xml(scale: Option<u32>) -> String {
        let scale = scale.map_or(String::new(), |s| format!("<Map><Scale>{s}</Scale></Map>"));
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<CourseData xmlns="http://www.orienteering.org/datastandard/3.0" iofVersion="3.0" creator="test">
  <Event><Name>Probe</Name></Event>
  <RaceCourseData>{scale}
    <Control type="Start"><Id>S1</Id><MapPosition x="80" y="40" unit="mm"/></Control>
    <Control type="Control"><Id>31</Id><MapPosition x="100" y="50" unit="mm"/></Control>
    <Control type="CrossingPoint"><Id>X1</Id><MapPosition x="105" y="60" unit="mm"/></Control>
    <Control type="Finish"><Id>F1</Id><MapPosition x="110" y="70" unit="mm"/></Control>
    <Course><Name>K</Name>
      <CourseControl type="Start"><Control>S1</Control></CourseControl>
      <CourseControl type="Control"><Control>31</Control></CourseControl>
      <CourseControl type="CrossingPoint"><Control>X1</Control></CourseControl>
      <CourseControl type="Finish"><Control>F1</Control></CourseControl>
    </Course>
  </RaceCourseData>
</CourseData>"#
        )
    }

    /// What the course sheet of "K" prints, in mm on paper.
    #[derive(Debug)]
    struct Printed {
        circle_diameter: f64,
        line: f64,
        start_side: f64,
        finish: (f64, f64),
        number_height: f64,
        crossing_reach: f64,
        first_aid: (f64, f64),
        hatch_line: f64,
        forbidden_arm: f64,
    }

    /// The sizes on the sheet of "K" with a first aid post at (90, 60), a single forbidden-route cross at
    /// (85, 70) and an out-of-bounds square from (60, 60) to (70, 70) added.
    fn printed(
        (_dir, project, map, mut courses): (
            tempfile::TempDir,
            LoadedProject,
            MapDocument,
            LoadedCourses,
        ),
    ) -> Printed {
        let at = |x: f64, y: f64| MapCoord::new((x * 1000.0) as i32, (y * 1000.0) as i32);
        let all = crate::course::CourseFilter::All;
        courses.event.specials.extend([
            Special::FirstAid {
                pt: at(90.0, 60.0),
                courses: all.clone(),
                id: None,
            },
            Special::ForbiddenRoute {
                path: vec![at(85.0, 70.0)],
                courses: all.clone(),
                id: None,
            },
            Special::OutOfBounds {
                rings: vec![vec![
                    at(60.0, 60.0),
                    at(70.0, 60.0),
                    at(70.0, 70.0),
                    at(60.0, 70.0),
                ]],
                courses: all,
                id: None,
            },
        ]);
        let course = course(&courses, "K");
        let layout = layout_for(&project.project, "K").0;
        let (page, report) =
            render_course_sheet(&project, &map, &courses, course, layout, &LogoCache::new())
                .unwrap();
        let gf = report.print.geometry_factor;
        let p = |x: f64, y: f64| on_page(&page, gf, at(x, y));
        let d = |a: Pt, b: Pt| (a.0 - b.0).hypot(a.1 - b.1);
        let shapes: Vec<&Shape> = page
            .overprint
            .lower
            .iter()
            .chain(&page.overprint.upper)
            .collect();
        // Strokes of at least `points` points, every one within `r` of `centre`: (largest distance from it,
        // width, points). Radii and point counts keep the legs out.
        let near = |centre: Pt, r: f64, points: usize| -> Vec<(f64, f64, Vec<Pt>)> {
            shapes
                .iter()
                .filter_map(|s| match s {
                    Shape::Stroke { path, width, .. }
                        if path.len() >= points && path.iter().all(|q| d(*q, centre) < r) =>
                    {
                        let far = path.iter().map(|q| d(*q, centre)).fold(0.0, f64::max);
                        Some((far, *width, path.clone()))
                    }
                    _ => None,
                })
                .collect()
        };
        let circle = near(p(100.0, 50.0), 4.5, 9);
        let start = near(p(80.0, 40.0), 6.5, 3);
        let triangle = start.iter().find(|s| s.2.len() == 3).expect("start");
        let mut finish: Vec<f64> = near(p(110.0, 70.0), 5.0, 9)
            .iter()
            .map(|s| 2.0 * s.0)
            .collect();
        finish.sort_by(f64::total_cmp);
        let number_height = shapes
            .iter()
            .find_map(|s| match s {
                Shape::Text { size, .. } => Some(*size),
                _ => None,
            })
            .expect("a number");
        let crossing = near(p(105.0, 60.0), 3.0, 3);
        let first_aid = near(p(90.0, 60.0), 3.5, 2);
        let forbidden = near(p(85.0, 70.0), 3.0, 2);
        // Hatch lines are strokes clipped to the area.
        let hatch = shapes
            .iter()
            .find_map(|s| match s {
                Shape::Clipped { inner, .. } => match inner.as_ref() {
                    Shape::Stroke { width, .. } => Some(*width),
                    _ => None,
                },
                _ => None,
            })
            .expect("hatch");
        Printed {
            circle_diameter: 2.0 * circle[0].0,
            line: circle[0].1,
            start_side: d(triangle.2[0], triangle.2[1]),
            finish: (finish[0], finish[finish.len() - 1]),
            number_height,
            crossing_reach: crossing.iter().map(|s| s.0).fold(0.0, f64::max),
            first_aid: (2.0 * first_aid[0].0, first_aid[0].1),
            hatch_line: hatch,
            forbidden_arm: 2.0 * forbidden.iter().map(|s| s.0).fold(0.0, f64::max),
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn course_symbols_take_the_standard_size_at_the_print_scale() {
        // ruling R35: a forest map drawn at 1:10000 (OOM's sample) prints course symbols at 150 % of the
        // ISOM 2017-2 base values at 1:10000 and at 100 % at 1:15000, whatever scale the map is drawn at.
        let forest = |scale| {
            printed(project_on(
                "forest-sample.omap",
                |_| {},
                "forest",
                &probe_xml(Some(10000)),
                scale,
            ))
        };
        let (at10, at15) = (forest(10000), forest(15000));
        for (p, f) in [(&at10, 1.5), (&at15, 1.0)] {
            assert!(close(p.circle_diameter, 5.0 * f), "{p:?}");
            assert!(close(p.line, 0.35 * f), "{p:?}");
            assert!(close(p.start_side, 6.0 * f), "{p:?}");
            assert!(
                close(p.finish.0, 4.0 * f) && close(p.finish.1, 6.0 * f),
                "{p:?}"
            );
            // Crossing point 710: the ends of its arcs lie (1500, 950)/3000 of its length from the centre.
            assert!(
                close(p.crossing_reach, 1500f64.hypot(950.0) / 3000.0 * 3.0 * f),
                "{p:?}"
            );
            assert!(
                close(p.first_aid.0, 4.0 * f) && close(p.first_aid.1, 1.33 * f),
                "{p:?}"
            );
            assert!(close(p.hatch_line, 0.2 * f), "{p:?}");
            // Out-of-bounds route 711: arms 3.0 mm long (ISOM 2017-2 section 3.7).
            assert!(close(p.forbidden_arm, 3.0 * f), "{p:?}");
        }
        assert!(close(at10.circle_diameter, 7.5) && close(at15.circle_diameter, 5.0));
        assert!(
            close(at10.number_height / at15.number_height, 1.5),
            "{at10:?} {at15:?}"
        );
        // A sprint map drawn at 1:3000 prints its course at 133 % of the ISSprOM 2019-2 base values, as one
        // drawn at 1:4000 does.
        let sprint = |map_scale: u32| {
            printed(project_on(
                "luebeck.omap",
                |m| m.georef.scale_denominator = map_scale,
                "sprint",
                &probe_xml(None),
                3000,
            ))
        };
        for p in [sprint(3000), sprint(4000)] {
            assert!(close(p.circle_diameter, 6.0 * 4.0 / 3.0), "{p:?}");
            assert!(close(p.line, 0.35 * 4.0 / 3.0), "{p:?}");
            assert!(close(p.start_side, 7.0 * 4.0 / 3.0), "{p:?}");
        }
    }

    // --- Masks (ADR 0045) ---

    /// Whether the nonzero rule fills `p` with the rings of `clip`: the map shows there. The winding number of each
    /// ring is counted here from its edges, so this checks the orientation [`MapClip::new`] gives the rings, not
    /// the text of the PDF.
    fn visible(clip: &MapClip, (px, py): Pt) -> bool {
        let mut winding = 0i32;
        for ring in clip.rings() {
            for i in 0..ring.len() {
                let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                let left = (b.0 - a.0) * (py - a.1) - (px - a.0) * (b.1 - a.1);
                if a.1 <= py && b.1 > py && left > 0.0 {
                    winding += 1;
                } else if a.1 > py && b.1 <= py && left < 0.0 {
                    winding -= 1;
                }
            }
        }
        winding != 0
    }

    /// A mask of one polygon, points as written (map mm, y up).
    fn mask(id: &str, outline: Vec<[f64; 2]>, holes: Vec<Vec<[f64; 2]>>) -> Mask {
        Mask {
            id: id.into(),
            polygons: vec![crate::mask::MaskPolygon { outline, holes }],
        }
    }

    /// A square from (`x0`, `y0`) to (`x1`, `y1`), counter-clockwise with y up.
    fn sq(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<[f64; 2]> {
        vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
    }

    #[test]
    fn map_clip_ignores_ring_orientation() {
        // The same mask written counter-clockwise and clockwise, once with a closing point equal to the first:
        // the clip has the same rings with the same orientation on the page, and shows the map in the same places.
        let rev = |mut r: Vec<[f64; 2]>| {
            r.reverse();
            r
        };
        let ccw = mask(
            "m",
            sq(0.0, 0.0, 100.0, 100.0),
            vec![sq(10.0, 10.0, 30.0, 30.0)],
        );
        let cw = mask(
            "m",
            rev(sq(0.0, 0.0, 100.0, 100.0)),
            vec![rev(sq(10.0, 10.0, 30.0, 30.0))],
        );
        let mut closed_outline = sq(0.0, 0.0, 100.0, 100.0);
        closed_outline.push(closed_outline[0]);
        let closed = mask("m", closed_outline, vec![sq(10.0, 10.0, 30.0, 30.0)]);
        let to_page = |(x, y): Pt| (x + 100.0, y + 150.0);
        let frame = PaperBox {
            min: (50.0, 0.0),
            max: (250.0, 200.0),
        };
        // Twice the shoelace sum in page mm (y down): negative runs counter-clockwise on paper.
        let area = |r: &[Pt]| -> f64 {
            (0..r.len())
                .map(|i| {
                    let (p, q) = (r[i], r[(i + 1) % r.len()]);
                    p.0 * q.1 - q.0 * p.1
                })
                .sum()
        };
        let clips: Vec<MapClip> = [&ccw, &cw, &closed]
            .into_iter()
            .map(|m| map_clip(m, frame, to_page))
            .collect();
        for clip in &clips {
            let signs: Vec<bool> = clip.rings().iter().map(|r| area(r) < 0.0).collect();
            // The frame and the hole counter-clockwise on paper, the outline clockwise.
            assert_eq!(signs, [true, false, true]);
        }
        for at in [(5.3, -5.3), (20.3, -20.3), (50.3, -50.3), (120.3, -5.3)] {
            let shown = visible(&clips[0], to_page(at));
            assert_eq!(shown, !crate::mask::contains(&ccw, at), "{at:?}");
            for clip in &clips[1..] {
                assert_eq!(visible(clip, to_page(at)), shown, "{at:?}");
            }
        }
    }

    #[test]
    fn overlapping_holes_unite() {
        // Two windows that overlap, one written counter-clockwise and one clockwise: the clip shows the map in
        // both and in their overlap, and hides it in the outline outside them, exactly where `contains` says the
        // mask is not.
        let mut second = sq(-10.0, -30.0, 50.0, 30.0);
        second.reverse();
        let m = mask(
            "w",
            sq(-100.0, -100.0, 100.0, 100.0),
            vec![sq(-50.0, -30.0, 10.0, 30.0), second],
        );
        // A page transform as a sheet has one: scale 1.5, shifted; the frame wider than the outline.
        let to_page = |(x, y): Pt| (x * 1.5 + 200.0, y * 1.5 + 150.0);
        let frame = PaperBox {
            min: (200.0 - 180.0, 150.0 - 180.0),
            max: (200.0 + 180.0, 150.0 + 180.0),
        };
        let clip = map_clip(&m, frame, to_page);
        assert_eq!(clip.rings().len(), 4);
        let mut checked = 0;
        for i in 0..42 {
            for j in 0..42 {
                // Never on a ring: no sample coordinate is a multiple of 10.
                let at = (-117.3 + f64::from(i) * 5.7, -117.7 + f64::from(j) * 5.7);
                let p = to_page(at);
                assert!(
                    frame.min.0 < p.0
                        && p.0 < frame.max.0
                        && frame.min.1 < p.1
                        && p.1 < frame.max.1
                );
                assert_eq!(
                    visible(&clip, to_page(at)),
                    !crate::mask::contains(&m, at),
                    "{at:?}"
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 42 * 42);
        // The overlap, each window alone, the outline, and the frame outside the outline.
        assert!(visible(&clip, to_page((0.3, 0.3))));
        assert!(visible(&clip, to_page((-40.3, 0.3))));
        assert!(visible(&clip, to_page((40.3, 0.3))));
        assert!(!visible(&clip, to_page((0.3, 60.3))));
        assert!(visible(&clip, to_page((110.3, 110.3))));
    }

    #[test]
    fn mask_hides_the_base_area_outside_windows() {
        // A sprint map with its base area and a mask with one window around control 31 (20, -30 in the file, y
        // up): the base area is a map primitive like the others, painted first, inside the mask's block.
        use crate::print::base_fixture::{base_part, with_walkable_area};
        let body = format!(
            "{A4}[[masks]]\nid = \"w\"\npolygons = [[[[-2000, -2000], [2000, -2000], [2000, 2000], [-2000, 2000]], \
             [[5, -45], [35, -45], [35, -15], [5, -15]]]]\n[[assignments]]\ncourse = \"Kurz\"\nlayout = \"a4\"\n\
             mask = \"w\"\n"
        );
        let (_dir, project, map, courses) = course_project(&body);
        let mut based = map.clone();
        based.symbol_set = with_walkable_area(&map.symbol_set);
        let part = base_part(
            &based.symbol_set,
            (-1_000_000, -1_000_000),
            (1_200_000, 900_000),
        );
        based.parts.insert(0, part);
        let layout = layout_for(&project.project, "Kurz").0;
        let (page, report) = render_course_sheet(
            &project,
            &based,
            &courses,
            course(&courses, "Kurz"),
            layout,
            &LogoCache::new(),
        )
        .unwrap();
        assert!(page.primitives[0].base, "the base area is painted first");
        let [clip] = page.map_clips.as_slice() else {
            panic!("one mask: {:?}", page.map_clips.len())
        };
        let gf = report.print.geometry_factor;
        let at = |x: i32, y: i32| on_page(&page, gf, MapCoord::new(x, y));
        // Control 31 in the window; the start (-50, 60 in the file) and the finish (60, 20) under the mask, both in
        // the frame. The internal frame has y down.
        let frame = page.clip;
        for p in [
            at(20_000, 30_000),
            at(-50_000, -60_000),
            at(60_000, -20_000),
        ] {
            assert!(
                frame.min.0 < p.0 && p.0 < frame.max.0 && frame.min.1 < p.1 && p.1 < frame.max.1
            );
        }
        assert!(visible(clip, at(20_000, 30_000)));
        assert!(!visible(clip, at(-50_000, -60_000)));
        assert!(!visible(clip, at(60_000, -20_000)));
        // In the PDF the first fill, the base area's, comes after the mask's clip.
        let pdf = crate::render::pdf::content_of(
            &crate::render::pdf::write_pdf(&page, &crate::render::pdf::PdfMeta::new("t"))
                .unwrap()
                .0,
        );
        let text = String::from_utf8(pdf).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        let clips: Vec<usize> = (0..lines.len()).filter(|&i| lines[i] == "W").collect();
        // The page clip, then the mask's clip around the map before the lower purple and again after it (R11).
        assert_eq!(clips.len(), 3, "page clip and the mask twice");
        let first_fill = lines.iter().position(|l| l.ends_with(" k")).unwrap();
        assert!(clips[1] < first_fill);
        // The overprint is the same as without the mask.
        let (plain, _, _) = sheet(A4, "Kurz");
        assert_eq!(plain.overprint.upper, page.overprint.upper);
        assert!(plain.map_clips.is_empty());
    }

    #[test]
    fn generated_windows_keep_their_gap_in_the_pdf() {
        // End to end (ADR 0047, ADR 0063): a window mask generated for Kurz, whose controls 31 and 32 are moved 20 mm
        // apart, written to the project file and printed. Windows of 15 mm would overlap, so both shrink to 8.5 mm and
        // keep 3 mm between them: the map shows in both and is hidden in the gap. Hand-written holes that overlap
        // still unite (`overlapping_holes_unite`).
        use crate::mask_build::{DEFAULT_GAP_MM, Selection, WindowOptions, map_extent, windows};
        let (_dir, project, map, mut courses) = course_project(A4);
        let control = courses
            .event
            .controls
            .iter_mut()
            .find(|c| c.code == "32")
            .unwrap();
        // 31 is at (20, -30) with y up, (20 000, 30 000) µm in the internal frame.
        control.position = Some(MapCoord::new(40_000, 30_000));
        let options = WindowOptions {
            selection: Selection::All,
            start: false,
            finish: false,
            radius_mm: 15.0,
            per_control: Default::default(),
            gap_mm: DEFAULT_GAP_MM,
            // The layout prints at the map scale.
            map_mm_per_print_mm: 1.0,
        };
        let (mask, list) = windows(
            "window-Kurz",
            course(&courses, "Kurz"),
            &courses.event,
            &options,
            map_extent(&map),
        )
        .unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.iter().all(|w| w.radius_mm == 8.5), "{list:?}");
        let project =
            crate::project::append_mask(&project.path, &mask, Some(("Kurz", "a4")), "").unwrap();
        let layout = layout_for(&project.project, "Kurz").0;
        let (page, report) = render_course_sheet(
            &project,
            &map,
            &courses,
            course(&courses, "Kurz"),
            layout,
            &LogoCache::new(),
        )
        .unwrap();
        let [clip] = page.map_clips.as_slice() else {
            panic!("one mask: {:?}", page.map_clips.len())
        };
        let gf = report.print.geometry_factor;
        let at = |x: i32, y: i32| on_page(&page, gf, MapCoord::new(x, y));
        // Each centre, and 8 mm out from each, away from the other and towards it.
        for p in [
            (20_000, 30_000),
            (40_000, 30_000),
            (12_000, 30_000),
            (48_000, 30_000),
            (28_000, 30_000),
            (32_000, 30_000),
        ] {
            assert!(visible(clip, at(p.0, p.1)), "{p:?}");
        }
        // The gap from 28.5 to 31.5 mm, beside both windows, 9 mm out from 31, and the start, which has no window
        // by default, are under the mask.
        for p in [
            (30_000, 30_000),
            (28_800, 30_000),
            (31_200, 30_000),
            (11_000, 30_000),
            (30_000, 50_000),
            (30_000, 10_000),
            (4_000, 30_000),
            (-50_000, -60_000),
        ] {
            assert!(!visible(clip, at(p.0, p.1)), "{p:?}");
        }
        // The start and the finish are named by `check`; the controls are not.
        let named: Vec<&crate::courses::StopName> = report
            .notes
            .iter()
            .filter_map(|n| match n {
                SheetNote::ControlInMaskedArea { controls, .. } => Some(controls),
                _ => None,
            })
            .flatten()
            .collect();
        assert_eq!(named, [&start(), &finish()]);
        // The PDF clips the map with the mask (around the map before and after the lower purple).
        let pdf = crate::render::pdf::content_of(
            &crate::render::pdf::write_pdf(&page, &crate::render::pdf::PdfMeta::new("t"))
                .unwrap()
                .0,
        );
        let text = String::from_utf8(pdf).unwrap();
        assert_eq!(text.lines().filter(|l| *l == "W").count(), 3);
    }

    #[test]
    fn white_outs_of_a_course_each_clip_the_map() {
        // Ruling R8: two overlapping white-outs of one course are two masks; the map is hidden in either.
        let (_dir, project, map, mut courses) = course_project(A4);
        for (i, ring) in [sq(-60.0, 30.0, 0.0, 90.0), sq(-20.0, 50.0, 40.0, 100.0)]
            .into_iter()
            .enumerate()
        {
            courses.white_outs.push(crate::courses::WhiteOut {
                mask: mask(&crate::project::white_out_id(i + 1), ring, Vec::new()),
                courses: crate::course::CourseFilter::All,
            });
        }
        let layout = layout_for(&project.project, "Kurz").0;
        let (page, report) = render_course_sheet(
            &project,
            &map,
            &courses,
            course(&courses, "Kurz"),
            layout,
            &LogoCache::new(),
        )
        .unwrap();
        assert_eq!(page.map_clips.len(), 2);
        let gf = report.print.geometry_factor;
        let at = |x: f64, y: f64| {
            on_page(
                &page,
                gf,
                MapCoord::new((x * 1000.0) as i32, (-y * 1000.0) as i32),
            )
        };
        let frame = page.clip;
        let shown = |p: Pt| {
            assert!(
                frame.min.0 < p.0 && p.0 < frame.max.0 && frame.min.1 < p.1 && p.1 < frame.max.1
            );
            page.map_clips.iter().all(|c| visible(c, p))
        };
        assert!(!shown(at(-50.3, 40.3)), "in the first only");
        assert!(!shown(at(30.3, 95.3)), "in the second only");
        assert!(!shown(at(-10.3, 60.3)), "in both");
        assert!(shown(at(60.3, 10.3)), "in neither");
        // Each alone hides its own square only.
        assert!(visible(&page.map_clips[0], at(30.3, 95.3)));
        assert!(visible(&page.map_clips[1], at(-50.3, 40.3)));
    }

    #[test]
    fn masks_beyond_the_cap_are_left_out_with_a_warning() {
        // Ruling R10: 2000 white-outs of one course; the sheet applies the first 1000 and says so. However many
        // masks, the content stream nests no deeper than with one.
        let render = |count: usize| {
            let (_dir, project, map, mut courses) = course_project(A4);
            for i in 0..count {
                let (x, y) = ((i % 50) as f64 * 3.0 - 75.0, (i / 50) as f64 * 3.0 - 60.0);
                courses.white_outs.push(crate::courses::WhiteOut {
                    mask: mask(
                        &crate::project::white_out_id(i + 1),
                        sq(x, y, x + 1.0, y + 1.0),
                        Vec::new(),
                    ),
                    courses: crate::course::CourseFilter::All,
                });
            }
            let layout = layout_for(&project.project, "Kurz").0;
            let (page, report) = render_course_sheet(
                &project,
                &map,
                &courses,
                course(&courses, "Kurz"),
                layout,
                &LogoCache::new(),
            )
            .unwrap();
            let pdf = crate::render::pdf::content_of(
                &crate::render::pdf::write_pdf(&page, &crate::render::pdf::PdfMeta::new("t"))
                    .unwrap()
                    .0,
            );
            let (mut depth, mut max) = (0, 0);
            for line in String::from_utf8(pdf).unwrap().lines() {
                match line {
                    "q" => {
                        depth += 1;
                        max = max.max(depth);
                    }
                    "Q" => depth -= 1,
                    _ => {}
                }
            }
            (page.map_clips.len(), report.notes, max)
        };
        let (applied, notes, depth) = render(2000);
        assert_eq!(applied, MAX_APPLIED_MASKS);
        let capped: Vec<&SheetNote> = notes
            .iter()
            .filter(|n| matches!(n, SheetNote::MasksCapped { .. }))
            .collect();
        assert_eq!(
            capped,
            [&SheetNote::MasksCapped {
                course: "Kurz".into(),
                applied: 1000,
                total: 2000,
            }]
        );
        assert!(!capped[0].is_info(), "a warning");
        let (one, notes, depth_one) = render(1);
        assert_eq!(one, 1);
        assert!(
            !notes
                .iter()
                .any(|n| matches!(n, SheetNote::MasksCapped { .. }))
        );
        assert_eq!(depth, depth_one);
        assert_eq!(render(MAX_APPLIED_MASKS).0, MAX_APPLIED_MASKS);
    }

    // --- check against the masks (ADR 0046) ---

    /// A course coordinate from map mm as written in masks and the course file (y up).
    fn up(x: f64, y: f64) -> MapCoord {
        MapCoord::new((x * 1000.0) as i32, (-y * 1000.0) as i32)
    }

    /// The project body with one mask `w` of `polygons` (TOML) assigned to Kurz on the A4 layout.
    fn with_mask(polygons: &str) -> String {
        format!(
            "{A4}[[masks]]\nid = \"w\"\npolygons = {polygons}\n[[assignments]]\ncourse = \"Kurz\"\n\
             layout = \"a4\"\nmask = \"w\"\n"
        )
    }

    /// The whole map hidden, with the window `window` (TOML ring), e.g. around control 31.
    fn hidden_but(window: &str) -> String {
        with_mask(&format!(
            "[[[[-2000, -2000], [2000, -2000], [2000, 2000], [-2000, 2000]], {window}]]"
        ))
    }

    const AROUND_31: &str = "[[5, -45], [35, -45], [35, -15], [5, -15]]";
    /// A window around the whole course of Kurz and the specials of [`specials_of_every_kind`].
    const AROUND_ALL: &str = "[[-70, -140], [120, -140], [120, 80], [-70, 80]]";

    fn square_ring(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<MapCoord> {
        vec![up(x0, y0), up(x1, y0), up(x1, y1), up(x0, y1)]
    }

    /// Kurz with every kind of object `check` looks at under a mask, all outside the window around 31 and inside
    /// [`AROUND_ALL`]: the leg S1-31 a marked route, control 32 a crossing point, two marked routes, a forbidden
    /// route, a boundary, an out-of-bounds and a dangerous area, a text. The other course does not count.
    fn specials_of_every_kind(courses: &mut LoadedCourses) {
        use crate::course::{ControlKind, CourseFilter};
        let kurz = || CourseFilter::Only(vec!["Kurz".into()]);
        let c = courses
            .event
            .courses
            .iter_mut()
            .find(|c| c.name == "Kurz")
            .unwrap();
        c.legs[0].marked_route = true;
        c.stops[2].kind = Some(ControlKind::CrossingPoint);
        courses.event.specials.extend([
            Special::MarkedRoute {
                path: vec![up(-60.0, 0.0), up(-20.0, 0.0)],
                courses: CourseFilter::All,
                id: None,
            },
            Special::MarkedRoute {
                path: vec![up(-60.0, -60.0), up(-20.0, -60.0)],
                courses: kurz(),
                id: None,
            },
            Special::MarkedRoute {
                path: vec![up(-60.0, -70.0), up(-20.0, -70.0)],
                courses: CourseFilter::Only(vec!["D/H 12".into()]),
                id: None,
            },
            Special::ForbiddenRoute {
                path: vec![up(-30.0, 30.0)],
                courses: kurz(),
                id: None,
            },
            Special::Boundary {
                path: vec![up(70.0, -60.0), up(90.0, -60.0)],
                courses: kurz(),
                id: None,
            },
            Special::OutOfBounds {
                rings: vec![square_ring(60.0, -100.0, 80.0, -80.0)],
                courses: kurz(),
                id: None,
            },
            Special::Dangerous {
                rings: vec![square_ring(-40.0, -110.0, -30.0, -100.0)],
                courses: kurz(),
                id: None,
            },
            Special::Text {
                pt: up(0.0, 40.0),
                text: "Wasser".into(),
                size_mm: 3.0,
                courses: kurz(),
                id: None,
            },
        ]);
    }

    /// The stops of Kurz as `ControlInMaskedArea` names them: the start and the finish by kind.
    fn start() -> crate::courses::StopName {
        crate::courses::StopName::Start("S1".into())
    }

    fn finish() -> crate::courses::StopName {
        crate::courses::StopName::Finish("F1".into())
    }

    fn control(code: &str) -> crate::courses::StopName {
        crate::courses::StopName::Control(code.into())
    }

    /// The notes about masks of a sheet report.
    fn mask_check_notes(report: &SheetReport) -> Vec<&SheetNote> {
        report
            .notes
            .iter()
            .filter(|n| {
                matches!(
                    n,
                    SheetNote::OverprintInMaskedArea { .. }
                        | SheetNote::ControlInMaskedArea { .. }
                        | SheetNote::MaskCoversNothing { .. }
                        | SheetNote::WhiteOutsCoverNothing { .. }
                        | SheetNote::MaskLeavesNoWindow { .. }
                        | SheetNote::MaskCheckLimited { .. }
                )
            })
            .collect()
    }

    fn in_mask(kind: super::super::MaskedObject, count: usize) -> SheetNote {
        SheetNote::OverprintInMaskedArea {
            course: "Kurz".into(),
            kind,
            count,
        }
    }

    #[test]
    fn check_warns_for_control_in_masked_area() {
        // Only control 31 has a window; the start, 32 and the finish lie in the frame where the map is hidden.
        let (_, report) = ppen_sheet(&hidden_but(AROUND_31), "Kurz", |_| {});
        let notes = mask_check_notes(&report);
        let want = SheetNote::ControlInMaskedArea {
            course: "Kurz".into(),
            controls: vec![start(), control("32"), finish()],
        };
        assert_eq!(notes, [&want]);
        assert!(!want.is_info(), "a warning");
        assert!(report.needs_attention());
        // The overprint is drawn all the same.
        let (plain, _) = ppen_sheet(A4, "Kurz", |_| {});
        let (masked, _) = ppen_sheet(&hidden_but(AROUND_31), "Kurz", |_| {});
        assert_eq!(plain.overprint.lower, masked.overprint.lower);
        assert_eq!(plain.overprint.upper, masked.overprint.upper);
    }

    #[test]
    fn check_warns_for_overprint_in_masked_area() {
        use super::super::MaskedObject::*;
        let (page, report) = ppen_sheet(&hidden_but(AROUND_31), "Kurz", specials_of_every_kind);
        let notes = mask_check_notes(&report);
        let want = [
            SheetNote::ControlInMaskedArea {
                course: "Kurz".into(),
                controls: vec![start(), finish()],
            },
            // The leg from S1 and two of the three marked routes; the third belongs to D/H 12.
            in_mask(MarkedRoute, 3),
            in_mask(CrossingPoint, 1),
            in_mask(ForbiddenRoute, 1),
            in_mask(Boundary, 1),
            in_mask(OutOfBoundsArea, 1),
            in_mask(DangerousArea, 1),
            in_mask(Text, 1),
        ];
        assert_eq!(notes, want.iter().collect::<Vec<_>>());
        assert!(want.iter().all(|n| !n.is_info()));
        // Masked or not, every object is drawn.
        let (plain, _) = ppen_sheet(A4, "Kurz", specials_of_every_kind);
        assert_eq!(plain.overprint.upper, page.overprint.upper);
        assert_eq!(plain.overprint.lower, page.overprint.lower);
    }

    #[test]
    fn no_warning_when_the_overprint_lies_in_a_window() {
        let (_, report) = ppen_sheet(&hidden_but(AROUND_ALL), "Kurz", specials_of_every_kind);
        assert_eq!(mask_check_notes(&report), Vec::<&SheetNote>::new());
        // Without a mask there is nothing to check.
        let (_, plain) = ppen_sheet(A4, "Kurz", specials_of_every_kind);
        assert_eq!(mask_check_notes(&plain), Vec::<&SheetNote>::new());
    }

    #[test]
    fn an_area_round_a_window_counts_by_its_inside() {
        // An out-of-bounds area whose outline lies wholly in a window, while a second white-out hides a square
        // inside it: its hatch is drawn over blank paper there.
        let (_, report) = ppen_sheet(A4, "Kurz", |c| {
            let masks = [
                mask(
                    "whiteout-1",
                    sq(-2000.0, -2000.0, 2000.0, 2000.0),
                    vec![sq(-70.0, -140.0, 120.0, 80.0)],
                ),
                mask("whiteout-2", sq(0.0, -60.0, 20.0, -40.0), Vec::new()),
            ];
            for m in masks {
                c.white_outs.push(crate::courses::WhiteOut {
                    mask: m,
                    courses: crate::course::CourseFilter::All,
                });
            }
            c.event.specials.push(Special::OutOfBounds {
                rings: vec![square_ring(-10.0, -70.0, 30.0, -35.0)],
                courses: crate::course::CourseFilter::All,
                id: None,
            })
        });
        assert_eq!(
            mask_check_notes(&report),
            [&in_mask(super::super::MaskedObject::OutOfBoundsArea, 1)]
        );
    }

    #[test]
    fn mask_covers_nothing_is_info() {
        let far = with_mask("[[[[1000, 1000], [1100, 1000], [1100, 1100], [1000, 1100]]]]");
        let (_, report) = ppen_sheet(&far, "Kurz", specials_of_every_kind);
        let want = SheetNote::MaskCoversNothing {
            course: "Kurz".into(),
            mask: "w".into(),
        };
        assert_eq!(mask_check_notes(&report), [&want]);
        assert!(want.is_info());
        let (_, plain) = ppen_sheet(A4, "Kurz", specials_of_every_kind);
        assert_eq!(report.needs_attention(), plain.needs_attention());
        // A frame wholly inside a window sees nothing hidden either.
        let (_, report) = ppen_sheet(
            &hidden_but("[[-1000, -1000], [1000, -1000], [1000, 1000], [-1000, 1000]]"),
            "Kurz",
            |_| {},
        );
        assert_eq!(mask_check_notes(&report), [&want]);
    }

    #[test]
    fn description_box_over_a_window_is_noted() {
        // A window course: the box in the frame's top-right corner. With the only window round control 31 at the
        // frame centre, the box lies over masked paper only; with a window over the whole frame it covers part of
        // that window and hides map the sheet would show (ruling R15).
        let covers = |report: &SheetReport| -> Vec<SheetNote> {
            report
                .notes
                .iter()
                .filter(|n| matches!(n, SheetNote::ElementCoversWindow { .. }))
                .cloned()
                .collect()
        };
        let boxed = |window: &str| {
            format!(
                "{}[[masks]]\nid = \"w\"\npolygons = [[[[-2000, -2000], [2000, -2000], [2000, 2000], \
                 [-2000, 2000]], {window}]]\n[[assignments]]\ncourse = \"Kurz\"\nlayout = \"a4\"\nmask = \"w\"\n",
                with_box(TOP_RIGHT, "")
            )
        };
        let (_, report) = ppen_sheet(&boxed(AROUND_31), "Kurz", as_ppen);
        assert_eq!(covers(&report), Vec::<SheetNote>::new());
        let (_, report) = ppen_sheet(
            &boxed("[[-1000, -1000], [1000, -1000], [1000, 1000], [-1000, 1000]]"),
            "Kurz",
            as_ppen,
        );
        let over = |element: &str| SheetNote::ElementCoversWindow {
            course: "Kurz".into(),
            element: element.into(),
            mask: "w".into(),
        };
        // The layout has no passe-partout, so the footer sits in the frame and covers the window too.
        assert_eq!(covers(&report), [over("descriptions"), over("footer")]);
        assert!(!over("descriptions").is_info() && report.needs_attention());
        // No mask, no such note.
        let (_, report) = ppen_sheet(&with_box(TOP_RIGHT, ""), "Kurz", as_ppen);
        assert_eq!(covers(&report), Vec::<SheetNote>::new());
    }

    #[test]
    fn white_outs_that_hide_nothing_fold_into_one_line() {
        // Three white-outs of the course file far outside the frame: one Info line for the course, with all three
        // ids, instead of one per white-out (ruling R12).
        let far = |i: f64| {
            mask(
                "",
                sq(1000.0 + 200.0 * i, 1000.0, 1100.0 + 200.0 * i, 1100.0),
                vec![],
            )
        };
        let add = |c: &mut LoadedCourses, n: usize| {
            for i in 0..n {
                let mut m = far(i as f64);
                m.id = format!("whiteout-{}", i + 1);
                c.white_outs.push(crate::courses::WhiteOut {
                    mask: m,
                    courses: crate::course::CourseFilter::All,
                });
            }
        };
        let (_, report) = ppen_sheet(A4, "Kurz", |c| add(c, 3));
        let want = SheetNote::WhiteOutsCoverNothing {
            course: "Kurz".into(),
            ids: vec![
                "whiteout-1".into(),
                "whiteout-2".into(),
                "whiteout-3".into(),
            ],
        };
        assert_eq!(mask_check_notes(&report), [&want]);
        assert!(want.is_info());
        // A `[[masks]]` entry standing in for the second keeps a line of its own.
        let stand_in = format!(
            "{A4}[[masks]]\nid = \"whiteout-2\"\npolygons = [[[[900, 900], [950, 900], [950, 950]]]]\n"
        );
        let (_, report) = ppen_sheet(&stand_in, "Kurz", |c| add(c, 3));
        assert_eq!(
            mask_check_notes(&report),
            [
                &SheetNote::MaskCoversNothing {
                    course: "Kurz".into(),
                    mask: "whiteout-2".into(),
                },
                &SheetNote::WhiteOutsCoverNothing {
                    course: "Kurz".into(),
                    ids: vec!["whiteout-1".into(), "whiteout-3".into()],
                },
            ]
        );
    }

    #[test]
    fn mask_leaves_no_window_is_warning() {
        let all = with_mask("[[[[-2000, -2000], [2000, -2000], [2000, 2000], [-2000, 2000]]]]");
        let (_, report) = ppen_sheet(&all, "Kurz", |_| {});
        let none_left = SheetNote::MaskLeavesNoWindow {
            course: "Kurz".into(),
            mask: "w".into(),
        };
        let controls = SheetNote::ControlInMaskedArea {
            course: "Kurz".into(),
            controls: vec![start(), control("31"), control("32"), finish()],
        };
        assert_eq!(mask_check_notes(&report), [&none_left, &controls]);
        assert!(!none_left.is_info());
        // A window in the frame, however small, leaves the map there.
        let (_, report) = ppen_sheet(&hidden_but(AROUND_31), "Kurz", |_| {});
        assert!(!report.notes.contains(&none_left));
    }

    #[test]
    fn any_of_several_masks_hides() {
        // Ruling R8: two white-outs, one over control 31, one over the finish. Each hides its own control.
        let (_, report) = ppen_sheet(A4, "Kurz", |courses| {
            for (i, ring) in [sq(5.0, -45.0, 35.0, -15.0), sq(50.0, 10.0, 70.0, 30.0)]
                .into_iter()
                .enumerate()
            {
                courses.white_outs.push(crate::courses::WhiteOut {
                    mask: mask(&crate::project::white_out_id(i + 1), ring, Vec::new()),
                    courses: crate::course::CourseFilter::All,
                });
            }
        });
        assert_eq!(
            mask_check_notes(&report),
            [&SheetNote::ControlInMaskedArea {
                course: "Kurz".into(),
                controls: vec![control("31"), finish()],
            }]
        );
    }

    #[test]
    fn sampling_budget_is_bounded() {
        // A hostile marked route: 10 000 points zigzagging across the window, kilometres of path. The check stops
        // at its budget and says so; it finds nothing hidden, because the route stays in the window. The check is
        // called on its own: drawing the route's dashes takes far longer than checking it.
        let (_dir, project, _map, mut courses) = course_project(&hidden_but(AROUND_ALL));
        let path = (0..10_000)
            .map(|i| {
                if i % 2 == 0 {
                    up(-60.0, -130.0)
                } else {
                    up(110.0, 70.0)
                }
            })
            .collect();
        courses.event.specials.push(Special::MarkedRoute {
            path,
            courses: crate::course::CourseFilter::All,
            id: None,
        });
        let assigned = mask_for(&project.project, &courses, "Kurz");
        let masks: Vec<Applied<'_>> = assigned
            .iter()
            .map(|m| Applied {
                mask: m.mask(),
                white_out: false,
            })
            .collect();
        // Page = map mm shifted, the frame 200 by 280 mm round the course.
        let to_page = |c: MapCoord| {
            (
                f64::from(c.x) / 1000.0 + 100.0,
                f64::from(c.y) / 1000.0 + 140.0,
            )
        };
        let page_to_map = |(x, y): Pt| (x - 100.0, y - 140.0);
        let frame = PaperBox {
            min: (0.0, 0.0),
            max: (200.0, 280.0),
        };
        let kind = project.project.map.kind;
        let started = std::time::Instant::now();
        let notes = super::super::mask_check::mask_notes(
            course(&courses, "Kurz"),
            &courses.event,
            &masks,
            frame,
            &to_page,
            &page_to_map,
            &project.rules.course_symbol(kind),
            1.0,
        );
        let limited = SheetNote::MaskCheckLimited {
            course: "Kurz".into(),
        };
        assert_eq!(notes, std::slice::from_ref(&limited));
        assert!(!limited.is_info());
        assert!(started.elapsed() < std::time::Duration::from_secs(20));
    }

    #[test]
    fn unused_mask_is_printed_once_per_run() {
        let body =
            format!("{A4}[[masks]]\nid = \"spare\"\npolygons = [[[[0, 0], [10, 0], [10, 10]]]]\n");
        let (_dir, _project, _map, courses) = course_project(&body);
        let unused = SheetNote::Mask(crate::courses::MaskNote::MaskUnused { id: "spare".into() });
        assert!(course_event_notes(&courses).contains(&unused));
        assert!(unused.is_info());
    }

    #[test]
    fn a_masked_course_names_one_mask_id_per_map_clip() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/luebeck-training-window");
        let loaded = crate::project::load(&dir.join("freemapper-print.toml")).unwrap();
        let map = crate::omap::read::read_map_file(&loaded.map_path()).unwrap();
        let courses = crate::courses::load_courses(&loaded, &map)
            .unwrap()
            .unwrap();
        let course = courses
            .event
            .courses
            .iter()
            .find(|c| c.name == "Fenster")
            .unwrap();
        let (layout, _) = crate::courses::layout_for(&loaded.project, &course.name);
        let (page, report) = render_course_sheet(
            &loaded,
            &map,
            &courses,
            course,
            layout,
            &crate::sheet::LogoCache::new(),
        )
        .unwrap();
        assert!(!page.map_clips.is_empty());
        assert_eq!(report.mask_ids.len(), page.map_clips.len());
        assert_eq!(report.mask_ids, ["window-Fenster"]);
        assert!(!report.numbers.is_empty());
    }
}
