//! Control number positions on a course sheet (spec §5.7, ADR 0038): the four levels, the course file's
//! positions turned into placements, the obstacles of a sheet, the automatic placement and the check. The
//! scoring and the search live in [`crate::placement`], which knows nothing of these types.
//!
//! Levels, the more specific winning: (1) automatic placement, only where the course file gives no position;
//! (2) the course file; (3) a correction for the layout; (4) a correction for the sheet (the course). A
//! correction without a gap keeps the gap of the level below. Numbers are never moved silently: the sheet draws
//! the resolved position, and `check` reports a conflict with a proposal.

use std::collections::HashMap;

use crate::course::model::Offset;
use crate::course::{Course, FittedEvent, NumberPlacement, Stale, Stop, base_changed, layout_for};
use crate::omap::MapCoord;
use crate::overprint::{FullStrength, full_strength, labels, leg_lines, number_size, stop_symbols};
use crate::placement::{
    self, GapRange, Legs, OUTSIDE, Obstacle, ObstacleShape, Placement, Rect, Score, Target, Tier,
};
use crate::project::LoadedProject;
use crate::render::bounds::shape_bounds;
use crate::render::page::{PaperBox, Pt, RenderedPage, Shape, SymbolClass};
use crate::rules::{CourseSymbolSizes, PlacementWeights, RuleSet};

/// The gaps numbers may have under `rules` (ruling R29): the `number_gap` rule's range, else the fixed
/// [`placement::DEFAULT_GAP_MM`].
pub fn gap_range(rules: &RuleSet) -> GapRange {
    rules
        .number_gap()
        .map_or(GapRange::DEFAULT, |(min, max)| GapRange { min, max })
}

/// What automatic placement starts from: the outer side of a control without legs (as freemapper-courseplan),
/// and the smallest gap of `gaps`. A correction over an automatic number without a gap of its own takes this
/// gap.
pub fn automatic_start(gaps: GapRange) -> Placement {
    Placement {
        direction: 45.0,
        gap_mm: gaps.min,
    }
}

/// Where a number position comes from (spec §5.7), the more specific winning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Placed by freemapper-print: the course file has no position.
    Automatic = 1,
    /// The course file's position.
    CourseFile = 2,
    /// A correction for the layout in the project file.
    Layout = 3,
    /// A correction for the sheet (the course) in the project file.
    Sheet = 4,
}

/// The number of one numbered stop and where it goes.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedNumber {
    /// Index of the stop in the course.
    pub stop: usize,
    pub control: String,
    /// The number as printed: the running number, or the code on a score course.
    pub text: String,
    /// Direction and gap (print mm at the base scale). For [`Level::Automatic`], [`automatic_start`] until
    /// [`place_on_sheet`] places it.
    pub placement: Placement,
    pub level: Level,
    /// The course file's placement (level 2), when it gives one.
    pub base: Option<Placement>,
}

impl From<NumberPlacement> for Placement {
    fn from(p: NumberPlacement) -> Self {
        Placement {
            direction: p.direction,
            gap_mm: p.gap_mm,
        }
    }
}

impl From<Placement> for NumberPlacement {
    fn from(p: Placement) -> Self {
        NumberPlacement {
            direction: p.direction,
            gap_mm: p.gap_mm,
        }
    }
}

/// The placement of a number whose text centre the course file puts at `offset` from the circle centre (map mm,
/// map frame), ruling R12. The offset is scaled to print mm at `print_scale` (the course's print scale in
/// Purple Pen, else the map scale): Purple Pen adds the offset to the control in map coordinates and, with its
/// default item scaling, draws course symbols at their nominal print size at that scale
/// (`CourseFormatter.CreateControlNumber`, `CourseView.CourseObjRatio`, commit 201417f). So the box of `text`
/// and the circle take the nominal sizes of `sizes`, and the gap is the shortest distance from the box to the
/// outer edge of the circle line, in print mm at the base scale (ruling R4).
pub fn file_placement(
    offset: Offset,
    text: &str,
    sizes: &CourseSymbolSizes,
    map_scale: u32,
    print_scale: Option<u32>,
) -> Placement {
    let to_print = f64::from(map_scale) / f64::from(print_scale.unwrap_or(map_scale).max(1));
    let centre = (offset.dx_mm * to_print, offset.dy_mm * to_print);
    placement::placement_of(
        (0.0, 0.0),
        r_outer(sizes, 1.0),
        centre,
        number_size(text, sizes.number_height_mm),
    )
}

