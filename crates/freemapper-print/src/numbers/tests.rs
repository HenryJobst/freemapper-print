use super::*;
use crate::courses::{LoadedCourses, layout_for};
use crate::omap::MapDocument;
use crate::render::page::{FillRule, Primitive, RenderColour};
use crate::sheet::course_sheet::course_project;
use crate::sheet::{LogoCache, SheetNote, SheetReport, render_course_sheet};

const A4: &str = "[[layouts]]\nid = \"a4\"\norientation = \"portrait\"\nnorth_lines = \"never\"\n";

/// A4 with the frame moved so that control 32 (map x 100 mm) lies 10 mm from the right edge of the frame
/// (x 202 mm on the page): the frame centre (page x 105) shows map x 13.
const A4_32_AT_EDGE: &str = "[[layouts]]\nid = \"a4\"\norientation = \"portrait\"\nnorth_lines = \"never\"\n\
                             map_frame = { centre_map_mm = [13.0, -30.0] }\n";

fn course<'a>(courses: &'a LoadedCourses, name: &str) -> &'a Course<MapCoord> {
    courses
        .event
        .courses
        .iter()
        .find(|c| c.name == name)
        .unwrap()
}

fn sheet_of(
    project: &LoadedProject,
    map: &MapDocument,
    courses: &LoadedCourses,
    name: &str,
) -> (RenderedPage, SheetReport) {
    let layout = layout_for(&project.project, name).0;
    render_course_sheet(
        project,
        map,
        courses,
        course(courses, name),
        layout,
        &LogoCache::new(),
    )
    .unwrap()
}

fn number_conflicts(report: &SheetReport) -> Vec<&NumberConflict> {
    report
        .notes
        .iter()
        .filter_map(|n| match n {
            SheetNote::NumberConflict(c) => Some(c),
            _ => None,
        })
        .collect()
}

/// A `[[numbers]]` entry.
fn correction(control: &str, scope: &str, direction: f64, gap: Option<f64>) -> String {
    let gap = gap.map(|g| format!("gap_mm = {g}\n")).unwrap_or_default();
    format!("[[numbers]]\ncontrol = \"{control}\"\n{scope}\ndirection = {direction}\n{gap}")
}

/// The map position of control `code`.
fn position(courses: &LoadedCourses, code: &str) -> MapCoord {
    courses
        .event
        .controls
        .iter()
        .find(|c| c.code == code)
        .and_then(|c| c.position)
        .unwrap()
}

#[test]
fn file_offset_round_trips_at_the_print_scale() {
    let sizes = crate::rules::base().course_symbol(crate::scale::MapKind::Sprint);
    let offset = Offset {
        dx_mm: 8.0,
        dy_mm: -6.0,
    };
    // At the map scale the box centre lies at the offset itself, on the ray to it.
    let p = file_placement(offset, "12", &sizes, 1.0);
    assert!(
        (p.direction - (6f64).atan2(8.0).to_degrees()).abs() < 1e-9,
        "{p:?}"
    );
    let size = number_size("12", sizes.number_height_mm);
    let b = placement::number_box((0.0, 0.0), r_outer(&sizes, 1.0), size, p);
    assert!((b.centre().0 - 8.0).abs() < 1e-9 && (b.centre().1 + 6.0).abs() < 1e-9);
    // Purple Pen prints the course at 1:5000 from a 1:4000 map without item scaling (ratio 5000/4000): the
    // symbols take 1.25 mm of map per nominal mm, so the 10 mm of map are 8 mm at the nominal size.
    let at_5000 = file_placement(offset, "12", &sizes, 1.25);
    let b = placement::number_box((0.0, 0.0), r_outer(&sizes, 1.0), size, at_5000);
    assert!((b.centre().0 - 6.4).abs() < 1e-9 && (b.centre().1 + 4.8).abs() < 1e-9);
    assert!(at_5000.gap_mm < p.gap_mm);
}

