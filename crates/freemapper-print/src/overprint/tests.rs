use std::f64::consts::PI;

use super::*;
use crate::course::model::Offset;
use crate::course::{CourseEvent, CourseFilter, CourseSource, Leg, SourceFormat, Special, Stop};
use crate::render::draw::FillRule;
use crate::render::page::{PaperBox, Primitive, RenderedPage};
use crate::render::pdf::{PdfMeta, content_of, write_pdf};
use crate::scale::{MapKind, geometry_factor, symbol_factor};

fn sizes() -> CourseSymbolSizes {
    crate::rules::base().course_symbol(MapKind::Sprint)
}

fn um(mm: f64) -> i32 {
    (mm * 1000.0).round() as i32
}

fn control(code: &str, kind: ControlKind, x_mm: f64, y_mm: f64) -> Control<MapCoord> {
    Control {
        code: code.into(),
        kind,
        position: Some(MapCoord::new(um(x_mm), um(y_mm))),
        geo: None,
        description: None,
    }
}

fn stop(code: &str) -> Stop {
    Stop {
        control: code.into(),
        kind: None,
        number: None,
        number_text_at: None,
        circle_gaps: Vec::new(),
    }
}

fn leg() -> Leg<MapCoord> {
    Leg {
        bends: Vec::new(),
        gaps: Vec::new(),
        marked_route: false,
    }
}

fn course(kind: CourseKind, stops: Vec<Stop>) -> Course<MapCoord> {
    let legs = (1..stops.len()).map(|_| leg()).collect();
    Course {
        name: "A".into(),
        family: None,
        kind,
        legs,
        stops,
        climb_m: None,
        length_m: None,
        print_scale_hint: None,
    }
}

fn event(controls: Vec<Control<MapCoord>>, specials: Vec<Special<MapCoord>>) -> FittedEvent {
    CourseEvent {
        controls,
        courses: Vec::new(),
        specials,
        source: CourseSource {
            format: SourceFormat::PurplePen,
            event_name: None,
            map_scale: None,
        },
        notes: Vec::new(),
    }
}

/// Map µm → page mm at geometry factor `gf`, with the map origin at (100, 100) on the page.
fn page_of(gf: f64) -> impl Fn(MapCoord) -> Pt {
    move |c: MapCoord| {
        (
            f64::from(c.x) / 1000.0 * gf + 100.0,
            f64::from(c.y) / 1000.0 * gf + 100.0,
        )
    }
}

fn dist(a: Pt, b: Pt) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// The circles around `centre` among `shapes`: radius and line width of every stroke whose points all
/// lie at one distance from `centre`.
fn circles(shapes: &[Shape], centre: Pt) -> Vec<(f64, f64)> {
    shapes
        .iter()
        .filter_map(|s| match s {
            Shape::Stroke { path, width, .. } if path.len() > 8 => {
                let r = dist(path[0], centre);
                path.iter()
                    .all(|p| (dist(*p, centre) - r).abs() < 1e-9)
                    .then_some((r, *width))
            }
            _ => None,
        })
        .collect()
}

/// The open strokes of two points or more that are no circle arcs: legs and lines.
fn lines(shapes: &[Shape]) -> Vec<(Vec<Pt>, f64)> {
    shapes
        .iter()
        .filter_map(|s| match s {
            Shape::Stroke {
                path,
                closed: false,
                width,
                ..
            } if path.len() <= 8 => Some((path.clone(), *width)),
            _ => None,
        })
        .collect()
}

fn texts(shapes: &[Shape]) -> Vec<(Pt, f64, String)> {
    shapes
        .iter()
        .filter_map(|s| match s {
            Shape::Text {
                origin, size, text, ..
            } => Some((*origin, *size, text.clone())),
            _ => None,
        })
        .collect()
}

/// Start at (0, 0), control 31 at (60, 0) and finish at (120, 0) map mm.
fn three_stops() -> (Course<MapCoord>, FittedEvent) {
    let ev = event(
        vec![
            control("S1", ControlKind::Start, 0.0, 0.0),
            control("31", ControlKind::Control, 60.0, 0.0),
            control("F1", ControlKind::Finish, 120.0, 0.0),
        ],
        Vec::new(),
    );
    let c = course(CourseKind::Normal, vec![stop("S1"), stop("31"), stop("F1")]);
    (c, ev)
}

