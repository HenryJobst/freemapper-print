//! Control number placement: the box a number takes next to its circle, how a position scores against
//! obstacles, and the search for the best position (ADR 0038). freemapper-courseplan places numbers by the
//! same rules (its ADR 0030 and 0031) and takes this module over once it moves into a crate of its own
//! (spec §5.7). It therefore uses nothing else of freemapper-print; callers turn their circles, legs, sheet
//! elements and map detail into [`Obstacle`]s.
//!
//! Conventions: coordinates are millimetres with y down (page or map), so "north" is -y. A direction is in
//! degrees, 0 = east, counter-clockwise as seen on paper, so 90 points to -y.
//!
//! The rules, decided by the user on 2026-11-09 and shared with freemapper-courseplan:
//! 1. The number covers no point or line symbol of the map; area symbols may be covered and cost little.
//! 2. The preferred direction is "behind": the continuation of the incoming leg past the control.
//! 3. If the box at "behind" comes within half a line width of the outgoing leg, the preferred direction is the
//!    outer bisector, the middle of the larger sector between the two legs.
//! 4. If the preferred place is blocked, the free place that deviates least from it wins.
//!
//! [`best`] ranks the candidates lexicographically (courseplan ADR 0031): overprint conflicts (own circles,
//! legs, other numbers, sheet elements, leaving the bounds), then covered map symbols, then the angle to the
//! preferred direction, the angle to the outer bisector, the distance to the current gap, the soft cost of
//! covered areas, the direction and the gap.

/// Gaps (mm, before the caller's unit) that [`best`] tries.
pub const GAPS_MM: [f64; 3] = [0.3, 0.8, 1.5];
/// Step (degrees) between the candidate directions.
pub const CANDIDATE_STEP_DEG: f64 = 15.0;
/// Number of candidate directions: steps of 15 degrees once around.
pub const CANDIDATES: usize = 24;
/// The label [`score`] gives a box that leaves its bounds.
pub const OUTSIDE: &str = "outside";

const EPS: f64 = 1e-9;
/// Corners of the polygon that stands in for a circle when an overlap area is measured.
const CIRCLE_CORNERS: usize = 32;

type Pt = (f64, f64);

/// An axis-parallel rectangle (mm, y down).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub min: Pt,
    pub max: Pt,
}

impl Rect {
    /// The rectangle of `size` (width, height) centred on `centre`.
    pub fn centred(centre: Pt, size: (f64, f64)) -> Rect {
        Rect {
            min: (centre.0 - size.0 / 2.0, centre.1 - size.1 / 2.0),
            max: (centre.0 + size.0 / 2.0, centre.1 + size.1 / 2.0),
        }
    }

    pub fn centre(&self) -> Pt {
        (
            (self.min.0 + self.max.0) / 2.0,
            (self.min.1 + self.max.1) / 2.0,
        )
    }

    pub fn size(&self) -> (f64, f64) {
        (self.max.0 - self.min.0, self.max.1 - self.min.1)
    }

    /// The rectangle grown by `d` on every side.
    pub fn grown(&self, d: f64) -> Rect {
        Rect {
            min: (self.min.0 - d, self.min.1 - d),
            max: (self.max.0 + d, self.max.1 + d),
        }
    }

    /// Whether the insides of the two rectangles overlap; touching edges do not count.
    pub fn overlaps(&self, other: &Rect) -> bool {
        self.min.0 < other.max.0 - EPS
            && other.min.0 < self.max.0 - EPS
            && self.min.1 < other.max.1 - EPS
            && other.min.1 < self.max.1 - EPS
    }

    /// Whether the rectangles share at least one point.
    fn touches(&self, other: &Rect) -> bool {
        self.min.0 <= other.max.0
            && other.min.0 <= self.max.0
            && self.min.1 <= other.max.1
            && other.min.1 <= self.max.1
    }

    /// Whether `inner` lies wholly inside (edges may coincide).
    pub fn contains_rect(&self, inner: &Rect) -> bool {
        inner.min.0 >= self.min.0 - EPS
            && inner.min.1 >= self.min.1 - EPS
            && inner.max.0 <= self.max.0 + EPS
            && inner.max.1 <= self.max.1 + EPS
    }

