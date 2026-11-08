//! Copied from freemapper 743ce4b (ADR 0014).
//! Path geometry for rendering (`docs/render/oom-render-reference.md` §2, §7): parts, Bézier
//! flattening, arc length, dashes, mid-symbol positions and border offsets.
//!
//! Coordinates are paper millimetres with y down, as in OOM.

use super::symbols::{Dash, Mid, MidPlacement};
use crate::omap::MapCoord;
use crate::omap::coord::{FLAG_CLOSE, FLAG_CURVE_START, FLAG_DASH, FLAG_HOLE};

pub type Pt = (f64, f64);

/// One part of a path: a polyline (Béziers flattened) and whether it is closed.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub points: Vec<Pt>,
    pub closed: bool,
    /// Indices in `points` of the coordinates flagged as dash points, ascending.
    pub dash_points: Vec<usize>,
}

/// Bézier flattening: chords at most 1 mm, control polygon at most 0.005 mm longer than the chord.
const MAX_CHORD: f64 = 1.0;
const MAX_ERROR: f64 = 0.005;
/// At most this many dashes or mid symbols per part; more means a broken symbol definition, and
/// the line is drawn solid (dashes) or without mid symbols.
const MAX_REPEATS: usize = 100_000;
/// Points closer than this are skipped when looking for a direction (0.025 mm).
const TANGENT_EPSILON: f64 = 0.025;

fn mm(c: MapCoord) -> Pt {
    (f64::from(c.x) / 1000.0, f64::from(c.y) / 1000.0)
}

fn dist(a: Pt, b: Pt) -> f64 {
    (b.0 - a.0).hypot(b.1 - a.1)
}

fn sub(a: Pt, b: Pt) -> Pt {
    (a.0 - b.0, a.1 - b.1)
}

fn normalize(v: Pt) -> Pt {
    let l = v.0.hypot(v.1);
    if l > 0.0 {
        (v.0 / l, v.1 / l)
    } else {
        (0.0, 0.0)
    }
}

/// Right of travel with y down: (x, y) → (−y, x).
fn perp_right(v: Pt) -> Pt {
    (-v.1, v.0)
}

fn lerp(a: Pt, b: Pt, t: f64) -> Pt {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

/// Appends the cubic Bézier p0…p3 without p0, halving until flat enough.
fn flatten_cubic(p0: Pt, p1: Pt, p2: Pt, p3: Pt, out: &mut Vec<Pt>, depth: u32) {
    let chord = dist(p0, p3);
    let polygon = dist(p0, p1) + dist(p1, p2) + dist(p2, p3);
    if depth >= 16 || (chord <= MAX_CHORD && polygon - chord <= MAX_ERROR) {
        out.push(p3);
        return;
    }
    let (a, b, c) = (lerp(p0, p1, 0.5), lerp(p1, p2, 0.5), lerp(p2, p3, 0.5));
    let (d, e) = (lerp(a, b, 0.5), lerp(b, c, 0.5));
    let m = lerp(d, e, 0.5);
    flatten_cubic(p0, a, d, m, out, depth + 1);
    flatten_cubic(m, e, c, p3, out, depth + 1);
}

/// The parts of an OOM path: they end at coordinates flagged `FLAG_HOLE`; a part is closed when
/// its last coordinate is flagged `FLAG_CLOSE`. Béziers are flattened.
pub fn parts(coords: &[MapCoord]) -> Vec<Part> {
    let n = coords.len();
    let mut parts = Vec::new();
    let mut points: Vec<Pt> = Vec::new();
    let mut dash_points = Vec::new();
    // The point just pushed for `c`: a dash point if `c` is flagged.
    let mark = |c: MapCoord, points: &Vec<Pt>, dash_points: &mut Vec<usize>| {
        if c.flags & FLAG_DASH != 0 {
            dash_points.push(points.len() - 1);
        }
    };
    let mut i = 0;
    // At each step `coords[i]` is the last point of the part so far.
    while i < n {
        if points.is_empty() {
            points.push(mm(coords[i]));
            mark(coords[i], &points, &mut dash_points);
        }
        let c = coords[i];
        if c.flags & FLAG_HOLE != 0 || i + 1 == n {
            parts.push(Part {
                points: std::mem::take(&mut points),
                closed: c.flags & FLAG_CLOSE != 0,
                dash_points: std::mem::take(&mut dash_points),
            });
            i += 1;
        } else if c.flags & FLAG_CURVE_START != 0 && i + 3 < n {
            let p0 = mm(c);
            let (p1, p2, p3) = (mm(coords[i + 1]), mm(coords[i + 2]), mm(coords[i + 3]));
            flatten_cubic(p0, p1, p2, p3, &mut points, 0);
            mark(coords[i + 3], &points, &mut dash_points);
            i += 3;
        } else {
            points.push(mm(coords[i + 1]));
            mark(coords[i + 1], &points, &mut dash_points);
            i += 1;
        }
    }
    parts
}

pub fn length(points: &[Pt]) -> f64 {
    points.windows(2).map(|w| dist(w[0], w[1])).sum()
}

/// Cumulative arc length at every point.
fn cumulative(points: &[Pt]) -> Vec<f64> {
    let mut cum = Vec::with_capacity(points.len());
    let mut total = 0.0;
    for (i, p) in points.iter().enumerate() {
        if i > 0 {
            total += dist(points[i - 1], *p);
        }
        cum.push(total);
    }
    cum
}

/// The point at arc length `s` and the index of the segment it lies on.
fn locate(points: &[Pt], cum: &[f64], s: f64) -> (Pt, usize) {
    let s = s.clamp(0.0, *cum.last().unwrap_or(&0.0));
    let i = cum
        .partition_point(|&c| c < s)
        .saturating_sub(1)
        .min(points.len().saturating_sub(2));
    let seg = cum[i + 1] - cum[i];
    let t = if seg > 0.0 { (s - cum[i]) / seg } else { 0.0 };
    (lerp(points[i], points[i + 1], t), i)
}

/// Position and direction (radians, y down) at arc length `s`: the segment direction, the
/// bisector at a vertex, the single direction at an open end.
pub fn at(part: &Part, s: f64) -> (Pt, f64) {
    at_on(part, &cumulative(&part.points), s)
}

/// [`at`] with the cumulative lengths of the part computed once.
fn at_on(part: &Part, cum: &[f64], s: f64) -> (Pt, f64) {
    let points = &part.points;
    if points.len() < 2 {
        return (points.first().copied().unwrap_or((0.0, 0.0)), 0.0);
    }
    let (pos, _) = locate(points, cum, s);
    let n = points.len();
    // The nearest points before and after `pos` that are not within the epsilon.
    let after = |from: usize| -> Option<Pt> {
        let mut k = from;
        loop {
            if k < n && dist(points[k], pos) > TANGENT_EPSILON {
                return Some(points[k]);
            }
            k += 1;
            if k >= n {
                return None;
            }
        }
    };
    let i_next = cum.partition_point(|&c| c <= s);
    let i_prev = cum.partition_point(|&c| c < s);
    let mut next = after(i_next.min(n));
    let mut prev = (0..i_prev.min(n))
        .rev()
        .map(|k| points[k])
        .find(|&p| dist(p, pos) > TANGENT_EPSILON);
    if part.closed {
        if next.is_none() {
            next = points
                .iter()
                .skip(1)
                .copied()
                .find(|&p| dist(p, pos) > TANGENT_EPSILON);
        }
        if prev.is_none() {
            prev = points
                .iter()
                .rev()
                .skip(1)
                .copied()
                .find(|&p| dist(p, pos) > TANGENT_EPSILON);
        }
    }
    let out = next.map(|p| normalize(sub(p, pos))).unwrap_or((0.0, 0.0));
    let inc = prev.map(|p| normalize(sub(pos, p))).unwrap_or((0.0, 0.0));
    let t = (out.0 + inc.0, out.1 + inc.1);
    (pos, t.1.atan2(t.0))
}

/// Direction (radians) from the first point to the first point farther than 0.025 mm: the
/// orientation of a start symbol.
pub fn start_angle(part: &Part) -> f64 {
    let first = part.points[0];
    part.points
        .iter()
        .find(|&&p| dist(p, first) > TANGENT_EPSILON)
        .map_or(0.0, |&p| (p.1 - first.1).atan2(p.0 - first.0))
}

/// Direction (radians) from the last point farther than 0.025 mm to the last point: the
/// orientation of an end symbol.
pub fn end_angle(part: &Part) -> f64 {
    let last = *part.points.last().expect("parts are not empty");
    part.points
        .iter()
        .rev()
        .find(|&&p| dist(p, last) > TANGENT_EPSILON)
        .map_or(0.0, |&p| (last.1 - p.1).atan2(last.0 - p.0))
}

/// The polyline between arc lengths `a` and `b`, through the vertices in between.
fn between(points: &[Pt], cum: &[f64], a: f64, b: f64) -> Vec<Pt> {
    between_lengths(points, cum, a, b).0
}

/// [`between`] with the arc length of every point.
fn between_lengths(points: &[Pt], cum: &[f64], a: f64, b: f64) -> (Vec<Pt>, Vec<f64>) {
    let (start, i) = locate(points, cum, a);
    let (end, j) = locate(points, cum, b);
    let mut out = vec![start];
    let mut lengths = vec![a];
    for k in i + 1..=j {
        if cum[k] > a && cum[k] < b {
            out.push(points[k]);
            lengths.push(cum[k]);
        }
    }
    out.push(end);
    lengths.push(b);
    (out, lengths)
}

/// The arc lengths that end the sections of [start, end] (reference §2.2): every dash point
/// strictly inside, then `end` [VP `PathCoordVector::findNextDashPoint`].
fn section_ends(part: &Part, cum: &[f64], start: f64, end: f64) -> Vec<f64> {
    let mut ends: Vec<f64> = part
        .dash_points
        .iter()
        .filter_map(|&i| cum.get(i).copied())
        .filter(|&c| c > start && c < end)
        .collect();
    ends.dedup();
    ends.push(end);
    ends
}

/// The dashes of a part as polylines ([`dash_pattern`] over the whole part, halves at the
/// closing point joined); one polyline (the whole part) when it is too short.
///
/// At most `max` dashes; the flag is true when that limit, or `MAX_REPEATS` (the part is then
/// drawn solid), changed the result.
pub fn dashes(part: &Part, dash: &Dash, max: usize) -> (Vec<Vec<Pt>>, bool) {
    let pattern = dash_pattern(part, 0.0, length(&part.points), dash, None, max);
    let pieces = pieces(part, &pattern.intervals);
    (
        pieces.into_iter().map(|(p, _)| p).collect(),
        pattern.limited,
    )
}

/// Dashes and mid-symbol spots of a dashed line.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DashPattern {
    /// Arc-length intervals [a, b] to stroke, in order.
    pub intervals: Vec<(f64, f64)>,
    /// Positions and directions of the mid symbols.
    pub spots: Vec<(Pt, f64)>,
    /// The output limit or `MAX_REPEATS` changed the result.
    pub limited: bool,
    /// Steps taken: every interval and spot, and every mid-symbol position tried and skipped.
    pub steps: usize,
}