#[test]
fn circle_and_line_sizes_scale_with_symbol_factor() {
    let sf = symbol_factor(MapKind::Sprint, 4000, 3000).unwrap();
    let gf = geometry_factor(4000, 3000);
    assert!((sf - 4.0 / 3.0).abs() < 1e-12, "{sf}");
    let (c, ev) = three_stops();
    let o = overprint(&c, &ev, &[], page_of(gf), &sizes(), sf);
    let centre = (60.0 * gf + 100.0, 100.0);
    let found = circles(&o.lower, centre);
    assert_eq!(found.len(), 1, "{found:?}");
    let (r, width) = found[0];
    assert!(
        (2.0 * r - 6.0 * 4.0 / 3.0).abs() < 1e-9,
        "diameter {}",
        2.0 * r
    );
    assert!((width - 0.35 * 4.0 / 3.0).abs() < 1e-9, "width {width}");
    // The legs are as wide as the circle line.
    let legs = lines(&o.lower);
    assert_eq!(legs.len(), 2, "{legs:?}");
    assert!(legs.iter().all(|(_, w)| (w - 0.35 * sf).abs() < 1e-9));
    // The finish: inner and outer circle.
    let mut finish = circles(&o.lower, (120.0 * gf + 100.0, 100.0));
    finish.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert_eq!(finish.len(), 2, "{finish:?}");
    assert!((2.0 * finish[0].0 - 5.0 * sf).abs() < 1e-9);
    assert!((2.0 * finish[1].0 - 7.0 * sf).abs() < 1e-9);
}

#[test]
fn start_points_to_first_control() {
    let ev = event(
        vec![
            control("S1", ControlKind::Start, 0.0, 0.0),
            control("31", ControlKind::Control, 30.0, 40.0),
        ],
        Vec::new(),
    );
    let c = course(CourseKind::Normal, vec![stop("S1"), stop("31")]);
    let o = overprint(&c, &ev, &[], page_of(1.0), &sizes(), 1.0);
    let triangle = o
        .lower
        .iter()
        .find_map(|s| match s {
            Shape::Stroke {
                path, closed: true, ..
            } if path.len() == 3 => Some(path.clone()),
            _ => None,
        })
        .expect("a start triangle");
    let centroid = (
        triangle.iter().map(|p| p.0).sum::<f64>() / 3.0,
        triangle.iter().map(|p| p.1).sum::<f64>() / 3.0,
    );
    assert!(dist(centroid, (100.0, 100.0)) < 1e-9, "{centroid:?}");
    for i in 0..3 {
        let side = dist(triangle[i], triangle[(i + 1) % 3]);
        assert!((side - 7.0).abs() < 1e-9, "side {side}");
    }
    // The apex is the corner farthest towards the control, and it lies on the line to it.
    let towards = (40.0_f64).atan2(30.0);
    let apex = triangle
        .iter()
        .max_by(|a, b| {
            let d = |p: &Pt| (p.0 - 100.0) * towards.cos() + (p.1 - 100.0) * towards.sin();
            d(a).total_cmp(&d(b))
        })
        .unwrap();
    let angle = (apex.1 - 100.0).atan2(apex.0 - 100.0);
    assert!((angle - towards).abs() < 1e-9, "{angle} vs {towards}");
}

#[test]
fn leg_stops_at_circles_and_honours_gaps() {
    let ev = event(
        vec![
            control("31", ControlKind::Control, 0.0, 0.0),
            control("32", ControlKind::Control, 100.0, 0.0),
        ],
        Vec::new(),
    );
    let mut c = course(CourseKind::Normal, vec![stop("31"), stop("32")]);
    c.legs[0].gaps = vec![(20.0, 30.0)];
    // At the map scale, and at twice the map's size: the gap is map mm along the leg from the first
    // control's centre and grows with the geometry, the circles only with the symbol factor.
    for gf in [1.0, 2.0] {
        let o = overprint(&c, &ev, &[], page_of(gf), &sizes(), 1.0);
        let mut pieces: Vec<(f64, f64)> = lines(&o.lower)
            .iter()
            .map(|(p, _)| {
                assert!(p.iter().all(|q| (q.1 - 100.0).abs() < 1e-9));
                (p[0].0 - 100.0, p[p.len() - 1].0 - 100.0)
            })
            .collect();
        pieces.sort_by(|a, b| a.0.total_cmp(&b.0));
        let want = [(3.0, 20.0 * gf), (30.0 * gf, 100.0 * gf - 3.0)];
        assert_eq!(pieces.len(), 2, "{pieces:?}");
        for (got, want) in pieces.iter().zip(want) {
            assert!(
                (got.0 - want.0).abs() < 1e-9 && (got.1 - want.1).abs() < 1e-9,
                "gf {gf}: {got:?} vs {want:?}"
            );
        }
    }
    // A bend: the leg runs through it, and the gap counts along the bent leg.
    c.legs[0].bends = vec![MapCoord::new(0, um(-50.0))];
    let ev = event(
        vec![
            control("31", ControlKind::Control, 0.0, 0.0),
            control("32", ControlKind::Control, 50.0, -50.0),
        ],
        Vec::new(),
    );
    c.legs[0].gaps = vec![(45.0, 55.0)];
    let o = overprint(&c, &ev, &[], page_of(1.0), &sizes(), 1.0);
    let pieces = lines(&o.lower);
    assert_eq!(pieces.len(), 2, "{pieces:?}");
    let first = &pieces[0].0;
    assert!(dist(first[0], (100.0, 97.0)) < 1e-9, "{first:?}");
    assert!(
        dist(*first.last().unwrap(), (100.0, 55.0)) < 1e-9,
        "{first:?}"
    );
    let second = &pieces[1].0;
    assert!(dist(second[0], (105.0, 50.0)) < 1e-9, "{second:?}");
    assert!(
        dist(*second.last().unwrap(), (147.0, 50.0)) < 1e-9,
        "{second:?}"
    );
}