/// The course file's placement of the number `text` at `stop` (level 2), if the file gives one.
pub fn base_placement(
    stop: &Stop,
    text: &str,
    sizes: &CourseSymbolSizes,
    map_scale: u32,
    print_scale: Option<u32>,
) -> Option<Placement> {
    stop.number.map(Placement::from).or_else(|| {
        stop.number_text_at
            .map(|o| file_placement(o, text, sizes, map_scale, print_scale))
    })
}

/// Radius of the control circle plus half its line, times `factor`.
fn r_outer(sizes: &CourseSymbolSizes, factor: f64) -> f64 {
    (sizes.circle_diameter_mm / 2.0 + sizes.line_width_mm / 2.0) * factor
}

/// The numbers of `course` printed with the layout `layout_id`, with their positions resolved on levels 2 to 4
/// (spec §5.7); level 1 numbers keep [`automatic_start`] until [`place_on_sheet`]. `map_scale` is the map's
/// scale denominator.
pub fn resolve(
    course: &Course<MapCoord>,
    event: &FittedEvent,
    project: &LoadedProject,
    layout_id: &str,
    map_scale: u32,
) -> Vec<ResolvedNumber> {
    let sizes = project.rules.course_symbol(project.project.map.kind);
    let corrections = &project.project.numbers;
    labels(course, event)
        .into_iter()
        .map(|(stop, control, text)| {
            let code = &control.code;
            let base = base_placement(
                &course.stops[stop],
                &text,
                &sizes,
                map_scale,
                course.print_scale_hint,
            );
            let (mut placement, mut level) = match base {
                Some(b) => (b, Level::CourseFile),
                None => (automatic_start(gap_range(&project.rules)), Level::Automatic),
            };
            let layout = corrections.iter().find(|n| {
                &n.control == code && n.course.is_none() && n.layout.as_deref() == Some(layout_id)
            });
            let sheet = corrections
                .iter()
                .find(|n| &n.control == code && n.course.as_deref() == Some(course.name.as_str()));
            for (n, l) in [(layout, Level::Layout), (sheet, Level::Sheet)] {
                if let Some(n) = n {
                    placement = Placement {
                        direction: n.direction,
                        gap_mm: n.gap_mm.unwrap_or(placement.gap_mm),
                    };
                    level = l;
                }
            }
            ResolvedNumber {
                stop,
                control: code.clone(),
                text,
                placement,
                level,
                base,
            }
        })
        .collect()
}

/// A [`Stale::CorrectionOverridesChangedBase`] for every correction with `base_seen` that no longer matches
/// ([`base_changed`]) any placement the course file now gives its control in the courses it applies to: its own
/// course, or every course printed with its layout ([`layout_for`]). A layout correction stands over several
/// courses whose file positions may differ, and `base_seen` holds only the one it was made against; so one
/// course still matching counts as unchanged. One entry per correction.
pub fn changed_bases(project: &LoadedProject, event: &FittedEvent, map_scale: u32) -> Vec<Stale> {
    let sizes = project.rules.course_symbol(project.project.map.kind);
    let mut out = Vec::new();
    for n in &project.project.numbers {
        let Some(seen) = &n.base_seen else { continue };
        let applies = |c: &Course<MapCoord>| match (&n.course, &n.layout) {
            (Some(course), _) => &c.name == course,
            (None, Some(layout)) => &layout_for(&project.project, &c.name).0.id == layout,
            (None, None) => false,
        };
        let unchanged = event.courses.iter().filter(|c| applies(c)).any(|c| {
            labels(c, event)
                .into_iter()
                .filter(|(_, control, _)| control.code == n.control)
                .any(|(stop, _, text)| {
                    let now = base_placement(
                        &c.stops[stop],
                        &text,
                        &sizes,
                        map_scale,
                        c.print_scale_hint,
                    );
                    now.is_some_and(|now| !base_changed(seen, &now.into()))
                })
        });
        if !unchanged {
            out.push(Stale::CorrectionOverridesChangedBase {
                control: n.control.clone(),
            });
        }
    }
    out
}

