//! Course sheets: one course on its layout (design §5.7). The sheet is the layout's sheet with the frame centred
//! on the course unless the layout sets `centre_map_mm`, the course placeholders filled, and the course overprint
//! on the map (ADR 0034).

use std::collections::HashSet;

use crate::course::{Course, CourseKind, FittedEvent, LoadedCourses, Special, layout_for};
use crate::omap::{MapCoord, MapDocument};
use crate::overprint::{applies, control_count, file_numbers, overprint};
use crate::project::{LayoutSpec, LoadedProject};
use crate::render::page::{Pt, RenderedPage};

use super::placeholders::CourseValues;
use super::{LogoCache, SheetError, SheetNote, SheetReport, render_body};

/// The sheet of `course` (one of `courses`) on `layout`: the map as [`super::render_sheet`] draws it, centred on
/// [`course_centre`] when the layout has no `centre_map_mm`, with the course overprint and the course
/// placeholders. Numbers sit where the course file puts them, else due east of their circle
/// ([`file_numbers`]).
///
/// Notes beyond the layout sheet's: [`SheetNote::CourseOutsideFrame`], [`SheetNote::DefaultLayout`] when
/// `layout` is the first layout taken for a course without assignment, [`SheetNote::NoDescriptionBox`],
/// [`SheetNote::OverprintCut`] and [`SheetNote::CourseValueUnknown`]. The notes of the course file as a whole
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
    let centre = course_centre(course, event);
    let (mut page, mut report) = render_body(project, map, layout, logos, Some(&values), centre)?;

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
    let numbers = file_numbers(course, event, to_page, &sizes, sf);
    page.overprint = overprint(course, event, &numbers, to_page, &sizes, sf).on_map(
        project.rules.purple_cmyk(),
        &page.colours,
        &page.primitives,
    );

    let notes = &mut report.notes;
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
    if page.overprint.cut {
        notes.push(SheetNote::OverprintCut {
            course: course.name.clone(),
        });
    }
    // Task 9 draws the box for `.ppen` courses.
    notes.push(SheetNote::NoDescriptionBox {
        course: course.name.clone(),
    });
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

/// The notes of the course file as a whole, once per run: what the reader left out or found, what the fit
/// found, and the entries of the project file that no longer match the course file.
pub fn course_event_notes(courses: &LoadedCourses) -> Vec<SheetNote> {
    let course = courses.notes.iter().cloned().map(SheetNote::Course);
    let fit = courses.fit_notes.iter().cloned().map(SheetNote::Fit);
    let stale = courses.stale.iter().cloned().map(SheetNote::Stale);
    course.chain(fit).chain(stale).collect()
}

/// The points of `course` that the frame should show: its stops, the bends of its legs and the points of the
/// special objects that apply to it (white-out is not drawn yet and does not count).
fn course_points<'a>(
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
    let specials = event.specials.iter().flat_map(move |special| {
        let points: Vec<MapCoord> = match special {
            Special::OutOfBounds { rings, courses } | Special::Dangerous { rings, courses }
                if applies(courses, &course.name) =>
            {
                rings.iter().flatten().copied().collect()
            }
            Special::ForbiddenRoute { path, courses }
            | Special::Boundary { path, courses }
            | Special::MarkedRoute { path, courses }
            | Special::Line { path, courses }
                if applies(courses, &course.name) =>
            {
                path.clone()
            }
            Special::FirstAid { pt, courses }
            | Special::Water { pt, courses }
            | Special::Text { pt, courses, .. }
                if applies(courses, &course.name) =>
            {
                vec![*pt]
            }
            _ => Vec::new(),
        };
        points
    });
    stops.chain(bends).chain(specials)
}

/// The centre of the box around [`course_points`] (map coordinates), or `None` for a course without any.
pub fn course_centre(course: &Course<MapCoord>, event: &FittedEvent) -> Option<MapCoord> {
    let mut points = course_points(course, event);
    let first = points.next()?;
    let (mut min, mut max) = ((first.x, first.y), (first.x, first.y));
    for p in points {
        min = (min.0.min(p.x), min.1.min(p.y));
        max = (max.0.max(p.x), max.1.max(p.y));
    }
    let mid = |a: i32, b: i32| ((i64::from(a) + i64::from(b)) / 2) as i32;
    Some(MapCoord::new(mid(min.0, max.0), mid(min.1, max.1)))
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
///    device name Windows reserves gets a `-`;
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
    if reserved_on_windows(&stem) {
        stem.push('-');
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
        assert!(report.notes.contains(&SheetNote::NoDescriptionBox {
            course: "Kurz".into()
        }));
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
        let centre = course_centre(course(&courses, "Kurz"), &courses.event).unwrap();
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
        let centre = course_centre(kurz, &courses.event).unwrap();
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
        // A layout sheet does not know the course placeholders.
        let (_, report) = super::super::render_sheet(&project, &map, layout, &logos).unwrap();
        assert!(
            report.notes.iter().any(
                |n| matches!(n, SheetNote::UnknownPlaceholder { name, .. } if name == "course")
            )
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
        assert_eq!(stem("Ça va? Øre"), "Ça va- Øre");
        let long = stem(&"ü".repeat(200));
        assert_eq!(long.len(), MAX_STEM_BYTES);
        assert_eq!(
            nfc_latin1("A\u{30A}c\u{327}y\u{308}x\u{308}"),
            "Åçÿx\u{308}"
        );
    }
}