/// Why dashing stopped early.
enum Stop {
    /// The step limit: keep what there is.
    Full,
    /// More than `MAX_REPEATS` steps: a broken definition, drawn solid without mid symbols.
    Repeats,
}

/// The state of [`dash_pattern`]: OOM's `processDashedLine` and `createDashGroups` on arc
/// lengths (mm).
struct Dasher<'a> {
    part: &'a Part,
    cum: &'a [f64],
    total: f64,
    /// Dash, break and in-group break length, dashes per group.
    d: f64,
    b: f64,
    g: f64,
    k: u32,
    half_outer: bool,
    placement: Option<MidPlacement>,
    /// Mid symbols per spot, their distance, `show_at_least_one_symbol`.
    m: u32,
    q: f64,
    at_least_one: bool,
    out: DashPattern,
    max: usize,
    steps: usize,
}

impl Dasher<'_> {
    /// Takes one step: past `MAX_REPEATS` the definition is broken, past `max` the caller's
    /// limit is reached.
    fn step(&mut self) -> Result<(), Stop> {
        if self.steps >= MAX_REPEATS {
            return Err(Stop::Repeats);
        }
        if self.steps >= self.max {
            return Err(Stop::Full);
        }
        self.steps += 1;
        Ok(())
    }

    fn interval(&mut self, a: f64, b: f64) -> Result<(), Stop> {
        self.step()?;
        self.out.intervals.push((a, b));
        Ok(())
    }

    fn place(&mut self, p: f64) -> Result<(), Stop> {
        self.step()?;
        self.out.spots.push(at_on(self.part, self.cum, p));
        Ok(())
    }

    /// [LS `processContinuousLine`]: the stretch [a, b], and if `set_mid` the spot's mid symbols
    /// centred on it.
    fn continuous(&mut self, a: f64, b: f64, set_mid: bool) -> Result<(), Stop> {
        self.interval(a, b)?;
        if set_mid {
            let msl = f64::from(self.m - 1) * self.q;
            if msl <= b - a {
                let mut pos = (a + b - msl) / 2.0;
                self.place(pos)?;
                for _ in 1..self.m {
                    pos += self.q;
                    self.place(pos)?;
                }
            }
        }
        Ok(())
    }

    /// [LS `createDashGroups`] for the section [start, end], the dash so far starting at
    /// `line_start`; returns where the next section's first dash starts.
    fn groups(
        &mut self,
        line_start: f64,
        start: f64,
        end: f64,
        is_part_start: bool,
        is_part_end: bool,
    ) -> Result<f64, Stop> {
        let (d, b, g, k, q, m) = (self.d, self.b, self.g, self.k, self.q, self.m);
        let (kf, mf) = (f64::from(k), f64::from(m));
        let closed = self.part.closed;
        // Inside a part the sections meet at dash points.
        let half_first = if is_part_start {
            self.half_outer || closed
        } else {
            k == 1
        };
        let half_last = if is_part_end {
            self.half_outer || closed
        } else {
            k == 1
        };
        let h = u32::from(half_first) + u32::from(half_last);
        let hf = f64::from(h);
        let period = kf * d + (kf - 1.0) * g + b;
        let length = end - start;
        let lpb = length + b;
        let nf = hf + (lpb - hf * d / 2.0) / period;
        let lo = nf.floor();
        let min_opt_dashes = 2.0 * kf - f64::from(h / 2);
        let sw = 0.2 * period / kf;
        let at_dash = self.placement == Some(MidPlacement::CentreOfDash);
        let mut set_mid = self.placement.is_some() && (length >= d - sw || self.at_least_one);
        if at_dash {
            if line_start < start || !is_part_start {
                // The dash through the dash point is drawn here: its mid symbols too.
                set_mid = false;
                let mut pos = start - (mf - 1.0) * q / 2.0;
                for _ in (0..m).step_by(2) {
                    if pos >= 0.0 {
                        self.place(pos)?;
                    } else {
                        self.step()?;
                    }
                    pos += q;
                }
            }
            if half_first {
                let mut pos = start + f64::from(m % 2 + 1) * q / 2.0;
                let mut s = 1;
                while s < m && pos <= self.total {
                    self.place(pos)?;
                    pos += q;
                    s += 2;
                }
            }
        }
        if length <= 0.0 || (lo <= 1.0 && length < 2.0 * period - min_opt_dashes * sw) {
            // Too short for dashes: solid if it ends the part, else handed to the next section.
            if !is_part_end {
                return Ok(line_start);
            }
            self.continuous(line_start, end, set_mid)?;
            return Ok(end);
        }
        let hi = nf.ceil();
        let n = if lo < 1.0 {
            hi
        } else {
            let dlo = (lpb - lo * period) / lo;
            let dhi = (hi * period - lpb) / hi;
            if dlo > dhi { hi } else { lo }
        };
        if n * kf > MAX_REPEATS as f64 {
            return Err(Stop::Repeats);
        }
        let dash_len =
            ((length - (n - 1.0) * b - n * (kf - 1.0) * g) * 2.0 / (2.0 * n * kf - hf)).max(0.0);
        let groups = n as u32;
        let mut cur = start;
        let mut dash_start = line_start;
        for grp in 1..=groups {
            let (first_grp, last_grp) = (grp == 1, grp == groups);
            if self.placement == Some(MidPlacement::CentreOfDashGroup) {
                let mut pos = cur;
                if last_grp && half_last {
                    pos = end;
                } else if !(first_grp && half_first) {
                    pos += (dash_len * kf + g * (kf - 1.0)) / 2.0;
                }
                pos -= q * (mf - 1.0) / 2.0;
                for _ in 0..m {
                    if pos > end {
                        break;
                    }
                    if pos > start {
                        self.place(pos)?;
                    } else {
                        self.step()?;
                    }
                    pos += q;
                }
            }
            for i in 1..=k {
                let first = first_grp && i == 1;
                let last = last_grp && i == k;
                let has_start = !(first && half_first);
                let has_end = !(last && half_last);
                let half = has_start != has_end;
                let len = if half { dash_len / 2.0 } else { dash_len };
                if !first {
                    dash_start = cur;
                }
                if last && !is_part_end {
                    // The last dash before a dash point continues in the next section.
                    return Ok(dash_start);
                }
                let dash_end = cur + len;
                self.continuous(dash_start, dash_end, at_dash && !half)?;
                cur += len;
                dash_start = dash_end;
                if i < k {
                    cur += g;
                }
            }
            if grp < groups {
                if self.placement == Some(MidPlacement::CentreOfGap) {
                    let mut pos = cur + (b - q * (mf - 1.0)) / 2.0;
                    for _ in 0..m {
                        if pos > end {
                            break;
                        }
                        if pos < start {
                            self.step()?;
                        } else {
                            self.place(pos)?;
                        }
                        pos += q;
                    }
                }
                cur += b;
            }
        }
        if half_last && at_dash {
            // The half dash at the closing point (or end) carries the joined dash's symbols.
            let mut pos = end - (mf - 1.0) * q / 2.0;
            for _ in (0..m).step_by(2) {
                self.place(pos)?;
                pos += q;
            }
        }
        Ok(end)
    }
}

