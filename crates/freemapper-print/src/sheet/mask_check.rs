//! `check` against the masks of a course sheet (ADR 0046): what of the course overprint lies where a mask hides
//! the map, and masks that hide nothing or the whole frame. The overprint is drawn all the same; the notes exist
//! because the runner sees such an object without the map under it.
//!
//! Points are tested with [`mask::polygon_contains`], the test [`mask::contains`] makes, in the internal frame
//! (map mm, y down). A point counts when it lies inside the map frame and any mask of the sheet hides it.

use std::collections::BTreeMap;

use crate::course::{ControlKind, Course, CourseKind, FittedEvent, Special};
use crate::courses::StopName;
use crate::mask::{self, Mask, MaskPolygon, to_internal};
use crate::omap::MapCoord;
use crate::overprint::{applies, leg_lines};
use crate::render::page::{PaperBox, Pt};
use crate::rules::CourseSymbolSizes;

use super::{MaskedObject, SheetNote};

/// Longest distance (page mm) between two points sampled along a path of the overprint.
pub const SAMPLE_STEP_MM: f64 = 1.0;
/// Spacing (page mm) of the grid of points sampled inside an out-of-bounds or dangerous area.
pub const AREA_GRID_MM: f64 = 2.0;
/// Most points one sheet samples. A hostile route of kilometres would otherwise take millions.
pub const MAX_MASK_SAMPLES: usize = 200_000;
/// Most elementary steps (box tests and ring edges) the check of one sheet takes. Thousands of white-outs with
/// many points each would otherwise make every sample slow.
pub const MAX_MASK_CHECK_STEPS: u64 = 50_000_000;

/// The masks of a sheet, polygon by polygon with their boxes in the internal frame, and what the check may
/// still spend.
struct Hider<'a> {
    polygons: Vec<(&'a MaskPolygon, [f64; 4], u64)>,
    frame: PaperBox,
    page_to_map: &'a dyn Fn(Pt) -> Pt,
    samples: usize,
    steps: u64,
    limited: bool,
}

/// The box (min x, min y, max x, max y) of the internal points of `ring`, written y up.
fn ring_box(ring: &[[f64; 2]]) -> [f64; 4] {
    ring.iter().map(|&p| to_internal(p)).fold(
        [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ],
        |b, (x, y)| [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)],
    )
}

fn polygon_points(p: &MaskPolygon) -> u64 {
    (p.outline.len() + p.holes.iter().map(Vec::len).sum::<usize>()) as u64
}