#[test]
fn levels_resolve_most_specific_wins() {
    let body = format!(
        "{A4}[[layouts]]\nid = \"a3\"\npaper = \"A3\"\norientation = \"portrait\"\n{}{}{}{}",
        // 31: a layout correction with a gap, and one for the sheet of Kurz without a gap.
        correction("31", "layout = \"a4\"", 180.0, Some(1.0)),
        correction("31", "course = \"Kurz\"", 270.0, None),
        // 32: a layout correction without a gap over an automatic number.
        correction("32", "layout = \"a4\"", 90.0, None),
        // 33: a correction for another layout does not apply.
        correction("33", "layout = \"a3\"", 0.0, Some(2.0)),
    );
    let (_dir, project, _, courses) = course_project(&body);
    let kurz = resolve(course(&courses, "Kurz"), &courses.event, &project, "a4");
    let sizes = project.rules.course_symbol(project.project.map.kind);
    // The file puts 31's number 4 mm right and 3 mm up (IOF XML, y up).
    let base = file_placement(
        Offset {
            dx_mm: 4.0,
            dy_mm: -3.0,
        },
        "1",
        &sizes,
        1.0,
    );
    assert_eq!(kurz.len(), 2);
    assert_eq!(
        (kurz[0].control.as_str(), kurz[0].text.as_str()),
        ("31", "1")
    );
    assert_eq!(kurz[0].level, Level::Sheet);
    assert_eq!(kurz[0].base, Some(base));
    assert_eq!(
        kurz[0].placement,
        Placement {
            direction: 270.0,
            gap_mm: 1.0
        }
    );
    assert_eq!(kurz[1].level, Level::Layout);
    assert_eq!(kurz[1].base, None);
    assert_eq!(
        kurz[1].placement,
        Placement {
            direction: 90.0,
            gap_mm: automatic_start(gap_range(&project.rules)).gap_mm
        }
    );
    // D/H 12 on the same layout: 31 takes the layout's correction only, 33 is automatic.
    let dh = resolve(course(&courses, "D/H 12"), &courses.event, &project, "a4");
    assert_eq!(dh[0].control, "33");
    assert_eq!(
        (dh[0].level, dh[0].placement),
        (Level::Automatic, automatic_start(gap_range(&project.rules)))
    );
    assert_eq!(dh[1].level, Level::Layout);
    assert_eq!(
        dh[1].placement,
        Placement {
            direction: 180.0,
            gap_mm: 1.0
        }
    );
    // On the A3 layout 33's correction applies. The file gives 31 a position in Kurz only (per course), so in
    // D/H 12 it is automatic there.
    let dh = resolve(course(&courses, "D/H 12"), &courses.event, &project, "a3");
    assert_eq!(dh[0].level, Level::Layout);
    assert_eq!((dh[1].level, dh[1].base), (Level::Automatic, None));
    let kurz = resolve(course(&courses, "Kurz"), &courses.event, &project, "a3");
    let expected = Placement {
        direction: 270.0,
        gap_mm: base.gap_mm,
    };
    assert_eq!((kurz[0].level, kurz[0].placement), (Level::Sheet, expected));
    // Without corrections: the file's position, else automatic.
    let (_dir, project, _, courses) = course_project(A4);
    let kurz = resolve(course(&courses, "Kurz"), &courses.event, &project, "a4");
    assert_eq!(kurz[0].level, Level::CourseFile);
    assert_eq!(kurz[1].level, Level::Automatic);
}

#[test]
fn file_positions_draw_where_the_file_puts_them() {
    // Without corrections the number of 31 sits at the file's offset: 4 mm right and 3 mm up of the circle
    // centre, at 1:4000 on a 1:4000 map.
    let (_dir, project, map, courses) = course_project(A4);
    let (page, report) = sheet_of(&project, &map, &courses, "Kurz");
    let gf = report.print.geometry_factor;
    let to_page = |c: MapCoord| {
        (
            page.page_mm.0 / 2.0 + f64::from(c.x) / 1000.0 * gf - page.centre_mm.0,
            page.page_mm.1 / 2.0 + f64::from(c.y) / 1000.0 * gf - page.centre_mm.1,
        )
    };
    let at = to_page(position(&courses, "31"));
    let centre = text_centre(&page, "1");
    assert!(
        (centre.0 - (at.0 + 4.0)).abs() < 1e-6 && (centre.1 - (at.1 - 3.0)).abs() < 1e-6,
        "{centre:?} {at:?}"
    );
}