/// The dashes (arc-length intervals) and mid-symbol spots of a dashed line on [start, end] of
/// `part` (reference §2.2, §2.5) [LS `processDashedLine`, `createDashGroups`]. The pattern
/// restarts at dash points. `mid`: the mid symbol of the dashed line, placed by its
/// [`MidPlacement`].
///
/// At most `max` steps (intervals, spots and skipped mid-symbol positions together); `limited`
/// is true when that limit, or `MAX_REPEATS` steps (the stretch is then one solid interval
/// without spots), changed the result.
pub fn dash_pattern(
    part: &Part,
    start: f64,
    end: f64,
    dash: &Dash,
    mid: Option<&Mid>,
    max: usize,
) -> DashPattern {
    let cum = cumulative(&part.points);
    let (d, b, g) = (
        dash.dash / 1000.0,
        dash.gap / 1000.0,
        dash.in_group_gap / 1000.0,
    );
    let k = dash.group.max(1);
    let q = mid.map_or(0.0, |m| m.distance / 1000.0);
    let period = f64::from(k) * d + f64::from(k - 1) * g + b;
    let solid = |limited, steps| DashPattern {
        intervals: vec![(start, end)],
        spots: Vec::new(),
        limited,
        steps,
    };
    let usable = d > 0.0
        && b >= 0.0
        && g >= 0.0
        && q >= 0.0
        && period.is_finite()
        && q.is_finite()
        && start.is_finite()
        && end.is_finite();
    if !usable || end <= start {
        return solid(false, 1);
    }
    let mut dasher = Dasher {
        part,
        total: *cum.last().unwrap_or(&0.0),
        cum: &cum,
        d,
        b,
        g,
        k,
        half_outer: dash.half_outer,
        placement: mid.map(|m| m.placement),
        m: mid.map_or(1, |m| m.per_spot.max(1)),
        q,
        at_least_one: mid.is_some_and(|m| m.at_least_one),
        out: DashPattern::default(),
        max,
        steps: 0,
    };
    let ends = section_ends(part, &cum, start, end);
    let mut run = || -> Result<(), Stop> {
        let (mut line_start, mut groups_start) = (start, start);
        for (i, &groups_end) in ends.iter().enumerate() {
            let is_part_end = i + 1 == ends.len();
            line_start =
                dasher.groups(line_start, groups_start, groups_end, i == 0, is_part_end)?;
            groups_start = groups_end;
        }
        Ok(())
    };
    let result = run();
    let steps = dasher.steps;
    match result {
        Ok(()) => DashPattern {
            steps,
            ..dasher.out
        },
        Err(Stop::Full) => DashPattern {
            limited: true,
            steps,
            ..dasher.out
        },
        Err(Stop::Repeats) => solid(true, steps),
    }
}