    fn contains(&self, p: Pt) -> bool {
        self.min.0 <= p.0 && p.0 <= self.max.0 && self.min.1 <= p.1 && p.1 <= self.max.1
    }

    fn corners(&self) -> [Pt; 4] {
        [
            self.min,
            (self.max.0, self.min.1),
            self.max,
            (self.min.0, self.max.1),
        ]
    }

    /// Shortest distance between the rectangle and `p`, 0 inside.
    pub fn distance_to_point(&self, p: Pt) -> f64 {
        let dx = (self.min.0 - p.0).max(0.0).max(p.0 - self.max.0);
        let dy = (self.min.1 - p.1).max(0.0).max(p.1 - self.max.1);
        dx.hypot(dy)
    }

    /// Shortest distance between the rectangle and the segment `a`–`b`, 0 when they meet.
    pub fn distance_to_segment(&self, a: Pt, b: Pt) -> f64 {
        if self.contains(a) || self.contains(b) {
            return 0.0;
        }
        let c = self.corners();
        if (0..4).any(|i| segments_cross(a, b, c[i], c[(i + 1) % 4])) {
            return 0.0;
        }
        let ends = self.distance_to_point(a).min(self.distance_to_point(b));
        c.iter()
            .map(|&corner| point_segment_distance(corner, a, b))
            .fold(ends, f64::min)
    }
}

fn orient(a: Pt, b: Pt, c: Pt) -> f64 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

/// Whether the segments cross properly; touching is left to the distance tests.
fn segments_cross(a: Pt, b: Pt, c: Pt, d: Pt) -> bool {
    let (d1, d2) = (orient(c, d, a), orient(c, d, b));
    let (d3, d4) = (orient(a, b, c), orient(a, b, d));
    ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
}

fn point_segment_distance(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 < EPS * EPS {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    };
    (p.0 - (a.0 + t * dx)).hypot(p.1 - (a.1 + t * dy))
}

/// Whether `p` lies inside the closed `ring` (even-odd).
fn inside_ring(p: Pt, ring: &[Pt]) -> bool {
    let mut inside = false;
    let n = ring.len();
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) / (b.1 - a.1) * (b.0 - a.0) {
            inside = !inside;
        }
    }
    inside
}

/// The part of `ring` inside `r` (Sutherland–Hodgman against the four sides).
fn clip_to_rect(ring: &[Pt], r: &Rect) -> Vec<Pt> {
    let mut out: Vec<Pt> = ring.to_vec();
    // Each side: a test "inside" and the crossing point of a segment with the side.
    let sides: [(usize, f64, bool); 4] = [
        (0, r.min.0, true),
        (0, r.max.0, false),
        (1, r.min.1, true),
        (1, r.max.1, false),
    ];
    for (axis, at, lower) in sides {
        let input = std::mem::take(&mut out);
        let n = input.len();
        let coord = |p: Pt| if axis == 0 { p.0 } else { p.1 };
        let inside = |p: Pt| {
            if lower {
                coord(p) >= at
            } else {
                coord(p) <= at
            }
        };
        let cross = |a: Pt, b: Pt| {
            let t = (at - coord(a)) / (coord(b) - coord(a));
            (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1))
        };
        for i in 0..n {
            let (a, b) = (input[i], input[(i + 1) % n]);
            match (inside(a), inside(b)) {
                (true, true) => out.push(b),
                (true, false) => out.push(cross(a, b)),
                (false, true) => {
                    out.push(cross(a, b));
                    out.push(b);
                }
                (false, false) => {}
            }
        }
    }
    out
}