#[test]
fn short_leg_not_drawn() {
    let ev = event(
        vec![
            control("31", ControlKind::Control, 0.0, 0.0),
            control("32", ControlKind::Control, 5.0, 0.0),
        ],
        Vec::new(),
    );
    let c = course(CourseKind::Normal, vec![stop("31"), stop("32")]);
    let o = overprint(&c, &ev, &[], page_of(1.0), &sizes(), 1.0);
    assert!(lines(&o.lower).is_empty(), "{:?}", o.lower);
    assert_eq!(circles(&o.lower, (100.0, 100.0)).len(), 1);
    assert_eq!(circles(&o.lower, (105.0, 100.0)).len(), 1);
}

#[test]
fn score_course_has_no_legs_and_shows_codes() {
    let ev = event(
        vec![
            control("S1", ControlKind::Start, 0.0, 0.0),
            control("31", ControlKind::Control, 60.0, 0.0),
            control("47", ControlKind::Control, 60.0, 60.0),
            control("F1", ControlKind::Finish, 0.0, 60.0),
        ],
        Vec::new(),
    );
    let c = course(
        CourseKind::Score,
        vec![stop("S1"), stop("31"), stop("47"), stop("F1")],
    );
    let numbers = file_numbers(&c, &ev, page_of(1.0), &sizes(), 1.0);
    let o = overprint(&c, &ev, &numbers, page_of(1.0), &sizes(), 1.0);
    assert!(lines(&o.lower).is_empty(), "{:?}", o.lower);
    let shown: Vec<String> = texts(&o.upper).into_iter().map(|t| t.2).collect();
    assert_eq!(shown, ["31", "47"]);
}

#[test]
fn circle_gaps_leave_arcs_open() {
    let ev = event(
        vec![control("31", ControlKind::Control, 0.0, 0.0)],
        Vec::new(),
    );
    let mut s = stop("31");
    // Across +x, as Purple Pen stores it: from -30° to 30°, measured from +x towards +y.
    s.circle_gaps = vec![(-30.0, 30.0), (80.0, 100.0)];
    let c = course(CourseKind::Normal, vec![s]);
    let o = overprint(&c, &ev, &[], page_of(1.0), &sizes(), 1.0);
    let arcs: Vec<&Vec<Pt>> = o
        .lower
        .iter()
        .filter_map(|s| match s {
            Shape::Stroke {
                path,
                closed: false,
                ..
            } => Some(path),
            _ => None,
        })
        .collect();
    assert_eq!(arcs.len(), 2, "{arcs:?}");
    let deg = |p: &Pt| ((p.1 - 100.0).atan2(p.0 - 100.0) * 180.0 / PI).rem_euclid(360.0);
    for p in arcs.iter().flat_map(|a| a.iter()) {
        assert!((dist(*p, (100.0, 100.0)) - 3.0).abs() < 1e-9);
        let a = deg(p);
        let inside = |lo: f64, hi: f64| a > lo + 1e-6 && a < hi - 1e-6;
        assert!(
            !inside(-1.0, 30.0) && !inside(330.0, 361.0) && !inside(80.0, 100.0),
            "{a}"
        );
    }
    // The arcs end exactly at the gap edges; +y is down on the page as on the map.
    let ends: Vec<f64> = arcs
        .iter()
        .flat_map(|a| [deg(&a[0]), deg(a.last().unwrap())])
        .collect();
    for want in [30.0, 80.0, 100.0, 330.0] {
        assert!(
            ends.iter().any(|e| (e - want).abs() < 1e-6),
            "{want} in {ends:?}"
        );
    }
}