/// The polylines of arc-length `intervals` of `part` and whether each is closed: the whole part
/// is the part itself (closed if the part is); on a closed part the first piece is appended to
/// the last when they meet at the closing point [RI `LineRenderable`, `connectPath`].
pub fn pieces(part: &Part, intervals: &[(f64, f64)]) -> Vec<(Vec<Pt>, bool)> {
    let cum = cumulative(&part.points);
    let total = *cum.last().unwrap_or(&0.0);
    const EPS: f64 = 1e-9;
    if let [(a, b)] = intervals
        && *a <= EPS
        && *b >= total - EPS
    {
        return vec![(part.points.clone(), part.closed)];
    }
    let mut out: Vec<(Vec<Pt>, bool)> = intervals
        .iter()
        .map(|&(a, b)| (between(&part.points, &cum, a, b), false))
        .collect();
    let meet = intervals.first().is_some_and(|f| f.0 <= EPS)
        && intervals.last().is_some_and(|l| l.1 >= total - EPS);
    if part.closed && out.len() > 1 && meet {
        let (first, _) = out.remove(0);
        let last = &mut out.last_mut().expect("more than one").0;
        last.extend(first.into_iter().skip(1));
    }
    out
}

/// The parts of the arc-length intervals `a` that lie inside intervals of `b` (both ascending).
pub fn intersect(a: &[(f64, f64)], b: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        let lo = a[i].0.max(b[j].0);
        let hi = a[i].1.min(b[j].1);
        if lo < hi {
            out.push((lo, hi));
        }
        if a[i].1 < b[j].1 {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

/// Positions and directions of a line's mid symbols over the whole part (reference §2.4), at
/// most `max`; the flag is true when that limit, or `MAX_REPEATS` (no mid symbols then), changed
/// the result.
pub fn mid_positions(part: &Part, mid: &Mid, max: usize) -> (Vec<(Pt, f64)>, bool) {
    mid_positions_in(part, 0.0, length(&part.points), mid, max)
}

/// Collects mid-symbol spots up to a limit.
struct Spots<'a> {
    part: &'a Part,
    cum: &'a [f64],
    out: Vec<(Pt, f64)>,
    max: usize,
    limited: bool,
}

impl Spots<'_> {
    /// False once the limit is reached.
    fn place(&mut self, p: f64) -> bool {
        if self.out.len() < self.max {
            self.out.push(at_on(self.part, self.cum, p));
            true
        } else {
            self.limited = true;
            false
        }
    }
}

/// [`mid_positions`] on [start, end] of the part (start and end offsets), the pattern restarting
/// at every dash point [LS `createMidSymbolRenderables`].
pub fn mid_positions_in(
    part: &Part,
    start: f64,
    end: f64,
    mid: &Mid,
    max: usize,
) -> (Vec<(Pt, f64)>, bool) {
    let cum = cumulative(&part.points);
    let (s, e, q) = (
        mid.segment / 1000.0,
        mid.end / 1000.0,
        mid.distance / 1000.0,
    );
    let m = mid.per_spot.max(1);
    let msl = f64::from(m - 1) * q;
    let mut spots = Spots {
        part,
        cum: &cum,
        out: Vec::new(),
        max,
        limited: false,
    };
    let usable = s > 0.0 && s.is_finite() && e >= 0.0 && q >= 0.0 && msl.is_finite();
    if !usable || !start.is_finite() || !end.is_finite() || end <= start {
        return (spots.out, spots.limited);
    }
    // Spots per section, bounded: a broken definition gets no mid symbols.
    let bound = |n: f64| (n + 1.0) * f64::from(m) <= MAX_REPEATS as f64;
    if e == 0.0 && !part.closed && !spots.place(start) {
        return (spots.out, spots.limited);
    }
    let mut a = start;
    for b in section_ends(part, &cum, start, end) {
        let l = b - a;
        let seg = (l - 2.0 * e).max(0.0) - msl;
        let raw = (seg / (s + msl)).max(if e == 0.0 { 1.0 } else { 0.0 });
        let (lo, hi) = (raw.floor(), raw.ceil());
        if e > 0.0 {
            if l <= msl {
                if mid.at_least_one && !(spots.place(a) && spots.place(b)) {
                    return (spots.out, spots.limited);
                }
            } else {
                let dlo = (l - lo * s - (lo + 1.0) * msl - 2.0 * e).abs();
                let dhi = (l - hi * s - (hi + 1.0) * msl - 2.0 * e).abs();
                let (n, dev) = if dlo >= dhi { (hi, -dhi) } else { (lo, dlo) };
                if !bound(n) {
                    return (Vec::new(), true);
                }
                let ideal = n * s + 2.0 * e;
                let e2 = e + dev * e / ideal;
                let s2 = s + dev * s / ideal;
                if s2 >= 0.0 && (mid.at_least_one || hi > 0.0 || l > 2.0 * e - (s + msl) / 2.0) {
                    for i in 0..=n as u32 {
                        for j in 0..m {
                            if !spots.place(a + e2 + f64::from(i) * (s2 + msl) + f64::from(j) * q) {
                                return (spots.out, spots.limited);
                            }
                        }
                    }
                }
            }
        } else {
            if l > msl {
                let dlo = (l - lo * s - (lo + 1.0) * msl).abs() / lo;
                let dhi = (l - hi * s - (hi + 1.0) * msl).abs() / hi;
                let n = if dlo > dhi { hi } else { lo };
                if !bound(n) {
                    return (Vec::new(), true);
                }
                let step = (l - (n + 1.0) * msl) / n + msl;
                if step >= msl {
                    'spots: for i in 0..=n as u32 {
                        for j in 0..m {
                            if i == 0 && j == 0 {
                                continue;
                            }
                            if i == n as u32 && j == m - 1 {
                                break 'spots;
                            }
                            if !spots.place(a + f64::from(i) * step + f64::from(j) * q) {
                                return (spots.out, spots.limited);
                            }
                        }
                    }
                }
            }
            if !spots.place(b) {
                return (spots.out, spots.limited);
            }
        }
        a = b;
    }
    (spots.out, spots.limited)
}

