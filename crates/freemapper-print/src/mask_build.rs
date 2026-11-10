//! Generated masks (ADR 0047). For window orienteering the windows are circles around the controls of a course;
//! for corridor orienteering the corridor runs along a polyline the user gives, since no rule says where a corridor
//! goes. Both are holes in one rectangular outline that covers the map with a margin, so the map shows only in them.
//!
//! Radius and width are print millimetres on the sheet, like a gap: a window of 15 mm looks the same at every print
//! scale. The caller passes how many map millimetres one print millimetre is (print scale over map scale). Points
//! are map millimetres with y up, as in a `[[masks]]` table (ADR 0045).

use std::collections::{BTreeMap, HashSet};
use std::f64::consts::PI;

use crate::course::model::{ControlKind, Course, FittedEvent};
use crate::courses::StopName;
use crate::mask::{self, Budget, MAX_CHECK_STEPS, MAX_COORD_MM, Mask, MaskPolygon, Problem};
use crate::omap::{MapCoord, MapDocument};

/// Window radius when none is given (print mm): a window of 30 mm across shows about 120 m of ground at 1:4000.
pub const DEFAULT_RADIUS_MM: f64 = 15.0;
/// Corridor width when none is given (print mm): 40 m of ground at 1:4000, a street with its edges.
pub const DEFAULT_WIDTH_MM: f64 = 10.0;
/// Smallest radius or width (print mm).
pub const MIN_SIZE_MM: f64 = 1.0;
/// Largest radius or width (print mm): 200 mm across is most of an A4 sheet.
pub const MAX_SIZE_MM: f64 = 100.0;
/// Fewest vertices of a window circle.
pub const MIN_WINDOW_VERTICES: usize = 48;
/// Largest distance (print mm) between a window's edge and the circle it stands for.
pub const MAX_CHORD_ERROR_MM: f64 = 0.05;
/// Margin (map mm) of the outline around the map, the windows and the corridor.
pub const OUTLINE_MARGIN_MM: f64 = 100.0;
/// Most points of a corridor's polyline.
pub const MAX_CORRIDOR_POINTS: usize = 1000;
/// The longest mitre, as a multiple of the corridor width: from the outer to the inner corner of a turn, as SVG's
/// `stroke-miterlimit` measures it. Turns up to 120° get a pointed corner, sharper ones a bevelled one.
pub const MITRE_LIMIT: f64 = 2.0;
/// Points are written to 0.001 mm, the unit of an OOM map.
const ROUND: f64 = 1000.0;

/// A rectangle in map mm, y up: min x, min y, max x, max y.
pub type Extent = [f64; 4];

/// Why a mask could not be built. The CLI says it in the user's language.
#[derive(Debug, Clone, PartialEq)]
pub enum BuildError {
    /// A radius or width outside [`MIN_SIZE_MM`]..=[`MAX_SIZE_MM`] or not a finite number.
    SizeOutOfRange { value: f64 },
    /// `--controls` is neither a list of codes nor a range `FROM-TO`.
    BadSelection { text: String },
    /// A code that is no stop of the course; `known` lists the stops in course order, each code once.
    UnknownControl { code: String, known: Vec<StopName> },
    /// A range of control numbers outside 1..=`count`, or from above to.
    RangeOutOfCourse {
        from: usize,
        to: usize,
        count: usize,
    },
    /// A radius for a control that gets no window.
    RadiusForUnselected { code: String },
    /// Nothing to put a window around.
    NoControls,
    /// The control has no position on the map.
    NoPosition { code: String },
    /// The mask would have more points than a project file may hold ([`mask::MAX_TOTAL_POINTS`]).
    TooManyPoints,
    /// The points are not written as `x,y;x,y;…` with finite numbers.
    BadPoints { text: String },
    /// Fewer than 2 different points.
    TooFewPoints,
    /// More than [`MAX_CORRIDOR_POINTS`] points.
    TooManyCorridorPoints,
    /// A point beyond [`MAX_COORD_MM`].
    PointOutOfReach { at: [f64; 2] },
    /// At the polyline point `at` the corridor turns back on itself, or a segment next to it is too short for the
    /// width (the inner edges of the corridor would cross).
    SharpTurn { at: [f64; 2] },
    /// The corridor crosses itself near `near` (map mm, y up): where two of its edges meet.
    SelfIntersecting { near: [f64; 2] },
    /// The built mask fails the check of the project file; a gap in this module.
    Invalid(Problem),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hint = "simplify the polyline or reduce the width";
        match self {
            Self::SizeOutOfRange { value } => {
                write!(f, "{value} mm is outside {MIN_SIZE_MM} to {MAX_SIZE_MM} mm")
            }
            Self::BadSelection { text } => write!(
                f,
                "`{text}` is neither a list of control codes nor a range of control numbers"
            ),
            Self::UnknownControl { code, known } => write!(
                f,
                "the course has no control {code}; its controls are {}",
                known
                    .iter()
                    .map(StopName::code)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::RangeOutOfCourse { from, to, count } => write!(
                f,
                "the range {from}-{to} lies outside the control numbers 1 to {count}"
            ),
            Self::RadiusForUnselected { code } => {
                write!(f, "control {code} gets no window, so it takes no radius")
            }
            Self::NoControls => write!(f, "the selection has no control"),
            Self::NoPosition { code } => write!(f, "control {code} has no position on the map"),
            Self::TooManyPoints => write!(
                f,
                "the mask would have more than {} points",
                mask::MAX_TOTAL_POINTS
            ),
            Self::BadPoints { text } => {
                write!(f, "`{text}` is not a list of points x,y;x,y")
            }
            Self::TooFewPoints => write!(f, "a corridor needs at least 2 different points"),
            Self::TooManyCorridorPoints => {
                write!(f, "a corridor has at most {MAX_CORRIDOR_POINTS} points")
            }
            Self::PointOutOfReach { at } => write!(
                f,
                "the point {}, {} lies beyond the reach of an OOM map",
                at[0], at[1]
            ),
            Self::SharpTurn { at } => write!(
                f,
                "the corridor turns too sharply at {}, {} for its width; {hint}",
                at[0], at[1]
            ),
            Self::SelfIntersecting { near } => write!(
                f,
                "the corridor crosses itself near {}, {}; {hint}",
                near[0], near[1]
            ),
            Self::Invalid(problem) => write!(f, "the generated mask is not valid: {problem}"),
        }
    }
}

