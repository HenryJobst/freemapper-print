//! The IOF control description symbols (columns C to H and the instruction rows) and their German and English
//! texts, from Purple Pen's `symbols.xml` (ADR 0036).
//!
//! The file is `src/PurplePenCore/symbols.xml` of GitHub `petergolde/PurplePen`, commit
//! `201417f97bb69302e3e0db123f428870e0914515` (2026-11-07), embedded unchanged; `SymbolDB.cs` of the same
//! commit reads and draws it, and this reader follows it.
//!
//! - The root `symbols` holds `language` and `symbol` elements. A `symbol` has a `kind` (the column it
//!   usually goes in: `C`..`H`; `W`..`Z` and `V` are instruction rows, `@` and `A` are texts only), an `id` (the
//!   IOF 2004 number Purple Pen stores in a `.ppen`, also for 2018 descriptions) and an optional `standard`,
//!   a comma list of "2004" and "2018". Without it the symbol belongs to every standard. An id can appear
//!   once per standard; a lookup takes the first entry of the id that belongs to the standard.
//! - Shapes: `lines` (open polyline), `polygon` (closed outline), `filled-polygon`, `circle` (outline),
//!   `filled-circle`, `beziers` (open cubic chain, 3N+1 points) and `filled-beziers`, each with `point`
//!   children. `thickness` is the pen width, `radius` the radius to the middle of the pen; `ends` is round
//!   (default) or flat, `corners` round (default) or sharp (a miter join). Fills use the even-odd rule.
//! - A symbol box is 200 units square with its centre at (0, 0) and y up; kinds from `T` up are eight boxes
//!   wide (1600 units). This module turns that into cells: x right and y down, 0..1 for a square symbol and
//!   0..8 by 0..1 for a wide one. Widths and radii are in cells too. Beziers become polylines within
//!   [`FLATNESS`] of the curve.
//! - Texts: of a symbol's `text` elements in a language, `SymbolDB.FindBestText` takes the first singular
//!   one without a `case`, ignoring gender; without one in the language it takes English. This module does
//!   the same for German and English.
//!
//! The parser has the guards of the course readers (ADR 0019): no DOCTYPE, only the predefined entities,
//! nesting at most [`MAX_DEPTH`], at most [`MAX_POINTS_PER_SYMBOL`] points per symbol, and finite
//! coordinates inside the box. Unknown elements are skipped, as Purple Pen does.

use std::collections::HashMap;
use std::sync::OnceLock;

use quick_xml::events::BytesStart;

use crate::course::CourseError;
use crate::course::xml::{Handler, attr, drive, local, number};
use crate::project::SheetLang;
use crate::render::geometry::Pt;

/// The symbol file, embedded unchanged (see `data/README.md`).
const SYMBOLS_XML: &str = include_str!("../../data/purple-pen-symbols.xml");

/// Greatest distance between a bezier and the polyline that replaces it, in cells.
pub const FLATNESS: f64 = 0.005;
/// Most points one symbol may have, read and after flattening.
pub const MAX_POINTS_PER_SYMBOL: usize = 4_096;
/// Deepest XML nesting the symbol reader accepts (the real file nests four levels).
pub const MAX_DEPTH: usize = 16;
/// Most symbols in the file.
const MAX_SYMBOLS: usize = 2_000;
/// Most `text` elements in one symbol.
const MAX_TEXTS_PER_SYMBOL: usize = 1_000;
/// Longest symbol id and text, in characters.
const MAX_ID_LEN: usize = 64;
const MAX_TEXT_LEN: usize = 1_000;
/// Largest coordinate, width or radius, in Purple Pen units (a wide symbol reaches ±800).
const MAX_UNITS: f64 = 1_000.0;
/// Deepest subdivision of one bezier segment.
const MAX_BEZIER_DEPTH: u32 = 16;
/// Purple Pen units per cell.
const UNITS_PER_CELL: f64 = 200.0;

/// A description standard of the IOF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Standard {
    Iof2004,
    Iof2018,
    Iof2024,
}

impl Standard {
    /// The standard a course file names ("2004", "2018", "2024"); `None` for any other value.
    pub fn from_description(s: &str) -> Option<Standard> {
        match s.trim() {
            "2004" => Some(Standard::Iof2004),
            "2018" => Some(Standard::Iof2018),
            "2024" => Some(Standard::Iof2024),
            _ => None,
        }
    }