impl<'a> Hider<'a> {
    fn new(masks: &[&'a Mask], frame: PaperBox, page_to_map: &'a dyn Fn(Pt) -> Pt) -> Self {
        Hider {
            polygons: masks
                .iter()
                .flat_map(|m| &m.polygons)
                .map(|p| (p, ring_box(&p.outline), polygon_points(p)))
                .collect(),
            frame,
            page_to_map,
            samples: MAX_MASK_SAMPLES,
            steps: MAX_MASK_CHECK_STEPS,
            limited: false,
        }
    }

    /// Takes `steps` from the budget; `false` (and `limited` set) when they are not left.
    fn take(&mut self, steps: u64) -> bool {
        if self.limited || steps > self.steps {
            self.limited = true;
            self.steps = 0;
            return false;
        }
        self.steps -= steps;
        true
    }

    fn in_frame(&self, (x, y): Pt) -> bool {
        let f = self.frame;
        f.min.0 <= x && x <= f.max.0 && f.min.1 <= y && y <= f.max.1
    }

    /// Whether the page point `p` lies in the frame and some mask hides the map there; `None` when the budget ran
    /// out.
    fn hidden(&mut self, p: Pt) -> Option<bool> {
        if self.limited || self.samples == 0 {
            self.limited = true;
            return None;
        }
        self.samples -= 1;
        if !self.in_frame(p) {
            return Some(false);
        }
        let (x, y) = (self.page_to_map)(p);
        if !self.take(self.polygons.len() as u64) {
            return None;
        }
        for i in 0..self.polygons.len() {
            let (polygon, b, points) = self.polygons[i];
            if x < b[0] || x > b[2] || y < b[1] || y > b[3] {
                continue;
            }
            if !self.take(points) {
                return None;
            }
            if mask::polygon_contains(polygon, (x, y)) {
                return Some(true);
            }
        }
        Some(false)
    }

    /// Whether some point of the path `points` (page mm, closed back to its first point when `closed`) inside the
    /// frame is hidden, sampled at most [`SAMPLE_STEP_MM`] apart; `None` when the budget ran out.
    fn path_hidden(&mut self, points: &[Pt], closed: bool) -> Option<bool> {
        match points {
            [] => return Some(false),
            [one] => return self.hidden(*one),
            _ => {}
        }
        let n = points.len();
        let segments = if closed { n } else { n - 1 };
        for i in 0..segments {
            let (a, b) = (points[i], points[(i + 1) % n]);
            let Some((a, b)) = clip_segment(a, b, self.frame) else {
                continue;
            };
            let len = (b.0 - a.0).hypot(b.1 - a.1);
            let steps = (len / SAMPLE_STEP_MM)
                .ceil()
                .clamp(1.0, MAX_MASK_SAMPLES as f64) as usize;
            for k in 0..=steps {
                let t = k as f64 / steps as f64;
                if self.hidden((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t))? {
                    return Some(true);
                }
            }
        }
        Some(false)
    }

    /// Whether the area of `rings` (page mm, even-odd) is hidden anywhere in the frame: along its outline, or at
    /// a point of the [`AREA_GRID_MM`] grid inside it.
    fn area_hidden(&mut self, rings: &[Vec<Pt>]) -> Option<bool> {
        for ring in rings {
            if self.path_hidden(ring, true)? {
                return Some(true);
            }
        }
        let points: Vec<Pt> = rings.iter().flatten().copied().collect();
        let Some(b) = points
            .iter()
            .copied()
            .fold(None, |b: Option<PaperBox>, (x, y)| {
                Some(match b {
                    None => PaperBox {
                        min: (x, y),
                        max: (x, y),
                    },
                    Some(b) => PaperBox {
                        min: (b.min.0.min(x), b.min.1.min(y)),
                        max: (b.max.0.max(x), b.max.1.max(y)),
                    },
                })
            })
        else {
            return Some(false);
        };
        let f = self.frame;
        let (x0, y0) = (b.min.0.max(f.min.0), b.min.1.max(f.min.1));
        let (x1, y1) = (b.max.0.min(f.max.0), b.max.1.min(f.max.1));
        if !(x0 <= x1 && y0 <= y1) {
            return Some(false);
        }
        let cols = ((x1 - x0) / AREA_GRID_MM)
            .floor()
            .min(MAX_MASK_SAMPLES as f64) as usize;
        let rows = ((y1 - y0) / AREA_GRID_MM)
            .floor()
            .min(MAX_MASK_SAMPLES as f64) as usize;
        let inside_cost = points.len() as u64;
        for r in 0..=rows {
            for c in 0..=cols {
                let p = (x0 + c as f64 * AREA_GRID_MM, y0 + r as f64 * AREA_GRID_MM);
                if !self.take(inside_cost) {
                    return None;
                }
                let odd = rings.iter().filter(|r| mask::inside(r, p)).count() % 2 == 1;
                if odd && self.hidden(p)? {
                    return Some(true);
                }
            }
        }
        Some(false)
    }
}

/// The part of the segment `a b` inside `frame` (Liang-Barsky), `None` when none is.
pub(super) fn clip_segment(a: Pt, b: Pt, frame: PaperBox) -> Option<(Pt, Pt)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [
        (-dx, a.0 - frame.min.0),
        (dx, frame.max.0 - a.0),
        (-dy, a.1 - frame.min.1),
        (dy, frame.max.1 - a.1),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
        }
    }
    // A non-finite point fails this as well: no comparison with NaN holds.
    if t0
        .partial_cmp(&t1)
        .is_none_or(|o| o == std::cmp::Ordering::Greater)
    {
        return None;
    }
    Some((
        (a.0 + dx * t0, a.1 + dy * t0),
        (a.0 + dx * t1, a.1 + dy * t1),
    ))
}