/// What a number box conflicts with on the sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Conflict {
    /// The symbol (circle, triangle, finish circles, crossing point) of this control of the course.
    Symbol(String),
    /// The leg between these controls.
    Leg(String, String),
    /// The number at this control.
    Number(String),
    /// The sheet element with this id.
    Element(String),
    /// The box reaches past the map frame.
    OutsideFrame,
    /// Point symbols of the map it covers.
    PointSymbols(usize),
    /// Line symbols of the map it covers.
    LineSymbols(usize),
}

/// A control number in conflict on a course sheet, and a better position (spec §5.7). A warning: the sheet still
/// draws the number where its level puts it.
#[derive(Debug, Clone, PartialEq)]
pub struct NumberConflict {
    pub course: String,
    pub control: String,
    pub conflicts: Vec<Conflict>,
    /// The best position the search finds, to enter as a correction.
    pub proposal: NumberPlacement,
}

/// One number on the sheet.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedNumber {
    pub resolved: ResolvedNumber,
    /// The centre of its box on the page.
    pub centre: Pt,
    pub score: Score,
    /// The best position for it among the obstacles of the sheet, with its score.
    pub proposal: (Placement, Score),
}

/// The numbers of a course sheet and their conflicts.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetNumbers {
    /// In the order of [`resolve`].
    pub numbers: Vec<PlacedNumber>,
    pub conflicts: Vec<NumberConflict>,
    pub gaps_out_of_range: Vec<GapOutOfRange>,
}

/// A number whose gap, from the course file or a correction, lies outside the range of the `number_gap` rule
/// (ruling R29). A warning; the number is drawn with its gap.
#[derive(Debug, Clone, PartialEq)]
pub struct GapOutOfRange {
    pub course: String,
    pub control: String,
    pub gap_mm: f64,
    pub min: f64,
    pub max: f64,
}

/// The course drawn on a sheet, as the obstacles of its numbers: stop symbols (stop index, centre, outer
/// radius), leg lines (leg index and points) and half the line width, all in page mm.
#[derive(Debug, Clone, PartialEq)]
pub struct CourseShapes {
    pub symbols: Vec<(usize, Pt, f64)>,
    pub legs: Vec<(usize, Vec<Pt>)>,
    pub half_line: f64,
}

impl CourseShapes {
    pub fn new(
        course: &Course<MapCoord>,
        event: &FittedEvent,
        to_page: impl Fn(MapCoord) -> Pt + Copy,
        sizes: &CourseSymbolSizes,
        symbol_factor: f64,
    ) -> Self {
        CourseShapes {
            symbols: stop_symbols(course, event, to_page, sizes, symbol_factor),
            legs: leg_lines(course, event, to_page, sizes, symbol_factor),
            half_line: sizes.line_width_mm * symbol_factor / 2.0,
        }
    }
}

// Obstacle labels: the kind and an index, decoded by `conflict`.
const SYMBOL: char = 's';
const LEG: char = 'l';
const NUMBER: char = 'n';
const ELEMENT: char = 'e';
const MAP_POINT: char = 'p';
const MAP_LINE: char = 'm';