impl std::error::Error for BuildError {}

/// Which controls get a window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    /// Every numbered control of the course.
    All,
    /// The stops with these codes.
    Codes(Vec<String>),
    /// The controls with these numbers on the sheet, from 1 in course order (start and finish not counted), both
    /// ends included.
    Numbers { from: usize, to: usize },
}

impl Selection {
    /// Reads `--controls`: two whole numbers joined by `-` (`3-6`) are a range of control numbers, anything else a
    /// list of codes separated by commas (`31,32,35`).
    pub fn parse(text: &str) -> Result<Selection, BuildError> {
        let bad = || BuildError::BadSelection {
            text: text.to_string(),
        };
        let t = text.trim();
        let number = |s: &str| {
            (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
                .then(|| s.parse::<usize>().ok())
                .flatten()
        };
        if let Some((a, b)) = t.split_once('-')
            && let (Some(from), Some(to)) = (number(a.trim()), number(b.trim()))
        {
            return Ok(Selection::Numbers { from, to });
        }
        let codes: Vec<String> = t.split(',').map(|c| c.trim().to_string()).collect();
        if codes.iter().any(String::is_empty) {
            return Err(bad());
        }
        Ok(Selection::Codes(codes))
    }
}

/// What [`windows`] builds.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowOptions {
    pub selection: Selection,
    /// The start gets a window, whatever `selection` says (the CLI's default, ruling R13).
    pub start: bool,
    /// The finish gets a window, whatever `selection` says (the CLI's default).
    pub finish: bool,
    /// The radius of every window (print mm).
    pub radius_mm: f64,
    /// Radii (print mm) of single controls by code, in place of `radius_mm`.
    pub per_control: BTreeMap<String, f64>,
    /// Map mm per print mm: the print scale over the map scale.
    pub map_mm_per_print_mm: f64,
}

/// One window of a generated mask.
#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    /// The stop the window is around, the start and the finish by kind.
    pub stop: StopName,
    /// The centre of the control, map mm, y up.
    pub centre: [f64; 2],
    /// Print mm.
    pub radius_mm: f64,
    pub vertices: usize,
}

/// The box around every object point of `map` (map mm, y up); `None` for a map without objects.
pub fn map_extent(map: &MapDocument) -> Option<Extent> {
    let mut points = map
        .parts
        .iter()
        .flat_map(|p| &p.objects)
        .flat_map(|o| &o.coords)
        .map(written);
    let first = points.next()?;
    Some(points.fold([first[0], first[1], first[0], first[1]], grow))
}

/// A map point (µm, y down) as written in a mask (mm, y up).
fn written(c: &MapCoord) -> [f64; 2] {
    [f64::from(c.x) / 1000.0, -f64::from(c.y) / 1000.0]
}

fn grow(b: Extent, [x, y]: [f64; 2]) -> Extent {
    [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)]
}

/// `v` to 0.001 mm, without a negative zero.
fn rounded(v: f64) -> f64 {
    (v * ROUND).round() / ROUND + 0.0
}

fn size_in_range(value: f64) -> Result<(), BuildError> {
    if value.is_finite() && (MIN_SIZE_MM..=MAX_SIZE_MM).contains(&value) {
        Ok(())
    } else {
        Err(BuildError::SizeOutOfRange { value })
    }
}

/// The vertices of a window of `radius_mm` print mm: at least [`MIN_WINDOW_VERTICES`], and enough that no edge
/// lies more than [`MAX_CHORD_ERROR_MM`] inside the circle.
pub fn window_vertices(radius_mm: f64) -> usize {
    let needed = PI
        / (1.0 - MAX_CHORD_ERROR_MM / radius_mm)
            .clamp(-1.0, 1.0)
            .acos();
    MIN_WINDOW_VERTICES.max(needed.ceil() as usize)
}

