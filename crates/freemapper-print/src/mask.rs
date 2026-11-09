//! Mask variants for window and corridor orienteering (ADR 0045): polygons in map coordinates, each an outline
//! with holes. The masked area is the outline minus the union of its holes; the map is hidden there, and a hole
//! is a window onto the map.
//!
//! This module knows nothing of the project file or the course model, so that it can move into a core crate.
//! Points in a [`Mask`] are map millimetres as OOM's status bar shows them (y up, like `centre_map_mm`); the
//! functions that return [`Pt`] use the internal frame, map millimetres with y down.

use serde::{Deserialize, Serialize};

/// A point in the internal frame: map millimetres, y down.
pub type Pt = (f64, f64);

/// Most mask variants in one project file.
pub const MAX_MASKS: usize = 64;
/// Most polygons in one mask variant.
pub const MAX_POLYGONS: usize = 256;
/// Most points in one ring.
pub const MAX_RING_POINTS: usize = 4096;
/// Most points of all masks of one project file together.
pub const MAX_TOTAL_POINTS: usize = 100_000;
/// Longest mask id, in characters.
pub const MAX_ID_CHARS: usize = 64;
/// Farthest coordinate (map mm) in either direction: the reach of an OOM map, as for `centre_map_mm`.
pub const MAX_COORD_MM: f64 = 2_000_000.0;
/// Most elementary steps (segment pairs, point-in-ring edges, ring pairs) the checks of one project file take.
/// Masks within the point limits stay far below it unless their edges are built to defeat the sweep.
pub const MAX_CHECK_STEPS: u64 = 100_000_000;

/// A mask variant: a named set of polygons. A course prints with zero or one of them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mask {
    pub id: String,
    pub polygons: Vec<MaskPolygon>,
}

/// One polygon of a mask variant: the outline and its holes (windows), points `[x, y]` in map mm, y up. In the
/// file a polygon is a list of rings, the outline first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Vec<Vec<[f64; 2]>>", into = "Vec<Vec<[f64; 2]>>")]
pub struct MaskPolygon {
    pub outline: Vec<[f64; 2]>,
    pub holes: Vec<Vec<[f64; 2]>>,
}

impl TryFrom<Vec<Vec<[f64; 2]>>> for MaskPolygon {
    type Error = &'static str;

    fn try_from(mut rings: Vec<Vec<[f64; 2]>>) -> Result<Self, Self::Error> {
        if rings.is_empty() {
            return Err("a polygon needs at least its outline ring");
        }
        let outline = rings.remove(0);
        Ok(MaskPolygon {
            outline,
            holes: rings,
        })
    }
}

impl From<MaskPolygon> for Vec<Vec<[f64; 2]>> {
    fn from(p: MaskPolygon) -> Self {
        std::iter::once(p.outline).chain(p.holes).collect()
    }
}

impl MaskPolygon {
    /// The outline, then the holes.
    fn rings(&self) -> impl Iterator<Item = &Vec<[f64; 2]>> {
        std::iter::once(&self.outline).chain(&self.holes)
    }
}

impl Mask {
    /// Number of points in all rings.
    pub fn point_count(&self) -> usize {
        self.polygons
            .iter()
            .flat_map(MaskPolygon::rings)
            .map(Vec::len)
            .sum()
    }
}