/// The centre of the box of the control number `text` drawn in the upper purple of `page`.
fn text_centre(page: &RenderedPage, text: &str) -> Pt {
    let sizes = crate::rules::base().course_symbol(crate::scale::MapKind::Sprint);
    page.overprint
        .upper
        .iter()
        .find_map(|s| match s {
            Shape::Text {
                origin, text: t, ..
            } if t == text => {
                let (w, h) = number_size(text, sizes.number_height_mm);
                Some((origin.0 + w / 2.0, origin.1 - h / 2.0))
            }
            _ => None,
        })
        .unwrap()
}

#[test]
fn check_reports_number_outside_frame() {
    // 32 lies 10 mm from the right edge of the frame; its number 5 mm east of the circle reaches past it.
    let body = format!(
        "{A4_32_AT_EDGE}{}",
        correction("32", "layout = \"a4\"", 0.0, Some(5.0))
    );
    let (_dir, project, map, courses) = course_project(&body);
    let (page, report) = sheet_of(&project, &map, &courses, "Kurz");
    let conflicts = number_conflicts(&report);
    assert_eq!(conflicts.len(), 1, "{:?}", report.notes);
    let c = conflicts[0];
    assert_eq!((c.course.as_str(), c.control.as_str()), ("Kurz", "32"));
    assert_eq!(c.conflicts, [Conflict::OutsideFrame]);
    // The proposal lies inside the frame, within the profile's gaps.
    assert!((0.5..=5.0).contains(&c.proposal.gap_mm), "{:?}", c.proposal);
    let note = SheetNote::NumberConflict(c.clone());
    assert!(!note.is_info() && report.needs_attention());
    assert!(text_centre(&page, "2").0 > page.clip.max.0 - 1.0);
}

#[test]
fn check_reports_number_on_other_circle() {
    // 32 moved 8 mm east of 31; a number due east of 31 lies on 32's circle and on the leg between them.
    let body = format!(
        "{A4}{}",
        correction("31", "layout = \"a4\"", 0.0, Some(0.3))
    );
    let (_dir, project, map, mut courses) = course_project(&body);
    let at = position(&courses, "31");
    for c in &mut courses.event.controls {
        if c.code == "32" {
            c.position = Some(MapCoord::new(at.x + 8_000, at.y));
        }
    }
    let (_, report) = sheet_of(&project, &map, &courses, "Kurz");
    let conflicts = number_conflicts(&report);
    let c = conflicts.iter().find(|c| c.control == "31").unwrap();
    assert!(
        c.conflicts.contains(&Conflict::Symbol("32".into())),
        "{c:?}"
    );
    assert!(
        c.conflicts
            .contains(&Conflict::Leg("31".into(), "32".into())),
        "{c:?}"
    );
}

#[test]
fn numbers_never_moved_silently() {
    // A correction that conflicts is drawn where it says; the note carries a different proposal.
    let body = format!(
        "{A4_32_AT_EDGE}{}",
        correction("32", "layout = \"a4\"", 0.0, Some(5.0))
    );
    let (_dir, project, map, courses) = course_project(&body);
    let (page, report) = sheet_of(&project, &map, &courses, "Kurz");
    let gf = report.print.geometry_factor;
    let sf = report.print.symbol_factor;
    let c32 = position(&courses, "32");
    let at = (
        page.page_mm.0 / 2.0 + f64::from(c32.x) / 1000.0 * gf - page.centre_mm.0,
        page.page_mm.1 / 2.0 + f64::from(c32.y) / 1000.0 * gf - page.centre_mm.1,
    );
    let sizes = project.rules.course_symbol(project.project.map.kind);
    let size = number_size("2", sizes.number_height_mm * sf);
    let drawn = placement::number_box(
        at,
        r_outer(&sizes, sf),
        size,
        Placement {
            direction: 0.0,
            gap_mm: 5.0 * sf,
        },
    );
    let centre = text_centre(&page, "2");
    assert!(
        (centre.0 - drawn.centre().0).abs() < 1e-6 && (centre.1 - drawn.centre().1).abs() < 1e-6,
        "{centre:?} {drawn:?}"
    );
    let proposal = number_conflicts(&report)[0].proposal;
    assert_ne!(
        Placement::from(proposal),
        Placement {
            direction: 0.0,
            gap_mm: 5.0
        }
    );
}