/// The obstacles of a course sheet for its numbers, in page mm: the course's stop symbols and legs and the sheet
/// `elements` (id and box) as overprint; the map's point and line symbols as map symbols, except north lines;
/// and its areas and texts, by their boxes, as soft obstacles weighted by colour class ([`PlacementWeights`],
/// classes as [`full_strength`]). Map primitives only count where they reach into one of `regions`, so the list
/// stays short on a large map. The numbers themselves are added per number by [`place_on_sheet`].
pub fn obstacles(
    page: &RenderedPage,
    course: &CourseShapes,
    elements: &[(String, PaperBox)],
    weights: &PlacementWeights,
    regions: &[Rect],
) -> Vec<Obstacle> {
    let overprint = |shape, label: String| Obstacle {
        shape,
        weight: 0.0,
        tier: Tier::Overprint,
        label,
    };
    let mut out = Vec::new();
    for &(stop, centre, radius) in &course.symbols {
        out.push(overprint(
            ObstacleShape::Circle { centre, radius },
            format!("{SYMBOL}{stop}"),
        ));
    }
    for (leg, line) in &course.legs {
        for w in line.windows(2) {
            out.push(overprint(
                ObstacleShape::Segment {
                    a: w[0],
                    b: w[1],
                    half_width: course.half_line,
                },
                format!("{LEG}{leg}"),
            ));
        }
    }
    for (i, (_, b)) in elements.iter().enumerate() {
        out.push(overprint(
            ObstacleShape::Rect(Rect {
                min: b.min,
                max: b.max,
            }),
            format!("{ELEMENT}{i}"),
        ));
    }
    let weight_of: HashMap<i32, f64> = page
        .colours
        .iter()
        .map(|c| {
            let w = match full_strength(&c.name, c.cmyk) {
                Some(FullStrength::Black) => weights.black,
                Some(FullStrength::Brown) => weights.brown,
                Some(FullStrength::Blue) => weights.blue,
                None => weights.other,
            };
            (c.priority, w)
        })
        .collect();
    let near = |r: &Rect| regions.iter().any(|g| touches(g, r));
    for p in &page.primitives {
        if p.north_line {
            continue;
        }
        let Some((min, max)) = shape_bounds(&p.shape) else {
            continue;
        };
        let bounds = Rect { min, max };
        if !near(&bounds) {
            continue;
        }
        match p.class {
            SymbolClass::Point | SymbolClass::Line => {
                let kind = if p.class == SymbolClass::Point {
                    MAP_POINT
                } else {
                    MAP_LINE
                };
                let mut shapes = Vec::new();
                outline(&p.shape, &near, &mut shapes);
                out.extend(shapes.into_iter().map(|shape| Obstacle {
                    shape,
                    weight: 0.0,
                    tier: Tier::MapSymbol,
                    label: format!("{kind}{}", p.object),
                }));
            }
            SymbolClass::Area | SymbolClass::Text => out.push(Obstacle {
                shape: ObstacleShape::Rect(bounds),
                weight: weight_of.get(&p.priority).copied().unwrap_or(weights.other),
                tier: Tier::Soft,
                label: String::new(),
            }),
        }
    }
    out
}

fn touches(a: &Rect, b: &Rect) -> bool {
    a.min.0 <= b.max.0 && b.min.0 <= a.max.0 && a.min.1 <= b.max.1 && b.min.1 <= a.max.1
}

/// The shapes a point or line symbol primitive paints: a polygon per filled ring, a segment per stroked piece
/// that reaches `near`, the box of a text; a clipped shape counts as its inner shape.
fn outline(shape: &Shape, near: &impl Fn(&Rect) -> bool, out: &mut Vec<ObstacleShape>) {
    match shape {
        Shape::Fill { rings, .. } => out.extend(
            rings
                .iter()
                .filter(|r| r.len() > 2)
                .map(|r| ObstacleShape::Polygon(r.clone())),
        ),
        Shape::Stroke {
            path,
            closed,
            width,
            ..
        } => {
            let close = (*closed && path.len() > 2).then(|| [path[path.len() - 1], path[0]]);
            let pairs = path.windows(2).map(|w| [w[0], w[1]]).chain(close);
            for [a, b] in pairs {
                let half_width = width / 2.0;
                let segment = ObstacleShape::Segment { a, b, half_width };
                if near(&segment.bbox()) {
                    out.push(segment);
                }
            }
        }
        Shape::Text { .. } => {
            if let Some((min, max)) = shape_bounds(shape) {
                out.push(ObstacleShape::Rect(Rect { min, max }));
            }
        }
        Shape::Clipped { inner, .. } => outline(inner, near, out),
    }
}