#[test]
fn numbers_at_file_position_or_due_east() {
    let ev = event(
        vec![
            control("S1", ControlKind::Start, 0.0, 0.0),
            control("31", ControlKind::Control, 60.0, 0.0),
            control("32", ControlKind::Control, 60.0, 60.0),
        ],
        Vec::new(),
    );
    let mut with_offset = stop("32");
    with_offset.number_text_at = Some(Offset {
        dx_mm: -5.0,
        dy_mm: 4.0,
    });
    let c = course(
        CourseKind::Normal,
        vec![stop("S1"), stop("31"), with_offset],
    );
    let sf = 4.0 / 3.0;
    let gf = 4.0 / 3.0;
    let numbers = file_numbers(&c, &ev, page_of(gf), &sizes(), sf);
    assert_eq!(numbers.len(), 2, "{numbers:?}");
    assert_eq!(numbers[0].0, "31");
    assert_eq!(numbers[1].0, "32");
    // Due east: the box's left edge 1 mm beyond the outer edge of the circle line.
    let face = crate::render::fonts::FontFace::Regular;
    let size = 4.0 * sf / crate::render::fonts::cap_height(face);
    let w = crate::render::fonts::text_width("1", face) * size;
    let r_outer = (3.0 + 0.175) * sf;
    let east = numbers[0].1;
    assert!((east.0 - w / 2.0 - (60.0 * gf + 100.0 + r_outer + 1.0)).abs() < 1e-9);
    assert!((east.1 - 100.0).abs() < 1e-9);
    // From the file: the offset is map mm from the circle centre, so it grows with the geometry.
    let at = numbers[1].1;
    assert!(dist(at, ((60.0 - 5.0) * gf + 100.0, (60.0 + 4.0) * gf + 100.0)) < 1e-9);
    // Drawn centred on those points, digits as tall as the profile says.
    let o = overprint(&c, &ev, &numbers, page_of(gf), &sizes(), sf);
    let shown = texts(&o.upper);
    assert_eq!(shown.len(), 2, "{shown:?}");
    for ((origin, font, text), (_, centre)) in shown.iter().zip(&numbers) {
        assert!((font - size).abs() < 1e-9);
        let w = crate::render::fonts::text_width(text, face) * font;
        let cap = crate::render::fonts::cap_height(face) * font;
        assert!((cap - 4.0 * sf).abs() < 1e-9);
        assert!(dist((origin.0 + w / 2.0, origin.1 - cap / 2.0), *centre) < 1e-9);
    }
    assert_eq!(shown[0].2, "1");
    assert_eq!(shown[1].2, "2");
    // Numbers are upper purple only.
    assert!(texts(&o.lower).is_empty());
}

/// The layers `special` alone lands in: (lower shapes, upper shapes).
fn layers_of(special: Special<MapCoord>) -> (Vec<Shape>, Vec<Shape>) {
    let ev = event(Vec::new(), vec![special]);
    let c = course(CourseKind::Normal, Vec::new());
    let o = overprint(&c, &ev, &[], page_of(1.0), &sizes(), 1.0);
    (o.lower, o.upper)
}

fn ring() -> Vec<Vec<MapCoord>> {
    vec![vec![
        MapCoord::new(0, 0),
        MapCoord::new(um(20.0), 0),
        MapCoord::new(um(20.0), um(10.0)),
        MapCoord::new(0, um(10.0)),
        MapCoord::new(0, 0),
    ]]
}

fn path(len_mm: f64) -> Vec<MapCoord> {
    vec![MapCoord::new(0, 0), MapCoord::new(um(len_mm), 0)]
}

fn pt() -> MapCoord {
    MapCoord::new(um(10.0), um(10.0))
}

#[test]
fn specials_go_to_their_layer() {
    let all = CourseFilter::All;
    let lower_only = |s: Special<MapCoord>| {
        let (l, u) = layers_of(s.clone());
        assert!(!l.is_empty() && u.is_empty(), "{s:?}: {l:?} / {u:?}");
        l
    };
    let upper_only = |s: Special<MapCoord>| {
        let (l, u) = layers_of(s.clone());
        assert!(l.is_empty() && !u.is_empty(), "{s:?}: {l:?} / {u:?}");
        u
    };
    // Areas: hatched upper purple, clipped to the area.
    for s in [
        Special::OutOfBounds {
            rings: ring(),
            courses: all.clone(),
        },
        Special::Dangerous {
            rings: ring(),
            courses: all.clone(),
        },
    ] {
        let u = upper_only(s);
        assert!(u.iter().all(|s| matches!(s, Shape::Clipped { .. })));
    }
    let boundary = lower_only(Special::Boundary {
        path: path(30.0),
        courses: all.clone(),
    });
    assert!(matches!(boundary[..], [Shape::Stroke { width, .. }] if (width - 1.0).abs() < 1e-9));
    let line = lower_only(Special::Line {
        path: path(30.0),
        courses: all.clone(),
    });
    assert!(matches!(line[..], [Shape::Stroke { width, .. }] if (width - 0.35).abs() < 1e-9));
    // A forbidden route of one point is Purple Pen's forbidden crossing: one cross.
    let cross = lower_only(Special::ForbiddenRoute {
        path: vec![pt()],
        courses: all.clone(),
    });
    assert_eq!(cross.len(), 2, "{cross:?}");
    // Two or more points: the line with crosses along it.
    let route = lower_only(Special::ForbiddenRoute {
        path: path(30.0),
        courses: all.clone(),
    });
    assert!(route.len() >= 5, "{route:?}");
    let marked = upper_only(Special::MarkedRoute {
        path: path(30.0),
        courses: all.clone(),
    });
    // Dashes of 2 mm with gaps of 0.5 mm.
    assert_eq!(marked.len(), 12, "{marked:?}");
    lower_only(Special::FirstAid {
        pt: pt(),
        courses: all.clone(),
    });
    lower_only(Special::Water {
        pt: pt(),
        courses: all.clone(),
    });
    let text = upper_only(Special::Text {
        pt: pt(),
        text: "Zuschauer".into(),
        size_mm: 3.0,
        courses: all.clone(),
    });
    assert!(matches!(&text[..], [Shape::Text { text, .. }] if text == "Zuschauer"));
    // Not drawn: white-out (masks come in M4) and specials of other courses.
    let (l, u) = layers_of(Special::WhiteOut {
        rings: ring(),
        courses: all,
    });
    assert!(l.is_empty() && u.is_empty());
    let (l, u) = layers_of(Special::FirstAid {
        pt: pt(),
        courses: CourseFilter::Only(vec!["B".into()]),
    });
    assert!(l.is_empty() && u.is_empty());
    let (l, _) = layers_of(Special::FirstAid {
        pt: pt(),
        courses: CourseFilter::Only(vec!["B".into(), "A".into()]),
    });
    assert!(!l.is_empty());
}

