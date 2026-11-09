//! Course sheets: one course on its layout (design §5.7). The sheet is the layout's sheet with the frame centred
//! on the course unless the layout sets `centre_map_mm`, the course placeholders filled, the course overprint
//! on the map (ADR 0034) and the control description box (ADR 0037).

use std::collections::HashSet;

use crate::course::{
    Course, CourseKind, FittedEvent, LoadedCourses, SourceFormat, Special, layout_for,
};
use crate::describe::{DescriptionHeader, description_box, has_descriptions, moved};
use crate::numbers::{place_on_sheet, resolve};
use crate::omap::{MapCoord, MapDocument};
use crate::overprint::{applies, control_count, overprint};
use crate::print::Orientation;
use crate::project::{ElementKind, ElementSpec, LayoutSpec, LoadedProject};
use crate::render::page::{Pt, RenderedPage};
use crate::scale::geometry_factor;

use super::layout::{anchor_point, inside};
use super::placeholders::CourseValues;
use super::{
    LogoCache, NoBoxReason, PlacedElement, SheetContext, SheetError, SheetNote, SheetReport,
    layout_elements, render_body,
};

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
/// [`SheetNote::OverprintCut`], [`SheetNote::CourseValueUnknown`], [`SheetNote::NumberConflict`] and
/// [`SheetNote::NumberGapOutOfRange`]. The notes of the course file as a whole
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
    let sf = report.print.symbol_factor;
    // Numbers sit where their level puts them (spec §5.7); a conflict is reported, never moved away.
    let placed = place_on_sheet(
        course,
        event,
        resolve(course, event, project, &layout.id, map_scale),
        &page,
        to_page,
        &rects,
        &sizes,
        sf,
        &project.rules.placement_weights(),
        project
            .rules
            .number_gap()
            .map(|(min, max)| crate::placement::GapRange { min, max }),
        &|id| {
            map.symbol_set
                .code_of(id)
                .map_or_else(|| format!("#{id}"), str::to_string)
        },
    );
    let centres: Vec<(String, Pt)> = placed
        .numbers
        .iter()
        .map(|n| (n.resolved.control.clone(), n.centre))
        .collect();
    page.overprint = overprint(course, event, &centres, to_page, &sizes, sf).on_map(
        project.rules.purple_cmyk(),
        &page.colours,
        &page.primitives,
    );

    let notes = &mut report.notes;
    notes.extend(placed.conflicts.into_iter().map(SheetNote::NumberConflict));
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
    let r = (sizes.circle_diameter_mm / 2.0 + sizes.line_width_mm / 2.0) * sf;
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
        &rects, course, event, &to_page, &sizes, sf,
    ));
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
/// around it out to the outer edge of its line at the symbol factor `sf`: the control circle, the outer finish
/// circle, and the circle through the corners of the start (or map exchange) triangle, so a box may count just
/// before it touches a triangle's side. Crossing points and ends of marked routes do not count.
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

/// The notes of the course file as a whole, once per run: what the reader left out or found, what the fit
/// found, and the entries of the project file that no longer match the course file.
pub fn course_event_notes(courses: &LoadedCourses) -> Vec<SheetNote> {
    let course = courses.notes.iter().cloned().map(SheetNote::Course);
    let fit = courses.fit_notes.iter().cloned().map(SheetNote::Fit);
    let stale = courses.stale.iter().cloned().map(SheetNote::Stale);
    course.chain(fit).chain(stale).collect()
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

/// The points of `special` when it applies to the course named `course` (white-out is not drawn yet and does
/// not count).
fn special_points(special: &Special<MapCoord>, course: &str) -> Vec<MapCoord> {
    match special {
        Special::OutOfBounds { rings, courses } | Special::Dangerous { rings, courses }
            if applies(courses, course) =>
        {
            rings.iter().flatten().copied().collect()
        }
        Special::ForbiddenRoute { path, courses }
        | Special::Boundary { path, courses }
        | Special::MarkedRoute { path, courses }
        | Special::Line { path, courses }
            if applies(courses, course) =>
        {
            path.clone()
        }
        Special::FirstAid { pt, courses }
        | Special::Water { pt, courses }
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
    let courses = crate::course::load_courses(&project, &map)
        .unwrap()
        .unwrap();
    (dir, project, map, courses)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::page::{Shape, SheetItem};

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
}
