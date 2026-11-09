//! The course overprint: one course as purple shapes in page millimetres, split into the lower and the
//! upper purple of IOF "Printing and Colour Definitions" Rev. 4 (spec §5.9, ADR 0034). The PDF writer paints
//! both layers in the course colour with the blend mode Multiply.

mod symbols;

use std::collections::HashMap;
use std::f64::consts::PI;

use crate::course::{
    Control, ControlKind, Course, CourseFilter, CourseKind, FittedEvent, Special, Stop,
};
use crate::omap::MapCoord;
use crate::render::page::{DirectColour, Pt, RenderColour, Shape};
use crate::rules::CourseSymbolSizes;

/// The course colour of IOF Rev. 4, CMYK 35/85/0/0, until a rule set gives its own
/// ([`Overprint::on_map`]).
pub const IOF_PURPLE: [f64; 4] = [0.35, 0.85, 0.0, 0.0];

/// The gap between a circle and a number placed due east of it, in print mm (spec: the shortest distance
/// from the outer edge of the circle line to the number box).
pub const EAST_NUMBER_GAP_MM: f64 = 1.0;

/// The overprint of one course on one page. Empty on pages without a course, so they stay as before.
#[derive(Debug, Clone, PartialEq)]
pub struct Overprint {
    /// Lower purple: start, controls, legs, finish, out-of-bounds boundaries, forbidden routes, crossing
    /// points, first aid and water. Painted below full-strength black, brown and blue.
    pub lower: Vec<Shape>,
    /// Upper purple: control numbers, map exchanges, out-of-bounds and dangerous areas, marked routes and
    /// texts. Painted over all map primitives.
    pub upper: Vec<Shape>,
    /// The priority before whose first primitive the lower layer is painted ([`full_strength_priority`]).
    /// `None` paints it after all map primitives, just before the upper layer.
    pub below_priority: Option<i32>,
    pub colour: DirectColour,
    /// A limit ([`MAX_REPEATS`]) left out hatch lines, dashes or crosses.
    pub cut: bool,
}

impl Default for Overprint {
    fn default() -> Self {
        Overprint {
            lower: Vec::new(),
            upper: Vec::new(),
            below_priority: None,
            colour: DirectColour { cmyk: IOF_PURPLE },
            cut: false,
        }
    }
}

impl Overprint {
    /// Nothing to paint.
    pub fn is_empty(&self) -> bool {
        self.lower.is_empty() && self.upper.is_empty()
    }

    /// The overprint in `purple` (the rule set's course colour), with its lower layer below the
    /// full-strength colours among `colours` (the page's map colours).
    pub fn on_map(self, purple: [f64; 4], colours: &[RenderColour]) -> Self {
        Overprint {
            colour: DirectColour { cmyk: purple },
            below_priority: full_strength_priority(colours),
            ..self
        }
    }
}

/// Most hatch lines, dashes and crosses one overprint draws. An A3 page needs a few thousand; a special
/// object spanning kilometres of map would need millions. What lies beyond is left out and
/// [`Overprint::cut`] is set.
pub const MAX_REPEATS: usize = 200_000;

/// A full-strength map colour, by CMYK.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FullStrength {
    Black,
    Brown,
    Blue,
}

/// Tolerance for the exact CMYK values (0 and 1) of [`full_strength`].
const CMYK_EPS: f64 = 1e-3;

/// The full-strength class of `cmyk`: black is 0/0/0/1; brown has m ≥ 0.5, y ≥ 0.75, c ≤ 0.3 and
/// k ≤ 0.2; blue has c ≥ 0.9, m ≤ 0.1, y ≤ 0.1 and k = 0.
pub fn full_strength(cmyk: [f64; 4]) -> Option<FullStrength> {
    let [c, m, y, k] = cmyk;
    let zero = |v: f64| v.abs() <= CMYK_EPS;
    if zero(c) && zero(m) && zero(y) && (k - 1.0).abs() <= CMYK_EPS {
        Some(FullStrength::Black)
    } else if m >= 0.5 - CMYK_EPS
        && y >= 0.75 - CMYK_EPS
        && c <= 0.3 + CMYK_EPS
        && k <= 0.2 + CMYK_EPS
    {
        Some(FullStrength::Brown)
    } else if c >= 0.9 - CMYK_EPS && m <= 0.1 + CMYK_EPS && y <= 0.1 + CMYK_EPS && zero(k) {
        Some(FullStrength::Blue)
    } else {
        None
    }
}

/// The highest priority number among the full-strength black, brown and blue colours: the one painted
/// first. `None` when the map has none of them.
pub fn full_strength_priority(colours: &[RenderColour]) -> Option<i32> {
    colours
        .iter()
        .filter(|c| full_strength(c.cmyk).is_some())
        .map(|c| c.priority)
        .max()
}