/// Direction (radians, y down) and scale at point `i` of a polyline: the bisector of the
/// incoming and outgoing direction and 1 / cos(half the turn), the single direction and 1 at an
/// end, perpendicular and infinite at a reversal. Neighbours within 0.025 mm are skipped; closed
/// lines wrap around [VP `calculateTangentScaling`, `calculateIncomingTangent`,
/// `calculateOutgoingTangent`].
pub fn tangent_scaling(points: &[Pt], closed: bool, i: usize) -> (f64, f64) {
    let (dir, scale) = tangent_scaling_vector(points, closed, i);
    (dir.1.atan2(dir.0), scale)
}

fn incoming(points: &[Pt], closed: bool, i: usize) -> Option<Pt> {
    let far = |k: &usize| dist(points[i], points[*k]) > TANGENT_EPSILON;
    let n = points.len();
    (0..i)
        .rev()
        .find(far)
        .or_else(|| {
            (closed && n > i + 2)
                .then(|| (i + 1..n).rev().find(far))
                .flatten()
        })
        .map(|k| sub(points[i], points[k]))
}

fn outgoing(points: &[Pt], closed: bool, i: usize) -> Option<Pt> {
    let far = |k: &usize| dist(points[i], points[*k]) > TANGENT_EPSILON;
    (i + 1..points.len())
        .find(far)
        .or_else(|| closed.then(|| (0..i).find(far)).flatten())
        .map(|k| sub(points[k], points[i]))
}

fn tangent_scaling_vector(points: &[Pt], closed: bool, i: usize) -> (Pt, f64) {
    match (incoming(points, closed, i), outgoing(points, closed, i)) {
        (inc, None) => (inc.map_or((0.0, 0.0), normalize), 1.0),
        (None, Some(out)) => (normalize(out), 1.0),
        (Some(inc), Some(out)) => {
            let (inc, out) = (normalize(inc), normalize(out));
            let sum = (inc.0 + out.0, inc.1 + out.1);
            if sum.0.hypot(sum.1) < 1e-12 {
                (perp_right(out), f64::INFINITY)
            } else {
                let dir = normalize(sum);
                (dir, 1.0 / (dir.0 * inc.0 + dir.1 * inc.1))
            }
        }
    }
}

/// Overlap of a pointed cap into the line (mm) [LS `createPointedLineCap`].
const CAP_OVERLAP: f64 = 0.05;

/// The filled taper of a pointed cap on [a, b] of `part` (reference §2.7)
/// [LS `createPointedLineCap`]: the half width grows from 0 at the line's end to `half_width`
/// at `cap_length` from it, widened by 1 / cos(half the turn) at corners (at most twice the
/// half width); beyond 0.2 mm the cap reaches 0.05 mm into the line. The ring runs out along the
/// right side and back along the left side.
pub fn pointed_cap(
    part: &Part,
    a: f64,
    b: f64,
    is_end: bool,
    half_width: f64,
    cap_length: f64,
) -> Vec<Pt> {
    let cum = cumulative(&part.points);
    let (middle, lengths) = between_lengths(&part.points, &cum, a, b);
    let n = middle.len();
    let mut right = Vec::with_capacity(n + 1);
    let mut left = Vec::with_capacity(n + 1);
    for (i, (&c, &len)) in middle.iter().zip(&lengths).enumerate() {
        let from_tip = if is_end { b - len } else { len - a };
        let factor = (from_tip / cap_length).clamp(0.0, 1.0);
        let (dir, scale) = tangent_scaling_vector(&middle, false, i);
        let side = normalize(perp_right(dir));
        let radius = (half_width * factor * scale).clamp(0.0, 2.0 * half_width);
        right.push((c.0 + radius * side.0, c.1 + radius * side.1));
        left.push((c.0 - radius * side.0, c.1 - radius * side.1));
    }
    if b - a > 4.0 * CAP_OVERLAP {
        // The tangent at the wide end, along the line towards its middle.
        let wide = if is_end { 0 } else { n - 1 };
        let tangent = if is_end {
            outgoing(&middle, false, wide).map(|t| {
                let t = normalize(t);
                (-t.0 * CAP_OVERLAP, -t.1 * CAP_OVERLAP)
            })
        } else {
            incoming(&middle, false, wide).map(|t| {
                let t = normalize(t);
                (t.0 * CAP_OVERLAP, t.1 * CAP_OVERLAP)
            })
        };
        if let Some(t) = tangent {
            let r = perp_right(if is_end { t } else { (-t.0, -t.1) });
            let r_point = (right[wide].0 + t.0 + r.0, right[wide].1 + t.1 + r.1);
            let l_point = (left[wide].0 + t.0 - r.0, left[wide].1 + t.1 - r.1);
            if is_end {
                right.insert(0, r_point);
                left.insert(0, l_point);
            } else {
                right.push(r_point);
                left.push(l_point);
            }
        }
    }
    // Right side, then the left side back (without its tip at an end cap).
    let skip = usize::from(is_end);
    let back = left.len() - skip;
    right.extend(left[..back].iter().rev());
    right
}