#[test]
fn proposal_is_conflict_free() {
    // Every number of both Lübeck courses gets a sheet correction pointing along its outgoing leg, so each one
    // lies on a leg. The proposals, accepted as sheet corrections, give sheets without number conflicts.
    let (_dir, _, _, courses) = course_project(A4);
    let mut body = A4.to_string();
    for name in ["Kurz", "D/H 12"] {
        let c = course(&courses, name);
        for pair in c.stops.windows(2) {
            let (from, to) = (
                position(&courses, &pair[0].control),
                position(&courses, &pair[1].control),
            );
            if !pair[0].control.starts_with('3') {
                continue;
            }
            let (dx, dy) = (f64::from(to.x - from.x), f64::from(from.y - to.y));
            let direction = placement::normalize_direction(dy.atan2(dx).to_degrees());
            body.push_str(&correction(
                &pair[0].control,
                &format!("course = \"{name}\""),
                (direction * 10.0).round() / 10.0,
                Some(0.3),
            ));
        }
    }
    let (_dir, project, map, courses) = course_project(&body);
    let mut accepted = A4.to_string();
    let mut seen = 0;
    for name in ["Kurz", "D/H 12"] {
        let (_, report) = sheet_of(&project, &map, &courses, name);
        let conflicts = number_conflicts(&report);
        assert_eq!(conflicts.len(), 2, "{name}: {:?}", report.notes);
        for c in conflicts {
            assert!(
                c.conflicts.iter().any(|k| matches!(k, Conflict::Leg(..))),
                "{c:?}"
            );
            accepted.push_str(&correction(
                &c.control,
                &format!("course = \"{name}\""),
                c.proposal.direction,
                Some(c.proposal.gap_mm),
            ));
            seen += 1;
        }
    }
    assert_eq!(seen, 4);
    let (_dir, project, map, courses) = course_project(&accepted);
    for name in ["Kurz", "D/H 12"] {
        let (_, report) = sheet_of(&project, &map, &courses, name);
        assert!(
            number_conflicts(&report).is_empty(),
            "{name}: {:?}",
            report.notes
        );
    }
}

#[test]
fn automatic_numbers_are_free_and_behind() {
    // Kurz: 32 has no position in the file. It is placed free of conflicts, and behind the circle when that is
    // free: the incoming leg from 31 runs south-east, so behind is south-east too.
    let (_dir, project, map, courses) = course_project(A4);
    let (page, report) = sheet_of(&project, &map, &courses, "Kurz");
    assert!(number_conflicts(&report).is_empty(), "{:?}", report.notes);
    let gf = report.print.geometry_factor;
    let to_page = |c: MapCoord| {
        (
            page.page_mm.0 / 2.0 + f64::from(c.x) / 1000.0 * gf - page.centre_mm.0,
            page.page_mm.1 / 2.0 + f64::from(c.y) / 1000.0 * gf - page.centre_mm.1,
        )
    };
    let (a, b) = (
        to_page(position(&courses, "31")),
        to_page(position(&courses, "32")),
    );
    let behind = placement::direction_to(a, b).unwrap();
    let centre = text_centre(&page, "2");
    let drawn = placement::direction_to(b, centre).unwrap();
    assert!(
        placement::angle_diff(drawn, behind) < 0.5,
        "{drawn} {behind}"
    );
}

#[test]
fn corrections_over_a_changed_base_are_stale() {
    let (_dir, project, _, _) = course_project(A4);
    let sizes = project.rules.course_symbol(project.project.map.kind);
    let base = file_placement(
        Offset {
            dx_mm: 4.0,
            dy_mm: -3.0,
        },
        "1",
        &sizes,
        1.0,
    );
    let with_seen = |seen: Placement, scope: &str| {
        format!(
            "{A4}{}base_seen = {{ direction = {}, gap_mm = {} }}\n",
            correction("31", scope, 90.0, None),
            seen.direction,
            seen.gap_mm
        )
    };
    let stale = |body: &str| {
        let (_dir, _, _, courses) = course_project(body);
        courses
            .stale
            .iter()
            .filter(|s| matches!(s, Stale::CorrectionOverridesChangedBase { .. }))
            .count()
    };
    // Seen as the file has it now: nothing to report, for a sheet or a layout correction.
    assert_eq!(stale(&with_seen(base, "course = \"Kurz\"")), 0);
    assert_eq!(stale(&with_seen(base, "layout = \"a4\"")), 0);
    // The file moved the number by more than the tolerance since.
    let moved = Placement {
        direction: base.direction + 5.0,
        ..base
    };
    assert_eq!(stale(&with_seen(moved, "course = \"Kurz\"")), 1);
    assert_eq!(stale(&with_seen(moved, "layout = \"a4\"")), 1);
}

