//! Copied from freemapper 743ce4b (ADR 0014).
//! Map objects → drawing primitives tagged with a colour priority (`docs/render/oom-render-reference.md`).
//!
//! Paper millimetres with y down.

use super::geometry::{self, Part, Pt};
use super::metrics::{ASCENT, LINE_SPACING, text_width};
use super::symbols::{
    Cap, Join, LineDef, Part as SymbolPart, Pattern, PointDef, RenderSymbols, Symbol, TextDef,
};
use crate::omap::{MapCoord, MapObject, MapObjectKind, PatternOrigin};

#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    /// Even-odd fill of closed rings.
    Fill { rings: Vec<Vec<Pt>> },
    Stroke {
        path: Vec<Pt>,
        closed: bool,
        width: f64,
        cap: Cap,
        join: Join,
    },
    /// One line of text; `origin` is the start of the baseline.
    Text {
        origin: Pt,
        size: f64,
        bold: bool,
        text: String,
    },
    /// `inner` painted inside the even-odd clip of the closed `rings` (area patterns).
    Clipped {
        rings: Vec<Vec<Pt>>,
        inner: Box<Shape>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Primitive {
    /// Colour priority (= colour index); 0 is painted last, on top.
    pub priority: i32,
    pub shape: Shape,
}

/// Segments of a circle drawn as a polygon.
const CIRCLE_SEGMENTS: usize = 48;

fn circle(centre: Pt, radius: f64) -> Vec<Pt> {
    (0..=CIRCLE_SEGMENTS)
        .map(|i| {
            let a = i as f64 / CIRCLE_SEGMENTS as f64 * std::f64::consts::TAU;
            (centre.0 + radius * a.cos(), centre.1 + radius * a.sin())
        })
        .collect()
}

/// OOM 0.001 mm → mm.
fn mm(v: f64) -> f64 {
    v / 1000.0
}

/// The primitives of one map object; none when its symbol is unknown or its geometry unusable.
pub fn primitives(object: &MapObject, symbols: &RenderSymbols) -> Vec<Primitive> {
    let Some(symbol) = symbols.symbols.get(&object.symbol_id) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    match &object.kind {
        MapObjectKind::Point { rotation_rad } => {
            if let (Symbol::Point(def), Some(c)) = (symbol, object.coords.first()) {
                let r = if def.rotatable { -rotation_rad } else { 0.0 };
                out = point_symbol(def, (mm(f64::from(c.x)), mm(f64::from(c.y))), r, symbols, 0);
            }
        }
        MapObjectKind::Path => path_symbol(
            symbol,
            &object.coords,
            object.pattern.as_ref(),
            symbols,
            &mut out,
            0,
        ),
        MapObjectKind::Text {
            text,
            h_align,
            v_align,
            ..
        } => {
            if let (Symbol::Text(def), Some(c)) = (symbol, object.coords.first()) {
                text_primitives(
                    def,
                    text,
                    *h_align,
                    *v_align,
                    (mm(f64::from(c.x)), mm(f64::from(c.y))),
                    &mut out,
                );
            }
        }
    }
    // A colour missing from the colour table is not drawn (as in OOM).
    out.retain(|p| symbols.colour(p.priority).is_some());
    out
}

/// Nesting depth of symbols (combined parts, point elements) beyond which drawing stops: a
/// symbol set that refers to itself must not overflow the stack.
const MAX_DEPTH: u32 = 8;

/// A symbol drawn on path coordinates (map objects and point symbol elements); `pattern` is the
/// object's pattern rotation and origin.
fn path_symbol(
    symbol: &Symbol,
    coords: &[MapCoord],
    pattern: Option<&PatternOrigin>,
    symbols: &RenderSymbols,
    out: &mut Vec<Primitive>,
    depth: u32,
) {
    if depth > MAX_DEPTH {
        return;
    }
    match symbol {
        Symbol::Area { color, patterns } => {
            let parts = geometry::parts(coords);
            // The fill and the patterns' clip path need more than two points in the first part.
            if !parts.first().is_some_and(|p| p.points.len() > 2) {
                return;
            }
            let rings: Vec<Vec<Pt>> = parts
                .into_iter()
                .map(|p| p.points)
                .filter(|r| r.len() > 2)
                .collect();
            if *color >= 0 {
                out.push(Primitive {
                    priority: *color,
                    shape: Shape::Fill {
                        rings: rings.clone(),
                    },
                });
            }
            for p in patterns {
                area_pattern(p, &rings, pattern, symbols, out, depth);
            }
        }
        Symbol::Line(def) => line(def, coords, symbols, out, depth),
        Symbol::Combined(parts) => {
            for part in parts {
                let part_symbol = match part {
                    SymbolPart::Ref(id) => symbols.symbols.get(id),
                    SymbolPart::Inline(symbol) => Some(&**symbol),
                };
                if let Some(part_symbol) = part_symbol {
                    path_symbol(part_symbol, coords, pattern, symbols, out, depth + 1);
                }
            }
        }
        // A point symbol element on a point object: drawn at its coordinate, unrotated.
        Symbol::Point(def) => {
            if let Some(c) = coords.first() {
                out.extend(point_symbol(
                    def,
                    (mm(f64::from(c.x)), mm(f64::from(c.y))),
                    0.0,
                    symbols,
                    depth + 1,
                ));
            }
        }
        Symbol::Text(_) => {}
    }
}

/// Most lines one pattern draws in one area: a guard against absurd spacings, far beyond any
/// real map (a 1 m area with 0.1 mm spacing needs 10 000).
const MAX_PATTERN_LINES: usize = 100_000;

/// The rectangle a pattern fills (mm): the area's extent grown by the extent of what is drawn
/// on each line.
#[derive(Debug, Clone, Copy)]
struct Canvas {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

/// Lower-left and upper-right corner of `points` (mm), grown by `extra`.
fn extent<'a>(points: impl IntoIterator<Item = &'a Pt>, extra: f64) -> Option<(Pt, Pt)> {
    let mut points = points.into_iter();
    let first = *points.next()?;
    let (min, max) = points.fold((first, first), |(min, max), q| {
        (
            (min.0.min(q.0), min.1.min(q.1)),
            (max.0.max(q.0), max.1.max(q.1)),
        )
    });
    Some((
        (min.0 - extra, min.1 - extra),
        (max.0 + extra, max.1 + extra),
    ))
}

/// The extent of what `shapes` paint (texts left out), relative to the point they were drawn at.
fn shapes_extent(shapes: &[Primitive]) -> Option<(Pt, Pt)> {
    fn of(shape: &Shape) -> Option<(Pt, Pt)> {
        match shape {
            Shape::Fill { rings } => extent(rings.iter().flatten(), 0.0),
            Shape::Stroke { path, width, .. } => extent(path, width / 2.0),
            Shape::Text { .. } => None,
            Shape::Clipped { inner, .. } => of(inner),
        }
    }
    shapes.iter().filter_map(|p| of(&p.shape)).reduce(|a, b| {
        (
            (a.0.0.min(b.0.0), a.0.1.min(b.0.1)),
            (a.1.0.max(b.1.0), a.1.1.max(b.1.1)),
        )
    })
}

/// Even-odd test of `p` against closed `rings`.
fn inside(rings: &[Vec<Pt>], p: Pt) -> bool {
    let mut inside = false;
    for ring in rings {
        for (i, &a) in ring.iter().enumerate() {
            let b = ring[(i + 1) % ring.len()];
            if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) * (b.0 - a.0) / (b.1 - a.1) {
                inside = !inside;
            }
        }
    }
    inside
}