/// Area of a ring (shoelace), always positive.
fn ring_area(ring: &[Pt]) -> f64 {
    let n = ring.len();
    (0..n)
        .map(|i| {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum::<f64>()
        .abs()
        / 2.0
}

/// What an obstacle covers.
#[derive(Debug, Clone, PartialEq)]
pub enum ObstacleShape {
    Rect(Rect),
    Circle {
        centre: Pt,
        radius: f64,
    },
    /// A line from `a` to `b`, `half_width` to each side of it.
    Segment {
        a: Pt,
        b: Pt,
        half_width: f64,
    },
    /// A closed ring.
    Polygon(Vec<Pt>),
}

impl ObstacleShape {
    /// The box around the shape.
    pub fn bbox(&self) -> Rect {
        match self {
            ObstacleShape::Rect(r) => *r,
            ObstacleShape::Circle { centre, radius } => {
                Rect::centred(*centre, (2.0 * radius, 2.0 * radius))
            }
            ObstacleShape::Segment { a, b, half_width } => Rect {
                min: (a.0.min(b.0), a.1.min(b.1)),
                max: (a.0.max(b.0), a.1.max(b.1)),
            }
            .grown(*half_width),
            ObstacleShape::Polygon(ring) => {
                let first = ring.first().copied().unwrap_or((0.0, 0.0));
                ring.iter().fold(
                    Rect {
                        min: first,
                        max: first,
                    },
                    |r, p| Rect {
                        min: (r.min.0.min(p.0), r.min.1.min(p.1)),
                        max: (r.max.0.max(p.0), r.max.1.max(p.1)),
                    },
                )
            }
        }
    }

    /// Whether the shape meets the inside of `r`. A segment counts when `r` comes within its half width,
    /// touching included, as courseplan tests legs; a circle when `r` reaches inside it.
    pub fn hits(&self, r: &Rect) -> bool {
        match self {
            ObstacleShape::Rect(o) => o.overlaps(r),
            ObstacleShape::Circle { centre, radius } => r.distance_to_point(*centre) < *radius,
            ObstacleShape::Segment { a, b, half_width } => {
                r.distance_to_segment(*a, *b) <= *half_width
            }
            ObstacleShape::Polygon(ring) => {
                let n = ring.len();
                n > 0
                    && ((0..n).any(|i| r.distance_to_segment(ring[i], ring[(i + 1) % n]) <= EPS)
                        || inside_ring(r.centre(), ring))
            }
        }
    }

    /// The shape as one ring, for measuring overlap areas.
    fn ring(&self) -> Vec<Pt> {
        match self {
            ObstacleShape::Rect(r) => r.corners().to_vec(),
            ObstacleShape::Circle { centre, radius } => (0..CIRCLE_CORNERS)
                .map(|i| {
                    let a = i as f64 / CIRCLE_CORNERS as f64 * std::f64::consts::TAU;
                    (centre.0 + radius * a.cos(), centre.1 + radius * a.sin())
                })
                .collect(),
            ObstacleShape::Segment { a, b, half_width } => {
                let len = (b.0 - a.0).hypot(b.1 - a.1);
                if len < EPS {
                    return Rect::centred(*a, (2.0 * half_width, 2.0 * half_width))
                        .corners()
                        .to_vec();
                }
                let (nx, ny) = (
                    -(b.1 - a.1) / len * half_width,
                    (b.0 - a.0) / len * half_width,
                );
                vec![
                    (a.0 + nx, a.1 + ny),
                    (b.0 + nx, b.1 + ny),
                    (b.0 - nx, b.1 - ny),
                    (a.0 - nx, a.1 - ny),
                ]
            }
            ObstacleShape::Polygon(ring) => ring.clone(),
        }
    }

    /// The area (mm²) the shape shares with `r`.
    pub fn overlap_area(&self, r: &Rect) -> f64 {
        if let ObstacleShape::Rect(o) = self {
            let w = (o.max.0.min(r.max.0) - o.min.0.max(r.min.0)).max(0.0);
            let h = (o.max.1.min(r.max.1) - o.min.1.max(r.min.1)).max(0.0);
            return w * h;
        }
        ring_area(&clip_to_rect(&self.ring(), r))
    }
}

/// How an obstacle counts in the ranking (courseplan ADR 0031).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// The course's own circles, legs and other numbers, and the sheet elements: a hard conflict, ranked first.
    Overprint,
    /// A point or line symbol of the map: also a conflict (rule 1), ranked after the overprint.
    MapSymbol,
    /// Map detail that may be covered (area symbols): its weight × the covered mm² adds to the soft cost.
    Soft,
}

/// Something a number box should not cover. Obstacles with the same label count once.
#[derive(Debug, Clone, PartialEq)]
pub struct Obstacle {
    pub shape: ObstacleShape,
    /// Cost per covered mm²; only [`Tier::Soft`] uses it.
    pub weight: f64,
    pub tier: Tier,
    pub label: String,
}

/// Where a number sits: a direction from the circle centre and the gap between the outer edge of the circle
/// line and the number box (agreed with freemapper-courseplan, their ADR 0030).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// Degrees in [0, 360), 0 = east, counter-clockwise.
    pub direction: f64,
    pub gap_mm: f64,
}