/// [`SheetNote::ElementCoversWindow`] for each element in `rects` (id and page box) that meets a hole of one of
/// the masks the sheet applies (ruling R15): on a window course the windows are the only map the sheet shows. Only
/// the part of an element inside the map `frame` counts. A hole ring is tested by its box first, then by its edges
/// through the element's box, then by whether it holds the box. `map_to_page` places a point of the internal frame
/// on the page. One note per element and mask.
pub(super) fn elements_over_windows(
    course: &str,
    rects: &[(String, PaperBox)],
    masks: &[&Mask],
    frame: PaperBox,
    map_to_page: &dyn Fn(Pt) -> Pt,
) -> Vec<SheetNote> {
    let holes: Vec<(usize, Vec<Pt>, PaperBox)> = masks
        .iter()
        .enumerate()
        .flat_map(|(i, m)| {
            m.polygons
                .iter()
                .flat_map(move |p| p.holes.iter().map(move |h| (i, h)))
        })
        .filter_map(|(i, h)| {
            let ring: Vec<Pt> = h.iter().map(|&p| map_to_page(to_internal(p))).collect();
            let b = page_box(&ring)?;
            Some((i, ring, b))
        })
        .collect();
    let mut notes = Vec::new();
    if holes.is_empty() {
        return notes;
    }
    for (id, rect) in rects {
        let r = PaperBox {
            min: (rect.min.0.max(frame.min.0), rect.min.1.max(frame.min.1)),
            max: (rect.max.0.min(frame.max.0), rect.max.1.min(frame.max.1)),
        };
        if !(r.min.0 <= r.max.0 && r.min.1 <= r.max.1) {
            continue;
        }
        let mut met: Vec<usize> = Vec::new();
        for (i, ring, b) in &holes {
            if met.contains(i)
                || b.max.0 < r.min.0
                || b.min.0 > r.max.0
                || b.max.1 < r.min.1
                || b.min.1 > r.max.1
            {
                continue;
            }
            let n = ring.len();
            let crosses = (0..n).any(|k| clip_segment(ring[k], ring[(k + 1) % n], r).is_some());
            let centre = ((r.min.0 + r.max.0) / 2.0, (r.min.1 + r.max.1) / 2.0);
            if crosses || mask::inside(ring, centre) {
                met.push(*i);
            }
        }
        met.sort_unstable();
        notes.extend(met.into_iter().map(|i| SheetNote::ElementCoversWindow {
            course: course.to_string(),
            element: id.clone(),
            mask: masks[i].id.clone(),
        }));
    }
    notes
}

/// The box around `points`, `None` for none.
fn page_box(points: &[Pt]) -> Option<PaperBox> {
    let (&first, rest) = points.split_first()?;
    Some(rest.iter().fold(
        PaperBox {
            min: first,
            max: first,
        },
        |b, &(x, y)| PaperBox {
            min: (b.min.0.min(x), b.min.1.min(y)),
            max: (b.max.0.max(x), b.max.1.max(y)),
        },
    ))
}

/// Counts one more object of `kind`.
fn found(counts: &mut BTreeMap<MaskedObject, usize>, kind: MaskedObject) {
    *counts.entry(kind).or_default() += 1;
}

/// What one polygon of a mask does to the frame `r` (internal frame: min x, min y, max x, max y).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cover {
    Nothing,
    Part,
    All,
}

/// Whether the ring `ring` (written y up) has a point in `r` or an edge through it.
fn ring_meets(ring: &[[f64; 2]], r: [f64; 4]) -> bool {
    let frame = PaperBox {
        min: (r[0], r[1]),
        max: (r[2], r[3]),
    };
    let n = ring.len();
    (0..n).any(|i| {
        let (a, b) = (to_internal(ring[i]), to_internal(ring[(i + 1) % n]));
        clip_segment(a, b, frame).is_some()
    })
}

/// How much of the frame `r` the polygon hides, with its outline's box `b`. The outline's edge through the frame
/// hides a part next to it, as holes lie inside the outline. When the frame lies wholly inside the outline, it is
/// all hidden unless a hole reaches it; a frame wholly in one hole sees nothing hidden. A frame that only the
/// union of overlapping holes covers counts as partly hidden.
fn cover(polygon: &MaskPolygon, b: [f64; 4], r: [f64; 4]) -> Cover {
    if b[2] < r[0] || b[0] > r[2] || b[3] < r[1] || b[1] > r[3] {
        return Cover::Nothing;
    }
    if ring_meets(&polygon.outline, r) {
        return Cover::Part;
    }
    let centre = ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);
    if !mask::ring_contains(&polygon.outline, centre) {
        return Cover::Nothing;
    }
    let mut touched = false;
    for hole in &polygon.holes {
        let meets = ring_meets(hole, r);
        if !meets && mask::ring_contains(hole, centre) {
            return Cover::Nothing;
        }
        touched |= meets || mask::ring_contains(hole, centre);
    }
    if touched { Cover::Part } else { Cover::All }
}

/// A mask a sheet applies, and whether it is a white-out of the course file (not a `[[masks]]` entry).
#[derive(Debug, Clone, Copy)]
pub(super) struct Applied<'a> {
    pub mask: &'a Mask,
    pub white_out: bool,
}