/// The pattern lines through `canvas` as (first, second) points, and the along-line shift that
/// the pattern origin adds (reference §4.3) [AS `FillPattern::createRenderables<T>`].
///
/// Lines are `n · (p − origin) = offset + k · spacing` with the normal `n = (sin r, cos r)`
/// (paper, y down); the cases and the direction from `first` to `second` follow OOM exactly.
fn pattern_lines(
    c: Canvas,
    rotation: f64,
    spacing: f64,
    line_offset: f64,
    origin: Pt,
) -> (Vec<(Pt, Pt)>, f64) {
    use std::f64::consts::FRAC_PI_2;
    let (sin, cos) = rotation.sin_cos();
    let mut delta_along = cos * origin.0 - sin * origin.1;
    let offset = line_offset + sin * origin.0 + cos * origin.1;
    let mut lines = Vec::new();
    if (rotation - FRAC_PI_2).abs() < 0.0001 {
        // Vertical lines, drawn downwards.
        delta_along = -delta_along;
        let mut cur = offset + ((c.left - offset) / spacing).ceil() * spacing;
        while cur < c.right && lines.len() < MAX_PATTERN_LINES {
            lines.push(((cur, c.top), (cur, c.bottom)));
            cur += spacing;
        }
    } else if rotation.abs() < 0.0001 {
        // Horizontal lines, drawn eastwards.
        let mut cur = offset + ((c.top - offset) / spacing).ceil() * spacing;
        while cur < c.bottom && lines.len() < MAX_PATTERN_LINES {
            lines.push(((c.left, cur), (c.right, cur)));
            cur += spacing;
        }
    } else {
        let (dist_x, dist_y) = (spacing / sin, spacing / cos);
        let (mut offset_x, mut offset_y) = (offset / sin, offset / cos);
        let tan = rotation.tan();
        if rotation < FRAC_PI_2 {
            // From the top edge (start) to the left edge (end), first line at the upper left.
            delta_along = -delta_along;
            offset_x -= c.top / tan;
            offset_y -= c.left * tan;
            let mut start = (
                offset_x + ((c.left - offset_x) / dist_x).ceil() * dist_x,
                c.top,
            );
            let mut end = (
                c.left,
                offset_y + ((c.top - offset_y) / dist_y).ceil() * dist_y,
            );
            while lines.len() < MAX_PATTERN_LINES {
                if start.0 > c.right {
                    start.1 += (start.0 - c.right) / dist_x * dist_y;
                    start.0 = c.right;
                }
                if end.1 > c.bottom {
                    end.0 += (end.1 - c.bottom) / dist_y * dist_x;
                    end.1 = c.bottom;
                }
                if start.1 > c.bottom {
                    break;
                }
                lines.push((start, end));
                start.0 += dist_x;
                end.1 += dist_y;
            }
        } else {
            // From the bottom edge (start) to the left edge (end), first line at the lower left.
            offset_x -= c.bottom / tan;
            offset_y -= c.left * tan;
            let mut start = (
                offset_x + ((c.left - offset_x) / dist_x).ceil() * dist_x,
                c.bottom,
            );
            let mut end = (
                c.left,
                offset_y + ((c.bottom - offset_y) / dist_y).ceil() * dist_y,
            );
            while lines.len() < MAX_PATTERN_LINES {
                if start.0 > c.right {
                    start.1 += (start.0 - c.right) / dist_x * dist_y;
                    start.0 = c.right;
                }
                if end.1 < c.top {
                    end.0 += (end.1 - c.top) / dist_y * dist_x;
                    end.1 = c.top;
                }
                if start.1 < c.top {
                    break;
                }
                lines.push((start, end));
                start.0 += dist_x;
                end.1 += dist_y;
            }
        }
    }
    (lines, delta_along)
}