/// How a number box fares: the labels of what it hits and the soft cost.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Score {
    /// Overprint obstacles hit, each label once in obstacle order, and [`OUTSIDE`] when the box leaves the
    /// bounds.
    pub hard: Vec<String>,
    /// Map point and line symbols covered, each label once.
    pub covered: Vec<String>,
    /// Sum over soft obstacles of weight × covered mm².
    pub cost: f64,
}

impl Score {
    /// No conflict of either tier.
    pub fn is_free(&self) -> bool {
        self.hard.is_empty() && self.covered.is_empty()
    }
}

/// A direction brought into [0, 360); a value that rounds up to 360 and -0 give 0.
pub fn normalize_direction(direction: f64) -> f64 {
    let d = direction.rem_euclid(360.0);
    if d >= 360.0 || d == 0.0 { 0.0 } else { d }
}

/// The smaller angle (degrees) between two directions.
pub fn angle_diff(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

/// Direction (degrees, [0, 360)) from `from` to `to`; `None` when they coincide.
pub fn direction_to(from: Pt, to: Pt) -> Option<f64> {
    let (dx, dy) = (to.0 - from.0, from.1 - to.1);
    (dx.hypot(dy) > EPS).then(|| normalize_direction(dy.atan2(dx).to_degrees()))
}

/// Distance along a ray with direction cosines `c`, `s` (both ≥ 0) at which a box with half sizes `a`, `b`
/// centred on the ray is `reach` away from the origin (ruling R4, courseplan's `box_distance`).
fn box_distance(c: f64, s: f64, a: f64, b: f64, reach: f64) -> f64 {
    // The nearest point of the box is on its near vertical edge, its near horizontal edge or the corner
    // between them.
    if s < EPS || (c > EPS && (reach + a) / c * s <= b) {
        return (reach + a) / c;
    }
    if c < EPS || (reach + b) / s * c <= a {
        return (reach + b) / s;
    }
    let p = a * c + b * s;
    p + (p * p - a * a - b * b + reach * reach).max(0.0).sqrt()
}

/// The box of a number of `size` (width, height) placed at `p` around a circle at `centre` whose line ends at
/// `circle_outer_r` (radius plus half the line width). The box is centred on the ray in `p.direction`, at
/// the distance where its shortest distance to the circle's outer edge is `p.gap_mm` (ruling R4).
pub fn number_box(centre: Pt, circle_outer_r: f64, size: (f64, f64), p: Placement) -> Rect {
    let rad = p.direction.to_radians();
    let (cos, sin) = (rad.cos(), rad.sin());
    let reach = (circle_outer_r + p.gap_mm).max(0.0);
    let d = box_distance(cos.abs(), sin.abs(), size.0 / 2.0, size.1 / 2.0, reach);
    // y is down: a positive sine points to -y.
    Rect::centred((centre.0 + cos * d, centre.1 - sin * d), size)
}

/// The placement of a number box centred on `box_centre` (the inverse of [`number_box`]): the direction to the
/// box centre and its shortest distance to the circle's outer edge. Direction 0 when the centres coincide.
pub fn placement_of(
    centre: Pt,
    circle_outer_r: f64,
    box_centre: Pt,
    size: (f64, f64),
) -> Placement {
    let direction = direction_to(centre, box_centre).unwrap_or(0.0);
    let gap_mm = Rect::centred(box_centre, size).distance_to_point(centre) - circle_outer_r;
    Placement { direction, gap_mm }
}

/// How the box `box_` fares within `bounds` among `obstacles`: overprint obstacles hit and leaving the bounds
/// are hard conflicts, map symbols hit are covered, and each soft obstacle adds its weight × the covered mm².
pub fn score(box_: Rect, bounds: Rect, obstacles: &[Obstacle]) -> Score {
    let mut s = Score::default();
    let near = box_.grown(EPS);
    for o in obstacles {
        if !o.shape.bbox().touches(&near) {
            continue;
        }
        match o.tier {
            Tier::Overprint | Tier::MapSymbol => {
                let list = if o.tier == Tier::Overprint {
                    &mut s.hard
                } else {
                    &mut s.covered
                };
                if !list.contains(&o.label) && o.shape.hits(&box_) {
                    list.push(o.label.clone());
                }
            }
            Tier::Soft => s.cost += o.weight * o.shape.overlap_area(&box_),
        }
    }
    if !bounds.contains_rect(&box_) {
        s.hard.push(OUTSIDE.to_string());
    }
    s
}

/// The legs at a control, for the preferred direction.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Legs {
    /// Direction (degrees) from the control to the point before it on the incoming leg (the last bend, or the
    /// previous control; for the first control the start).
    pub incoming: Option<f64>,
    /// The outgoing leg from the control centre through its bends to the next control; empty without one.
    pub outgoing: Vec<Pt>,
    /// Half the width of a leg line.
    pub half_line: f64,
}