#[test]
fn crossing_point_lower_and_map_exchange_upper() {
    let ev = event(
        vec![
            control("S1", ControlKind::Start, 0.0, 0.0),
            control("CROSS1", ControlKind::CrossingPoint, 30.0, 0.0),
            control("31", ControlKind::Control, 60.0, 0.0),
            control("F1", ControlKind::Finish, 90.0, 0.0),
        ],
        Vec::new(),
    );
    let mut exchange = stop("31");
    exchange.kind = Some(ControlKind::MapExchange);
    let c = course(
        CourseKind::Normal,
        vec![stop("S1"), stop("CROSS1"), exchange, stop("F1")],
    );
    let numbers = file_numbers(&c, &ev, page_of(1.0), &sizes(), 1.0);
    let o = overprint(&c, &ev, &numbers, page_of(1.0), &sizes(), 1.0);
    // The map exchange control keeps its circle (lower) and number, and gets a triangle (upper).
    assert_eq!(circles(&o.lower, (160.0, 100.0)).len(), 1);
    assert!(
        o.upper
            .iter()
            .any(|s| matches!(s, Shape::Stroke { path, closed: true, .. } if path.len() == 3))
    );
    assert_eq!(texts(&o.upper).len(), 1);
    // The leg runs through the crossing point, which is two curved lines across it.
    let near_cross = |s: &Shape| match s {
        Shape::Stroke { path, .. } => path.iter().all(|p| dist(*p, (130.0, 100.0)) <= 1.8),
        _ => false,
    };
    assert_eq!(o.lower.iter().filter(|s| near_cross(s)).count(), 2);
    assert!(lines(&o.lower).iter().any(
        |(p, _)| p[0].0 < 130.0 && p.last().unwrap().0 > 130.0 || p.last().unwrap().0 == 130.0
    ));
}

#[test]
fn marked_route_leg_is_dashed_upper() {
    let ev = event(
        vec![
            control("31", ControlKind::Control, 0.0, 0.0),
            control("F1", ControlKind::Finish, 30.0, 0.0),
        ],
        Vec::new(),
    );
    let mut c = course(CourseKind::Normal, vec![stop("31"), stop("F1")]);
    c.legs[0].marked_route = true;
    let o = overprint(&c, &ev, &[], page_of(1.0), &sizes(), 1.0);
    assert!(lines(&o.lower).is_empty());
    let dashes = lines(&o.upper);
    assert!(dashes.len() > 5, "{dashes:?}");
    for (p, _) in &dashes {
        let len = dist(p[0], *p.last().unwrap());
        assert!(len <= 2.0 + 1e-9, "{len}");
    }
}

#[test]
fn full_strength_by_name_or_cmyk() {
    use FullStrength::{Black, Blue, Brown};
    let fs = full_strength;
    // By name: black, brown or blue, alone or at 100 %, whatever the CMYK.
    assert_eq!(fs("Black", [0.0, 0.0, 0.0, 0.9]), Some(Black));
    assert_eq!(fs("  brown 100 % ", [0.0, 0.5, 0.8, 0.0]), Some(Brown));
    assert_eq!(fs("Blue 100%", [0.87, 0.18, 0.0, 0.0]), Some(Blue));
    assert_eq!(fs("BLUE", [0.87, 0.18, 0.0, 0.0]), Some(Blue));
    assert_eq!(fs("Blue 50%", [0.435, 0.09, 0.0, 0.0]), None);
    assert_eq!(fs("Brown 50%", [0.0, 0.28, 0.5, 0.09]), None);
    assert_eq!(fs("Blue100%", [0.87, 0.18, 0.0, 0.0]), None);
    // By CMYK, ±0.02: black, blue, brown of Rev. 4 and of Rev. 3.
    assert_eq!(fs("Black for open land", [0.0, 0.0, 0.0, 1.0]), Some(Black));
    assert_eq!(fs("x", [0.0, 0.0, 0.0, 0.985]), Some(Black));
    assert_eq!(fs("x", [0.0, 0.0, 0.0, 0.97]), None);
    assert_eq!(fs("x", [1.0, 0.0, 0.0, 0.0]), Some(Blue));
    assert_eq!(fs("x", [0.25, 0.75, 1.0, 0.0]), Some(Brown));
    assert_eq!(fs("x", [0.0, 0.56, 1.0, 0.18]), Some(Brown));
    assert_eq!(fs("x", [0.015, 0.545, 0.985, 0.195]), Some(Brown));
    assert_eq!(fs("x", [0.03, 0.56, 1.0, 0.18]), None);
    assert_eq!(fs("x", [0.0, 0.27, 0.79, 0.0]), None, "brown 50 %");
    assert_eq!(
        fs("OpenOrienteering Orange", [0.125, 0.544, 0.895, 0.0]),
        None
    );
    assert_eq!(fs("Black 60%", [0.0, 0.0, 0.0, 0.6]), None);
    assert_eq!(fs("Purple", [0.35, 0.85, 0.0, 0.0]), None);
}