/// The lines or point placements of one area pattern, clipped to the area's `rings` unless the
/// pattern says otherwise (reference §4.2–4.5) [AS `FillPattern::createRenderables`,
/// `createPointPatternLine`].
fn area_pattern(
    pattern: &Pattern,
    rings: &[Vec<Pt>],
    object: Option<&PatternOrigin>,
    symbols: &RenderSymbols,
    out: &mut Vec<Primitive>,
    depth: u32,
) {
    let (angle, rotatable, spacing, offset, no_clipping) = match pattern {
        Pattern::Lines {
            angle_rad,
            rotatable,
            spacing,
            offset,
            no_clipping,
            ..
        }
        | Pattern::Points {
            angle_rad,
            rotatable,
            spacing,
            offset,
            no_clipping,
            ..
        } => (*angle_rad, *rotatable, *spacing, *offset, *no_clipping),
    };
    let Some((min, max)) = extent(rings.iter().flatten(), 0.0) else {
        return;
    };
    if spacing <= 0.0 {
        return;
    }
    // The object's pattern rotation and origin count only for a rotatable pattern.
    let (delta, origin) = match object {
        Some(o) if rotatable => (
            o.rotation_rad,
            (mm(f64::from(o.origin.x)), mm(f64::from(o.origin.y))),
        ),
        _ => (0.0, (0.0, 0.0)),
    };
    let mut rotation = (angle + delta) % std::f64::consts::PI;
    if rotation < 0.0 {
        rotation += std::f64::consts::PI;
    }
    let clip = |shape: Shape| {
        if no_clipping {
            shape
        } else {
            Shape::Clipped {
                rings: rings.to_vec(),
                inner: Box::new(shape),
            }
        }
    };
    // The canvas grows by the extent (lo, hi) of what one line or point paints around its
    // centre: left − hi.x, top − hi.y, right − lo.x, bottom − lo.y.
    let canvas = |(lo, hi): (Pt, Pt)| Canvas {
        left: min.0 - hi.0,
        top: min.1 - hi.1,
        right: max.0 - lo.0,
        bottom: max.1 - lo.1,
    };
    match pattern {
        Pattern::Lines { color, width, .. } => {
            let width = mm(*width);
            let m = width / 2.0;
            let (lines, _) = pattern_lines(
                canvas(((-m, -m), (m, m))),
                rotation,
                mm(spacing),
                mm(offset),
                origin,
            );
            for (first, second) in lines {
                if let Some(p) = stroke(
                    *color,
                    vec![first, second],
                    false,
                    width,
                    Cap::Flat,
                    Join::Miter,
                ) {
                    out.push(Primitive {
                        priority: p.priority,
                        shape: clip(p.shape),
                    });
                }
            }
        }
        Pattern::Points {
            offset_along,
            distance,
            symbol,
            ..
        } => {
            if *distance <= 0.0 {
                return;
            }
            // Extent of the point object at (0, 0) with the object's pattern rotation.
            let at_origin = point_symbol(
                symbol,
                (0.0, 0.0),
                if symbol.rotatable { -delta } else { 0.0 },
                symbols,
                depth + 1,
            );
            let point_extent = shapes_extent(&at_origin).unwrap_or(((0.0, 0.0), (0.0, 0.0)));
            let (lines, delta_along) = pattern_lines(
                canvas(point_extent),
                rotation,
                mm(spacing),
                mm(offset),
                origin,
            );
            let step = mm(*distance);
            for (first, second) in lines {
                let length = (second.0 - first.0).hypot(second.1 - first.1);
                if length <= 0.0 {
                    continue;
                }
                let dir = ((second.0 - first.0) / length, (second.1 - first.1) / length);
                let along = dir.0 * first.0 + dir.1 * first.1 - mm(*offset_along) - delta_along;
                let mut cur = (along / step).ceil() * step - along;
                while cur < length {
                    let at = (first.0 + dir.0 * cur, first.1 + dir.1 * cur);
                    cur += step;
                    // Unclipped points are kept only when their centre lies in the area.
                    if no_clipping && !inside(rings, at) {
                        continue;
                    }
                    for p in point_symbol(symbol, at, -delta, symbols, depth + 1) {
                        out.push(Primitive {
                            priority: p.priority,
                            shape: clip(p.shape),
                        });
                    }
                }
            }
        }
    }
}