#[test]
fn obstacles_of_map_symbols_by_class() {
    let square = |x: f64| vec![(x, 0.0), (x + 1.0, 0.0), (x + 1.0, 1.0), (x, 1.0)];
    let mut point = Primitive::new(
        1,
        Shape::Fill {
            rings: vec![square(0.0)],
            rule: FillRule::EvenOdd,
        },
    );
    point.class = SymbolClass::Point;
    point.object = 7;
    let mut line = Primitive::new(
        2,
        Shape::Stroke {
            path: vec![(0.0, 2.0), (5.0, 2.0)],
            closed: false,
            width: 0.2,
            cap: crate::render::page::Cap::Flat,
            join: crate::render::page::Join::Miter,
        },
    );
    line.class = SymbolClass::Line;
    line.object = 8;
    let mut north = line.clone();
    north.north_line = true;
    let area = Primitive::new(
        3,
        Shape::Fill {
            rings: vec![square(2.0)],
            rule: FillRule::EvenOdd,
        },
    );
    let far = Primitive::new(
        3,
        Shape::Fill {
            rings: vec![square(500.0)],
            rule: FillRule::EvenOdd,
        },
    );
    // The base area of a sprint map covers everything, and is no obstacle: it is the ground, like white paper.
    let mut base = Primitive::new(
        4,
        Shape::Fill {
            rings: vec![vec![
                (-50.0, -50.0),
                (300.0, -50.0),
                (300.0, 350.0),
                (-50.0, 350.0),
            ]],
            rule: FillRule::EvenOdd,
        },
    );
    base.base = true;
    let colour = |priority: i32, name: &str, cmyk: [f64; 4]| RenderColour {
        priority,
        name: name.into(),
        cmyk,
        rgb: [0.0; 3],
        opacity: 1.0,
    };
    let page = RenderedPage {
        primitives: vec![base, point, line, north, area, far],
        clip: PaperBox {
            min: (0.0, 0.0),
            max: (210.0, 297.0),
        },
        page_mm: (210.0, 297.0),
        centre_mm: (0.0, 0.0),
        colours: vec![
            colour(1, "Black 100%", [0.0, 0.0, 0.0, 1.0]),
            colour(2, "Brown 100%", [0.25, 0.75, 1.0, 0.0]),
            colour(3, "Black for buildings", [0.0, 0.0, 0.0, 1.0]),
            colour(
                4,
                "Lower brown 30% for walkable area",
                [0.06, 0.23, 0.33, 0.0],
            ),
        ],
        sheet: Vec::new(),
        overprint: Default::default(),
        map_clips: Vec::new(),
    };
    let course = CourseShapes {
        symbols: vec![(1, (50.0, 50.0), 3.175)],
        legs: vec![(0, vec![(40.0, 50.0), (46.0, 50.0)])],
        half_line: 0.175,
    };
    let elements = vec![(
        "title".to_string(),
        PaperBox {
            min: (0.0, 280.0),
            max: (210.0, 297.0),
        },
    )];
    let weights = PlacementWeights {
        black: 10.0,
        brown: 4.0,
        blue: 4.0,
        other: 1.0,
    };
    let region = Rect {
        min: (-10.0, -10.0),
        max: (20.0, 20.0),
    };
    let o = obstacles(&page, &course, &elements, &weights, &[region]);
    let tiers: Vec<(Tier, &str)> = o.iter().map(|o| (o.tier, o.label.as_str())).collect();
    assert_eq!(
        tiers,
        [
            (Tier::Overprint, "s1"),
            (Tier::Overprint, "l0"),
            (Tier::Overprint, "e0"),
            (Tier::MapSymbol, "p7/0"),
            (Tier::MapSymbol, "m8/0"),
            (Tier::Soft, ""),
        ]
    );
    // "Black for buildings" is not canonical, but its CMYK is full black.
    assert_eq!(o[5].weight, 10.0);
}