/// The polyline offset by `shift` mm to the right of travel (negative: left), mitred at corners
/// (reference §2.3).
pub fn offset(points: &[Pt], closed: bool, shift: f64) -> Vec<Pt> {
    let n = points.len();
    if n < 2 {
        return points.to_vec();
    }
    // Without the duplicated closing point, so that tangents wrap around.
    let pts: Vec<Pt> = if closed && points[0] == points[n - 1] {
        points[..n - 1].to_vec()
    } else {
        points.to_vec()
    };
    let m = pts.len();
    let add = |a: Pt, v: Pt, f: f64| (a.0 + v.0 * f, a.1 + v.1 * f);
    let mut out = Vec::with_capacity(m + 1);
    for i in 0..m {
        let c = pts[i];
        let prev = if i > 0 {
            Some(pts[i - 1])
        } else if closed {
            Some(pts[m - 1])
        } else {
            None
        };
        let next = if i + 1 < m {
            Some(pts[i + 1])
        } else if closed {
            Some(pts[0])
        } else {
            None
        };
        let t_in = prev.map(|p| normalize(sub(c, p)));
        let t_out = next.map(|p| normalize(sub(p, c)));
        match (t_in, t_out) {
            (None, Some(t)) | (Some(t), None) => out.push(add(c, perp_right(t), shift)),
            (Some(t_in), Some(t_out)) => {
                let mid = normalize((t_in.0 + t_out.0, t_in.1 + t_out.1));
                let cross = t_out.0 * t_in.1 - t_in.0 * t_out.1;
                let base = add(c, perp_right(t_out), shift);
                if cross.abs() < 1e-12 && mid != (0.0, 0.0) {
                    out.push(base);
                } else if cross * shift > 0.0 {
                    // Outer side: the miter point, or a bevel for very sharp corners.
                    if mid.0 * t_in.0 + mid.1 * t_in.1 <= 0.4472 {
                        out.push(add(c, perp_right(t_in), shift));
                        out.push(base);
                    } else {
                        let pr = perp_right(mid);
                        let phi = (pr.0 * t_in.0 + pr.1 * t_in.1).clamp(-1.0, 1.0).acos();
                        let off = (shift / phi.tan()).abs();
                        out.push(add(base, t_out, -off));
                    }
                } else {
                    // Inner side: the intersection of the offset lines, bounded by the segments.
                    let pr = perp_right(mid);
                    let phi = (pr.0 * t_in.0 + pr.1 * t_in.1).clamp(-1.0, 1.0).acos();
                    let off = -(shift / phi.tan()).abs();
                    let len_in = prev.map_or(f64::INFINITY, |p| dist(p, c));
                    let len_out = next.map_or(f64::INFINITY, |p| dist(p, c));
                    if phi.tan() >= 1.0 || off.abs() < len_in.min(len_out) {
                        out.push(add(base, t_out, -off));
                    } else if len_in < len_out {
                        out.push(add(base, t_out, len_in));
                    } else {
                        out.push(add(add(c, perp_right(t_in), shift), t_in, -len_out));
                    }
                }
            }
            (None, None) => out.push(c),
        }
    }
    if closed {
        let first = out[0];
        out.push(first);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::symbols::PointDef;

    fn line(points: &[Pt]) -> Part {
        Part {
            points: points.to_vec(),
            closed: false,
            dash_points: Vec::new(),
        }
    }

    fn dash_506() -> Dash {
        Dash {
            dash: 1500.0,
            gap: 370.0,
            group: 1,
            in_group_gap: 0.0,
            half_outer: false,
        }
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn parts_split_at_holes_and_know_closed() {
        let mut coords = vec![
            MapCoord::new(0, 0),
            MapCoord::new(1000, 0),
            MapCoord::new(1000, 1000),
            MapCoord::new(0, 0),
        ];
        coords[3].flags = FLAG_HOLE | FLAG_CLOSE;
        coords.extend([MapCoord::new(5000, 0), MapCoord::new(6000, 0)]);
        let parts = parts(&coords);
        assert_eq!(parts.len(), 2);
        assert!(parts[0].closed && !parts[1].closed);
        assert_eq!(parts[1].points, [(5.0, 0.0), (6.0, 0.0)]);
    }

    #[test]
    fn bezier_is_flattened_within_tolerance() {
        // A quarter circle of radius 10 mm as one cubic Bézier.
        let k = 0.552_284_75 * 10_000.0;
        let mut coords = vec![
            MapCoord::new(10_000, 0),
            MapCoord::new(10_000, k as i32),
            MapCoord::new(k as i32, 10_000),
            MapCoord::new(0, 10_000),
        ];
        coords[0].flags = FLAG_CURVE_START;
        let part = &parts(&coords)[0];
        assert!(part.points.len() > 10);
        for w in part.points.windows(2) {
            let chord = ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt();
            assert!(chord <= MAX_CHORD + 1e-9);
        }
        for p in &part.points {
            let r = (p.0 * p.0 + p.1 * p.1).sqrt();
            assert!((r - 10.0).abs() < 0.01, "{p:?} at r {r}");
        }
        let quarter = std::f64::consts::FRAC_PI_2 * 10.0;
        assert!((length(&part.points) - quarter).abs() < 0.01);
    }

    #[test]
    fn dashes_are_stretched_to_whole_dashes() {
        let dashes = dashes(&line(&[(0.0, 0.0), (10.0, 0.0)]), &dash_506(), usize::MAX).0;
        assert_eq!(dashes.len(), 6);
        let lengths: Vec<f64> = dashes.iter().map(|d| length(d)).collect();
        let d = (10.0 - 5.0 * 0.37) / 6.0;
        assert!(lengths.iter().all(|&l| near(l, d)), "{lengths:?}");
        assert!(near(dashes[0][0].0, 0.0) && near(dashes[5].last().unwrap().0, 10.0));
        assert!(near(dashes[1][0].0, d + 0.37));
    }

    #[test]
    fn dashes_run_through_corners() {
        // 6 mm: three dashes of 1.753 mm; the second spans the corner at 3 mm.
        let dashes = dashes(
            &line(&[(0.0, 0.0), (3.0, 0.0), (3.0, 3.0)]),
            &dash_506(),
            usize::MAX,
        )
        .0;
        assert_eq!(dashes.len(), 3);
        assert!(dashes[1].contains(&(3.0, 0.0)), "{dashes:?}");
        let total: f64 = dashes.iter().map(|d| length(d)).sum();
        assert!(near(total + (dashes.len() - 1) as f64 * 0.37, 6.0));
    }

    #[test]
    fn short_dashed_line_is_solid() {
        let dashes = dashes(&line(&[(0.0, 0.0), (2.5, 0.0)]), &dash_506(), usize::MAX).0;
        assert_eq!(dashes, [vec![(0.0, 0.0), (2.5, 0.0)]]);
    }

    #[test]
    fn closed_dash_halves_meet() {
        let square = Part {
            points: vec![(0.0, 0.0), (5.0, 0.0), (5.0, 5.0), (0.0, 5.0), (0.0, 0.0)],
            closed: true,
            dash_points: Vec::new(),
        };
        let dashes = dashes(&square, &dash_506(), usize::MAX).0;
        // The first and last half dashes form one dash across the closing point.
        let at_origin = |p: &Pt| p.0.abs() < 1e-9 && p.1.abs() < 1e-9;
        let joined = dashes.iter().find(|d| d.iter().any(at_origin)).unwrap();
        let first = joined.first().unwrap();
        let last = joined.last().unwrap();
        assert!(
            first.1 > 0.0 && first.0 == 0.0,
            "starts on the left edge: {joined:?}"
        );
        assert!(
            last.0 > 0.0 && last.1 == 0.0,
            "ends on the top edge: {joined:?}"
        );
        let total: f64 = dashes.iter().map(|d| length(d)).sum();
        assert!(near(total + dashes.len() as f64 * 0.37, 20.0), "{total}");
    }

    fn fence_mid() -> Mid {
        Mid {
            symbol: Box::new(PointDef {
                rotatable: true,
                dot: None,
                ring: None,
                elements: Vec::new(),
            }),
            segment: 3750.0,
            end: 2325.0,
            per_spot: 1,
            distance: 0.0,
            at_least_one: false,
            placement: MidPlacement::CentreOfDash,
        }
    }

    #[test]
    fn mid_symbols_fill_the_line() {
        // 20 mm: n·3.75 + 4.65 closest to 20 for n = 4 (19.65); scaled by 20/19.65.
        let spots = mid_positions(&line(&[(0.0, 0.0), (20.0, 0.0)]), &fence_mid(), usize::MAX).0;
        assert_eq!(spots.len(), 5);
        let f = 20.0 / 19.65;
        assert!(near(spots[0].0.0, 2.325 * f));
        assert!(near(spots[1].0.0 - spots[0].0.0, 3.75 * f));
        assert!(near(spots[4].0.0, 20.0 - 2.325 * f));
        assert!(spots.iter().all(|s| near(s.1, 0.0)));
    }

    #[test]
    fn short_fence_has_no_tick() {
        assert!(
            mid_positions(&line(&[(0.0, 0.0), (2.5, 0.0)]), &fence_mid(), usize::MAX)
                .0
                .is_empty()
        );
        let one = mid_positions(&line(&[(0.0, 0.0), (3.0, 0.0)]), &fence_mid(), usize::MAX).0;
        assert_eq!(one.len(), 1);
        assert!(near(one[0].0.0, 1.5));
    }

    #[test]
    fn absurd_patterns_are_bounded() {
        let long = line(&[(0.0, 0.0), (10_000.0, 0.0)]);
        let tiny = Dash {
            dash: 0.001,
            gap: 0.0,
            group: 1_000_000,
            in_group_gap: 0.0,
            half_outer: false,
        };
        let (pieces, limited) = dashes(&long, &tiny, usize::MAX);
        assert!(limited);
        assert!(pieces.len() <= MAX_REPEATS, "{}", pieces.len());
        let mut mid = fence_mid();
        mid.segment = 0.001;
        mid.end = 0.001;
        mid.per_spot = 1_000_000;
        assert!(mid_positions(&long, &mid, usize::MAX).0.len() <= MAX_REPEATS);
        let nan = Dash {
            dash: f64::NAN,
            ..dash_506()
        };
        assert_eq!(
            dashes(&long, &nan, usize::MAX),
            (vec![long.points.clone()], false),
            "solid"
        );
    }

    #[test]
    fn dashes_and_mid_symbols_stop_at_the_limit() {
        let long = line(&[(0.0, 0.0), (1_000.0, 0.0)]);
        let (pieces, limited) = dashes(&long, &dash_506(), 100);
        assert_eq!((pieces.len(), limited), (100, true));
        let (all, limited) = dashes(&long, &dash_506(), usize::MAX);
        assert!(all.len() > 100 && !limited);
        let (spots, limited) = mid_positions(&long, &fence_mid(), 10);
        assert_eq!((spots.len(), limited), (10, true));
    }

    #[test]
    fn huge_dash_groups_do_not_overflow() {
        let huge = Dash {
            group: u32::MAX,
            ..dash_506()
        };
        assert_eq!(
            dashes(&line(&[(0.0, 0.0), (10.0, 0.0)]), &huge, usize::MAX)
                .0
                .len(),
            1
        );
    }

    #[test]
    fn tangent_at_vertex_is_the_bisector() {
        let part = line(&[(0.0, 0.0), (5.0, 0.0), (5.0, 5.0)]);
        let (p, angle) = at(&part, 5.0);
        assert_eq!(p, (5.0, 0.0));
        assert!(near(angle, std::f64::consts::FRAC_PI_4));
        let (_, along) = at(&part, 2.0);
        assert!(near(along, 0.0));
        let (q, down) = at(&part, 7.5);
        assert!(near(q.1, 2.5) && near(down, std::f64::consts::FRAC_PI_2));
    }

    #[test]
    fn offset_of_a_right_angle_is_mitred() {
        // Going east then south (y down): the right side is the inside of the corner.
        let right = offset(&[(0.0, 0.0), (5.0, 0.0), (5.0, 5.0)], false, 1.0);
        assert_eq!(right.len(), 3);
        assert!(near(right[0].0, 0.0) && near(right[0].1, 1.0));
        assert!(near(right[1].0, 4.0) && near(right[1].1, 1.0), "{right:?}");
        let left = offset(&[(0.0, 0.0), (5.0, 0.0), (5.0, 5.0)], false, -1.0);
        assert!(near(left[1].0, 6.0) && near(left[1].1, -1.0), "{left:?}");
        assert!(near(left[2].0, 6.0) && near(left[2].1, 5.0));
    }

    fn dot_mid(placement: MidPlacement) -> Mid {
        Mid {
            placement,
            ..fence_mid()
        }
    }

    /// x of every spot, rounded to 1 µm.
    fn xs(spots: &[(Pt, f64)]) -> Vec<f64> {
        spots
            .iter()
            .map(|s| (s.0.0 * 1000.0).round() / 1000.0)
            .collect()
    }

    fn rounded(v: impl IntoIterator<Item = f64>) -> Vec<f64> {
        v.into_iter()
            .map(|x| (x * 1000.0).round() / 1000.0)
            .collect()
    }

    #[test]
    fn mid_symbols_on_dashed_line() {
        // 10 mm of 506: six dashes of d = (10 − 5 · 0.37) / 6 (reference §2.2).
        let part = line(&[(0.0, 0.0), (10.0, 0.0)]);
        let d = (10.0 - 5.0 * 0.37) / 6.0;
        let on_dash = dash_pattern(
            &part,
            0.0,
            10.0,
            &dash_506(),
            Some(&dot_mid(MidPlacement::CentreOfDash)),
            usize::MAX,
        );
        assert_eq!(on_dash.intervals.len(), 6);
        let centres = rounded((0..6).map(|i| f64::from(i) * (d + 0.37) + d / 2.0));
        assert_eq!(xs(&on_dash.spots), centres);
        assert!(on_dash.spots.iter().all(|s| near(s.1, 0.0)));
        // A group of one dash: the group centre is the dash centre.
        let on_group = dash_pattern(
            &part,
            0.0,
            10.0,
            &dash_506(),
            Some(&dot_mid(MidPlacement::CentreOfDashGroup)),
            usize::MAX,
        );
        assert_eq!(xs(&on_group.spots), centres);
        let on_gap = dash_pattern(
            &part,
            0.0,
            10.0,
            &dash_506(),
            Some(&dot_mid(MidPlacement::CentreOfGap)),
            usize::MAX,
        );
        let gaps = rounded((0..5).map(|i| f64::from(i) * (d + 0.37) + d + 0.185));
        assert_eq!(xs(&on_gap.spots), gaps);
    }

    #[test]
    fn mid_symbols_in_dash_groups_and_short_lines() {
        // Groups of two 1 mm dashes, 0.2 mm apart, 0.5 mm between groups, on 10 mm: n = 4
        // groups (deviation 0.1 < 0.45), dashes stretched to (10 − 3 · 0.5 − 4 · 0.2) / 8.
        let part = line(&[(0.0, 0.0), (10.0, 0.0)]);
        let pairs = Dash {
            dash: 1000.0,
            gap: 500.0,
            group: 2,
            in_group_gap: 200.0,
            half_outer: false,
        };
        let p = dash_pattern(
            &part,
            0.0,
            10.0,
            &pairs,
            Some(&dot_mid(MidPlacement::CentreOfDashGroup)),
            usize::MAX,
        );
        assert_eq!(p.intervals.len(), 8);
        let d = (10.0 - 3.0 * 0.5 - 4.0 * 0.2) / 8.0;
        let group = 2.0 * d + 0.2;
        let centres = rounded((0..4).map(|i| f64::from(i) * (group + 0.5) + group / 2.0));
        assert_eq!(xs(&p.spots), centres);
        // Too short for dashes: drawn solid, with one mid symbol in its middle.
        let short = line(&[(0.0, 0.0), (2.5, 0.0)]);
        let p = dash_pattern(
            &short,
            0.0,
            2.5,
            &dash_506(),
            Some(&dot_mid(MidPlacement::CentreOfDash)),
            usize::MAX,
        );
        assert_eq!(p.intervals, [(0.0, 2.5)]);
        assert_eq!(xs(&p.spots), [1.25]);
    }

    #[test]
    fn closed_dashed_line_has_its_mid_symbol_at_the_closing_point() {
        let square = Part {
            points: vec![(0.0, 0.0), (5.0, 0.0), (5.0, 5.0), (0.0, 5.0), (0.0, 0.0)],
            closed: true,
            dash_points: Vec::new(),
        };
        let p = dash_pattern(
            &square,
            0.0,
            20.0,
            &dash_506(),
            Some(&dot_mid(MidPlacement::CentreOfDash)),
            usize::MAX,
        );
        // Half dashes at both ends form one dash: one spot per joined dash, the last at the
        // closing point, none at the start.
        assert_eq!(p.spots.len(), p.intervals.len() - 1);
        let last = p.spots.last().unwrap().0;
        assert!(near(last.0, 0.0) && near(last.1, 0.0), "{last:?}");
        let pieces = pieces(&square, &p.intervals);
        assert_eq!(pieces.len(), p.intervals.len() - 1, "halves joined");
    }

    #[test]
    fn dashes_restart_at_dash_points() {
        // A dash point at 4 mm splits the pattern: [0, 4] and [4, 10] are stretched on their own,
        // and the half dashes on either side of the dash point form one dash through it.
        let mut part = line(&[(0.0, 0.0), (4.0, 0.0), (10.0, 0.0)]);
        part.dash_points = vec![1];
        let p = dash_pattern(&part, 0.0, 10.0, &dash_506(), None, usize::MAX);
        assert!(near(p.intervals[0].0, 0.0));
        assert!(near(p.intervals.last().unwrap().1, 10.0));
        let through = p
            .intervals
            .iter()
            .filter(|(a, b)| *a < 4.0 && *b > 4.0)
            .count();
        assert_eq!(through, 1, "{:?}", p.intervals);
        // First section: H = 1, n = 2 (deviation 0.065 < 0.935), so D = (4 − 0.37) · 2 / 3.
        let d1 = (4.0 - 0.37) * 2.0 / 3.0;
        assert!(near(p.intervals[0].1, d1), "{:?}", p.intervals);
        // Without the dash point, the 10 mm line has six whole dashes.
        part.dash_points.clear();
        assert_eq!(
            dash_pattern(&part, 0.0, 10.0, &dash_506(), None, usize::MAX)
                .intervals
                .len(),
            6
        );
    }

    #[test]
    fn mid_symbols_restart_at_dash_points() {
        // End length 0: a symbol at every section end; 3 mm segments in [0, 4] and [4, 10].
        let mut mid = fence_mid();
        mid.segment = 3000.0;
        mid.end = 0.0;
        let mut part = line(&[(0.0, 0.0), (4.0, 0.0), (10.0, 0.0)]);
        part.dash_points = vec![1];
        let spots = mid_positions(&part, &mid, usize::MAX).0;
        assert_eq!(xs(&spots), [0.0, 4.0, 7.0, 10.0]);
    }

    #[test]
    fn pointed_cap_polygon() {
        // Start cap over 2 mm of a line 1 mm wide: from the tip at (0, 0) to the full half width
        // at 2 mm, plus the 0.05 mm overlap into the line [LS `createPointedLineCap`].
        let part = line(&[(0.0, 0.0), (10.0, 0.0)]);
        let ring = pointed_cap(&part, 0.0, 2.0, false, 0.5, 2.0);
        assert_eq!(ring.first(), Some(&(0.0, 0.0)));
        assert_eq!(ring.last(), Some(&(0.0, 0.0)));
        let has = |p: Pt| ring.iter().any(|q| near(q.0, p.0) && near(q.1, p.1));
        assert!(has((2.0, 0.5)) && has((2.0, -0.5)), "{ring:?}");
        assert!(has((2.05, 0.45)) && has((2.05, -0.45)), "{ring:?}");
        // End cap: tip at (10, 0).
        let ring = pointed_cap(&part, 8.0, 10.0, true, 0.5, 2.0);
        assert!(ring.iter().any(|q| near(q.0, 10.0) && near(q.1, 0.0)));
        assert!(
            ring.iter()
                .all(|q| q.1.abs() <= 0.5 + 1e-9 && q.0 >= 7.95 - 1e-9)
        );
        // At a corner inside the cap the half width grows by 1 / cos(half the angle).
        let bent = line(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)]);
        let ring = pointed_cap(&bent, 0.0, 2.0, false, 0.5, 2.0);
        let r = 0.25 * std::f64::consts::SQRT_2;
        assert!(
            ring.iter()
                .any(|q| near(q.0, 1.0 - r * 0.5f64.sqrt()) && near(q.1, r * 0.5f64.sqrt())),
            "{ring:?}"
        );
    }

    #[test]
    fn tangent_scaling_at_a_corner() {
        let points = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)];
        let (angle, scale) = tangent_scaling(&points, false, 1);
        assert!(near(angle, std::f64::consts::FRAC_PI_4));
        assert!(near(scale, std::f64::consts::SQRT_2));
        assert_eq!(tangent_scaling(&points, false, 0), (0.0, 1.0));
        let (end, scale) = tangent_scaling(&points, false, 2);
        assert!(near(end, std::f64::consts::FRAC_PI_2) && scale == 1.0);
        // Reversal: perpendicular, unbounded scale.
        let back = [(0.0, 0.0), (1.0, 0.0), (0.0, 0.0)];
        assert_eq!(tangent_scaling(&back, false, 1).1, f64::INFINITY);
    }

    #[test]
    fn parts_record_dash_points() {
        let mut coords = vec![
            MapCoord::new(0, 0),
            MapCoord::new(1000, 0),
            MapCoord::new(2000, 0),
        ];
        coords[1].flags = FLAG_DASH;
        assert_eq!(parts(&coords)[0].dash_points, [1]);
    }
}