fn stroke(
    color: i32,
    path: Vec<Pt>,
    closed: bool,
    width: f64,
    cap: Cap,
    join: Join,
) -> Option<Primitive> {
    (color >= 0 && width > 0.0 && path.len() >= 2).then_some(Primitive {
        priority: color,
        shape: Shape::Stroke {
            path,
            closed,
            width,
            cap,
            join,
        },
    })
}

fn line(
    def: &LineDef,
    coords: &[MapCoord],
    symbols: &RenderSymbols,
    out: &mut Vec<Primitive>,
    depth: u32,
) {
    let parts: Vec<Part> = geometry::parts(coords)
        .into_iter()
        .filter(|p| p.points.len() >= 2 && geometry::length(&p.points) > 0.0)
        .collect();
    let width = mm(def.width);
    for part in &parts {
        // Main line, whole or dashed.
        match &def.dash {
            Some(dash) => {
                let dashes = geometry::dashes(part, dash);
                let solid = dashes.len() == 1 && dashes[0] == part.points;
                for d in dashes {
                    out.extend(stroke(
                        def.color,
                        d,
                        solid && part.closed,
                        width,
                        def.cap,
                        def.join,
                    ));
                }
            }
            None => out.extend(stroke(
                def.color,
                part.points.clone(),
                part.closed,
                width,
                def.cap,
                def.join,
            )),
        }
        // Borders on both sides, flat caps, miter (round if the line is round).
        if let Some(border) = &def.border {
            let shift = width / 2.0 + mm(border.shift);
            let join = if def.join == Join::Round {
                Join::Round
            } else {
                Join::Miter
            };
            let pieces: Vec<(Vec<Pt>, bool)> = match &border.dash {
                Some(dash) => {
                    let dashes = geometry::dashes(part, dash);
                    let solid = dashes.len() == 1 && dashes[0] == part.points;
                    dashes
                        .into_iter()
                        .map(|d| (d, solid && part.closed))
                        .collect()
                }
                None => vec![(part.points.clone(), part.closed)],
            };
            for (piece, closed) in pieces {
                for side in [-1.0, 1.0] {
                    let path = geometry::offset(&piece, closed, side * shift);
                    out.extend(stroke(
                        border.color,
                        path,
                        closed,
                        mm(border.width),
                        Cap::Flat,
                        join,
                    ));
                }
            }
        }
        // Mid symbols, local +y to the right of travel.
        if let Some(mid) = &def.mid {
            for (at, angle) in geometry::mid_positions(part, mid) {
                let r = if mid.symbol.rotatable { angle } else { 0.0 };
                out.extend(point_symbol(&mid.symbol, at, r, symbols, depth + 1));
            }
        }
    }
    // Start symbol on the first part (outgoing direction), end symbol on the last (incoming
    // direction), also on closed lines (reference §2.6).
    if let (Some(start), Some(first)) = (&def.start, parts.first()) {
        let angle = if start.rotatable {
            geometry::start_angle(first)
        } else {
            0.0
        };
        out.extend(point_symbol(
            start,
            first.points[0],
            angle,
            symbols,
            depth + 1,
        ));
    }
    if let (Some(end), Some(last)) = (&def.end, parts.last()) {
        let angle = if end.rotatable {
            geometry::end_angle(last)
        } else {
            0.0
        };
        let at = *last.points.last().expect("parts have two points");
        out.extend(point_symbol(end, at, angle, symbols, depth + 1));
    }
}

/// The primitives of a point symbol at `at` (mm), rotated by `rotation` (radians, y down).
pub fn point_primitives(
    def: &PointDef,
    at: Pt,
    rotation: f64,
    symbols: &RenderSymbols,
) -> Vec<Primitive> {
    point_symbol(def, at, rotation, symbols, 0)
}

fn point_symbol(
    def: &PointDef,
    at: Pt,
    rotation: f64,
    symbols: &RenderSymbols,
    depth: u32,
) -> Vec<Primitive> {
    let mut out = Vec::new();
    if depth > MAX_DEPTH {
        return out;
    }
    if let Some((color, radius)) = def.dot {
        out.push(Primitive {
            priority: color,
            shape: Shape::Fill {
                rings: vec![circle(at, mm(radius))],
            },
        });
    }
    if let Some((color, radius, width)) = def.ring {
        out.extend(stroke(
            color,
            circle(at, mm(radius)),
            true,
            mm(width),
            Cap::Flat,
            Join::Miter,
        ));
    }
    let (sin, cos) = rotation.sin_cos();
    for element in &def.elements {
        // Element coordinates rotated about the centre and moved there, back in 0.001 mm.
        let coords: Vec<MapCoord> = element
            .coords
            .iter()
            .map(|c| {
                let (ox, oy) = (f64::from(c.x), f64::from(c.y));
                MapCoord {
                    x: (ox * cos - oy * sin + at.0 * 1000.0).round() as i32,
                    y: (oy * cos + ox * sin + at.1 * 1000.0).round() as i32,
                    flags: c.flags,
                }
            })
            .collect();
        path_symbol(&element.symbol, &coords, None, symbols, &mut out, depth + 1);
    }
    out
}