/// The placement target of the number at stop `stop` of `course`: its circle and box on the page, and the legs
/// at it for the preferred direction (a score course has none).
fn target(
    course: &Course<MapCoord>,
    event: &FittedEvent,
    stop: usize,
    text: &str,
    to_page: &impl Fn(MapCoord) -> Pt,
    sizes: &CourseSymbolSizes,
    sf: f64,
) -> Option<Target> {
    let position = |i: usize| -> Option<Pt> {
        let code = &course.stops.get(i)?.control;
        let control = event.controls.iter().find(|c| &c.code == code)?;
        control.position.map(to_page)
    };
    let centre = position(stop)?;
    let mut legs = Legs {
        incoming: None,
        outgoing: Vec::new(),
        half_line: sizes.line_width_mm * sf / 2.0,
    };
    if course.kind == crate::course::CourseKind::Normal {
        // Leg `stop - 1` arrives here, leg `stop` leaves: the point before is the last bend or the stop before.
        if let Some(i) = stop.checked_sub(1)
            && let Some(leg) = course.legs.get(i)
        {
            let before = leg
                .bends
                .last()
                .map(|b| to_page(*b))
                .or_else(|| position(i));
            legs.incoming = before.and_then(|b| placement::direction_to(centre, b));
        }
        if let (Some(leg), Some(next)) = (course.legs.get(stop), position(stop + 1)) {
            legs.outgoing = std::iter::once(centre)
                .chain(leg.bends.iter().map(|b| to_page(*b)))
                .chain(std::iter::once(next))
                .collect();
        }
    }
    Some(Target {
        centre,
        circle_outer_r: r_outer(sizes, sf),
        size: number_size(text, sizes.number_height_mm * sf),
        unit: sf,
        legs,
        gaps: GapRange::DEFAULT,
    })
}

/// Places and checks the numbers of `course` on its sheet (spec §5.7). Level 1 numbers are placed by
/// [`placement::best`] in course order, each placed box an obstacle for the next; the others stay where their
/// level puts them. Then every number is scored among the course, the sheet `elements`, the map and the other
/// numbers, within the map frame (`page.clip`), and gets a proposal. A number with an overprint conflict, or
/// covering map symbols where the proposal covers fewer, gives a [`NumberConflict`]. `gap_rule` is the gap range
/// of the rule set, `None` without a `number_gap` rule: the search then tries [`placement::DEFAULT_GAP_MM`] alone,
/// and with a rule every number the file or a correction places outside the range gives a [`GapOutOfRange`].
/// `to_page` maps map coordinates to the page; `sf` is the symbol factor. Numbered stops without a map position
/// are left out.
#[allow(clippy::too_many_arguments)]
pub fn place_on_sheet(
    course: &Course<MapCoord>,
    event: &FittedEvent,
    resolved: Vec<ResolvedNumber>,
    page: &RenderedPage,
    to_page: impl Fn(MapCoord) -> Pt + Copy,
    elements: &[(String, PaperBox)],
    sizes: &CourseSymbolSizes,
    sf: f64,
    weights: &PlacementWeights,
    gap_rule: Option<GapRange>,
) -> SheetNumbers {
    let gaps = gap_rule.unwrap_or(GapRange::DEFAULT);
    let numbers: Vec<(ResolvedNumber, Target)> = resolved
        .into_iter()
        .filter_map(|r| {
            let mut t = target(course, event, r.stop, &r.text, &to_page, sizes, sf)?;
            t.gaps = gaps;
            Some((r, t))
        })
        .collect();
    let shapes = CourseShapes::new(course, event, to_page, sizes, sf);
    let regions: Vec<Rect> = numbers
        .iter()
        .map(|(r, t)| Rect::centred(t.centre, (0.0, 0.0)).grown(reach(t, r.placement.gap_mm)))
        .collect();
    let base = obstacles(page, &shapes, elements, weights, &regions);
    let bounds = Rect {
        min: page.clip.min,
        max: page.clip.max,
    };
    // Obstacles for number `i`: the base ones near it, without its own symbol (and any symbol at the same
    // place: the control visited again), plus the boxes of the other numbers placed so far.
    let for_number = |i: usize, boxes: &[Option<Rect>]| -> Vec<Obstacle> {
        let t = &numbers[i].1;
        let region = regions[i];
        let own: Vec<String> = shapes
            .symbols
            .iter()
            .filter(|(_, c, _)| (c.0 - t.centre.0).hypot(c.1 - t.centre.1) < 1e-6)
            .map(|(s, _, _)| format!("{SYMBOL}{s}"))
            .collect();
        let mut out: Vec<Obstacle> = base
            .iter()
            .filter(|o| touches(&o.shape.bbox(), &region) && !own.contains(&o.label))
            .cloned()
            .collect();
        for (j, b) in boxes.iter().enumerate() {
            if let (true, Some(b)) = (j != i, b) {
                out.push(Obstacle {
                    shape: ObstacleShape::Rect(*b),
                    weight: 0.0,
                    tier: Tier::Overprint,
                    label: format!("{NUMBER}{}", numbers[j].0.stop),
                });
            }
        }
        out
    };

    let mut boxes: Vec<Option<Rect>> = numbers
        .iter()
        .map(|(r, t)| (r.level != Level::Automatic).then(|| t.box_at(r.placement)))
        .collect();
    let mut placements: Vec<Placement> = numbers.iter().map(|(r, _)| r.placement).collect();
    for i in 0..numbers.len() {
        if numbers[i].0.level == Level::Automatic {
            let (p, _) = placement::best(
                &numbers[i].1,
                bounds,
                &for_number(i, &boxes),
                automatic_start(gaps),
            );
            placements[i] = p;
            boxes[i] = Some(numbers[i].1.box_at(p));
        }
    }

    let mut out = SheetNumbers {
        numbers: Vec::new(),
        conflicts: Vec::new(),
        gaps_out_of_range: Vec::new(),
    };
    for (i, (r, t)) in numbers.iter().enumerate() {
        if let Some(range) = gap_rule
            && r.level != Level::Automatic
            && !range.contains(r.placement.gap_mm)
        {
            out.gaps_out_of_range.push(GapOutOfRange {
                course: course.name.clone(),
                control: r.control.clone(),
                gap_mm: r.placement.gap_mm,
                min: range.min,
                max: range.max,
            });
        }
        let obstacles = for_number(i, &boxes);
        let placed = t.box_at(placements[i]);
        let score = placement::score(placed, bounds, &obstacles);
        let current = placements[i];
        let proposal = placement::best(t, bounds, &obstacles, current);
        let better_cover = proposal.1.covered.len() < score.covered.len();
        if !score.hard.is_empty() || (!score.covered.is_empty() && better_cover) {
            out.conflicts.push(NumberConflict {
                course: course.name.clone(),
                control: r.control.clone(),
                conflicts: conflicts(&score, course, elements),
                proposal: proposal.0.into(),
            });
        }
        out.numbers.push(PlacedNumber {
            resolved: ResolvedNumber {
                placement: current,
                ..r.clone()
            },
            centre: placed.centre(),
            score,
            proposal,
        });
    }
    out
}