/// What is wrong with a mask variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    /// The variant has no polygon.
    NoPolygon,
    /// More than [`MAX_POLYGONS`] polygons.
    TooManyPolygons,
    /// A ring has fewer than 3 distinct points.
    TooFewPoints,
    /// A ring has more than [`MAX_RING_POINTS`] points.
    TooManyPoints,
    /// A coordinate is not a finite number.
    NotFinite,
    /// A coordinate lies beyond [`MAX_COORD_MM`].
    OutOfReach,
    /// The points of a ring lie on one line.
    NoArea,
    /// A ring crosses or touches itself.
    SelfIntersecting,
    /// A hole is not wholly inside the outline of its polygon.
    HoleOutsideOutline,
    /// The outline overlaps, touches or contains the outline of the polygon `other`.
    PolygonsOverlap { other: usize },
    /// The checks would take more than [`MAX_CHECK_STEPS`].
    TooComplex,
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Problem::NoPolygon => write!(f, "a mask needs at least one polygon"),
            Problem::TooManyPolygons => {
                write!(f, "a mask has at most {MAX_POLYGONS} polygons")
            }
            Problem::TooFewPoints => write!(f, "a ring needs at least 3 different points"),
            Problem::TooManyPoints => write!(f, "a ring has at most {MAX_RING_POINTS} points"),
            Problem::NotFinite => write!(f, "must be a finite number"),
            Problem::OutOfReach => write!(
                f,
                "must lie between -2 000 000 and 2 000 000 mm, the reach of an OOM map"
            ),
            Problem::NoArea => write!(f, "the points of the ring lie on one line"),
            Problem::SelfIntersecting => write!(f, "the ring crosses or touches itself here"),
            Problem::HoleOutsideOutline => write!(
                f,
                "the hole must lie wholly inside the outline of its polygon (the first ring)"
            ),
            Problem::PolygonsOverlap { other } => write!(
                f,
                "the outline overlaps, touches or contains the outline of polygon {other}; \
                 the polygons of one mask must lie apart"
            ),
            Problem::TooComplex => write!(
                f,
                "the mask is too complex to check (more than {MAX_CHECK_STEPS} steps); \
                 use fewer points or polygons"
            ),
        }
    }
}

/// A problem and where it is: polygon, ring (0 is the outline, 1 the first hole) and point index, as far as
/// known. `point` of a ring that crosses itself is the first point of one of the crossing edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaskError {
    pub problem: Problem,
    pub polygon: Option<usize>,
    pub ring: Option<usize>,
    pub point: Option<usize>,
    /// For a non-finite or out-of-reach value: 0 for x, 1 for y.
    pub axis: Option<usize>,
}

/// The steps the checks may still take ([`MAX_CHECK_STEPS`] for a whole project file).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget(u64);

impl Budget {
    pub fn new(steps: u64) -> Self {
        Budget(steps)
    }

    fn take(&mut self, steps: u64) -> Result<(), MaskError> {
        if steps > self.0 {
            self.0 = 0;
            return Err(error(Problem::TooComplex, None, None, None));
        }
        self.0 -= steps;
        Ok(())
    }
}

fn error(
    problem: Problem,
    polygon: Option<usize>,
    ring: Option<usize>,
    point: Option<usize>,
) -> MaskError {
    MaskError {
        problem,
        polygon,
        ring,
        point,
        axis: None,
    }
}

/// Checks a mask variant (ADR 0045): at least one and at most [`MAX_POLYGONS`] polygons; rings of at least 3
/// different and at most [`MAX_RING_POINTS`] points with finite coordinates within [`MAX_COORD_MM`]; no ring
/// on one line, none that crosses or touches itself; every hole inside its outline; the outlines of different
/// polygons apart. Holes may overlap each other. The mask id is the caller's business.
pub fn check(mask: &Mask, budget: &mut Budget) -> Result<(), MaskError> {
    if mask.polygons.is_empty() {
        return Err(error(Problem::NoPolygon, None, None, None));
    }
    if mask.polygons.len() > MAX_POLYGONS {
        return Err(error(
            Problem::TooManyPolygons,
            Some(MAX_POLYGONS),
            None,
            None,
        ));
    }
    let mut rings = Vec::new();
    for (p, polygon) in mask.polygons.iter().enumerate() {
        for (r, points) in polygon.rings().enumerate() {
            rings.push(checked_ring(points, p, r)?);
        }
    }
    contacts(&rings, budget)?;
    containment(&rings, budget)
}

/// A ring in the internal frame without repeated points, with where it stands in the mask.
struct Ring {
    pts: Vec<Pt>,
    /// The index in the written ring of each point of `pts`.
    src: Vec<usize>,
    polygon: usize,
    /// 0 for the outline, 1 for the first hole.
    ring: usize,
    /// min x, min y, max x, max y.
    bbox: [f64; 4],
}