#[test]
fn gaps_outside_the_rule_are_reported_and_kept() {
    // iof-print allows 0.5 to 5 mm (ruling R29). A correction of 0.3 mm and the file's position of 31 in Kurz
    // (its box touches the circle: a gap just below 0) lie outside; 32's correction of 1 mm lies inside.
    let body = format!(
        "{A4}{}{}",
        correction("32", "layout = \"a4\"", 300.0, Some(1.0)),
        correction("33", "layout = \"a4\"", 180.0, Some(0.3)),
    );
    let (_dir, project, map, courses) = course_project(&body);
    let gaps = |name: &str| -> Vec<(String, f64)> {
        let (_, report) = sheet_of(&project, &map, &courses, name);
        report
            .notes
            .iter()
            .filter_map(|n| match n {
                SheetNote::NumberGapOutOfRange {
                    control,
                    gap_mm,
                    min,
                    max,
                    ..
                } => {
                    assert_eq!((*min, *max), (0.5, 5.0));
                    assert!(!n.is_info());
                    Some((control.clone(), *gap_mm))
                }
                _ => None,
            })
            .collect()
    };
    let kurz = gaps("Kurz");
    assert_eq!(kurz.len(), 1, "{kurz:?}");
    assert_eq!(kurz[0].0, "31");
    assert!(kurz[0].1 < 0.5);
    // D/H 12: 33's correction; 31 is automatic there and always within the range.
    assert_eq!(gaps("D/H 12"), [("33".to_string(), 0.3)]);
}

#[test]
fn automatic_gaps_stay_in_the_rule_range() {
    let (_dir, project, _, courses) = course_project(A4);
    let gaps = gap_range(&project.rules);
    assert_eq!((gaps.min, gaps.max), (0.5, 5.0));
    for name in ["Kurz", "D/H 12"] {
        let c = course(&courses, name);
        let resolved = resolve(c, &courses.event, &project, "a4");
        for r in resolved.iter().filter(|r| r.level == Level::Automatic) {
            assert_eq!(r.placement, automatic_start(gaps));
        }
    }
    // Without a rule the fixed gap of 0.5 mm.
    assert_eq!(GapRange::DEFAULT.steps(), [placement::DEFAULT_GAP_MM]);
}

/// The numbers of course `name` placed on a blank 400 mm page holding `primitives`, map mm + 200 on the page,
/// with the profile's gap range. Covered symbols are named "S<id>".
fn on_blank_page(
    project: &LoadedProject,
    courses: &LoadedCourses,
    name: &str,
    primitives: Vec<Primitive>,
) -> SheetNumbers {
    on_blank_page_within(
        project,
        courses,
        name,
        primitives,
        MAX_EVALUATIONS_PER_SHEET,
    )
}

/// [`on_blank_page`] with a search budget of `budget` candidate evaluations.
fn on_blank_page_within(
    project: &LoadedProject,
    courses: &LoadedCourses,
    name: &str,
    primitives: Vec<Primitive>,
    budget: usize,
) -> SheetNumbers {
    let c = course(courses, name);
    let resolved = resolve(c, &courses.event, project, "a4");
    let page = RenderedPage {
        primitives,
        clip: PaperBox {
            min: (0.0, 0.0),
            max: (400.0, 400.0),
        },
        page_mm: (400.0, 400.0),
        centre_mm: (0.0, 0.0),
        colours: Vec::new(),
        sheet: Vec::new(),
        overprint: Default::default(),
        map_clips: Vec::new(),
    };
    let to_page = |m: MapCoord| {
        (
            f64::from(m.x) / 1000.0 + 200.0,
            f64::from(m.y) / 1000.0 + 200.0,
        )
    };
    place_on_sheet(
        c,
        &courses.event,
        resolved,
        &page,
        to_page,
        &[],
        &project.rules.course_symbol(project.project.map.kind),
        1.0,
        &project.rules.placement_weights(),
        number_gap(&project.rules),
        &|id| format!("S{id}"),
        budget,
    )
}

/// A point symbol primitive of object `object` and symbol `symbol`: a filled square of side `side` at `at`.
fn point_symbol(at: Pt, side: f64, object: u32, symbol: u32) -> Primitive {
    let h = side / 2.0;
    let mut p = Primitive::new(
        1,
        Shape::Fill {
            rings: vec![vec![
                (at.0 - h, at.1 - h),
                (at.0 + h, at.1 - h),
                (at.0 + h, at.1 + h),
                (at.0 - h, at.1 + h),
            ]],
            rule: FillRule::EvenOdd,
        },
    );
    p.class = SymbolClass::Point;
    p.object = object;
    p.symbol = symbol;
    p
}