#[test]
fn lower_purple_goes_above_the_topmost_colour_that_is_not_full_strength() {
    let named = |priority: i32, name: &str, cmyk: [f64; 4]| RenderColour {
        name: name.into(),
        ..colour(priority, cmyk)
    };
    let black = named(1, "Black", [0.0, 0.0, 0.0, 1.0]);
    let green = named(2, "Green", [0.76, 0.0, 0.91, 0.0]);
    let blue = named(3, "Blue", [1.0, 0.0, 0.0, 0.0]);
    let grey = named(9, "Black 60%", [0.0, 0.0, 0.0, 0.6]);
    assert_eq!(
        full_strength_priority(&[black.clone(), green.clone(), blue.clone(), grey]),
        Some(1)
    );
    // Order in the slice does not matter, only priorities.
    assert_eq!(
        full_strength_priority(&[blue.clone(), green, black.clone()]),
        Some(1)
    );
    assert_eq!(full_strength_priority(&[black, blue]), None, "all full");
    assert_eq!(full_strength_priority(&[]), None, "no colours");
}

/// The map colours of a fixture and the page rendered from it at its own scale on A3.
fn fixture_page(file: &str, kind: MapKind) -> RenderedPage {
    let map = crate::omap::read::read_map_file(&std::path::PathBuf::from(format!(
        "{}/../../tests/fixtures/maps/{file}",
        env!("CARGO_MANIFEST_DIR")
    )))
    .unwrap();
    let options = crate::print::PrintOptions {
        print_scale: None,
        kind,
        paper: crate::print::Paper::A3,
        orientation: crate::print::Orientation::Auto,
        centre: None,
        show_sheet_objects: false,
        frame_margin_mm: None,
    };
    crate::print::render_page(&map, &options).unwrap().0
}

fn names(page: &RenderedPage, pick: impl Fn(&RenderColour) -> bool) -> Vec<(i32, String)> {
    page.colours
        .iter()
        .filter(|c| pick(c))
        .map(|c| (c.priority, c.name.clone()))
        .collect()
}

#[test]
fn luebeck_lower_purple_above_buildings_and_roads() {
    let page = fixture_page("luebeck.omap", MapKind::Sprint);
    let full = names(&page, |c| full_strength(&c.name, c.cmyk).is_some());
    assert_eq!(
        full,
        [
            (2, "Black 100%".to_string()),
            (7, "Blue 100%".to_string()),
            (8, "Brown 100%".to_string()),
            (15, "Black below lower light brown".to_string()),
            (16, "Blue 100% for area features".to_string()),
            (29, "Black for open land".to_string()),
        ]
    );
    // Over all colours of the set, the course purple itself (0) is the topmost colour that is not full
    // strength: the lower purple would go over everything. Only the colours the page paints count.
    assert_eq!(full_strength_priority(&page.colours), Some(-1));
    let o = Overprint::default().on_map(IOF_PURPLE, &page.colours, &page.primitives);
    // The page paints 2 (Black 100%), 10 (Black 50%, buildings), 14 (Lower brown 30%, roads), 15 and 16:
    // the lower purple goes right above the buildings, below black.
    assert_eq!(o.below_priority, Some(9));
    let below = o.below_priority.unwrap();
    for (priority, what) in [(10, "Black 50%"), (14, "Lower brown 30%")] {
        assert!(
            page.primitives.iter().any(|p| p.priority == priority),
            "{what}"
        );
        assert!(priority > below, "{what} lies under the lower purple");
    }
    assert!(2 <= below, "Black 100% lies over it");
}

#[test]
fn forest_lower_purple_above_brown_50_below_black() {
    let page = fixture_page("forest-sample.omap", MapKind::Forest);
    let full = names(&page, |c| full_strength(&c.name, c.cmyk).is_some());
    assert_eq!(
        full,
        [
            (1, "Black".to_string()),
            (3, "Black below light browns".to_string()),
            // "Opaque Blue" (4) is 87/18/0/0 and has no plain name: not full strength.
            (5, "Blue".to_string()),
            (7, "Brown".to_string()),
        ]
    );
    let o = Overprint::default().on_map(IOF_PURPLE, &page.colours, &page.primitives);
    // The topmost painted colour that is not full strength is Brown 50% (2).
    assert_eq!(o.below_priority, Some(1));
}