/// `points` (y up) in the internal frame, without consecutive repeats and without a closing point equal to the
/// first, with the written index of each point.
fn cleaned(points: &[[f64; 2]]) -> (Vec<Pt>, Vec<usize>) {
    let mut pts: Vec<Pt> = Vec::with_capacity(points.len());
    let mut src = Vec::with_capacity(points.len());
    for (i, &[x, y]) in points.iter().enumerate() {
        let p = (x, -y);
        if pts.last() != Some(&p) {
            pts.push(p);
            src.push(i);
        }
    }
    while pts.len() > 1 && pts.first() == pts.last() {
        pts.pop();
        src.pop();
    }
    (pts, src)
}

/// The checks of one ring on its own, apart from crossings: size, values, distinct points, not on one line.
fn checked_ring(points: &[[f64; 2]], p: usize, r: usize) -> Result<Ring, MaskError> {
    if points.len() > MAX_RING_POINTS {
        return Err(error(Problem::TooManyPoints, Some(p), Some(r), None));
    }
    for (k, pt) in points.iter().enumerate() {
        for (axis, v) in pt.iter().enumerate() {
            let problem = if !v.is_finite() {
                Problem::NotFinite
            } else if v.abs() > MAX_COORD_MM {
                Problem::OutOfReach
            } else {
                continue;
            };
            return Err(MaskError {
                axis: Some(axis),
                ..error(problem, Some(p), Some(r), Some(k))
            });
        }
    }
    let (pts, src) = cleaned(points);
    let mut distinct = pts.clone();
    distinct.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    distinct.dedup();
    if distinct.len() < 3 {
        return Err(error(Problem::TooFewPoints, Some(p), Some(r), None));
    }
    if pts.iter().all(|&c| orient(pts[0], pts[1], c) == 0.0) {
        return Err(error(Problem::NoArea, Some(p), Some(r), None));
    }
    let mut bbox = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for &(x, y) in &pts {
        bbox = [
            bbox[0].min(x),
            bbox[1].min(y),
            bbox[2].max(x),
            bbox[3].max(y),
        ];
    }
    Ok(Ring {
        pts,
        src,
        polygon: p,
        ring: r,
        bbox,
    })
}

/// Twice the signed area of the triangle `a`, `b`, `c`: positive when it turns counter-clockwise with the numbers
/// read in a y-up frame, 0 on one line.
fn orient(a: Pt, b: Pt, c: Pt) -> f64 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

/// Whether `c`, on the line through `a` and `b`, lies on the segment.
fn on_segment(a: Pt, b: Pt, c: Pt) -> bool {
    c.0 >= a.0.min(b.0) && c.0 <= a.0.max(b.0) && c.1 >= a.1.min(b.1) && c.1 <= a.1.max(b.1)
}

/// Whether the closed segments `p1 p2` and `q1 q2` have a point in common.
fn segments_meet(p1: Pt, p2: Pt, q1: Pt, q2: Pt) -> bool {
    let d1 = orient(q1, q2, p1);
    let d2 = orient(q1, q2, p2);
    let d3 = orient(p1, p2, q1);
    let d4 = orient(p1, p2, q2);
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return true;
    }
    (d1 == 0.0 && on_segment(q1, q2, p1))
        || (d2 == 0.0 && on_segment(q1, q2, p2))
        || (d3 == 0.0 && on_segment(p1, p2, q1))
        || (d4 == 0.0 && on_segment(p1, p2, q2))
}

/// An edge of a ring: from point `idx` to the next.
struct Edge {
    ring: usize,
    idx: usize,
    a: Pt,
    b: Pt,
}

impl Edge {
    fn x_range(&self) -> (f64, f64) {
        (self.a.0.min(self.b.0), self.a.0.max(self.b.0))
    }

    fn y_range(&self) -> (f64, f64) {
        (self.a.1.min(self.b.1), self.a.1.max(self.b.1))
    }
}