#[test]
fn covered_map_symbol_always_warns_and_names_it() {
    // Ruling R31: a number on a point symbol is a conflict even where the file put it, and the note names the
    // symbol and proposes a free place.
    let (_dir, project, _, courses) = course_project(A4);
    let blank = on_blank_page(&project, &courses, "Kurz", Vec::new());
    assert!(blank.conflicts.is_empty(), "{:?}", blank.conflicts);
    let at = blank.numbers[0].centre;
    let with = on_blank_page(
        &project,
        &courses,
        "Kurz",
        vec![point_symbol(at, 0.6, 5, 38)],
    );
    let c = &with.conflicts[0];
    assert_eq!(c.control, "31");
    assert_eq!(
        c.conflicts,
        [Conflict::MapSymbol {
            line: false,
            code: "S38".into(),
            count: 1
        }]
    );
    assert!(c.free, "{c:?}");
}

#[test]
fn no_free_place_proposes_the_least_covered() {
    // A point symbol under the whole course: no candidate is cleaner than the number's own place. The number
    // still gets a warning (before ruling R31 it got none), marked as having no free place.
    let (_dir, project, _, courses) = course_project(A4);
    let everywhere = point_symbol((200.0, 200.0), 390.0, 9, 41);
    let placed = on_blank_page(&project, &courses, "Kurz", vec![everywhere]);
    assert_eq!(placed.conflicts.len(), 2, "{:?}", placed.conflicts);
    for c in &placed.conflicts {
        assert!(!c.free, "{c:?}");
        assert!(c.conflicts.contains(&Conflict::MapSymbol {
            line: false,
            code: "S41".into(),
            count: 1
        }));
    }
    for n in &placed.numbers {
        assert!(n.proposal.1.hard.is_empty() && n.proposal.1.covered.len() == 1);
    }
}

#[test]
fn proposals_in_stop_order_take_different_places() {
    // Ruling R32. A score course visits 31 twice, and one correction puts both numbers on the same spot. Each
    // number conflicts with the other; proposed one after the other, they get different free places.
    let body = format!(
        "{A4}{}",
        correction("31", "layout = \"a4\"", 0.0, Some(0.5))
    );
    let (_dir, project, map, mut courses) = course_project(&body);
    for c in &mut courses.event.courses {
        if c.name == "Kurz" {
            c.kind = crate::course::CourseKind::Score;
            let again = c.stops[1].clone();
            c.stops.insert(2, again);
            c.stops.remove(3);
        }
    }
    assert_eq!(
        course(&courses, "Kurz")
            .stops
            .iter()
            .map(|s| s.control.as_str())
            .collect::<Vec<_>>(),
        ["S1", "31", "31", "F1"]
    );
    let (page, report) = sheet_of(&project, &map, &courses, "Kurz");
    let conflicts = number_conflicts(&report);
    assert_eq!(conflicts.len(), 2, "{:?}", report.notes);
    let gf = report.print.geometry_factor;
    let sf = report.print.symbol_factor;
    let c31 = position(&courses, "31");
    let at = (
        page.page_mm.0 / 2.0 + f64::from(c31.x) / 1000.0 * gf - page.centre_mm.0,
        page.page_mm.1 / 2.0 + f64::from(c31.y) / 1000.0 * gf - page.centre_mm.1,
    );
    let sizes = project.rules.course_symbol(project.project.map.kind);
    let size = number_size("31", sizes.number_height_mm * sf);
    let boxes: Vec<Rect> = conflicts
        .iter()
        .map(|c| {
            assert!(c.free, "{c:?}");
            let p = Placement::from(c.proposal);
            placement::number_box(
                at,
                r_outer(&sizes, sf),
                size,
                Placement {
                    gap_mm: p.gap_mm * sf,
                    ..p
                },
            )
        })
        .collect();
    assert!(!boxes[0].overlaps(&boxes[1]), "{boxes:?}");
}