    /// The `standard` value of Purple Pen's file that defines the symbols of this standard. Purple Pen knows
    /// only 2004 and 2018; 2024 kept the 2018 symbols.
    fn purple_pen(self) -> &'static str {
        match self {
            Standard::Iof2004 => "2004",
            Standard::Iof2018 | Standard::Iof2024 => "2018",
        }
    }
}

/// One stroked line of a symbol, in cells.
#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    pub points: Vec<Pt>,
    /// Pen width in cells.
    pub width: f64,
    /// The last point joins the first (a Purple Pen `polygon`).
    pub closed: bool,
    /// Round line ends; otherwise flat (butt).
    pub round_ends: bool,
    /// Round corners; otherwise sharp (miter).
    pub round_corners: bool,
}

/// A circle of a symbol, in cells.
#[derive(Debug, Clone, PartialEq)]
pub struct Circle {
    pub centre: Pt,
    /// Radius to the middle of the pen for an outline, to the edge for a disc.
    pub radius: f64,
    /// Pen width of an outline; `None` for a filled disc.
    pub stroke: Option<f64>,
}

/// The drawing of one symbol in cells: x right, y down, (0, 0) the top left corner of the box.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SymbolGeometry {
    /// Purple Pen's kind: the column the symbol usually goes in (`C`..`H`), `V`..`Z` for instruction rows,
    /// `@` and `A` for texts without a drawing.
    pub kind: char,
    /// Width of the box in cells: 1, or 8 for an instruction row. The height is always 1.
    pub width: f64,
    pub strokes: Vec<Stroke>,
    /// Filled polygons, closed implicitly, even-odd rule.
    pub fills: Vec<Vec<Pt>>,
    pub circles: Vec<Circle>,
}

impl SymbolGeometry {
    /// Whether the symbol has nothing to draw (a text-only entry).
    pub fn is_empty(&self) -> bool {
        self.strokes.is_empty() && self.fills.is_empty() && self.circles.is_empty()
    }
}

#[derive(Debug)]
struct Entry {
    id: String,
    /// Purple Pen standards the entry belongs to; `None` for all.
    standards: Option<Vec<String>>,
    geometry: SymbolGeometry,
    text_de: Option<String>,
    text_en: Option<String>,
}

impl Entry {
    fn in_standard(&self, std: &str) -> bool {
        self.standards
            .as_ref()
            .is_none_or(|s| s.iter().any(|x| x == std))
    }
}

#[derive(Debug)]
struct SymbolDb {
    entries: Vec<Entry>,
    by_id: HashMap<String, Vec<usize>>,
}

static DB: OnceLock<Result<SymbolDb, String>> = OnceLock::new();

/// The parsed embedded file; `None` only if it does not parse, which `all_symbols_parse` rules out.
fn db() -> Option<&'static SymbolDb> {
    DB.get_or_init(|| parse(SYMBOLS_XML)).as_ref().ok()
}

/// The symbol `id` (as a course file stores it, e.g. "1.10") as `standard` defines it; `None` if the id is
/// unknown or not part of that standard.
pub fn symbol(id: &str, standard: Standard) -> Option<&'static SymbolGeometry> {
    let db = db()?;
    db.by_id
        .get(id)?
        .iter()
        .map(|&i| &db.entries[i])
        .find(|e| e.in_standard(standard.purple_pen()))
        .map(|e| &e.geometry)
}

/// The description text of symbol `id` in `lang`, with placeholders such as `{0}` left in. Every version of
/// an id has the same texts in Purple Pen's file, so the text does not depend on the standard.
pub fn symbol_text(id: &str, lang: SheetLang) -> Option<&'static str> {
    let db = db()?;
    let e = &db.entries[*db.by_id.get(id)?.first()?];
    match lang {
        SheetLang::De => e.text_de.as_deref(),
        SheetLang::En => e.text_en.as_deref(),
    }
}

fn parse(src: &str) -> Result<SymbolDb, String> {
    let mut r = Reader::default();
    drive(src.as_bytes(), &mut r).map_err(message)?;
    if !r.seen_root {
        return Err("no <symbols> element".into());
    }
    let mut by_id: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, e) in r.entries.iter().enumerate() {
        by_id.entry(e.id.clone()).or_default().push(i);
    }
    Ok(SymbolDb {
        entries: r.entries,
        by_id,
    })
}