/// Whether a stop of effective kind `kind` at `control` carries a control number.
fn numbered(kind: ControlKind, control: &Control<MapCoord>) -> bool {
    kind == ControlKind::Control
        || (kind == ControlKind::MapExchange && control.kind == ControlKind::Control)
}

/// The numbered stops of `course` with their control and text: a normal course counts its controls from 1,
/// a score course shows the control codes.
fn labels<'a>(
    course: &'a Course<MapCoord>,
    event: &'a FittedEvent,
) -> Vec<(usize, &'a Control<MapCoord>, String)> {
    let controls: HashMap<&str, &Control<MapCoord>> = event
        .controls
        .iter()
        .map(|c| (c.code.as_str(), c))
        .collect();
    let mut out = Vec::new();
    for (i, stop) in course.stops.iter().enumerate() {
        let Some(control) = controls.get(stop.control.as_str()) else {
            continue;
        };
        if !numbered(stop.kind.unwrap_or(control.kind), control) {
            continue;
        }
        let text = match course.kind {
            CourseKind::Normal => (out.len() + 1).to_string(),
            CourseKind::Score => control.code.clone(),
        };
        out.push((i, *control, text));
    }
    out
}

/// The number positions the course file gives, as [`overprint`] takes them: per numbered stop the control
/// code and the centre of its number text in page mm. A stop with a position in the file
/// ([`crate::course::Stop::number_text_at`], an offset from the circle centre) keeps it; the others go due
/// east of their circle, [`EAST_NUMBER_GAP_MM`] from the outer edge of the circle line. Task 10's placement
/// replaces this.
pub fn file_numbers(
    course: &Course<MapCoord>,
    event: &FittedEvent,
    to_page: impl Fn(MapCoord) -> Pt,
    sizes: &CourseSymbolSizes,
    symbol_factor: f64,
) -> Vec<(String, Pt)> {
    let s = Scaled::new(sizes, symbol_factor);
    labels(course, event)
        .into_iter()
        .filter_map(|(i, control, text)| {
            let at = control.position?;
            let centre = match course.stops[i].number_text_at {
                Some(o) => to_page(MapCoord::new(
                    at.x.saturating_add(um(o.dx_mm)),
                    at.y.saturating_add(um(o.dy_mm)),
                )),
                None => {
                    let c = to_page(at);
                    let (_, w, _) = symbols::number_metrics(&text, s.number_height);
                    let r_outer = s.circle_r + s.line / 2.0;
                    (c.0 + r_outer + EAST_NUMBER_GAP_MM + w / 2.0, c.1)
                }
            };
            Some((control.code.clone(), centre))
        })
        .collect()
}

/// `mm` as whole µm, saturating far outside any map.
fn um(mm: f64) -> i32 {
    (mm * 1000.0).round() as i32
}

/// The course symbol sizes on the page: the profile's sizes times the symbol factor, radii from
/// diameters.
struct Scaled {
    circle_r: f64,
    line: f64,
    start_side: f64,
    finish_inner_r: f64,
    finish_outer_r: f64,
    number_height: f64,
    dash: f64,
    gap: f64,
    boundary: f64,
    crossing: f64,
    /// Width and spacing of the out-of-bounds and dangerous-area hatches.
    hatch_line: f64,
    hatch_spacing: f64,
}

/// Hatch line width and spacing at the base scale (mm): OOM's ISOM 2017 set, symbols 709 and 710.
const HATCH_LINE_MM: f64 = 0.25;
const HATCH_SPACING_MM: f64 = 0.6;

impl Scaled {
    fn new(sizes: &CourseSymbolSizes, f: f64) -> Self {
        Scaled {
            circle_r: sizes.circle_diameter_mm / 2.0 * f,
            line: sizes.line_width_mm * f,
            start_side: sizes.start_side_mm * f,
            finish_inner_r: sizes.finish_inner_mm / 2.0 * f,
            finish_outer_r: sizes.finish_outer_mm / 2.0 * f,
            number_height: sizes.number_height_mm * f,
            dash: sizes.marked_route_dash_mm * f,
            gap: sizes.marked_route_gap_mm * f,
            boundary: sizes.out_of_bounds_line_mm * f,
            crossing: sizes.crossing_point_mm * f,
            hatch_line: HATCH_LINE_MM * f,
            hatch_spacing: HATCH_SPACING_MM * f,
        }
    }
}

/// A stop drawn on the page.
struct Placed<'a> {
    at: Pt,
    kind: ControlKind,
    control: &'a Control<MapCoord>,
    stop: &'a Stop,
}