/// One text primitive per line, laid out per reference §6 (no rotation: the sheet texts are not
/// rotatable).
fn text_primitives(
    def: &TextDef,
    text: &str,
    h_align: u8,
    v_align: u8,
    anchor: Pt,
    out: &mut Vec<Primitive>,
) {
    if def.color < 0 {
        return;
    }
    let em = mm(def.size);
    let ascent = ASCENT * em;
    let line_step = def.line_spacing * LINE_SPACING * em;
    let lines: Vec<&str> = text.split('\n').collect();
    let mut y = if v_align == 0 { 0.0 } else { ascent };
    if v_align == 2 || v_align == 3 {
        let height = ascent + (lines.len().saturating_sub(1)) as f64 * line_step;
        y -= if v_align == 2 { height / 2.0 } else { height };
    }
    for line in lines {
        let width = text_width(line, def.bold) * em;
        let x = match h_align {
            1 => -width / 2.0,
            2 => -width,
            _ => 0.0,
        };
        if !line.is_empty() {
            out.push(Primitive {
                priority: def.color,
                shape: Shape::Text {
                    origin: (anchor.0 + x, anchor.1 + y),
                    size: em,
                    bold: def.bold,
                    text: line.to_string(),
                },
            });
        }
        y += line_step;
    }
}

#[cfg(test)]
mod tests {
    use super::super::symbols::Pattern;
    use super::*;
    use crate::omap::coord::{FLAG_CLOSE, FLAG_HOLE};
    use crate::omap::{PatternOrigin, SymbolSet, closed_ring};

    fn set() -> (RenderSymbols, SymbolSet) {
        let set = SymbolSet::issprom_2019().with_sheet_symbols();
        (RenderSymbols::from_set(&set), set)
    }

    fn object(
        set: &SymbolSet,
        code: &str,
        kind: MapObjectKind,
        coords: Vec<MapCoord>,
    ) -> MapObject {
        MapObject {
            symbol_id: set.id_for_code(code).unwrap(),
            kind,
            coords,
            tags: Vec::new(),
            pattern: None,
        }
    }

    fn square(side: i32) -> Vec<MapCoord> {
        closed_ring(&[(0, 0), (side, 0), (side, side), (0, side)])
    }