/// The stops of `course` that `options` selects, in course order and each code once, with their centres.
fn selected(
    course: &Course<MapCoord>,
    event: &FittedEvent,
    options: &WindowOptions,
) -> Result<Vec<(StopName, [f64; 2])>, BuildError> {
    let numbered = crate::overprint::labels(course, event);
    let kind_of = |i: usize| {
        let stop = &course.stops[i];
        event
            .controls
            .iter()
            .find(|c| c.code == stop.control)
            .map(|c| stop.kind.unwrap_or(c.kind))
    };
    let mut wanted: HashSet<usize> = match &options.selection {
        Selection::All => numbered.iter().map(|(i, _, _)| *i).collect(),
        Selection::Numbers { from, to } => {
            let count = numbered.len();
            if *from == 0 || from > to || *to > count {
                return Err(BuildError::RangeOutOfCourse {
                    from: *from,
                    to: *to,
                    count,
                });
            }
            numbered[from - 1..*to].iter().map(|(i, _, _)| *i).collect()
        }
        Selection::Codes(codes) => {
            let mut known: Vec<StopName> = Vec::new();
            for (i, s) in course.stops.iter().enumerate() {
                if !known.iter().any(|k| k.code() == s.control) {
                    known.push(StopName::of(course, event, i));
                }
            }
            if let Some(code) = codes.iter().find(|c| !known.iter().any(|k| k.code() == *c)) {
                return Err(BuildError::UnknownControl {
                    code: code.clone(),
                    known,
                });
            }
            (0..course.stops.len())
                .filter(|&i| codes.contains(&course.stops[i].control))
                .collect()
        }
    };
    for i in 0..course.stops.len() {
        match kind_of(i) {
            Some(ControlKind::Start) if options.start => {
                wanted.insert(i);
            }
            Some(ControlKind::Finish) if options.finish => {
                wanted.insert(i);
            }
            _ => {}
        }
    }
    let mut out: Vec<(StopName, [f64; 2])> = Vec::new();
    for (i, stop) in course.stops.iter().enumerate() {
        if !wanted.contains(&i) || out.iter().any(|(c, _)| c.code() == stop.control) {
            continue;
        }
        let position = event
            .controls
            .iter()
            .find(|c| c.code == stop.control)
            .and_then(|c| c.position)
            .ok_or_else(|| BuildError::NoPosition {
                code: stop.control.clone(),
            })?;
        out.push((StopName::of(course, event, i), written(&position)));
    }
    if out.is_empty() {
        return Err(BuildError::NoControls);
    }
    Ok(out)
}

/// The window mask `id` for `course`: one circular hole around each selected control, centred on it, with the
/// radius of `options` in print mm, in one rectangular outline that covers `extent` (the map) and the windows with
/// [`OUTLINE_MARGIN_MM`]. Windows may overlap; the clip unites them (ADR 0045). Points are rounded to 0.001 mm.
pub fn windows(
    id: &str,
    course: &Course<MapCoord>,
    event: &FittedEvent,
    options: &WindowOptions,
    extent: Option<Extent>,
) -> Result<(Mask, Vec<Window>), BuildError> {
    size_in_range(options.radius_mm)?;
    for r in options.per_control.values() {
        size_in_range(*r)?;
    }
    let chosen = selected(course, event, options)?;
    if let Some(code) = options
        .per_control
        .keys()
        .find(|code| !chosen.iter().any(|(c, _)| c.code() == code.as_str()))
    {
        return Err(BuildError::RadiusForUnselected { code: code.clone() });
    }
    let mut holes = Vec::new();
    let mut list = Vec::new();
    let mut points = 4;
    for (stop, centre) in chosen {
        let radius_mm = options
            .per_control
            .get(stop.code())
            .copied()
            .unwrap_or(options.radius_mm);
        let n = window_vertices(radius_mm);
        points += n;
        if points > mask::MAX_TOTAL_POINTS {
            return Err(BuildError::TooManyPoints);
        }
        let r = radius_mm * options.map_mm_per_print_mm;
        // Counter-clockwise with y up, from east.
        let ring: Vec<[f64; 2]> = (0..n)
            .map(|k| {
                let a = 2.0 * PI * k as f64 / n as f64;
                [
                    rounded(centre[0] + r * a.cos()),
                    rounded(centre[1] + r * a.sin()),
                ]
            })
            .collect();
        holes.push(ring);
        list.push(Window {
            stop,
            centre,
            radius_mm,
            vertices: n,
        });
    }
    let mask = Mask {
        id: id.to_string(),
        polygons: vec![MaskPolygon {
            outline: outline(extent, &holes),
            holes,
        }],
    };
    checked(&mask)?;
    Ok((mask, list))
}

/// A rectangle around `extent` and every point of `rings`, [`OUTLINE_MARGIN_MM`] away from both.
fn outline(extent: Option<Extent>, rings: &[Vec<[f64; 2]>]) -> Vec<[f64; 2]> {
    let mut points = rings.iter().flatten().copied();
    let start = extent.or_else(|| points.next().map(|p| [p[0], p[1], p[0], p[1]]));
    let b = points.fold(start.unwrap_or_default(), grow);
    let m = OUTLINE_MARGIN_MM;
    let (x0, y0, x1, y1) = (
        rounded((b[0] - m).floor()),
        rounded((b[1] - m).floor()),
        rounded((b[2] + m).ceil()),
        rounded((b[3] + m).ceil()),
    );
    vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
}

/// `mask` checked as the project file checks it.
fn checked(mask: &Mask) -> Result<(), BuildError> {
    mask::check(mask, &mut Budget::new(MAX_CHECK_STEPS)).map_err(|e| BuildError::Invalid(e.problem))
}