/// A number to place.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub centre: Pt,
    /// Radius of the circle plus half its line width.
    pub circle_outer_r: f64,
    /// Width and height of the number box.
    pub size: (f64, f64),
    /// Coordinate millimetres per gap millimetre: a gap of `g` lies `g × unit` from the circle. The symbol
    /// factor when placements hold print mm at the base scale and coordinates are page mm.
    pub unit: f64,
    pub legs: Legs,
}

impl Target {
    /// The box at `p`, its gap times [`Target::unit`].
    pub fn box_at(&self, p: Placement) -> Rect {
        number_box(
            self.centre,
            self.circle_outer_r,
            self.size,
            Placement {
                direction: p.direction,
                gap_mm: p.gap_mm * self.unit,
            },
        )
    }

    /// The box reaches at most this far from the centre at any candidate gap.
    fn reach(&self) -> f64 {
        let largest = GAPS_MM.iter().copied().fold(0.0, f64::max);
        self.circle_outer_r + largest * self.unit + self.size.0.hypot(self.size.1)
    }

    /// The outgoing leg without the part inside the circle.
    fn outgoing_outside(&self) -> Vec<Pt> {
        trim_start(&self.legs.outgoing, self.circle_outer_r)
    }

    /// The preferred direction and the outer bisector for a number at `gap`; `current` gives both when the
    /// control has no legs (score courses).
    pub fn directions(&self, gap: f64, current: Placement) -> (f64, f64) {
        let incoming = self.legs.incoming;
        let outgoing = match self.legs.outgoing.as_slice() {
            [a, b, ..] => direction_to(*a, *b),
            _ => None,
        };
        let Some(outer) = outer_bisector(incoming, outgoing) else {
            return (current.direction, current.direction);
        };
        let blocked = incoming.is_some_and(|a| {
            let behind = self.box_at(Placement {
                direction: normalize_direction(a + 180.0),
                gap_mm: gap,
            });
            self.outgoing_outside()
                .windows(2)
                .any(|w| behind.distance_to_segment(w[0], w[1]) <= self.legs.half_line)
        });
        let preferred = match incoming {
            Some(a) if !blocked => normalize_direction(a + 180.0),
            _ => outer,
        };
        (preferred, outer)
    }
}

/// Removes the first `dist` mm of the polyline.
fn trim_start(points: &[Pt], dist: f64) -> Vec<Pt> {
    let mut left = dist;
    for (i, w) in points.windows(2).enumerate() {
        let len = (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1);
        if len > left {
            let t = left / len;
            let mut out = vec![(
                w[0].0 + (w[1].0 - w[0].0) * t,
                w[0].1 + (w[1].1 - w[0].1) * t,
            )];
            out.extend_from_slice(&points[i + 1..]);
            return out;
        }
        left -= len;
    }
    Vec::new()
}