fn message(e: CourseError) -> String {
    match e {
        CourseError::Xml(s) | CourseError::Limit(s) | CourseError::Unsupported(s) => s,
        CourseError::Io(e) => e.to_string(),
    }
}

fn bad(msg: impl Into<String>) -> CourseError {
    CourseError::Xml(msg.into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShapeKind {
    Lines,
    Polygon,
    FilledPolygon,
    Circle,
    FilledCircle,
    Beziers,
    FilledBeziers,
}

impl ShapeKind {
    fn from_name(name: &str) -> Option<ShapeKind> {
        Some(match name {
            "lines" => ShapeKind::Lines,
            "polygon" => ShapeKind::Polygon,
            "filled-polygon" => ShapeKind::FilledPolygon,
            "circle" => ShapeKind::Circle,
            "filled-circle" => ShapeKind::FilledCircle,
            "beziers" => ShapeKind::Beziers,
            "filled-beziers" => ShapeKind::FilledBeziers,
            _ => return None,
        })
    }
}

struct Shape {
    kind: ShapeKind,
    thickness: f64,
    radius: f64,
    round_ends: bool,
    round_corners: bool,
    /// Points in Purple Pen units, y up.
    points: Vec<Pt>,
}

struct TextCandidate {
    lang: String,
    plural: bool,
    has_case: bool,
    text: String,
}

struct SymbolBuilder {
    id: String,
    standards: Option<Vec<String>>,
    geometry: SymbolGeometry,
    texts: Vec<TextCandidate>,
    /// Points read, and points in the geometry; each is capped at `MAX_POINTS_PER_SYMBOL`.
    read_points: usize,
    out_points: usize,
}

impl SymbolBuilder {
    /// A point in cells from Purple Pen units.
    fn cell(&self, p: Pt) -> Pt {
        (
            (p.0 + self.geometry.width * UNITS_PER_CELL / 2.0) / UNITS_PER_CELL,
            (UNITS_PER_CELL / 2.0 - p.1) / UNITS_PER_CELL,
        )
    }

    fn spend(&mut self, n: usize) -> Result<(), CourseError> {
        self.out_points += n;
        if self.out_points > MAX_POINTS_PER_SYMBOL {
            return Err(too_many_points(&self.id));
        }
        Ok(())
    }

    fn add(&mut self, s: Shape) -> Result<(), CourseError> {
        let n = s.points.len();
        let need = |min: usize, exact: bool| -> Result<(), CourseError> {
            if n < min || (exact && n != min) {
                return Err(bad(format!(
                    "symbol {}: a {:?} needs {}{min} points, has {n}",
                    self.id,
                    s.kind,
                    if exact { "" } else { "at least " }
                )));
            }
            Ok(())
        };
        let w = s.thickness / UNITS_PER_CELL;
        match s.kind {
            ShapeKind::Lines | ShapeKind::Polygon => {
                need(if s.kind == ShapeKind::Lines { 2 } else { 3 }, false)?;
                self.spend(n)?;
                let points = s.points.iter().map(|&p| self.cell(p)).collect();
                self.geometry.strokes.push(Stroke {
                    points,
                    width: w,
                    closed: s.kind == ShapeKind::Polygon,
                    round_ends: s.round_ends,
                    round_corners: s.round_corners,
                });
            }
            ShapeKind::FilledPolygon => {
                need(3, false)?;
                self.spend(n)?;
                let points = s.points.iter().map(|&p| self.cell(p)).collect();
                self.geometry.fills.push(points);
            }
            ShapeKind::Circle | ShapeKind::FilledCircle => {
                need(1, true)?;
                self.spend(1)?;
                let centre = self.cell(s.points[0]);
                self.geometry.circles.push(Circle {
                    centre,
                    radius: s.radius / UNITS_PER_CELL,
                    stroke: (s.kind == ShapeKind::Circle).then_some(w),
                });
            }
            ShapeKind::Beziers | ShapeKind::FilledBeziers => {
                if n < 4 || !(n - 1).is_multiple_of(3) {
                    return Err(bad(format!(
                        "symbol {}: a bezier chain needs 3N+1 points, has {n}",
                        self.id
                    )));
                }
                let ctrl: Vec<Pt> = s.points.iter().map(|&p| self.cell(p)).collect();
                let mut out = Vec::new();
                let mut budget = MAX_POINTS_PER_SYMBOL.saturating_sub(self.out_points);
                flatten(&ctrl, &mut out, &mut budget).map_err(|()| too_many_points(&self.id))?;
                self.spend(out.len())?;
                if s.kind == ShapeKind::Beziers {
                    self.geometry.strokes.push(Stroke {
                        points: out,
                        width: w,
                        closed: false,
                        round_ends: s.round_ends,
                        round_corners: true,
                    });
                } else {
                    self.geometry.fills.push(out);
                }
            }
        }
        Ok(())
    }

    fn finish(self) -> Result<Entry, CourseError> {
        if self.texts.is_empty() {
            return Err(bad(format!("symbol {} has no text", self.id)));
        }
        Ok(Entry {
            text_de: best_text(&self.texts, "de"),
            text_en: best_text(&self.texts, "en"),
            id: self.id,
            standards: self.standards,
            geometry: self.geometry,
        })
    }
}

fn too_many_points(id: &str) -> CourseError {
    CourseError::Limit(format!(
        "symbol {id} has more than {MAX_POINTS_PER_SYMBOL} points"
    ))
}

/// Purple Pen's `FindBestText` for `lang` without gender and case: the language before English before any
/// other, singular before plural, no case before a case, then file order.
fn best_text(texts: &[TextCandidate], lang: &str) -> Option<String> {
    texts
        .iter()
        .min_by_key(|t| {
            let l = match (t.lang == lang, t.lang == "en") {
                (true, _) => 0,
                (false, true) => 50,
                (false, false) => 100,
            };
            l + if t.plural { 10 } else { 0 } + u32::from(t.has_case)
        })
        .map(|t| t.text.clone())
}

/// Appends the polyline for a chain of cubic beziers (3N+1 points) to `out`, starting with its first point.
/// `budget` is the number of points still allowed; `Err` when it runs out.
fn flatten(ctrl: &[Pt], out: &mut Vec<Pt>, budget: &mut usize) -> Result<(), ()> {
    push(out, ctrl[0], budget)?;
    for i in (0..ctrl.len() - 1).step_by(3) {
        cubic(
            [ctrl[i], ctrl[i + 1], ctrl[i + 2], ctrl[i + 3]],
            0,
            out,
            budget,
        )?;
    }
    Ok(())
}

fn push(out: &mut Vec<Pt>, p: Pt, budget: &mut usize) -> Result<(), ()> {
    *budget = budget.checked_sub(1).ok_or(())?;
    out.push(p);
    Ok(())
}

/// The curve lies in the hull of its control points, so when both inner points are within `FLATNESS` of the
/// chord, so is the curve.
fn cubic(c: [Pt; 4], depth: u32, out: &mut Vec<Pt>, budget: &mut usize) -> Result<(), ()> {
    let [p0, p1, p2, p3] = c;
    if depth >= MAX_BEZIER_DEPTH
        || (seg_dist(p1, p0, p3) <= FLATNESS && seg_dist(p2, p0, p3) <= FLATNESS)
    {
        return push(out, p3, budget);
    }
    let mid = |a: Pt, b: Pt| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let (a, b, c2) = (mid(p0, p1), mid(p1, p2), mid(p2, p3));
    let (d, e) = (mid(a, b), mid(b, c2));
    let m = mid(d, e);
    cubic([p0, a, d, m], depth + 1, out, budget)?;
    cubic([m, e, c2, p3], depth + 1, out, budget)
}

/// Distance from `p` to the segment `a`–`b`.
fn seg_dist(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (x, y) = (a.0 + t * dx, a.1 + t * dy);
    ((p.0 - x).powi(2) + (p.1 - y).powi(2)).sqrt()
}

/// The streaming reader. Each open element is on `stack` with whether this reader uses it; skipped
/// elements and everything inside them are ignored.
#[derive(Default)]
struct Reader {
    stack: Vec<(String, bool)>,
    seen_root: bool,
    entries: Vec<Entry>,
    symbol: Option<SymbolBuilder>,
    shape: Option<Shape>,
    text: Option<TextCandidate>,
}

impl Reader {
    /// The parent element, if this reader uses it.
    fn active_parent(&self) -> Option<&str> {
        match self.stack.last() {
            Some((n, true)) => Some(n),
            _ => None,
        }
    }

    fn begin_symbol(&mut self, e: &BytesStart<'_>) -> Result<(), CourseError> {
        if self.entries.len() >= MAX_SYMBOLS {
            return Err(CourseError::Limit(format!(
                "more than {MAX_SYMBOLS} symbols"
            )));
        }
        let kind = attr(e, "kind")?
            .and_then(|k| k.chars().next())
            .ok_or_else(|| bad("a symbol without kind"))?;
        let id = attr(e, "id")?
            .filter(|i| !i.is_empty())
            .ok_or_else(|| bad("a symbol without id"))?;
        if id.chars().count() > MAX_ID_LEN {
            return Err(CourseError::Limit(format!(
                "symbol id longer than {MAX_ID_LEN} characters"
            )));
        }
        let standards = attr(e, "standard")?
            .filter(|s| !s.is_empty())
            .map(|s| s.split(',').map(|x| x.trim().to_string()).collect());
        self.symbol = Some(SymbolBuilder {
            id,
            standards,
            geometry: SymbolGeometry {
                kind,
                width: if kind >= 'T' { 8.0 } else { 1.0 },
                ..Default::default()
            },
            texts: Vec::new(),
            read_points: 0,
            out_points: 0,
        });
        Ok(())
    }

    fn begin_text(&mut self, e: &BytesStart<'_>) -> Result<(), CourseError> {
        let lang = attr(e, "lang")?.ok_or_else(|| bad("a symbol text without lang"))?;
        let plural = match attr(e, "plural")?.as_deref().map(str::trim) {
            None | Some("false" | "0") => false,
            Some("true" | "1") => true,
            Some(other) => return Err(bad(format!("bad plural value {other:?}"))),
        };
        let has_case = attr(e, "case")?.is_some_and(|c| !c.is_empty());
        self.text = Some(TextCandidate {
            lang,
            plural,
            has_case,
            text: String::new(),
        });
        Ok(())
    }

    fn begin_shape(&mut self, kind: ShapeKind, e: &BytesStart<'_>) -> Result<(), CourseError> {
        let size = |name: &str, required: bool| -> Result<f64, CourseError> {
            match attr(e, name)? {
                Some(v) => {
                    let v = number(&v, name)?;
                    if !(0.0..=MAX_UNITS).contains(&v) {
                        return Err(bad(format!("{name} {v} out of range")));
                    }
                    Ok(v)
                }
                None if required => Err(bad(format!("a symbol shape without {name}"))),
                None => Ok(0.0),
            }
        };
        let stroked = matches!(
            kind,
            ShapeKind::Lines | ShapeKind::Polygon | ShapeKind::Circle | ShapeKind::Beziers
        );
        let round = |name: &str, other: &str| -> Result<bool, CourseError> {
            match attr(e, name)?.as_deref() {
                None | Some("round") => Ok(true),
                Some(v) if v == other => Ok(false),
                Some(v) => Err(bad(format!("bad {name} value {v:?}"))),
            }
        };
        let round_corners = round("corners", "sharp")?;
        self.shape = Some(Shape {
            kind,
            thickness: size("thickness", stroked)?,
            radius: size(
                "radius",
                matches!(kind, ShapeKind::Circle | ShapeKind::FilledCircle),
            )?,
            // Purple Pen gives a polygon outline round ends only with round corners.
            round_ends: if kind == ShapeKind::Polygon {
                round_corners
            } else {
                round("ends", "flat")?
            },
            round_corners,
            points: Vec::new(),
        });
        Ok(())
    }

    fn point(&mut self, e: &BytesStart<'_>) -> Result<(), CourseError> {
        let coord = |name: &str| -> Result<f64, CourseError> {
            let v = attr(e, name)?.ok_or_else(|| bad(format!("a point without {name}")))?;
            let v = number(&v, name)?;
            if v.abs() > MAX_UNITS {
                return Err(bad(format!("point {name} {v} out of range")));
            }
            Ok(v)
        };
        let p = (coord("x")?, coord("y")?);
        let (Some(sym), Some(shape)) = (self.symbol.as_mut(), self.shape.as_mut()) else {
            return Ok(());
        };
        sym.read_points += 1;
        if sym.read_points > MAX_POINTS_PER_SYMBOL {
            return Err(too_many_points(&sym.id));
        }
        shape.points.push(p);
        Ok(())
    }
}

impl Handler for Reader {
    fn begin(&mut self, e: &BytesStart<'_>) -> Result<(), CourseError> {
        if self.stack.len() >= MAX_DEPTH {
            return Err(CourseError::Limit(format!(
                "nesting too deep: more than {MAX_DEPTH} levels of elements"
            )));
        }
        let name = local(e);
        let used = match (self.stack.is_empty(), self.active_parent(), name.as_str()) {
            (true, _, "symbols") => {
                self.seen_root = true;
                true
            }
            (true, _, other) => {
                return Err(bad(format!(
                    "root element is <{other}>, expected <symbols>"
                )));
            }
            (_, Some("symbols"), "symbol") => {
                self.begin_symbol(e)?;
                true
            }
            (_, Some("symbol"), "text") => {
                self.begin_text(e)?;
                true
            }
            (_, Some("symbol"), shape) => match ShapeKind::from_name(shape) {
                Some(kind) => {
                    self.begin_shape(kind, e)?;
                    true
                }
                None => false,
            },
            (_, Some(parent), "point") if ShapeKind::from_name(parent).is_some() => {
                self.point(e)?;
                true
            }
            _ => false,
        };
        self.stack.push((name, used));
        Ok(())
    }

    fn end(&mut self) -> Result<(), CourseError> {
        let Some((name, true)) = self.stack.pop() else {
            return Ok(());
        };
        match name.as_str() {
            "text" => {
                if let (Some(t), Some(sym)) = (self.text.take(), self.symbol.as_mut()) {
                    if t.text.chars().count() > MAX_TEXT_LEN {
                        return Err(CourseError::Limit(format!(
                            "symbol {} has a text longer than {MAX_TEXT_LEN} characters",
                            sym.id
                        )));
                    }
                    if sym.texts.len() >= MAX_TEXTS_PER_SYMBOL {
                        return Err(CourseError::Limit(format!(
                            "symbol {} has more than {MAX_TEXTS_PER_SYMBOL} texts",
                            sym.id
                        )));
                    }
                    sym.texts.push(t);
                }
            }
            "symbol" => {
                if let Some(sym) = self.symbol.take() {
                    self.entries.push(sym.finish()?);
                }
            }
            other if ShapeKind::from_name(other).is_some() => {
                if let (Some(s), Some(sym)) = (self.shape.take(), self.symbol.as_mut()) {
                    sym.add(s)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn text(&mut self, s: &str) {
        if let (Some(("text", true)), Some(t)) = (
            self.stack.last().map(|(n, u)| (n.as_str(), *u)),
            self.text.as_mut(),
        ) {
            // Longer texts are cut here and refused when the element ends.
            if t.text.chars().count() <= MAX_TEXT_LEN {
                t.text.push_str(s);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Pt, b: Pt) -> bool {
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
    }

    #[test]
    fn all_symbols_parse() {
        let expected = SYMBOLS_XML.matches("<symbol ").count();
        assert_eq!(expected, 201);
        let db = db().expect("embedded symbol file parses");
        assert_eq!(db.entries.len(), expected);
    }

    #[test]
    fn known_symbol_geometry() {
        // 1.1 terrace: two beziers of thickness 12.5; the first runs from (-60,-70) via (-50,-30), (-50,30) to
        // (-60,70) in Purple Pen's box (-100..100, y up).
        let g = symbol("1.1", Standard::Iof2018).expect("1.1 exists");
        assert_eq!(g.kind, 'D');
        assert_eq!(g.width, 1.0);
        assert!(g.fills.is_empty() && g.circles.is_empty());
        assert_eq!(g.strokes.len(), 2);
        let s = &g.strokes[0];
        assert!((s.width - 0.0625).abs() < 1e-12);
        assert!(!s.closed && s.round_ends && s.round_corners);
        assert!(close(s.points[0], (0.2, 0.85)), "{:?}", s.points[0]);
        assert!(close(*s.points.last().unwrap(), (0.2, 0.15)));
        // Every point of the curve lies within the flattening tolerance of the polyline.
        let curve = |t: f64| {
            let (p0, p1, p2, p3) = ((0.2, 0.85), (0.25, 0.65), (0.25, 0.35), (0.2, 0.15));
            let u = 1.0 - t;
            let b = |a: f64, b: f64, c: f64, d: f64| {
                u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
            };
            (b(p0.0, p1.0, p2.0, p3.0), b(p0.1, p1.1, p2.1, p3.1))
        };
        for i in 0..=100 {
            let p = curve(i as f64 / 100.0);
            let d = s
                .points
                .windows(2)
                .map(|w| seg_dist(p, w[0], w[1]))
                .fold(f64::INFINITY, f64::min);
            assert!(d <= FLATNESS + 1e-9, "t={i}%: {d}");
        }
        assert!(s.points.len() <= 32, "{} points", s.points.len());

        // 0.1N, northern: a straight shaft and a sharp-cornered arrow head.
        let n = symbol("0.1N", Standard::Iof2004).expect("0.1N exists");
        assert_eq!(n.strokes[0].points, vec![(0.5, 0.825), (0.5, 0.175)]);
        assert!(!n.strokes[1].round_corners && n.strokes[1].round_ends);

        // 1.10 knoll differs by standard: a disc of radius 23 (2004) or 30 (2018).
        let r = |s| symbol("1.10", s).expect("1.10").circles[0].clone();
        assert_eq!(r(Standard::Iof2004).radius, 23.0 / 200.0);
        assert_eq!(r(Standard::Iof2018).radius, 30.0 / 200.0);
        assert_eq!(r(Standard::Iof2024).radius, 30.0 / 200.0);
        assert_eq!(r(Standard::Iof2018).stroke, None);
        assert_eq!(r(Standard::Iof2018).centre, (0.5, 0.5));
        // 11.7 bend is column G in 2004 and column F in 2018; 12.3 radio exists only in 2004, 5.25 only from
        // 2018.
        assert_eq!(symbol("11.7", Standard::Iof2004).map(|g| g.kind), Some('G'));
        assert_eq!(symbol("11.7", Standard::Iof2018).map(|g| g.kind), Some('F'));
        assert!(symbol("12.3", Standard::Iof2004).is_some());
        assert!(symbol("12.3", Standard::Iof2018).is_none());
        assert!(symbol("5.25", Standard::Iof2004).is_none());
        assert!(symbol("5.25", Standard::Iof2024).is_some());
        assert!(symbol("no-such-id", Standard::Iof2018).is_none());

        // 13.1, an instruction row, is eight cells wide; the circle is at x = -700.
        let w = symbol("13.1", Standard::Iof2018).expect("13.1");
        assert_eq!(w.width, 8.0);
        assert_eq!(w.circles[0].centre, (0.5, 0.5));
        assert_eq!(w.circles[0].stroke, Some(0.05));

        // Filled parts stay apart from strokes: 5.25 has one bezier outline, one disc and one polygon.
        let f = symbol("5.25", Standard::Iof2018).expect("5.25");
        assert_eq!(f.strokes.len(), 2);
        assert!(f.strokes[1].closed);
        assert_eq!(f.circles.len(), 1);
        assert!(f.fills.is_empty());
    }

    #[test]
    fn every_symbol_stays_in_its_box_and_under_the_cap() {
        let db = db().expect("parses");
        for e in &db.entries {
            let g = &e.geometry;
            let pts: Vec<Pt> = g
                .strokes
                .iter()
                .flat_map(|s| s.points.iter().copied())
                .chain(g.fills.iter().flatten().copied())
                .chain(g.circles.iter().map(|c| c.centre))
                .collect();
            assert!(pts.len() <= MAX_POINTS_PER_SYMBOL, "{}", e.id);
            for p in pts {
                assert!(
                    (-0.01..=g.width + 0.01).contains(&p.0) && (-0.01..=1.01).contains(&p.1),
                    "{} {:?}",
                    e.id,
                    p
                );
            }
        }
    }

    #[test]
    fn standards_from_the_file() {
        assert_eq!(Standard::from_description("2004"), Some(Standard::Iof2004));
        assert_eq!(
            Standard::from_description(" 2018 "),
            Some(Standard::Iof2018)
        );
        assert_eq!(Standard::from_description("2024"), Some(Standard::Iof2024));
        assert_eq!(Standard::from_description("2000"), None);
    }

    #[test]
    fn texts_in_de_and_en() {
        assert_eq!(symbol_text("1.1", SheetLang::De), Some("Terrasse"));
        assert_eq!(symbol_text("1.1", SheetLang::En), Some("terrace"));
        // Placeholders stay for the description box to fill.
        assert_eq!(symbol_text("11.7", SheetLang::De), Some("{0}, Biegung"));
        assert_eq!(symbol_text("11.7", SheetLang::En), Some("bend in {0}"));
        // Singular before plural even when the plural comes first in the file; the first gender wins.
        assert_eq!(
            symbol_text("5.25", SheetLang::De),
            Some("Blumenbeet / Verbotenes Gebiet")
        );
        assert_eq!(symbol_text("8.1", SheetLang::De), Some("niedriges {0}"));
        assert_eq!(
            symbol_text("course_length_climb", SheetLang::En),
            Some("Length {0}, climb {1}")
        );
        assert_eq!(symbol_text("no-such-id", SheetLang::De), None);
        // Every symbol has both texts, and versions of one id share them, so the text needs no standard.
        let db = db().expect("parses");
        for e in &db.entries {
            assert!(e.text_de.is_some() && e.text_en.is_some(), "{}", e.id);
            for other in db.entries.iter().filter(|o| o.id == e.id) {
                assert_eq!(other.text_de, e.text_de, "{}", e.id);
                assert_eq!(other.text_en, e.text_en, "{}", e.id);
            }
        }
    }

    #[test]
    fn parser_is_bounded() {
        // Only the embedded file is ever read; these feed the parser hostile variants directly.
        let doctype = "<?xml version=\"1.0\"?><!DOCTYPE symbols [<!ENTITY a \"b\">]><symbols/>";
        assert!(parse(doctype).unwrap_err().contains("DOCTYPE"));
        let deep = format!(
            "<symbols><symbol kind=\"D\" id=\"x\"><text lang=\"en\">x</text>{}{}</symbol></symbols>",
            "<a>".repeat(MAX_DEPTH),
            "</a>".repeat(MAX_DEPTH)
        );
        assert!(parse(&deep).unwrap_err().contains("nesting"));
        let entity =
            "<symbols><symbol kind=\"D\" id=\"x\"><text lang=\"en\">&e;</text></symbol></symbols>";
        assert!(parse(entity).is_err());
        let not_a_number = "<symbols><symbol kind=\"D\" id=\"x\"><text lang=\"en\">x</text>\
            <lines thickness=\"NaN\"><point x=\"0\" y=\"0\"/><point x=\"1\" y=\"1\"/></lines></symbol></symbols>";
        assert!(parse(not_a_number).is_err());
        let short_bezier = "<symbols><symbol kind=\"D\" id=\"x\"><text lang=\"en\">x</text>\
            <beziers thickness=\"1\"><point x=\"0\" y=\"0\"/><point x=\"1\" y=\"1\"/></beziers></symbol></symbols>";
        assert!(parse(short_bezier).is_err());
        let far = "<symbols><symbol kind=\"D\" id=\"x\"><text lang=\"en\">x</text>\
            <filled-circle radius=\"5\"><point x=\"1e9\" y=\"0\"/></filled-circle></symbol></symbols>";
        assert!(parse(far).is_err());
        let many_points = format!(
            "<symbols><symbol kind=\"D\" id=\"x\"><text lang=\"en\">x</text><lines thickness=\"1\">{}</lines>\
             </symbol></symbols>",
            "<point x=\"0\" y=\"0\"/>".repeat(MAX_POINTS_PER_SYMBOL + 1)
        );
        assert!(parse(&many_points).unwrap_err().contains("points"));
        let no_text = "<symbols><symbol kind=\"D\" id=\"x\"/></symbols>";
        assert!(parse(no_text).is_err());
        let wrong_root = "<course-scribe-event/>";
        assert!(parse(wrong_root).is_err());
    }
}