/// The notes of a course sheet about its `masks` (those it applies): [`SheetNote::MaskCoversNothing`] per
/// `[[masks]]` entry and one [`SheetNote::WhiteOutsCoverNothing`] for the white-outs of the course file that hide
/// nothing (ruling R12), [`SheetNote::MaskLeavesNoWindow`] per mask, [`SheetNote::ControlInMaskedArea`],
/// [`SheetNote::OverprintInMaskedArea`] per kind, and [`SheetNote::MaskCheckLimited`] when the budget ran out.
/// `to_page` places a course coordinate on the page, `page_to_map` turns a page point into the internal frame,
/// and `frame` is the map frame on the page. `sizes` and `course_factor` are the ones the overprint is drawn
/// with, so a marked-route leg is sampled where it is drawn, outside its circles. No masks, no notes.
#[allow(clippy::too_many_arguments)]
pub(super) fn mask_notes(
    course: &Course<MapCoord>,
    event: &FittedEvent,
    applied: &[Applied<'_>],
    frame: PaperBox,
    to_page: &dyn Fn(MapCoord) -> Pt,
    page_to_map: &dyn Fn(Pt) -> Pt,
    sizes: &CourseSymbolSizes,
    course_factor: f64,
) -> Vec<SheetNote> {
    let mut notes = Vec::new();
    if applied.is_empty() {
        return notes;
    }
    let masks: Vec<&Mask> = applied.iter().map(|a| a.mask).collect();
    let name = || course.name.clone();
    let mut hider = Hider::new(&masks, frame, page_to_map);

    // The frame in the internal frame; the page and the map share the direction of y.
    let (ax, ay) = page_to_map(frame.min);
    let (bx, by) = page_to_map(frame.max);
    let r = [ax.min(bx), ay.min(by), ax.max(bx), ay.max(by)];
    let mut idle_white_outs: Vec<String> = Vec::new();
    for a in applied {
        let m = a.mask;
        let mut covers = Cover::Nothing;
        for p in &m.polygons {
            if !hider.take(polygon_points(p)) {
                break;
            }
            match cover(p, ring_box(&p.outline), r) {
                Cover::All => {
                    covers = Cover::All;
                    break;
                }
                Cover::Part => covers = Cover::Part,
                Cover::Nothing => {}
            }
        }
        if hider.limited {
            break;
        }
        match covers {
            Cover::Nothing if a.white_out => idle_white_outs.push(m.id.clone()),
            Cover::Nothing => notes.push(SheetNote::MaskCoversNothing {
                course: name(),
                mask: m.id.clone(),
            }),
            Cover::All if !course.legs.is_empty() => notes.push(SheetNote::MaskLeavesNoWindow {
                course: name(),
                mask: m.id.clone(),
            }),
            _ => {}
        }
    }

    if !idle_white_outs.is_empty() {
        notes.push(SheetNote::WhiteOutsCoverNothing {
            course: name(),
            ids: idle_white_outs,
        });
    }

    let mut controls: Vec<StopName> = Vec::new();
    let mut counts: BTreeMap<MaskedObject, usize> = BTreeMap::new();
    'check: {
        if hider.limited {
            break 'check;
        }
        // Stops by their centres, as their effective kind draws them.
        for stop in &course.stops {
            let Some(control) = event.controls.iter().find(|c| c.code == stop.control) else {
                continue;
            };
            let Some(at) = control.position else {
                continue;
            };
            let kind = stop.kind.unwrap_or(control.kind);
            if kind == ControlKind::EndOfMarkedRoute {
                continue;
            }
            let Some(hidden) = hider.hidden(to_page(at)) else {
                break 'check;
            };
            if !hidden {
                continue;
            }
            if kind == ControlKind::CrossingPoint {
                found(&mut counts, MaskedObject::CrossingPoint);
            } else if !controls.iter().any(|c| c.code() == stop.control) {
                controls.push(StopName::new(&stop.control, kind));
            }
        }
        // Marked-route legs, from where they leave their first stop's symbol to where they reach the second's.
        if course.kind == CourseKind::Normal {
            let mut legs: BTreeMap<usize, Vec<Vec<Pt>>> = BTreeMap::new();
            for (i, piece) in leg_lines(course, event, to_page, sizes, course_factor) {
                if course.legs[i].marked_route {
                    legs.entry(i).or_default().push(piece);
                }
            }
            for pieces in legs.values() {
                let mut hidden = false;
                for piece in pieces {
                    let Some(h) = hider.path_hidden(piece, false) else {
                        break 'check;
                    };
                    if h {
                        hidden = true;
                        break;
                    }
                }
                if hidden {
                    found(&mut counts, MaskedObject::MarkedRoute);
                }
            }
        }
        let path = |p: &[MapCoord]| -> Vec<Pt> { p.iter().map(|c| to_page(*c)).collect() };
        for special in &event.specials {
            let (kind, hidden) = match special {
                Special::MarkedRoute { path: p, courses }
                    if applies(courses, &course.name) && p.len() > 1 =>
                {
                    (
                        MaskedObject::MarkedRoute,
                        hider.path_hidden(&path(p), false),
                    )
                }
                Special::ForbiddenRoute { path: p, courses } if applies(courses, &course.name) => (
                    MaskedObject::ForbiddenRoute,
                    hider.path_hidden(&path(p), false),
                ),
                Special::Boundary { path: p, courses }
                    if applies(courses, &course.name) && p.len() > 1 =>
                {
                    (MaskedObject::Boundary, hider.path_hidden(&path(p), false))
                }
                Special::OutOfBounds { rings, courses } if applies(courses, &course.name) => {
                    let rings: Vec<Vec<Pt>> = rings.iter().map(|r| path(r)).collect();
                    (MaskedObject::OutOfBoundsArea, hider.area_hidden(&rings))
                }
                Special::Dangerous { rings, courses } if applies(courses, &course.name) => {
                    let rings: Vec<Vec<Pt>> = rings.iter().map(|r| path(r)).collect();
                    (MaskedObject::DangerousArea, hider.area_hidden(&rings))
                }
                Special::Text { pt, courses, .. } if applies(courses, &course.name) => {
                    (MaskedObject::Text, hider.hidden(to_page(*pt)))
                }
                _ => continue,
            };
            match hidden {
                None => break 'check,
                Some(true) => found(&mut counts, kind),
                Some(false) => {}
            }
        }
    }
    if !controls.is_empty() {
        notes.push(SheetNote::ControlInMaskedArea {
            course: name(),
            controls,
        });
    }
    notes.extend(
        counts
            .into_iter()
            .map(|(kind, count)| SheetNote::OverprintInMaskedArea {
                course: name(),
                kind,
                count,
            }),
    );
    if hider.limited {
        notes.push(SheetNote::MaskCheckLimited { course: name() });
    }
    notes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> PaperBox {
        PaperBox {
            min: (0.0, 0.0),
            max: (100.0, 100.0),
        }
    }

    #[test]
    fn segments_are_clipped_to_the_frame() {
        let f = frame();
        assert_eq!(
            clip_segment((-50.0, 50.0), (150.0, 50.0), f),
            Some(((0.0, 50.0), (100.0, 50.0)))
        );
        assert_eq!(clip_segment((-50.0, -1.0), (150.0, -1.0), f), None);
        assert_eq!(
            clip_segment((10.0, 10.0), (20.0, 30.0), f),
            Some(((10.0, 10.0), (20.0, 30.0)))
        );
        assert_eq!(clip_segment((-10.0, 50.0), (-5.0, 200.0), f), None);
        // One point inside: kept.
        assert_eq!(
            clip_segment((5.0, 5.0), (5.0, 5.0), f),
            Some(((5.0, 5.0), (5.0, 5.0)))
        );
    }

    #[test]
    fn samples_stop_at_the_budget() {
        // A mask far off the path: every sample is tested and none is hidden. 10 000 points 141 mm apart ask for
        // about 1.4 million samples; the hider takes exactly its budget and then refuses.
        let m = Mask {
            id: "m".into(),
            polygons: vec![MaskPolygon {
                outline: vec![[500.0, -500.0], [600.0, -500.0], [600.0, -600.0]],
                holes: vec![],
            }],
        };
        let masks = [&m];
        let page_to_map = |p: Pt| p;
        let mut hider = Hider::new(&masks, frame(), &page_to_map);
        let path: Vec<Pt> = (0..10_000)
            .map(|i| {
                if i % 2 == 0 {
                    (0.0, 0.0)
                } else {
                    (100.0, 100.0)
                }
            })
            .collect();
        assert_eq!(hider.path_hidden(&path, false), None);
        assert!(hider.limited);
        assert_eq!(hider.samples, 0);
        assert_eq!(hider.hidden((1.0, 1.0)), None);
        // A short path stays well within it and takes one sample per millimetre and the end.
        let mut hider = Hider::new(&masks, frame(), &page_to_map);
        assert_eq!(
            hider.path_hidden(&[(0.0, 10.0), (10.0, 10.0)], false),
            Some(false)
        );
        assert_eq!(MAX_MASK_SAMPLES - hider.samples, 11);
        // Outside the frame nothing is sampled.
        assert_eq!(
            hider.path_hidden(&[(-50.0, -10.0), (150.0, -10.0)], false),
            Some(false)
        );
        assert_eq!(MAX_MASK_SAMPLES - hider.samples, 11);
    }

    #[test]
    fn elements_over_windows_count_by_the_hole_rings() {
        // One mask: the whole frame masked but a window from (40, 40) to (60, 60) on the page (the page is the
        // internal frame here; the rings are written y up).
        let m = Mask {
            id: "w".into(),
            polygons: vec![MaskPolygon {
                outline: vec![
                    [-50.0, 50.0],
                    [150.0, 50.0],
                    [150.0, -150.0],
                    [-50.0, -150.0],
                ],
                holes: vec![vec![
                    [40.0, -40.0],
                    [60.0, -40.0],
                    [60.0, -60.0],
                    [40.0, -60.0],
                ]],
            }],
        };
        let rect = |x0: f64, y0: f64, x1: f64, y1: f64| PaperBox {
            min: (x0, y0),
            max: (x1, y1),
        };
        let rects = vec![
            // Over part of the window.
            ("part".to_string(), rect(55.0, 55.0, 70.0, 70.0)),
            // Wholly inside it.
            ("inside".to_string(), rect(45.0, 45.0, 50.0, 50.0)),
            // Around it.
            ("around".to_string(), rect(30.0, 30.0, 70.0, 70.0)),
            // Over masked paper only, and across the window's box but not the window.
            ("paper".to_string(), rect(5.0, 5.0, 30.0, 30.0)),
            // Outside the frame, though the hole would reach it there.
            ("outside".to_string(), rect(-20.0, 45.0, -1.0, 50.0)),
        ];
        let notes = elements_over_windows("K", &rects, &[&m], frame(), &|p| p);
        let named: Vec<&str> = notes
            .iter()
            .map(|n| match n {
                SheetNote::ElementCoversWindow {
                    element,
                    mask,
                    course,
                } => {
                    assert_eq!((mask.as_str(), course.as_str()), ("w", "K"));
                    element.as_str()
                }
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(named, ["part", "inside", "around"]);
        assert!(!notes[0].is_info());
        // A mask without holes (a white-out) has no window to cover.
        let mut plain = m.clone();
        plain.polygons[0].holes.clear();
        assert!(elements_over_windows("K", &rects, &[&plain], frame(), &|p| p).is_empty());
    }

    #[test]
    fn cover_tells_nothing_part_and_all() {
        let square =
            |x0: f64, y0: f64, x1: f64, y1: f64| vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
        // The frame from (0, 0) to (100, 100) in the internal frame: y up in the rings, so -100..0 written.
        let r = [0.0, 0.0, 100.0, 100.0];
        let p = |outline: Vec<[f64; 2]>, holes: Vec<Vec<[f64; 2]>>| MaskPolygon { outline, holes };
        let of = |poly: &MaskPolygon| cover(poly, ring_box(&poly.outline), r);
        assert_eq!(
            of(&p(square(200.0, -50.0, 300.0, 0.0), vec![])),
            Cover::Nothing
        );
        assert_eq!(of(&p(square(50.0, -50.0, 300.0, 0.0), vec![])), Cover::Part);
        assert_eq!(
            of(&p(square(-10.0, -110.0, 110.0, 10.0), vec![])),
            Cover::All
        );
        // A hole that reaches the frame, and a hole that holds it.
        assert_eq!(
            of(&p(
                square(-10.0, -110.0, 110.0, 10.0),
                vec![square(40.0, -60.0, 60.0, -40.0)]
            )),
            Cover::Part
        );
        assert_eq!(
            of(&p(
                square(-50.0, -150.0, 150.0, 50.0),
                vec![square(-20.0, -120.0, 120.0, 20.0)]
            )),
            Cover::Nothing
        );
        // A ring around the frame, outside the box test but not holding it: an L-shape.
        assert_eq!(
            of(&p(
                vec![
                    [-50.0, 50.0],
                    [150.0, 50.0],
                    [150.0, -150.0],
                    [120.0, -150.0],
                    [120.0, 20.0],
                    [-50.0, 20.0]
                ],
                vec![]
            )),
            Cover::Nothing
        );
    }
}