impl Placed<'_> {
    /// Where a leg starts or ends: the circle line, the start triangle's corner, the outer finish circle,
    /// or the centre of a crossing point or the end of a marked route.
    fn radius(&self, s: &Scaled) -> f64 {
        match self.kind {
            ControlKind::Start => symbols::triangle_radius(s.start_side),
            ControlKind::Control => s.circle_r,
            ControlKind::Finish => s.finish_outer_r,
            ControlKind::CrossingPoint | ControlKind::EndOfMarkedRoute => 0.0,
            ControlKind::MapExchange if self.control.kind == ControlKind::Control => s.circle_r,
            ControlKind::MapExchange => symbols::triangle_radius(s.start_side),
        }
    }
}

fn applies(filter: &CourseFilter, course: &str) -> bool {
    match filter {
        CourseFilter::All => true,
        CourseFilter::Only(names) => names.iter().any(|n| n == course),
    }
}

/// The overprint of `course`: its stops, legs, the specials that apply to it and the control numbers at
/// `numbers` (control code and text centre in page mm, matched to the numbered stops in order; a stop
/// without an entry gets no number). `to_page` maps a map coordinate to page mm (the map frame and the page
/// share the y direction); sizes are `sizes` × `symbol_factor`, leg gaps and offsets scale with the
/// geometry factor `to_page` applies. Returns `below_priority` unset and the IOF purple:
/// [`Overprint::on_map`] sets both for a page.
pub fn overprint(
    course: &Course<MapCoord>,
    event: &FittedEvent,
    numbers: &[(String, Pt)],
    to_page: impl Fn(MapCoord) -> Pt,
    sizes: &CourseSymbolSizes,
    symbol_factor: f64,
) -> Overprint {
    let s = Scaled::new(sizes, symbol_factor);
    // The geometry factor, from the transform itself: page mm per map mm.
    let geometry = symbols::dist(
        to_page(MapCoord::new(0, 0)),
        to_page(MapCoord::new(1000, 0)),
    );
    let mut budget = symbols::Budget::new(MAX_REPEATS);
    let mut o = Overprint::default();
    let controls: HashMap<&str, &Control<MapCoord>> = event
        .controls
        .iter()
        .map(|c| (c.code.as_str(), c))
        .collect();
    let placed: Vec<Option<Placed>> = course
        .stops
        .iter()
        .map(|stop| {
            let control = *controls.get(stop.control.as_str())?;
            Some(Placed {
                at: to_page(control.position?),
                kind: stop.kind.unwrap_or(control.kind),
                control,
                stop,
            })
        })
        .collect();
    let at = |i: usize| placed.get(i).and_then(Option::as_ref);
    // The page points of leg `i`: from its first stop through its bends to its second.
    let leg_points = |i: usize| -> Option<Vec<Pt>> {
        let leg = course.legs.get(i)?;
        let mut points = vec![at(i)?.at];
        points.extend(leg.bends.iter().map(|b| to_page(*b)));
        points.push(at(i + 1)?.at);
        Some(points)
    };

    for (i, p) in placed.iter().enumerate() {
        let Some(p) = p else { continue };
        // The direction a start or map exchange triangle points in: along the leg to the next stop (to its
        // first bend, if it has one), else north.
        let ahead = leg_points(i)
            .and_then(|l| symbols::direction(l[0], l[1]))
            .unwrap_or(-PI / 2.0);
        match p.kind {
            ControlKind::Start => {
                o.lower
                    .push(symbols::triangle(p.at, s.start_side, ahead, s.line))
            }
            ControlKind::Control => o.lower.extend(symbols::circle(
                p.at,
                s.circle_r,
                s.line,
                &p.stop.circle_gaps,
            )),
            ControlKind::Finish => {
                for r in [s.finish_inner_r, s.finish_outer_r] {
                    o.lower
                        .extend(symbols::circle(p.at, r, s.line, &p.stop.circle_gaps));
                }
            }
            ControlKind::CrossingPoint => {
                // The way through runs along the course: from the point before to the point after.
                let before = i
                    .checked_sub(1)
                    .and_then(leg_points)
                    .and_then(|l| l.get(l.len() - 2).copied());
                let after = leg_points(i).map(|l| l[1]);
                let angle = match (before, after) {
                    (Some(b), Some(a)) => symbols::direction(b, a),
                    (Some(b), None) => symbols::direction(b, p.at),
                    (None, Some(a)) => symbols::direction(p.at, a),
                    (None, None) => None,
                };
                o.lower.extend(symbols::crossing_point(
                    p.at,
                    s.crossing,
                    angle.unwrap_or(0.0),
                    s.line,
                ));
            }
            ControlKind::EndOfMarkedRoute => {}
            ControlKind::MapExchange => {
                if p.control.kind == ControlKind::Control {
                    o.lower.extend(symbols::circle(
                        p.at,
                        s.circle_r,
                        s.line,
                        &p.stop.circle_gaps,
                    ));
                }
                o.upper
                    .push(symbols::triangle(p.at, s.start_side, ahead, s.line));
            }
        }
    }

    if course.kind == CourseKind::Normal {
        for (i, leg) in course.legs.iter().enumerate() {
            let (Some(points), Some(a), Some(b)) = (leg_points(i), at(i), at(i + 1)) else {
                continue;
            };
            let total = symbols::length(&points);
            let (from, to) = (a.radius(&s), total - b.radius(&s));
            if to <= from {
                continue;
            }
            let gaps: Vec<(f64, f64)> = leg
                .gaps
                .iter()
                .map(|(x, y)| (x * geometry, y * geometry))
                .collect();
            for piece in symbols::pieces(&points, from, to, &gaps) {
                if leg.marked_route {
                    o.upper
                        .extend(symbols::dashes(&piece, s.dash, s.gap, s.line, &mut budget));
                } else {
                    o.lower.push(symbols::line(piece, s.line));
                }
            }
        }
    }

    for special in &event.specials {
        special_shapes(
            special,
            &course.name,
            &to_page,
            &s,
            geometry,
            &mut budget,
            &mut o,
        );
    }

    let mut used = vec![false; numbers.len()];
    for (_, control, text) in labels(course, event) {
        let found = numbers
            .iter()
            .enumerate()
            .position(|(j, (code, _))| !used[j] && *code == control.code);
        if let Some(j) = found {
            used[j] = true;
            o.upper
                .push(symbols::number(&text, numbers[j].1, s.number_height));
        }
    }
    o.cut = budget.cut;
    o
}