#[test]
fn out_of_bounds_hatch_follows_the_profile() {
    /// Directions (degrees, counter-clockwise on paper, 0..180), spacing and width of the hatch lines.
    fn hatch_of(sizes: &CourseSymbolSizes) -> (Vec<i64>, f64, f64) {
        let (_, upper) = {
            let ev = event(
                Vec::new(),
                vec![Special::OutOfBounds {
                    rings: ring(),
                    courses: CourseFilter::All,
                }],
            );
            let c = course(CourseKind::Normal, Vec::new());
            let o = overprint(&c, &ev, &[], page_of(1.0), sizes, 1.0);
            (o.lower, o.upper)
        };
        let mut lines: Vec<(i64, Pt, Pt, f64)> = upper
            .iter()
            .map(|s| match s {
                Shape::Clipped { inner, .. } => match &**inner {
                    Shape::Stroke { path, width, .. } => {
                        let (a, b) = (path[0], path[1]);
                        // Paper y points down: counter-clockwise is -y.
                        let deg = (-(b.1 - a.1))
                            .atan2(b.0 - a.0)
                            .to_degrees()
                            .rem_euclid(180.0);
                        (deg.round() as i64 % 180, a, b, *width)
                    }
                    other => panic!("{other:?}"),
                },
                other => panic!("{other:?}"),
            })
            .collect();
        lines.sort_by_key(|l| l.0);
        let mut angles: Vec<i64> = lines.iter().map(|l| l.0).collect();
        angles.dedup();
        // Spacing: distance between the first two parallel lines of the first direction.
        let first: Vec<_> = lines.iter().filter(|l| l.0 == angles[0]).collect();
        let (p, q) = (first[0], first[1]);
        let d = (p.2.0 - p.1.0, p.2.1 - p.1.1);
        let len = d.0.hypot(d.1);
        let spacing = ((q.1.0 - p.1.0) * d.1 - (q.1.1 - p.1.1) * d.0).abs() / len;
        (angles, spacing, first[0].3)
    }
    let base = crate::rules::base();
    // ISSprOM 2019-2 and ISOM 2017-2, symbol 709: cross hatch at 45° and 135°, 1.2 mm apart, 0.2 mm wide.
    for kind in [MapKind::Sprint, MapKind::Forest] {
        let (angles, spacing, width) = hatch_of(&base.course_symbol(kind));
        assert_eq!(angles, [45, 135], "{kind:?}");
        assert!((spacing - 1.2).abs() < 1e-9, "{kind:?} {spacing}");
        assert!((width - 0.2).abs() < 1e-9, "{kind:?} {width}");
    }
    // The values come from the profile: ISOM 2000's vertical stripes (0.25 mm, 0.6 mm apart) as one
    // direction without a second.
    let isom2000 = CourseSymbolSizes {
        out_of_bounds_hatch_angle_degrees: 90.0,
        out_of_bounds_hatch_second_angle_degrees: None,
        out_of_bounds_hatch_spacing_mm: 0.6,
        out_of_bounds_hatch_line_mm: 0.25,
        ..base.course_symbol(MapKind::Forest)
    };
    let (angles, spacing, width) = hatch_of(&isom2000);
    assert_eq!(angles, [90]);
    assert!((spacing - 0.6).abs() < 1e-9 && (width - 0.25).abs() < 1e-9);
}

// --- The written PDF --------------------------------------------------------------------------

fn colour(priority: i32, cmyk: [f64; 4]) -> RenderColour {
    RenderColour {
        name: format!("colour {priority}"),
        priority,
        cmyk,
        rgb: [0.0; 3],
        opacity: 1.0,
    }
}

fn square(priority: i32) -> Primitive {
    Primitive {
        priority,
        shape: Shape::Fill {
            rings: vec![vec![(10.0, 10.0), (20.0, 10.0), (20.0, 20.0), (10.0, 20.0)]],
            rule: FillRule::EvenOdd,
        },
    }
}

/// A page with squares of priority 9, 2 and 1 and the overprint of
/// [`three_stops`] with numbers, laid on `colours`.
fn course_page(colours: Vec<RenderColour>) -> RenderedPage {
    let (c, ev) = three_stops();
    let numbers = file_numbers(&c, &ev, page_of(1.0), &sizes(), 1.0);
    let primitives = vec![square(9), square(2), square(1)];
    let o = overprint(&c, &ev, &numbers, page_of(1.0), &sizes(), 1.0).on_map(
        IOF_PURPLE,
        &colours,
        &primitives,
    );
    RenderedPage {
        primitives,
        clip: PaperBox {
            min: (0.0, 0.0),
            max: (210.0, 297.0),
        },
        page_mm: (210.0, 297.0),
        centre_mm: (0.0, 0.0),
        colours,
        sheet: Vec::new(),
        overprint: o,
    }
}

fn positions(haystack: &[u8], needle: &[u8]) -> Vec<usize> {
    haystack
        .windows(needle.len())
        .enumerate()
        .filter(|(_, w)| *w == needle)
        .map(|(i, _)| i)
        .collect()
}