/// Edges that meet where they must not: a ring with itself (beyond the shared point of neighbouring edges), a
/// hole with its outline, outlines of different polygons. Holes may meet each other. A sweep in x finds the
/// candidate pairs. Of several problems the first by kind (self-crossing, hole, polygons), polygon, ring and point
/// is reported.
fn contacts(rings: &[Ring], budget: &mut Budget) -> Result<(), MaskError> {
    let mut edges = Vec::new();
    for (ri, ring) in rings.iter().enumerate() {
        let n = ring.pts.len();
        for idx in 0..n {
            edges.push(Edge {
                ring: ri,
                idx,
                a: ring.pts[idx],
                b: ring.pts[(idx + 1) % n],
            });
        }
    }
    edges.sort_by(|e, f| e.x_range().0.partial_cmp(&f.x_range().0).expect("finite"));
    let mut best: Option<(u8, MaskError)> = None;
    let mut active: Vec<usize> = Vec::new();
    for (ei, e) in edges.iter().enumerate() {
        let (x0, _) = e.x_range();
        budget.take(active.len() as u64 + 1)?;
        active.retain(|&f| edges[f].x_range().1 >= x0);
        let (ey0, ey1) = e.y_range();
        for &fi in &active {
            let f = &edges[fi];
            let (fy0, fy1) = f.y_range();
            if fy1 < ey0 || fy0 > ey1 {
                continue;
            }
            if let Some(found) = contact(rings, e, f) {
                let key = |(rank, err): &(u8, MaskError)| (*rank, err.polygon, err.ring, err.point);
                if best.as_ref().is_none_or(|b| key(&found) < key(b)) {
                    best = Some(found);
                }
            }
        }
        active.push(ei);
    }
    match best {
        Some((_, err)) => Err(err),
        None => Ok(()),
    }
}

/// The problem when the edges `e` and `f` meet where they must not, with its rank for [`contacts`].
fn contact(rings: &[Ring], e: &Edge, f: &Edge) -> Option<(u8, MaskError)> {
    let (re, rf) = (&rings[e.ring], &rings[f.ring]);
    if e.ring == f.ring {
        let n = re.pts.len();
        let (first, second) = if (e.idx + 1) % n == f.idx {
            (e, f)
        } else if (f.idx + 1) % n == e.idx {
            (f, e)
        } else {
            if !segments_meet(e.a, e.b, f.a, f.b) {
                return None;
            }
            let point = re.src[e.idx.max(f.idx)];
            return Some((
                0,
                error(
                    Problem::SelfIntersecting,
                    Some(re.polygon),
                    Some(re.ring),
                    Some(point),
                ),
            ));
        };
        // Neighbours share a point; they must not run back over each other from it.
        let shared = first.b;
        let u = (first.a.0 - shared.0, first.a.1 - shared.1);
        let v = (second.b.0 - shared.0, second.b.1 - shared.1);
        let back = u.0 * v.1 - u.1 * v.0 == 0.0 && u.0 * v.0 + u.1 * v.1 > 0.0;
        return back.then(|| {
            (
                0,
                error(
                    Problem::SelfIntersecting,
                    Some(re.polygon),
                    Some(re.ring),
                    Some(re.src[second.idx]),
                ),
            )
        });
    }
    let found = if re.polygon == rf.polygon {
        // A hole and its outline; two holes may meet.
        let hole = match (re.ring, rf.ring) {
            (0, _) => rf,
            (_, 0) => re,
            _ => return None,
        };
        (
            1,
            error(
                Problem::HoleOutsideOutline,
                Some(hole.polygon),
                Some(hole.ring),
                None,
            ),
        )
    } else if re.ring == 0 && rf.ring == 0 {
        let (low, high) = if re.polygon < rf.polygon {
            (re, rf)
        } else {
            (rf, re)
        };
        (
            2,
            error(
                Problem::PolygonsOverlap { other: low.polygon },
                Some(high.polygon),
                None,
                None,
            ),
        )
    } else {
        // A hole and another polygon: covered by the hole inside its outline and the outlines apart.
        return None;
    };
    segments_meet(e.a, e.b, f.a, f.b).then_some(found)
}