/// Adds the shapes of `special` to `o` when it applies to `course`.
fn special_shapes(
    special: &Special<MapCoord>,
    course: &str,
    to_page: &impl Fn(MapCoord) -> Pt,
    s: &Scaled,
    geometry: f64,
    budget: &mut symbols::Budget,
    o: &mut Overprint,
) {
    let path = |p: &[MapCoord]| -> Vec<Pt> { p.iter().map(|c| to_page(*c)).collect() };
    let rings = |r: &[Vec<MapCoord>]| -> Vec<Vec<Pt>> { r.iter().map(|v| path(v)).collect() };
    match special {
        Special::OutOfBounds { rings: r, courses } if applies(courses, course) => {
            // Vertical stripes.
            o.upper.extend(symbols::hatch(
                rings(r),
                &[PI / 2.0],
                s.hatch_spacing,
                s.hatch_line,
                budget,
            ));
        }
        Special::Dangerous { rings: r, courses } if applies(courses, course) => {
            o.upper.extend(symbols::hatch(
                rings(r),
                &[PI / 4.0, -PI / 4.0],
                s.hatch_spacing,
                s.hatch_line,
                budget,
            ));
        }
        Special::ForbiddenRoute { path: p, courses } if applies(courses, course) => {
            let points = path(p);
            match points[..] {
                [] => {}
                // Purple Pen's forbidden crossing: one cross.
                [one] => o.lower.extend(symbols::cross(one, s.crossing, 0.0, s.line)),
                _ => {
                    o.lower
                        .extend(symbols::crosses_along(&points, s.crossing, s.line, budget));
                    o.lower.push(symbols::line(points, s.line));
                }
            }
        }
        Special::Boundary { path: p, courses } if applies(courses, course) && p.len() > 1 => {
            o.lower.push(symbols::line(path(p), s.boundary));
        }
        Special::MarkedRoute { path: p, courses } if applies(courses, course) && p.len() > 1 => {
            o.upper
                .extend(symbols::dashes(&path(p), s.dash, s.gap, s.line, budget));
        }
        Special::FirstAid { pt, courses } if applies(courses, course) => {
            o.lower.extend(symbols::first_aid(to_page(*pt), s.crossing));
        }
        Special::Water { pt, courses } if applies(courses, course) => {
            o.lower
                .extend(symbols::water(to_page(*pt), s.crossing, s.line));
        }
        Special::Text {
            pt,
            text,
            size_mm,
            courses,
        } if applies(courses, course) => {
            o.upper
                .extend(symbols::text_lines(text, to_page(*pt), size_mm * geometry));
        }
        Special::Line { path: p, courses } if applies(courses, course) && p.len() > 1 => {
            o.lower.push(symbols::line(path(p), s.line));
        }
        // White-out becomes a mask in M4.
        _ => {}
    }
}

#[cfg(test)]
mod tests;