const GREY: &[u8] = b"0 0 0 0.6 k";
const BLACK: &[u8] = b"0 0 0 1 k";
const GREEN: &[u8] = b"0.2 0 0.4 0 k";
const PURPLE_STROKE: &[u8] = b"0.35 0.85 0 0 K";
const PURPLE_FILL: &[u8] = b"0.35 0.85 0 0 k";
const MULTIPLY: &[u8] = b"/Mul gs";

#[test]
fn lower_layer_below_black_upper_above() {
    let colours = vec![
        colour(1, [0.0, 0.0, 0.0, 1.0]),
        colour(2, [0.2, 0.0, 0.4, 0.0]),
        colour(9, [0.0, 0.0, 0.0, 0.6]),
    ];
    let page = course_page(colours);
    // Green (2) is the topmost colour that is not full strength.
    assert_eq!(page.overprint.below_priority, Some(1));
    let (pdf, _) = write_pdf(&page, &PdfMeta::new("t")).unwrap();
    let content = content_of(&pdf);
    let gs = positions(&content, MULTIPLY);
    assert_eq!(gs.len(), 2, "one graphics state per layer");
    let grey = positions(&content, GREY)[0];
    let black = positions(&content, BLACK)[0];
    let green = positions(&content, GREEN)[0];
    let strokes = positions(&content, PURPLE_STROKE);
    let fills = positions(&content, PURPLE_FILL);
    // grey (9) → green (2) → lower purple → black (1) → upper purple.
    assert!(
        grey < green && green < gs[0] && gs[0] < black,
        "{grey} {green} {gs:?} {black}"
    );
    assert!(
        strokes.iter().any(|&s| gs[0] < s && s < black),
        "lower purple strokes"
    );
    assert!(black < gs[1], "upper purple after the last map primitive");
    assert!(fills.iter().all(|&f| f > gs[1]), "numbers are upper purple");
    // Both layers are clipped to the map frame: inside the page's clip block.
    let text = String::from_utf8_lossy(&content);
    let clip = text.find("re\nW\nn").unwrap();
    assert!(clip < gs[0]);
}

#[test]
fn overprint_uses_multiply() {
    let colours = vec![colour(2, [0.0, 0.0, 0.0, 1.0])];
    let page = course_page(colours);
    let (pdf, _) = write_pdf(&page, &PdfMeta::new("t")).unwrap();
    let raw = String::from_utf8_lossy(&pdf);
    assert_eq!(raw.matches("/BM /Multiply").count(), 1, "one ExtGState");
    assert!(raw.contains("/ExtGState <<\n      /Mul"), "{raw}");
    let content = String::from_utf8(content_of(&pdf)).unwrap();
    assert_eq!(content.matches("q\n/Mul gs\n").count(), 2, "{content}");
    // A page without a course has no ExtGState, so M1/M2 output stays as it was.
    let mut plain = page.clone();
    plain.overprint = Overprint::default();
    let (pdf, _) = write_pdf(&plain, &PdfMeta::new("t")).unwrap();
    let raw = String::from_utf8_lossy(&pdf);
    assert!(!raw.contains("ExtGState") && !raw.contains("Multiply"));
}

#[test]
fn no_full_strength_colours_paints_lower_on_top() {
    let colours = vec![
        colour(1, [0.2, 0.0, 0.4, 0.0]),
        colour(2, [0.0, 0.0, 0.0, 0.5]),
        colour(9, [0.0, 0.0, 0.0, 0.6]),
    ];
    let page = course_page(colours);
    // Above the topmost colour (1): after every map primitive.
    assert_eq!(page.overprint.below_priority, Some(0));
    let (pdf, _) = write_pdf(&page, &PdfMeta::new("t")).unwrap();
    let content = content_of(&pdf);
    let gs = positions(&content, MULTIPLY);
    assert_eq!(gs.len(), 2);
    let last_map = *[GREY, b"0 0 0 0.5 k".as_slice(), GREEN]
        .iter()
        .map(|c| positions(&content, c)[0])
        .collect::<Vec<_>>()
        .iter()
        .max()
        .unwrap();
    assert!(last_map < gs[0] && gs[0] < gs[1], "{last_map} {gs:?}");
    let strokes = positions(&content, PURPLE_STROKE);
    assert!(
        strokes.iter().any(|&s| gs[0] < s && s < gs[1]),
        "lower before upper"
    );
}

#[test]
fn overprint_without_map_primitives_is_clipped_too() {
    let mut page = course_page(vec![colour(2, [0.0, 0.0, 0.0, 1.0])]);
    page.primitives.clear();
    let (pdf, _) = write_pdf(&page, &PdfMeta::new("t")).unwrap();
    let content = String::from_utf8(content_of(&pdf)).unwrap();
    let clip = content
        .find("re\nW\nn")
        .expect("the frame clips the overprint");
    assert!(clip < content.find("/Mul gs").unwrap());
}