    fn strokes(p: &[Primitive]) -> Vec<(i32, f64)> {
        p.iter()
            .filter_map(|p| match &p.shape {
                Shape::Stroke { width, .. } => {
                    Some((p.priority, (width * 1000.0).round() / 1000.0))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn paved_area_edge_lies_below_its_fill() {
        let (r, s) = set();
        let p = primitives(&object(&s, "501", MapObjectKind::Path, square(10_000)), &r);
        let fills: Vec<i32> = p
            .iter()
            .filter(|p| matches!(p.shape, Shape::Fill { .. }))
            .map(|p| p.priority)
            .collect();
        assert_eq!(fills, [14]);
        assert_eq!(strokes(&p), [(15, 0.2)]);
        let closed = p
            .iter()
            .any(|p| matches!(p.shape, Shape::Stroke { closed: true, .. }));
        assert!(closed, "the edge of an area is a closed line");
    }

    #[test]
    fn bush_is_dot_and_ring() {
        let (r, s) = set();
        let p = primitives(
            &object(
                &s,
                "418",
                MapObjectKind::Point { rotation_rad: 0.0 },
                vec![MapCoord::new(5_000, 5_000)],
            ),
            &r,
        );
        assert_eq!(p.len(), 2);
        let Shape::Fill { rings } = &p.iter().find(|p| p.priority == 21).unwrap().shape else {
            panic!()
        };
        let r_dot = rings[0]
            .iter()
            .map(|q| (q.0 - 5.0).hypot(q.1 - 5.0))
            .fold(0.0, f64::max);
        assert!((r_dot - 0.1).abs() < 1e-6);
        let Shape::Stroke {
            path,
            width,
            closed,
            ..
        } = &p.iter().find(|p| p.priority == 3).unwrap().shape
        else {
            panic!()
        };
        assert!(*closed && (*width - 0.3).abs() < 1e-9);
        assert!((((path[0].0 - 5.0).hypot(path[0].1 - 5.0)) - 0.25).abs() < 1e-6);
    }

    #[test]
    fn fence_ticks_on_the_right() {
        let (r, s) = set();
        let fence = object(
            &s,
            "518",
            MapObjectKind::Path,
            vec![MapCoord::new(0, 0), MapCoord::new(20_000, 0)],
        );
        let p = primitives(&fence, &r);
        // Main line 0.4 mm and five ticks of two lines 0.14 mm each.
        let widths = strokes(&p);
        assert_eq!(widths.iter().filter(|w| w.1 == 0.4).count(), 1);
        assert_eq!(widths.iter().filter(|w| w.1 == 0.14).count(), 10);
        for prim in &p {
            if let Shape::Stroke { path, width, .. } = &prim.shape
                && (*width - 0.14).abs() < 1e-9
            {
                assert!(
                    path.iter().all(|q| q.1 >= -1e-9),
                    "right of travel (south): {path:?}"
                );
            }
        }
    }

    #[test]
    fn building_has_fill_and_outline() {
        let (r, s) = set();
        let mut coords = square(10_000);
        coords.extend(closed_ring(&[
            (2_000, 2_000),
            (4_000, 2_000),
            (4_000, 4_000),
            (2_000, 4_000),
        ]));
        let p = primitives(&object(&s, "521", MapObjectKind::Path, coords), &r);
        let Shape::Fill { rings } = &p.iter().find(|p| p.priority == 9).unwrap().shape else {
            panic!()
        };
        assert_eq!(rings.len(), 2, "outer ring and courtyard");
        assert_eq!(strokes(&p), [(2, 0.14), (2, 0.14)]);
        let _ = (FLAG_CLOSE, FLAG_HOLE);
    }

    #[test]
    fn text_alignment_uses_widths() {
        let (r, s) = set();
        let text = |h_align| {
            object(
                &s,
                "992",
                MapObjectKind::Text {
                    text: "Maßstab".into(),
                    h_align,
                    v_align: 0,
                    rotation_rad: 0.0,
                },
                vec![MapCoord::new(50_000, 20_000)],
            )
        };
        let em = 2.5;
        let width = text_width("Maßstab", false) * em;
        for (h_align, x) in [(0, 50.0), (1, 50.0 - width / 2.0), (2, 50.0 - width)] {
            let p = primitives(&text(h_align), &r);
            let Shape::Text { origin, size, .. } = &p[0].shape else {
                panic!()
            };
            assert!(
                (origin.0 - x).abs() < 1e-9 && (origin.1 - 20.0).abs() < 1e-9,
                "{h_align}: {origin:?}"
            );
            assert_eq!(*size, em);
            assert_eq!(p[0].priority, 2);
        }
    }

    fn closed_square(side: i32) -> Vec<MapCoord> {
        closed_ring(&[(0, 0), (side, 0), (side, side), (0, side)])
    }

    fn stroke_paths(p: &[Primitive], width: f64) -> Vec<(&Vec<Pt>, bool)> {
        p.iter()
            .filter_map(|p| match &p.shape {
                Shape::Stroke {
                    path,
                    closed,
                    width: w,
                    ..
                } if (w - width).abs() < 1e-9 => Some((path, *closed)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn fence_ring_does_not_wrap_around() {
        let (r, s) = set();
        // 80 mm ring: ticks keep e2 from the closing point (0, 0) on both sides.
        let p = primitives(
            &object(&s, "518", MapObjectKind::Path, closed_square(20_000)),
            &r,
        );
        let ticks = stroke_paths(&p, 0.14);
        assert_eq!(ticks.len() % 2, 0);
        let spots = ticks.len() / 2;
        assert_eq!(spots, 21, "n·3.75 + 4.65 closest to 80 mm");
        for (path, _) in &ticks {
            let d = path
                .iter()
                .map(|q| q.0.hypot(q.1))
                .fold(f64::INFINITY, f64::min);
            assert!(d > 1.5, "no tick at the closing point: {path:?}");
        }
        let main = stroke_paths(&p, 0.4);
        assert_eq!(main.len(), 1);
        assert!(main[0].1, "the ring itself is closed");
    }

    #[test]
    fn path_ring_borders_are_dashed_alike_on_both_sides() {
        let (r, s) = set();
        let p = primitives(
            &object(&s, "505.1", MapObjectKind::Path, closed_square(10_000)),
            &r,
        );
        let borders = stroke_paths(&p, 0.1);
        assert!(borders.len() > 4);
        assert_eq!(borders.len() % 2, 0, "one dash per side and position");
        assert!(
            borders.iter().all(|(_, closed)| !closed),
            "dashes are open strokes"
        );
    }

    #[test]
    fn short_path_ring_has_one_border_dash_per_side() {
        let (r, s) = set();
        // 2.4 mm around: two half dashes joined at the closing point and one gap (reference §2.2:
        // closed parts always have H = 2, so they are never drawn solid).
        let p = primitives(
            &object(&s, "505.1", MapObjectKind::Path, closed_square(600)),
            &r,
        );
        let borders = stroke_paths(&p, 0.1);
        assert_eq!(borders.len(), 2);
        assert!(borders.iter().all(|(_, closed)| !closed));
    }

    #[test]
    fn self_referencing_combined_symbol_stops() {
        let (mut r, s) = set();
        let id = s.id_for_code("521").unwrap();
        r.symbols
            .insert(id, Symbol::Combined(vec![SymbolPart::Ref(id)]));
        let p = primitives(&object(&s, "521", MapObjectKind::Path, square(10_000)), &r);
        assert!(p.is_empty());
    }

    #[test]
    fn end_bars_of_a_closed_line_follow_their_segments() {
        let set = SymbolSet::school_2019();
        let r = RenderSymbols::from_set(&set);
        let bench = object(&set, "543", MapObjectKind::Path, closed_square(5_000));
        let p = primitives(&bench, &r);
        // Bars along local y, 0.8 mm (enlarged 125 % in the school set? no: school additions are
        // final sizes): the start bar is perpendicular to the first segment (east), so vertical.
        let bars: Vec<&Vec<Pt>> = p
            .iter()
            .filter_map(|p| match &p.shape {
                Shape::Stroke { path, .. } if path.len() == 2 && geometry::length(path) < 1.0 => {
                    Some(path)
                }
                _ => None,
            })
            .collect();
        assert_eq!(bars.len(), 2, "{p:?}");
        let vertical = |b: &Vec<Pt>| (b[0].0 - b[1].0).abs() < 1e-6;
        let horizontal = |b: &Vec<Pt>| (b[0].1 - b[1].1).abs() < 1e-6;
        assert!(
            bars.iter().any(|b| vertical(b)),
            "start bar ⟂ east: {bars:?}"
        );
        assert!(
            bars.iter().any(|b| horizontal(b)),
            "end bar ⟂ north: {bars:?}"
        );
    }

    #[test]
    fn unknown_colours_are_left_out() {
        let (mut r, s) = set();
        let id = s.id_for_code("520").unwrap();
        r.symbols.insert(
            id,
            Symbol::Area {
                color: 99,
                patterns: Vec::new(),
            },
        );
        let p = primitives(&object(&s, "520", MapObjectKind::Path, square(10_000)), &r);
        assert!(p.is_empty());
    }

    const PATTERNED: u32 = 9_999;

    fn patterned_square(pattern: Pattern, origin: Option<PatternOrigin>) -> Vec<Primitive> {
        let (mut r, _) = set();
        r.symbols.insert(
            PATTERNED,
            Symbol::Area {
                color: -1,
                patterns: vec![pattern],
            },
        );
        let object = MapObject {
            symbol_id: PATTERNED,
            kind: MapObjectKind::Path,
            coords: square(10_000),
            tags: Vec::new(),
            pattern: origin,
        };
        primitives(&object, &r)
    }

    fn square_mm() -> Vec<Vec<Pt>> {
        vec![vec![
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 10.0),
            (0.0, 10.0),
            (0.0, 0.0),
        ]]
    }

    fn lines(rotatable: bool) -> Pattern {
        Pattern::Lines {
            angle_rad: 0.0,
            rotatable,
            spacing: 1_000.0,
            offset: 0.0,
            color: 3,
            width: 100.0,
            no_clipping: false,
        }
    }

    /// The clip rings and inner shape of every primitive, which must all be clipped.
    fn clipped(p: &[Primitive]) -> Vec<(&Vec<Vec<Pt>>, &Shape)> {
        p.iter()
            .map(|p| match &p.shape {
                Shape::Clipped { rings, inner } => (rings, &**inner),
                other => panic!("not clipped: {other:?}"),
            })
            .collect()
    }

    #[test]
    fn line_pattern_fills_square() {
        let p = patterned_square(lines(false), None);
        // Canvas = square grown by half the line width: y = 0, 1, …, 10 all lie inside it.
        assert_eq!(p.len(), 11);
        assert!(p.iter().all(|p| p.priority == 3));
        let mut ys = Vec::new();
        for (rings, inner) in clipped(&p) {
            assert_eq!(rings, &square_mm());
            let Shape::Stroke {
                path, width, cap, ..
            } = inner
            else {
                panic!("{inner:?}")
            };
            assert!((width - 0.1).abs() < 1e-12 && *cap == Cap::Flat);
            assert!((path[0].1 - path[1].1).abs() < 1e-9, "horizontal: {path:?}");
            assert!(
                path[0].0 <= 0.0 && path[1].0 >= 10.0,
                "spans the square: {path:?}"
            );
            ys.push(path[0].1);
        }
        let expected: Vec<f64> = (0..=10).map(f64::from).collect();
        assert!(
            ys.iter().zip(&expected).all(|(a, b)| (a - b).abs() < 1e-9),
            "{ys:?}"
        );
    }

    #[test]
    fn pattern_follows_object_rotation() {
        let origin = PatternOrigin {
            rotation_rad: std::f64::consts::FRAC_PI_2,
            origin: MapCoord::new(0, 0),
        };
        let p = patterned_square(lines(true), Some(origin));
        assert_eq!(p.len(), 11);
        for (_, inner) in clipped(&p) {
            let Shape::Stroke { path, .. } = inner else {
                panic!()
            };
            assert!((path[0].0 - path[1].0).abs() < 1e-9, "vertical: {path:?}");
        }
        // Not rotatable: the object rotation is ignored.
        let p = patterned_square(lines(false), Some(origin));
        for (_, inner) in clipped(&p) {
            let Shape::Stroke { path, .. } = inner else {
                panic!()
            };
            assert!((path[0].1 - path[1].1).abs() < 1e-9, "horizontal: {path:?}");
        }
    }

    #[test]
    fn point_pattern_positions() {
        let dot = PointDef {
            rotatable: false,
            dot: Some((3, 100.0)),
            ring: None,
            elements: Vec::new(),
        };
        let pattern = Pattern::Points {
            angle_rad: 0.0,
            rotatable: false,
            spacing: 2_000.0,
            offset: 0.0,
            offset_along: 0.0,
            distance: 2_000.0,
            symbol: Box::new(dot),
            no_clipping: false,
        };
        let p = patterned_square(pattern, None);
        // Canvas = square grown by the dot extent (0.1 mm): lines y = 0, 2, …, 10, points
        // x = 0, 2, …, 10 on each (reference §4.3).
        assert_eq!(p.len(), 36);
        let mut centres: Vec<(i64, i64)> = clipped(&p)
            .into_iter()
            .map(|(rings, inner)| {
                assert_eq!(rings, &square_mm());
                let Shape::Fill { rings } = inner else {
                    panic!("{inner:?}")
                };
                let n = rings[0].len() as f64;
                let (sx, sy) = rings[0]
                    .iter()
                    .fold((0.0, 0.0), |(x, y), q| (x + q.0, y + q.1));
                ((sx / n).round() as i64, (sy / n).round() as i64)
            })
            .collect();
        centres.sort_unstable();
        let expected: Vec<(i64, i64)> = (0..=5)
            .flat_map(|x| (0..=5).map(move |y| (2 * x, 2 * y)))
            .collect();
        assert_eq!(centres, expected);
    }

    #[test]
    fn diagonal_lines_keep_spacing_and_cover_the_square() {
        use std::f64::consts::FRAC_PI_4;
        // Corner projections on n = (sin r, cos r) of the canvas [−0.05, 10.05]²: k = 0..=14
        // for π/4 and k = −7..=7 for 3π/4.
        for (angle, ks) in [(FRAC_PI_4, 0..=14), (3.0 * FRAC_PI_4, -7..=7)] {
            let pattern = Pattern::Lines {
                angle_rad: angle,
                rotatable: false,
                spacing: 1_000.0,
                offset: 0.0,
                color: 3,
                width: 100.0,
                no_clipping: false,
            };
            let p = patterned_square(pattern, None);
            let (sin, cos) = angle.sin_cos();
            let mut ks_seen = Vec::new();
            for (_, inner) in clipped(&p) {
                let Shape::Stroke { path, .. } = inner else {
                    panic!()
                };
                let c: Vec<f64> = path.iter().map(|q| sin * q.0 + cos * q.1).collect();
                assert!((c[0] - c[1]).abs() < 1e-9, "{angle}: {path:?}");
                assert!((c[0] - c[0].round()).abs() < 1e-9, "spacing 1 mm: {c:?}");
                // Counter-clockwise on paper: direction ± (cos r, −sin r).
                let d = (path[1].0 - path[0].0, path[1].1 - path[0].1);
                assert!((d.0 * sin + d.1 * cos).abs() < 1e-9);
                ks_seen.push(c[0].round() as i32);
            }
            ks_seen.sort_unstable();
            assert_eq!(ks_seen, ks.collect::<Vec<_>>(), "{angle}");
        }
    }

    #[test]
    fn point_pattern_starts_at_pattern_origin() {
        let pattern = Pattern::Points {
            angle_rad: 0.0,
            rotatable: true,
            spacing: 2_000.0,
            offset: 0.0,
            offset_along: 0.0,
            distance: 2_000.0,
            symbol: Box::new(PointDef {
                rotatable: false,
                dot: Some((3, 100.0)),
                ring: None,
                elements: Vec::new(),
            }),
            no_clipping: false,
        };
        let origin = PatternOrigin {
            rotation_rad: 0.0,
            origin: MapCoord::new(500, 500),
        };
        let p = patterned_square(pattern, Some(origin));
        // Lines y = 0.5, 2.5, …, 8.5 and points x = 0.5, 2.5, …, 8.5.
        assert_eq!(p.len(), 25);
        for (_, inner) in clipped(&p) {
            let Shape::Fill { rings } = inner else {
                panic!()
            };
            let (x, y) = rings[0][CIRCLE_SEGMENTS / 4];
            // The quarter-circle point lies 0.1 mm below the centre.
            let (cx, cy) = (x, y - 0.1);
            assert!(
                [cx, cy]
                    .iter()
                    .all(|v| ((v - 0.5) / 2.0 - ((v - 0.5) / 2.0).round()).abs() < 1e-9),
                "({cx}, {cy})"
            );
        }
    }

    #[test]
    fn rotation_of_rotatable_points() {
        let (r, _) = set();
        let arrow = PointDef {
            rotatable: true,
            dot: None,
            ring: None,
            elements: vec![super::super::symbols::Element {
                symbol: Symbol::Line(LineDef {
                    color: 2,
                    width: 100.0,
                    cap: Cap::Flat,
                    join: Join::Miter,
                    dash: None,
                    border: None,
                    mid: None,
                    start: None,
                    end: None,
                }),
                coords: vec![MapCoord::new(0, 0), MapCoord::new(1_000, 0)],
            }],
        };
        // Rotation −π/2 (y down) turns local +x to paper up.
        let p = point_primitives(&arrow, (10.0, 10.0), -std::f64::consts::FRAC_PI_2, &r);
        let Shape::Stroke { path, .. } = &p[0].shape else {
            panic!()
        };
        assert!(
            (path[1].0 - 10.0).abs() < 1e-9 && (path[1].1 - 9.0).abs() < 1e-9,
            "{path:?}"
        );
    }
}