/// Reads `--points`: `x,y` pairs separated by `;`, map mm with y up. White space around the numbers and a final
/// `;` are allowed; decimals take a point.
pub fn parse_points(text: &str) -> Result<Vec<[f64; 2]>, BuildError> {
    let bad = || BuildError::BadPoints {
        text: text.to_string(),
    };
    let mut items: Vec<&str> = text.split(';').map(str::trim).collect();
    if items.len() > 1 && items.last() == Some(&"") {
        items.pop();
    }
    items
        .iter()
        .map(|item| {
            let (x, y) = item.split_once(',').ok_or_else(bad)?;
            let value = |s: &str| {
                s.trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite())
                    .ok_or_else(bad)
            };
            Ok([value(x)?, value(y)?])
        })
        .collect()
}

/// The corridor along `points` (map mm, y up), `width_mm` print mm wide, as a hole in a rectangle that covers
/// `extent` and the corridor with [`OUTLINE_MARGIN_MM`]. The edges run half the width to either side of the
/// polyline, with flat ends and mitred turns ([`MITRE_LIMIT`]; beyond it the corner is bevelled). A corridor whose
/// edges would cross is refused, never repaired: a turn back, a segment too short for the turn at its end, or a
/// polyline that comes back near itself. Points are rounded to 0.001 mm.
pub fn corridor(
    points: &[[f64; 2]],
    width_mm: f64,
    map_mm_per_print_mm: f64,
    extent: Option<Extent>,
) -> Result<MaskPolygon, BuildError> {
    size_in_range(width_mm)?;
    if points.len() > MAX_CORRIDOR_POINTS {
        return Err(BuildError::TooManyCorridorPoints);
    }
    if let Some(at) = points
        .iter()
        .find(|p| !p.iter().all(|v| v.is_finite() && v.abs() <= MAX_COORD_MM))
    {
        return Err(BuildError::PointOutOfReach { at: *at });
    }
    let mut path: Vec<[f64; 2]> = Vec::with_capacity(points.len());
    for &p in points {
        if path.last() != Some(&p) {
            path.push(p);
        }
    }
    if path.len() < 2 {
        return Err(BuildError::TooFewPoints);
    }
    let ring = edges(&path, width_mm * map_mm_per_print_mm / 2.0)?;
    let mut ring: Vec<[f64; 2]> = ring.iter().map(|p| p.map(rounded)).collect();
    ring.dedup();
    let alone = Mask {
        id: String::new(),
        polygons: vec![MaskPolygon {
            outline: ring.clone(),
            holes: Vec::new(),
        }],
    };
    if let Err(e) = mask::check(&alone, &mut Budget::new(MAX_CHECK_STEPS)) {
        return Err(match (e.problem, e.point) {
            (Problem::SelfIntersecting, Some(i)) => BuildError::SelfIntersecting {
                near: mask::crossing(&ring).unwrap_or(ring[i]).map(rounded),
            },
            (problem, _) => BuildError::Invalid(problem),
        });
    }
    let polygon = MaskPolygon {
        outline: outline(extent, std::slice::from_ref(&ring)),
        holes: vec![ring],
    };
    checked(&Mask {
        id: String::new(),
        polygons: vec![polygon.clone()],
    })?;
    Ok(polygon)
}

/// The ring of a corridor `h` either side of `path` (at least 2 points, no two following points equal): the left
/// edge forward, then the right edge back.
fn edges(path: &[[f64; 2]], h: f64) -> Result<Vec<[f64; 2]>, BuildError> {
    let n = path.len();
    let dirs: Vec<(f64, f64)> = path
        .windows(2)
        .map(|w| {
            let (dx, dy) = (w[1][0] - w[0][0], w[1][1] - w[0][1]);
            let len = dx.hypot(dy);
            (dx / len, dy / len)
        })
        .collect();
    let lengths: Vec<f64> = path
        .windows(2)
        .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
        .collect();
    // Left normals, y up.
    let normals: Vec<(f64, f64)> = dirs.iter().map(|&(x, y)| (-y, x)).collect();
    let at = |p: [f64; 2], (nx, ny): (f64, f64), s: f64| [p[0] + s * nx, p[1] + s * ny];
    let mut left = vec![at(path[0], normals[0], h)];
    let mut right = vec![at(path[0], normals[0], -h)];
    // How far along each segment the inner corner of a turn reaches from its vertex.
    let mut reach = vec![0.0; n];
    for i in 1..n - 1 {
        let (na, nb) = (normals[i - 1], normals[i]);
        let (da, db) = (dirs[i - 1], dirs[i]);
        let dot = da.0 * db.0 + da.1 * db.1;
        let cross = da.0 * db.1 - da.1 * db.0;
        if 1.0 + dot < 1e-9 {
            return Err(BuildError::SharpTurn { at: path[i] });
        }
        // The offset lines meet at p ± h m; |m| = 1 / cos(turn / 2).
        let m = ((na.0 + nb.0) / (1.0 + dot), (na.1 + nb.1) / (1.0 + dot));
        reach[i] = h * cross.abs() / (1.0 + dot);
        // From the outer to the inner corner the mitre is 2 h |m|; the limit is MITRE_LIMIT times the width 2 h.
        let pointed = m.0.hypot(m.1) <= MITRE_LIMIT;
        let (inner, outer) = if cross > 0.0 {
            (&mut left, &mut right)
        } else {
            (&mut right, &mut left)
        };
        let s = if cross > 0.0 { 1.0 } else { -1.0 };
        inner.push(at(path[i], m, s * h));
        if pointed {
            outer.push(at(path[i], m, -s * h));
        } else {
            outer.push(at(path[i], na, -s * h));
            outer.push(at(path[i], nb, -s * h));
        }
    }
    for (j, len) in lengths.iter().enumerate() {
        if reach[j] + reach[j + 1] > *len {
            let worse = if reach[j] >= reach[j + 1] { j } else { j + 1 };
            return Err(BuildError::SharpTurn { at: path[worse] });
        }
    }
    left.push(at(path[n - 1], normals[n - 2], h));
    right.push(at(path[n - 1], normals[n - 2], -h));
    right.reverse();
    left.extend(right);
    Ok(left)
}