#[test]
fn crossing_points_have_no_gap_check() {
    // A crossing point prints no number, so a correction for it with a gap outside the range is not checked.
    let body = format!(
        "{A4}{}",
        correction("32", "course = \"Kurz\"", 0.0, Some(0.3))
    );
    let (_dir, project, map, mut courses) = course_project(&body);
    for c in &mut courses.event.controls {
        if c.code == "32" {
            c.kind = crate::course::ControlKind::CrossingPoint;
        }
    }
    for c in &mut courses.event.courses {
        for stop in c.stops.iter_mut().filter(|s| s.control == "32") {
            stop.kind = Some(crate::course::ControlKind::CrossingPoint);
        }
    }
    let (_, report) = sheet_of(&project, &map, &courses, "Kurz");
    assert!(
        !report.notes.iter().any(|n| matches!(
            n,
            SheetNote::NumberGapOutOfRange { control, .. } if control == "32"
        )),
        "{:?}",
        report.notes
    );
    assert!(number_conflicts(&report).iter().all(|c| c.control != "32"));
}

#[test]
fn search_budget_bounds_the_work() {
    // Ruling R38. Kurz: 31 has the file's position, 32 is automatic; a point symbol under the whole course puts
    // both in conflict, so the sheet runs three searches: 32's placement and two proposals.
    let (_dir, project, _, courses) = course_project(A4);
    let everywhere = || vec![point_symbol((200.0, 200.0), 390.0, 9, 41)];
    let search = gap_range(&project.rules).steps().len() * placement::CANDIDATES;
    let full = on_blank_page_within(&project, &courses, "Kurz", everywhere(), 3 * search);
    assert!(!full.search_limited);
    assert_eq!(full.conflicts.len(), 2);
    assert!(full.conflicts.iter().all(|c| c.searched));
    let automatic = |n: &SheetNumbers| {
        n.numbers
            .iter()
            .find(|n| n.resolved.level == Level::Automatic)
            .unwrap()
            .resolved
            .placement
    };
    // One search less: 32 is placed as before, the last proposal is left out.
    let short = on_blank_page_within(&project, &courses, "Kurz", everywhere(), 3 * search - 1);
    assert!(short.search_limited);
    assert_eq!(automatic(&short), automatic(&full));
    assert_eq!(short.conflicts[0], full.conflicts[0]);
    let last = &short.conflicts[1];
    assert!(!last.searched && !last.free, "{last:?}");
    assert_eq!(last.conflicts, full.conflicts[1].conflicts);
    // No budget: 32 keeps the automatic start, and every conflict is still reported, without a proposal.
    let none = on_blank_page_within(&project, &courses, "Kurz", everywhere(), 0);
    assert!(none.search_limited);
    assert_eq!(automatic(&none), automatic_start(gap_range(&project.rules)));
    assert_eq!(none.conflicts.len(), 2);
    assert!(none.conflicts.iter().all(|c| !c.searched && !c.free));
}

#[test]
fn sheet_budget_shares_the_file_budget() {
    assert_eq!(sheet_budget(0), MAX_EVALUATIONS_PER_SHEET);
    assert_eq!(sheet_budget(1), MAX_EVALUATIONS_PER_SHEET);
    assert_eq!(sheet_budget(100), MAX_EVALUATIONS_PER_SHEET);
    assert_eq!(sheet_budget(1000), MAX_EVALUATIONS_PER_FILE / 1000);
    assert!(1000 * sheet_budget(1000) <= MAX_EVALUATIONS_PER_FILE);
}

#[test]
fn grid_finds_every_obstacle_near_a_box_in_order() {
    let boxes = vec![
        Rect::centred((10.0, 10.0), (1.0, 1.0)),
        // Wide: spans more cells than the grid files per box.
        Rect::centred((100.0, 100.0), (200.0, 200.0)),
        Rect::centred((50.0, 12.0), (2.0, 2.0)),
        Rect {
            min: (f64::NAN, 0.0),
            max: (1.0, 1.0),
        },
        Rect::centred((11.0, 9.0), (1.0, 1.0)),
    ];
    let grid = placement::BoxGrid::new(&boxes, GRID_CELL_MM, GRID_MAX_CELLS, GRID_WIDE_CELLS);
    assert_eq!(
        grid.query(&Rect::centred((10.5, 9.5), (2.0, 2.0))),
        [0, 1, 3, 4]
    );
    assert_eq!(
        grid.query(&Rect::centred((50.0, 12.0), (0.5, 0.5))),
        [1, 2, 3]
    );
    // Far outside the grid: the wide and the odd ones only, and clamped cells never panic.
    let far = grid.query(&Rect::centred((1e6, -1e6), (1.0, 1.0)));
    assert!(far.contains(&1) && far.contains(&3));
}