/// With no edges meeting: every hole inside its outline, and no outline inside another.
fn containment(rings: &[Ring], budget: &mut Budget) -> Result<(), MaskError> {
    let outlines: Vec<&Ring> = rings.iter().filter(|r| r.ring == 0).collect();
    for hole in rings.iter().filter(|r| r.ring > 0) {
        let outline = outlines[hole.polygon];
        budget.take(outline.pts.len() as u64)?;
        if !inside(&outline.pts, hole.pts[0]) {
            return Err(error(
                Problem::HoleOutsideOutline,
                Some(hole.polygon),
                Some(hole.ring),
                None,
            ));
        }
    }
    let within = |inner: &[f64; 4], outer: &[f64; 4]| {
        inner[0] >= outer[0] && inner[1] >= outer[1] && inner[2] <= outer[2] && inner[3] <= outer[3]
    };
    for (i, a) in outlines.iter().enumerate() {
        for b in &outlines[i + 1..] {
            budget.take(1)?;
            let a_in_b = within(&a.bbox, &b.bbox) && {
                budget.take(b.pts.len() as u64)?;
                inside(&b.pts, a.pts[0])
            };
            let b_in_a = !a_in_b && within(&b.bbox, &a.bbox) && {
                budget.take(a.pts.len() as u64)?;
                inside(&a.pts, b.pts[0])
            };
            if a_in_b || b_in_a {
                return Err(error(
                    Problem::PolygonsOverlap { other: a.polygon },
                    Some(b.polygon),
                    None,
                    None,
                ));
            }
        }
    }
    Ok(())
}

/// Whether `pt` lies inside the ring `pts` (crossing number). Points on the ring are undefined.
fn inside(pts: &[Pt], pt: Pt) -> bool {
    inside_by(pts.len(), |i| pts[i], pt)
}

/// [`inside`] for a ring as written (y up), without cleaning it: repeated points add edges of length 0, which
/// no ray crosses.
fn inside_written(points: &[[f64; 2]], pt: Pt) -> bool {
    inside_by(points.len(), |i| (points[i][0], -points[i][1]), pt)
}

fn inside_by(n: usize, at: impl Fn(usize) -> Pt, (px, py): Pt) -> bool {
    let mut odd = false;
    let mut j = n.wrapping_sub(1);
    for i in 0..n {
        let (xi, yi) = at(i);
        let (xj, yj) = at(j);
        if (yi > py) != (yj > py) && px < (xj - xi) * (py - yi) / (yj - yi) + xi {
            odd = !odd;
        }
        j = i;
    }
    odd
}

/// The rings of `mask` in the internal frame (y down), polygon by polygon, the outline first and then its holes.
/// Repeated points and a closing point equal to the first are dropped. The orientation is normalised by the sign
/// of [`signed_area`]: outlines negative (clockwise, read with the numbers in a y-up frame), holes positive.
pub fn masked_rings(mask: &Mask) -> Vec<Vec<Pt>> {
    let mut out = Vec::new();
    for polygon in &mask.polygons {
        for (r, ring) in polygon.rings().enumerate() {
            let mut pts = cleaned(ring).0;
            let area = signed_area(&pts);
            if (r == 0 && area > 0.0) || (r > 0 && area < 0.0) {
                pts.reverse();
            }
            out.push(pts);
        }
    }
    out
}

/// Whether `pt` (internal frame) lies in the masked area of `mask`: inside an outline and in none of its holes.
/// Points on a ring are undefined.
pub fn contains(mask: &Mask, pt: Pt) -> bool {
    mask.polygons
        .iter()
        .any(|p| inside_written(&p.outline, pt) && !p.holes.iter().any(|h| inside_written(h, pt)))
}