/// How far a number box can reach from its circle centre at any candidate gap or at `gap`.
fn reach(t: &Target, gap: f64) -> f64 {
    t.reach()
        .max(t.circle_outer_r + gap * t.unit + t.size.0.hypot(t.size.1))
}

/// The conflicts of `score` in the course's terms.
fn conflicts(
    score: &Score,
    course: &Course<MapCoord>,
    elements: &[(String, PaperBox)],
) -> Vec<Conflict> {
    let code = |stop: usize| {
        course
            .stops
            .get(stop)
            .map(|s| s.control.clone())
            .unwrap_or_default()
    };
    let mut out = Vec::new();
    for label in &score.hard {
        if label == OUTSIDE {
            out.push(Conflict::OutsideFrame);
            continue;
        }
        let mut chars = label.chars();
        let (Some(kind), Ok(index)) = (chars.next(), chars.as_str().parse::<usize>()) else {
            continue;
        };
        let c = match kind {
            SYMBOL => Conflict::Symbol(code(index)),
            LEG => Conflict::Leg(code(index), code(index + 1)),
            NUMBER => Conflict::Number(code(index)),
            ELEMENT => match elements.get(index) {
                Some((id, _)) => Conflict::Element(id.clone()),
                None => continue,
            },
            _ => continue,
        };
        if !out.contains(&c) {
            out.push(c);
        }
    }
    let count = |kind: char| score.covered.iter().filter(|l| l.starts_with(kind)).count();
    let (points, lines) = (count(MAP_POINT), count(MAP_LINE));
    if points > 0 {
        out.push(Conflict::PointSymbols(points));
    }
    if lines > 0 {
        out.push(Conflict::LineSymbols(lines));
    }
    out
}

#[cfg(test)]
mod tests;