/// A mask id `base` that `taken` does not have yet: `base`, then `base-2`, `base-3` … Control characters become
/// `_`, and `base` is cut so that the id stays within [`mask::MAX_ID_CHARS`].
pub fn unique_id(base: &str, taken: &[&str]) -> String {
    let clean: String = base
        .chars()
        .map(|c| if c.is_control() { '_' } else { c })
        .collect();
    let cut = |s: &str, room: usize| s.chars().take(room).collect::<String>();
    let first = cut(&clean, mask::MAX_ID_CHARS);
    if !taken.contains(&first.as_str()) {
        return first;
    }
    (2..)
        .map(|n| {
            let suffix = format!("-{n}");
            format!(
                "{}{suffix}",
                cut(&clean, mask::MAX_ID_CHARS - suffix.chars().count())
            )
        })
        .find(|id| !taken.contains(&id.as_str()))
        .expect("an unused id")
}

/// The id for a window mask of `course`: `window-<course>`, made unique.
pub fn window_id(course: &str, taken: &[&str]) -> String {
    unique_id(&format!("window-{course}"), taken)
}

/// The id for a corridor: `corridor-<n>` with the smallest unused `n` from 1.
pub fn corridor_id(taken: &[&str]) -> String {
    (1..)
        .map(|n| format!("corridor-{n}"))
        .find(|id| !taken.contains(&id.as_str()))
        .expect("an unused id")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::course::model::{Control, CourseKind, CourseSource, Leg, SourceFormat, Stop};
    use crate::mask::to_internal;

    /// An event with `controls` (code, kind, x, y in map mm with y up) and one course "A" over `stops`.
    fn event(controls: &[(&str, ControlKind, f64, f64)], stops: &[&str]) -> FittedEvent {
        let um = |v: f64| (v * 1000.0).round() as i32;
        FittedEvent {
            controls: controls
                .iter()
                .map(|&(code, kind, x, y)| Control {
                    id: None,
                    code: code.into(),
                    kind,
                    position: Some(MapCoord::new(um(x), um(-y))),
                    geo: None,
                    description: None,
                })
                .collect(),
            courses: vec![Course {
                id: None,
                name: "A".into(),
                family: None,
                kind: CourseKind::Normal,
                legs: (1..stops.len())
                    .map(|_| Leg {
                        bends: vec![],
                        gaps: vec![],
                        marked_route: false,
                    })
                    .collect(),
                stops: stops
                    .iter()
                    .map(|&c| Stop {
                        control: c.into(),
                        kind: None,
                        number: None,
                        number_text_at: None,
                        circle_gaps: vec![],
                        map_flip: false,
                    })
                    .collect(),
                climb_m: None,
                length_m: None,
                print_scale_hint: None,
                course_obj_ratio: None,
                secondary_title: None,
            }],
            specials: vec![],
            source: CourseSource {
                format: SourceFormat::IofXml,
                event_name: None,
                map_scale: Some(4000),
                item_scaling: None,
            },
            notes: vec![],
        }
    }

    /// Start S, controls 31 to 36 in a row 50 mm apart, finish F.
    fn row() -> FittedEvent {
        let mut controls = vec![("S", ControlKind::Start, -50.0, 0.0)];
        let codes = ["31", "32", "33", "34", "35", "36"];
        for (i, c) in codes.iter().enumerate() {
            controls.push((c, ControlKind::Control, 50.0 * i as f64, 10.0));
        }
        controls.push(("F", ControlKind::Finish, 300.0, 0.0));
        let mut stops = vec!["S"];
        stops.extend(codes);
        stops.push("F");
        event(&controls, &stops)
    }

    fn options(selection: Selection, radius_mm: f64, k: f64) -> WindowOptions {
        WindowOptions {
            selection,
            start: false,
            finish: false,
            radius_mm,
            per_control: BTreeMap::new(),
            map_mm_per_print_mm: k,
        }
    }

    fn build(o: &WindowOptions) -> Result<(Mask, Vec<Window>), BuildError> {
        let e = row();
        windows(
            "w",
            &e.courses[0],
            &e,
            o,
            Some([-400.0, -300.0, 600.0, 300.0]),
        )
    }

    fn codes(windows: &[Window]) -> Vec<&str> {
        windows.iter().map(|w| w.stop.code()).collect()
    }

    fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
        (a[0] - b[0]).hypot(a[1] - b[1])
    }

    #[test]
    fn windows_are_circles_around_the_control_centres() {
        let (mask, list) = build(&options(Selection::All, 15.0, 1.0)).unwrap();
        assert_eq!(mask.id, "w");
        assert_eq!(mask.polygons.len(), 1);
        let polygon = &mask.polygons[0];
        assert_eq!(polygon.holes.len(), 6);
        assert_eq!(codes(&list), ["31", "32", "33", "34", "35", "36"]);
        for (hole, w) in polygon.holes.iter().zip(&list) {
            assert!(hole.len() >= MIN_WINDOW_VERTICES, "{}", hole.len());
            assert_eq!(hole.len(), w.vertices);
            for &p in hole {
                assert!((dist(p, w.centre) - 15.0).abs() <= 0.001, "{p:?} {w:?}");
            }
            // The centre is in the window, a point just outside the radius in the masked area.
            let c = w.centre;
            assert!(!mask::contains(&mask, to_internal(c)));
            assert!(mask::contains(&mask, to_internal([c[0], c[1] + 15.2])));
            assert!(!mask::contains(&mask, to_internal([c[0], c[1] + 14.9])));
        }
        // The outline covers the map extent with the margin.
        let o = &polygon.outline;
        assert_eq!(o[0], [-500.0, -400.0]);
        assert_eq!(o[2], [700.0, 400.0]);
        // The chord error stays below 0.05 mm also for large radii.
        for r in [1.0, 15.0, 40.0, 100.0] {
            let n = window_vertices(r) as f64;
            assert!(r * (1.0 - (PI / n).cos()) <= MAX_CHORD_ERROR_MM, "{r}");
        }
        assert_eq!(window_vertices(15.0), 48);
        assert!(window_vertices(100.0) > 48);
    }

    #[test]
    fn window_radius_is_in_print_mm() {
        // Map 1:4000: at 1:4000 15 print mm are 15 map mm, at 1:3000 11.25, at 1:5000 18.75.
        for (k, map_r) in [(1.0, 15.0), (0.75, 11.25), (1.25, 18.75)] {
            let (mask, list) = build(&options(Selection::All, 15.0, k)).unwrap();
            for (hole, w) in mask.polygons[0].holes.iter().zip(&list) {
                assert_eq!(w.radius_mm, 15.0);
                for &p in hole {
                    assert!((dist(p, w.centre) - map_r).abs() <= 0.001, "{k}: {p:?}");
                }
            }
        }
    }

    #[test]
    fn per_control_radius_applies() {
        let mut o = options(Selection::All, 15.0, 1.0);
        o.per_control.insert("33".into(), 25.0);
        let (mask, list) = build(&o).unwrap();
        for (hole, w) in mask.polygons[0].holes.iter().zip(&list) {
            let want = if w.stop.code() == "33" { 25.0 } else { 15.0 };
            assert_eq!(w.radius_mm, want);
            assert!((dist(hole[0], w.centre) - want).abs() <= 0.001);
        }
        // Windows of 25 and 30 mm around 33 and 34, 50 mm apart, overlap, which is allowed.
        o.per_control.insert("34".into(), 30.0);
        assert!(build(&o).is_ok());
        // A radius for a control without a window, or out of range, is refused.
        let mut o = options(Selection::Codes(vec!["31".into()]), 15.0, 1.0);
        o.per_control.insert("32".into(), 20.0);
        assert_eq!(
            build(&o),
            Err(BuildError::RadiusForUnselected { code: "32".into() })
        );
        let mut o = options(Selection::All, 15.0, 1.0);
        o.per_control.insert("31".into(), 0.5);
        assert_eq!(build(&o), Err(BuildError::SizeOutOfRange { value: 0.5 }));
        assert!(matches!(
            build(&options(Selection::All, 101.0, 1.0)),
            Err(BuildError::SizeOutOfRange { .. })
        ));
        assert!(matches!(
            build(&options(Selection::All, f64::NAN, 1.0)),
            Err(BuildError::SizeOutOfRange { .. })
        ));
    }

    #[test]
    fn controls_selection_by_list_and_range() {
        assert_eq!(
            Selection::parse("31, 32,35"),
            Ok(Selection::Codes(vec![
                "31".into(),
                "32".into(),
                "35".into()
            ]))
        );
        assert_eq!(
            Selection::parse("3-6"),
            Ok(Selection::Numbers { from: 3, to: 6 })
        );
        assert_eq!(
            Selection::parse(" 2 - 4 "),
            Ok(Selection::Numbers { from: 2, to: 4 })
        );
        // A code with a dash that is not two numbers is a code.
        assert_eq!(
            Selection::parse("31-A"),
            Ok(Selection::Codes(vec!["31-A".into()]))
        );
        assert!(matches!(
            Selection::parse("31,,32"),
            Err(BuildError::BadSelection { .. })
        ));
        assert!(matches!(
            Selection::parse(""),
            Err(BuildError::BadSelection { .. })
        ));
        // A list: course order, each once.
        let (_, list) = build(&options(
            Selection::Codes(vec!["35".into(), "31".into(), "35".into()]),
            15.0,
            1.0,
        ))
        .unwrap();
        assert_eq!(codes(&list), ["31", "35"]);
        // A range of control numbers: 3-5 are 33, 34 and 35 (the start does not count).
        let (_, list) = build(&options(Selection::Numbers { from: 3, to: 5 }, 15.0, 1.0)).unwrap();
        assert_eq!(codes(&list), ["33", "34", "35"]);
        for (from, to) in [(0, 2), (4, 3), (5, 7)] {
            assert_eq!(
                build(&options(Selection::Numbers { from, to }, 15.0, 1.0)),
                Err(BuildError::RangeOutOfCourse { from, to, count: 6 })
            );
        }
        let err = build(&options(Selection::Codes(vec!["99".into()]), 15.0, 1.0)).unwrap_err();
        assert_eq!(
            err,
            BuildError::UnknownControl {
                code: "99".into(),
                known: std::iter::once(StopName::Start("S".into()))
                    .chain(
                        ["31", "32", "33", "34", "35", "36"].map(|c| StopName::Control(c.into()))
                    )
                    .chain([StopName::Finish("F".into())])
                    .collect()
            }
        );
        // The start and the finish get their windows whatever the selection, named by kind; without them only
        // the selected controls.
        let mut o = options(Selection::Numbers { from: 1, to: 1 }, 15.0, 1.0);
        o.start = true;
        o.finish = true;
        let (_, list) = build(&o).unwrap();
        assert_eq!(codes(&list), ["S", "31", "F"]);
        assert_eq!(list[0].stop, StopName::Start("S".into()));
        assert_eq!(list[2].stop, StopName::Finish("F".into()));
        o.finish = false;
        assert_eq!(codes(&build(&o).unwrap().1), ["S", "31"]);
        let (_, list) = build(&options(Selection::All, 15.0, 1.0)).unwrap();
        assert!(!codes(&list).contains(&"S") && !codes(&list).contains(&"F"));
        // A course without controls has nothing to select but its start and finish.
        let e = event(
            &[
                ("S", ControlKind::Start, 0.0, 0.0),
                ("F", ControlKind::Finish, 50.0, 0.0),
            ],
            &["S", "F"],
        );
        assert_eq!(
            windows(
                "w",
                &e.courses[0],
                &e,
                &options(Selection::All, 15.0, 1.0),
                None
            ),
            Err(BuildError::NoControls)
        );
        let mut o = options(Selection::All, 15.0, 1.0);
        o.start = true;
        o.finish = true;
        let (_, list) = windows("w", &e.courses[0], &e, &o, None).unwrap();
        assert_eq!(codes(&list), ["S", "F"]);
    }

    #[test]
    fn generated_mask_passes_validation() {
        let mut o = options(Selection::All, 30.0, 1.0);
        o.start = true;
        o.finish = true;
        let (mask, _) = build(&o).unwrap();
        assert_eq!(
            mask::check(&mask, &mut Budget::new(MAX_CHECK_STEPS)),
            Ok(())
        );
        let polygon = corridor(
            &[[0.0, 0.0], [100.0, 0.0], [150.0, 60.0], [100.0, 120.0]],
            10.0,
            1.0,
            Some([-400.0, -300.0, 600.0, 300.0]),
        )
        .unwrap();
        let mask = Mask {
            id: "c".into(),
            polygons: vec![polygon],
        };
        assert_eq!(
            mask::check(&mask, &mut Budget::new(MAX_CHECK_STEPS)),
            Ok(())
        );
        // Without a map extent the outline covers the windows.
        let e = row();
        let (mask, _) = windows(
            "w",
            &e.courses[0],
            &e,
            &options(Selection::All, 15.0, 1.0),
            None,
        )
        .unwrap();
        assert_eq!(
            mask::check(&mask, &mut Budget::new(MAX_CHECK_STEPS)),
            Ok(())
        );
    }

    /// Whether `p` (map mm, y up) lies in the corridor window of `polygon`.
    fn in_window(polygon: &MaskPolygon, p: [f64; 2]) -> bool {
        !mask::polygon_contains(polygon, to_internal(p))
    }

    #[test]
    fn corridor_is_a_simple_polygon() {
        // A street with a right angle, a gentle bend and a sharp one past the mitre limit (bevelled).
        let path = [
            [0.0, 0.0],
            [100.0, 0.0],
            [100.0, 80.0],
            [140.0, 100.0],
            [60.0, 120.0],
        ];
        let polygon = corridor(&path, 10.0, 1.0, None).unwrap();
        assert_eq!(polygon.holes.len(), 1);
        let ring = &polygon.holes[0];
        // Two flat ends of 2 points, a pointed corner on each side of three turns, one bevel point more.
        assert_eq!(ring.len(), 2 * 5 + 1, "{ring:?}");
        // Along the polyline the map shows; 5.2 mm beside a straight part it does not.
        for w in path.windows(2) {
            for t in [0.1, 0.5, 0.9] {
                let p = [
                    w[0][0] + t * (w[1][0] - w[0][0]),
                    w[0][1] + t * (w[1][1] - w[0][1]),
                ];
                assert!(in_window(&polygon, p), "{p:?}");
            }
        }
        assert!(!in_window(&polygon, [50.0, 5.2]));
        assert!(!in_window(&polygon, [50.0, -5.2]));
        assert!(in_window(&polygon, [50.0, 4.8]));
        // The right angle is pointed on the outside: (105, -5) is in, flat ends cut at x = 0.
        assert!(in_window(&polygon, [104.8, -4.8]));
        assert!(!in_window(&polygon, [-0.2, 0.0]));
        // A path that crosses itself is refused, not repaired.
        let err = corridor(
            &[
                [0.0, 0.0],
                [100.0, 0.0],
                [100.0, 50.0],
                [50.0, 50.0],
                [50.0, -50.0],
            ],
            10.0,
            1.0,
            None,
        )
        .unwrap_err();
        assert!(
            matches!(err, BuildError::SelfIntersecting { .. }),
            "{err:?}"
        );
        assert!(err.to_string().contains("reduce the width"), "{err}");
        // The message names where two edges of the corridor cross, within a width of where the line crosses itself
        // (66.7, 0), not the first point of one of the crossing edges, which can lie far away.
        let err = corridor(
            &[[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [50.0, -50.0]],
            10.0,
            1.0,
            None,
        )
        .unwrap_err();
        let BuildError::SelfIntersecting { near } = err else {
            panic!("{err:?}");
        };
        assert_eq!(near, [73.603, 5.0]);
        assert!((near[0] - 200.0 / 3.0).hypot(near[1]) < 10.0, "{near:?}");
        // Points: at least 2 different, finite, within reach.
        assert_eq!(
            corridor(&[[1.0, 1.0], [1.0, 1.0]], 10.0, 1.0, None),
            Err(BuildError::TooFewPoints)
        );
        assert!(matches!(
            corridor(&[[0.0, 0.0], [3_000_000.0, 0.0]], 10.0, 1.0, None),
            Err(BuildError::PointOutOfReach { .. })
        ));
    }

    #[test]
    fn corridor_with_a_sharp_turn_refused() {
        // Straight back: refused at the turning point.
        assert_eq!(
            corridor(&[[0.0, 0.0], [100.0, 0.0], [20.0, 0.0]], 10.0, 1.0, None),
            Err(BuildError::SharpTurn { at: [100.0, 0.0] })
        );
        // A hairpin whose second leg is shorter than the turn needs at this width.
        let hairpin = [[0.0, 0.0], [100.0, 0.0], [90.0, 8.0]];
        let err = corridor(&hairpin, 10.0, 1.0, None).unwrap_err();
        assert_eq!(err, BuildError::SharpTurn { at: [100.0, 0.0] });
        assert!(err.to_string().contains("simplify the polyline"), "{err}");
        // The same turn is fine with a narrow corridor, and bevelled.
        let polygon = corridor(&hairpin, 1.0, 1.0, None).unwrap();
        assert_eq!(polygon.holes[0].len(), 2 * 3 + 1);
        // A right angle with a leg shorter than half the width.
        assert!(matches!(
            corridor(&[[0.0, 0.0], [100.0, 0.0], [100.0, 3.0]], 10.0, 1.0, None),
            Err(BuildError::SharpTurn { .. })
        ));
    }

    #[test]
    fn corridor_width_applies() {
        // 10 print mm at 1:4000 on a 1:4000 map: 5 map mm to each side; at 1:3000 3.75.
        for (k, half) in [(1.0, 5.0), (0.75, 3.75)] {
            let polygon = corridor(&[[0.0, 0.0], [100.0, 0.0]], 10.0, k, None).unwrap();
            let mut ys: Vec<f64> = polygon.holes[0].iter().map(|p| p[1]).collect();
            ys.sort_by(f64::total_cmp);
            ys.dedup();
            assert_eq!(ys, [-half, half], "{k}");
            assert!(in_window(&polygon, [50.0, half - 0.1]));
            assert!(!in_window(&polygon, [50.0, half + 0.1]));
        }
        assert!(matches!(
            corridor(&[[0.0, 0.0], [100.0, 0.0]], 0.0, 1.0, None),
            Err(BuildError::SizeOutOfRange { .. })
        ));
    }

    #[test]
    fn points_are_read_as_x_comma_y() {
        assert_eq!(
            parse_points("1.5,2; -3,4 ;5 , -6;"),
            Ok(vec![[1.5, 2.0], [-3.0, 4.0], [5.0, -6.0]])
        );
        for bad in ["1,2;3", "1;2", "a,b", "1,2,3;4,5", "inf,1;2,3", ""] {
            assert!(
                matches!(parse_points(bad), Err(BuildError::BadPoints { .. })),
                "{bad}"
            );
        }
    }

    #[test]
    fn ids_are_made_unique() {
        assert_eq!(window_id("A", &[]), "window-A");
        assert_eq!(window_id("A", &["window-A", "window-A-2"]), "window-A-3");
        assert_eq!(corridor_id(&["corridor-1", "corridor-3"]), "corridor-2");
        let long = "x".repeat(100);
        let id = window_id(&long, &[]);
        assert_eq!(id.chars().count(), mask::MAX_ID_CHARS);
        let again = window_id(&long, &[id.as_str()]);
        assert!(again.ends_with("-2") && again.chars().count() == mask::MAX_ID_CHARS);
        assert_eq!(window_id("a\nb", &[]), "window-a_b");
    }
}