/// Twice the signed area of a ring (shoelace sum); positive is counter-clockwise when the numbers are read in a
/// y-up frame.
pub fn signed_area(ring: &[Pt]) -> f64 {
    let n = ring.len();
    (0..n)
        .map(|i| {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<[f64; 2]> {
        // Counter-clockwise on the map (y up).
        vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
    }

    fn reversed(mut ring: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
        ring.reverse();
        ring
    }

    /// A mask with one 0..100 square outline and the given holes.
    fn with_holes(holes: Vec<Vec<[f64; 2]>>) -> Mask {
        Mask {
            id: "m".into(),
            polygons: vec![MaskPolygon {
                outline: square(0.0, 0.0, 100.0, 100.0),
                holes,
            }],
        }
    }

    /// Map mm as displayed (y up) to the internal frame.
    fn internal(x: f64, y: f64) -> Pt {
        (x, -y)
    }

    #[test]
    fn ring_orientation_does_not_matter() {
        let ccw = with_holes(vec![square(10.0, 10.0, 30.0, 30.0)]);
        let cw = Mask {
            id: "m".into(),
            polygons: vec![MaskPolygon {
                outline: reversed(square(0.0, 0.0, 100.0, 100.0)),
                holes: vec![reversed(square(10.0, 10.0, 30.0, 30.0))],
            }],
        };
        let a = masked_rings(&ccw);
        let b = masked_rings(&cw);
        assert_eq!(a.len(), 2);
        // Outline first, clockwise (negative); the hole counter-clockwise (positive).
        assert!(
            signed_area(&a[0]) < 0.0 && signed_area(&a[1]) > 0.0,
            "{a:?}"
        );
        for (ra, rb) in a.iter().zip(&b) {
            assert_eq!(signed_area(ra), signed_area(rb));
            let mut sa = ra.clone();
            let mut sb = rb.clone();
            sa.sort_by(|p, q| p.partial_cmp(q).unwrap());
            sb.sort_by(|p, q| p.partial_cmp(q).unwrap());
            assert_eq!(sa, sb);
        }
        // In the internal frame, y down.
        assert!(a[0].contains(&internal(100.0, 100.0)), "{a:?}");
        for pt in [(5.0, 5.0), (20.0, 20.0), (50.0, 50.0), (150.0, 5.0)] {
            let pt = internal(pt.0, pt.1);
            assert_eq!(contains(&ccw, pt), contains(&cw, pt), "{pt:?}");
        }
        assert!(contains(&cw, internal(50.0, 50.0)));
        // A closing point equal to the first is dropped.
        let mut closed = square(0.0, 0.0, 100.0, 100.0);
        closed.push(closed[0]);
        let closed = Mask {
            id: "m".into(),
            polygons: vec![MaskPolygon {
                outline: closed,
                holes: vec![],
            }],
        };
        assert_eq!(masked_rings(&closed)[0].len(), 4);
    }

    #[test]
    fn contains_respects_holes() {
        let m = with_holes(vec![
            square(10.0, 10.0, 30.0, 30.0),
            square(60.0, 60.0, 80.0, 80.0),
        ]);
        assert!(!contains(&m, internal(20.0, 20.0)), "in a hole");
        assert!(!contains(&m, internal(70.0, 70.0)), "in the other hole");
        assert!(contains(&m, internal(50.0, 50.0)), "outline, no hole");
        assert!(contains(&m, internal(5.0, 95.0)), "outline, no hole");
        assert!(!contains(&m, internal(150.0, 50.0)), "outside");
        // y is turned: (20, -20) internal is (20, 20) on the map, in the hole; (20, 20) internal is outside.
        assert!(!contains(&m, (20.0, 20.0)));
    }

    #[test]
    fn overlapping_holes_unite_in_contains() {
        let m = with_holes(vec![
            square(10.0, 10.0, 50.0, 50.0),
            square(30.0, 30.0, 70.0, 70.0),
        ]);
        let mut budget = Budget::new(MAX_CHECK_STEPS);
        assert_eq!(check(&m, &mut budget), Ok(()));
        assert!(!contains(&m, internal(40.0, 40.0)), "in both holes");
        assert!(!contains(&m, internal(20.0, 20.0)), "in the first");
        assert!(!contains(&m, internal(60.0, 60.0)), "in the second");
        assert!(contains(&m, internal(20.0, 60.0)), "in neither");
        // A hole inside another hole is part of the window too.
        let nested = with_holes(vec![
            square(10.0, 10.0, 50.0, 50.0),
            square(20.0, 20.0, 30.0, 30.0),
        ]);
        assert_eq!(check(&nested, &mut budget), Ok(()));
        assert!(!contains(&nested, internal(25.0, 25.0)));
    }
}