/// The bisector of the larger sector between the legs as seen from the control (the outer bisector); with one
/// leg, its opposite; `None` without legs.
pub fn outer_bisector(incoming: Option<f64>, outgoing: Option<f64>) -> Option<f64> {
    match (incoming, outgoing) {
        (Some(a), Some(b)) => {
            let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
            let inner = hi - lo;
            Some(if inner >= 360.0 - inner {
                normalize_direction(lo + inner / 2.0)
            } else {
                normalize_direction(hi + (360.0 - inner) / 2.0)
            })
        }
        (Some(a), None) | (None, Some(a)) => Some(normalize_direction(a + 180.0)),
        (None, None) => None,
    }
}

/// Candidate `k` (0..[`CANDIDATES`]) around `preferred`: `preferred + 15·k` rounded to 0.1 degrees, so the value
/// evaluated is the value stored (courseplan ADR 0031).
pub fn candidate(preferred: f64, k: usize) -> f64 {
    normalize_direction(((preferred + CANDIDATE_STEP_DEG * k as f64) * 10.0).round() / 10.0)
}

/// The key a candidate is ranked by, smaller being better.
#[derive(Debug, Clone, Copy)]
struct Rank {
    hard: usize,
    covered: usize,
    off_preferred: f64,
    off_outer: f64,
    off_gap: f64,
    cost: f64,
    direction: f64,
    gap: f64,
}

impl Rank {
    fn beats(&self, other: &Rank) -> bool {
        use std::cmp::Ordering;
        let near = |a: f64, b: f64| -> Ordering {
            if a < b - EPS {
                Ordering::Less
            } else if a > b + EPS {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        };
        self.hard
            .cmp(&other.hard)
            .then(self.covered.cmp(&other.covered))
            .then(near(self.off_preferred, other.off_preferred))
            .then(near(self.off_outer, other.off_outer))
            .then(near(self.off_gap, other.off_gap))
            .then(near(self.cost, other.cost))
            .then(near(self.direction, other.direction))
            .then(near(self.gap, other.gap))
            == Ordering::Less
    }
}

/// The best placement for `target` within `bounds` among `obstacles`, and its score. Tries the
/// [`CANDIDATES`] directions around the preferred one ([`Target::directions`]) at each of [`GAPS_MM`], and ranks
/// them by overprint conflicts, covered map symbols, angle to the preferred direction, angle to the outer
/// bisector, distance to the gap of `current`, soft cost, direction and gap. Never fails: when every candidate
/// conflicts, the least bad one comes back with its conflicts.
pub fn best(
    target: &Target,
    bounds: Rect,
    obstacles: &[Obstacle],
    current: Placement,
) -> (Placement, Score) {
    let region = Rect::centred(target.centre, (0.0, 0.0)).grown(target.reach());
    let near: Vec<Obstacle> = obstacles
        .iter()
        .filter(|o| o.shape.bbox().touches(&region))
        .cloned()
        .collect();
    let mut found: Option<(Rank, Placement, Score)> = None;
    for gap in GAPS_MM {
        let (preferred, outer) = target.directions(gap, current);
        for k in 0..CANDIDATES {
            let direction = candidate(preferred, k);
            let p = Placement {
                direction,
                gap_mm: gap,
            };
            let s = score(target.box_at(p), bounds, &near);
            let rank = Rank {
                hard: s.hard.len(),
                covered: s.covered.len(),
                off_preferred: angle_diff(direction, preferred),
                off_outer: angle_diff(direction, outer),
                off_gap: (gap - current.gap_mm).abs(),
                cost: s.cost,
                direction,
                gap,
            };
            if found.as_ref().is_none_or(|(r, _, _)| rank.beats(r)) {
                found = Some((rank, p, s));
            }
        }
    }
    found.map_or((current, Score::default()), |(_, p, s)| (p, s))
}

#[cfg(test)]
mod tests;
