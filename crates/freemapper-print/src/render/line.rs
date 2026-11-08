//! Line symbols (`docs/render/oom-render-reference.md` §2): main line, dashes, pointed caps,
//! start and end offsets, borders, mid, start, end and dash symbols.
//!
//! Paper millimetres with y down.

use super::draw::{Primitive, Shape, Sink, mm, point_symbol, point_symbol_scaled, stroke};
use super::geometry::{self, Part, Pt};
use super::symbols::{Border, Cap, DashSymbol, Join, LineDef, PointDef, RenderSymbols};
use crate::omap::MapCoord;

/// A line symbol on path coordinates [LS `createRenderables`].
pub(super) fn line(
    def: &LineDef,
    coords: &[MapCoord],
    symbols: &RenderSymbols,
    sink: &mut Sink,
    depth: u32,
) {
    let parts: Vec<Part> = geometry::parts(coords)
        .into_iter()
        .filter(|p| p.points.len() >= 2 && geometry::length(&p.points) > 0.0)
        .collect();
    for part in &parts {
        if sink.stop() {
            return;
        }
        if let Some(dash_symbol) = &def.dash_symbol {
            dash_symbols(dash_symbol, part, symbols, sink, depth);
        }
        single_path(def, part, symbols, sink, depth);
    }
    // Start symbol on the first part (outgoing direction), end symbol on the last (incoming
    // direction), also on closed lines (reference §2.6).
    if let (Some(start), Some(first)) = (&def.start, parts.first()) {
        let angle = if start.rotatable {
            geometry::start_angle(first)
        } else {
            0.0
        };
        point_symbol(start, first.points[0], angle, symbols, sink, depth + 1);
    }
    if let (Some(end), Some(last)) = (&def.end, parts.last()) {
        let angle = if end.rotatable {
            geometry::end_angle(last)
        } else {
            0.0
        };
        let at = *last.points.last().expect("parts have two points");
        point_symbol(end, at, angle, symbols, sink, depth + 1);
    }
}

/// The dash symbol at every dash point of `part`, turned to the bisector and scaled by
/// 1 / cos(half the turn), at most 2 (reference §2.7) [LS `createDashSymbolRenderables`].
fn dash_symbols(
    dash_symbol: &DashSymbol,
    part: &Part,
    symbols: &RenderSymbols,
    sink: &mut Sink,
    depth: u32,
) {
    let last = part.points.len() - 1;
    for &i in &part.dash_points {
        if sink.stop() {
            return;
        }
        // Not at the ends when suppressed there; a closed part's closing point counts once.
        let skip = if dash_symbol.suppress_at_ends {
            i == 0 || i == last
        } else {
            part.closed && i == 0
        };
        if skip {
            continue;
        }
        let (angle, scaling) = geometry::tangent_scaling(&part.points, part.closed, i);
        let rotation = if dash_symbol.symbol.rotatable {
            angle
        } else {
            0.0
        };
        let scale = if dash_symbol.scale {
            scaling.min(2.0)
        } else {
            1.0
        };
        point_symbol_scaled(
            &dash_symbol.symbol,
            part.points[i],
            rotation,
            scale,
            symbols,
            sink,
            depth + 1,
        );
    }
}

/// One part of a line: offsets and pointed caps, main line (whole or dashed), borders and mid
/// symbols [LS `createSinglePathRenderables`].
fn single_path(def: &LineDef, part: &Part, symbols: &RenderSymbols, sink: &mut Sink, depth: u32) {
    let width = mm(def.width);
    let total = geometry::length(&part.points);
    let mut create_line = def.color >= 0 && width > 0.0;
    let (mut start, mut end) = (0.0, total);
    // Open parts only: the offsets cut the line short, scaled down together when they exceed
    // its length, which then has no main line, borders or mid symbols.
    if !part.closed && (def.start_offset > 0.0 || def.end_offset > 0.0) {
        let mut start_offset = mm(def.start_offset.max(0.0));
        let mut end_offset = mm(def.end_offset.max(0.0));
        let sum = start_offset + end_offset;
        if sum > total {
            create_line = false;
            start_offset *= total / sum;
            end_offset *= total / sum;
        }
        let cut_start = start_offset;
        let cut_end = (total - end_offset).max(cut_start);
        if def.start_offset > 0.0 {
            if def.pointed_caps {
                pointed_cap(def, part, (0.0, cut_start), false, sink);
            }
            start = cut_start;
        }
        if def.end_offset > 0.0 {
            if def.pointed_caps {
                pointed_cap(def, part, (cut_end, total), true, sink);
            }
            end = cut_end;
        }
        if !create_line {
            return;
        }
    }
    // The stretches of the main line, and the mid symbols.
    let processed = match &def.dash {
        Some(dash) => {
            let pattern =
                geometry::dash_pattern(part, start, end, dash, def.mid.as_ref(), sink.left());
            charge_skipped(&pattern, sink);
            if let Some(mid) = &def.mid {
                mid_symbols(&mid.symbol, &pattern.spots, symbols, sink, depth);
            }
            pattern.intervals
        }
        None => {
            if let Some(mid) = &def.mid {
                let (spots, limited) =
                    geometry::mid_positions_in(part, start, end, mid, sink.left());
                if limited {
                    sink.mark_cut();
                }
                mid_symbols(&mid.symbol, &spots, symbols, sink, depth);
            }
            vec![(start, end)]
        }
    };
    if create_line {
        for (path, closed) in geometry::pieces(part, &processed) {
            if sink.stop() {
                return;
            }
            sink.extend(stroke(def.color, path, closed, width, def.cap, def.join));
        }
    }
    if let Some(border) = def
        .border
        .as_ref()
        .filter(|b| b.color >= 0 && b.width > 0.0)
    {
        borders(def, border, part, (start, end), &processed, sink);
    }
}

/// Charges the steps of a dash pattern that draw nothing themselves (skipped mid-symbol
/// positions, the work before a fallback to solid) and marks a limited pattern as cut.
fn charge_skipped(pattern: &geometry::DashPattern, sink: &mut Sink) {
    if pattern.limited {
        sink.mark_cut();
    }
    let drawn = pattern.intervals.len() + pattern.spots.len();
    sink.charge(pattern.steps.saturating_sub(drawn));
}

/// Mid symbols at `spots`, local +y to the right of travel.
fn mid_symbols(
    symbol: &PointDef,
    spots: &[(Pt, f64)],
    symbols: &RenderSymbols,
    sink: &mut Sink,
    depth: u32,
) {
    for &(at, angle) in spots {
        if sink.stop() {
            return;
        }
        let r = if symbol.rotatable { angle } else { 0.0 };
        point_symbol(symbol, at, r, symbols, sink, depth + 1);
    }
}

/// The filled taper of a pointed cap on `range` in the line colour (reference §2.7).
fn pointed_cap(def: &LineDef, part: &Part, range: (f64, f64), is_end: bool, sink: &mut Sink) {
    let width = mm(def.width);
    if def.color < 0 || width <= 0.0 {
        return;
    }
    let cap_length = mm(if is_end {
        def.end_offset
    } else {
        def.start_offset
    });
    let ring = geometry::pointed_cap(part, range.0, range.1, is_end, width / 2.0, cap_length);
    sink.charge_points(ring.len());
    sink.push(Primitive {
        priority: def.color,
        shape: Shape::Fill { rings: vec![ring] },
    });
}

/// Borders on both sides of the processed line (`processed`: the main line's stretches on
/// `range`), flat caps, miter (round if the line is round) [LS `createBorderLines`]. A dashed
/// border takes its own pattern on the centre line, cut to the main dashes.
fn borders(
    def: &LineDef,
    border: &Border,
    part: &Part,
    range: (f64, f64),
    processed: &[(f64, f64)],
    sink: &mut Sink,
) {
    let shift = mm(def.width) / 2.0 + mm(border.shift);
    let join = if def.join == Join::Round {
        Join::Round
    } else {
        Join::Miter
    };
    let intervals = match &border.dash {
        Some(dash) => {
            let pattern = geometry::dash_pattern(part, range.0, range.1, dash, None, sink.left());
            charge_skipped(&pattern, sink);
            if def.dash.is_some() {
                geometry::intersect(&pattern.intervals, processed)
            } else {
                pattern.intervals
            }
        }
        None => processed.to_vec(),
    };
    for (piece, closed) in geometry::pieces(part, &intervals) {
        if sink.stop() {
            return;
        }
        // A piece without output (a degenerate offset) still costs one unit.
        let len = sink.out.len();
        for side in [-1.0, 1.0] {
            let path = geometry::offset(&piece, closed, side * shift);
            sink.extend(stroke(
                border.color,
                path,
                closed,
                mm(border.width),
                Cap::Flat,
                join,
            ));
        }
        sink.settle(len);
    }
}

#[cfg(test)]
mod tests {
    use super::super::draw::{
        BUDGET_SLACK, MAX_PRIMITIVES_PER_OBJECT, Primitive, Shape, primitives, primitives_counted,
        work_units,
    };
    use super::super::symbols::{DashSymbol, Element, Mid, MidPlacement, PointDef, Symbol};
    use super::*;
    use crate::omap::coord::FLAG_DASH;
    use crate::omap::{MapObject, MapObjectKind, SymbolSet};

    const LINE: u32 = 9_998;

    fn plain(width: f64) -> LineDef {
        LineDef {
            color: 2,
            width,
            cap: Cap::Flat,
            join: Join::Miter,
            pointed_caps: false,
            start_offset: 0.0,
            end_offset: 0.0,
            dash: None,
            border: None,
            mid: None,
            start: None,
            end: None,
            dash_symbol: None,
        }
    }

    fn draw(def: LineDef, coords: Vec<MapCoord>) -> Vec<Primitive> {
        let mut r = RenderSymbols::from_set(&SymbolSet::issprom_2019());
        r.symbols.insert(LINE, Symbol::Line(def));
        let object = MapObject {
            symbol_id: LINE,
            kind: MapObjectKind::Path,
            coords,
            tags: Vec::new(),
            pattern: None,
        };
        primitives(&object, &r)
    }

    fn ten_mm() -> Vec<MapCoord> {
        vec![MapCoord::new(0, 0), MapCoord::new(10_000, 0)]
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    fn strokes(p: &[Primitive]) -> Vec<&Vec<Pt>> {
        p.iter()
            .filter_map(|p| match &p.shape {
                Shape::Stroke { path, .. } => Some(path),
                _ => None,
            })
            .collect()
    }

    fn fills(p: &[Primitive]) -> Vec<&Vec<Pt>> {
        p.iter()
            .filter_map(|p| match &p.shape {
                Shape::Fill { rings } => Some(&rings[0]),
                _ => None,
            })
            .collect()
    }

    /// Half the width of a cap polygon at `x`: its largest |y| among the vertices there.
    fn half_width_at(ring: &[Pt], x: f64) -> Option<f64> {
        ring.iter()
            .filter(|q| near(q.0, x))
            .map(|q| q.1.abs())
            .reduce(f64::max)
    }

    #[test]
    fn pointed_cap_tapers_to_zero() {
        // 10 mm, 1 mm wide, cap length 2 mm at both ends [LS `createPointedLineCap`].
        let def = LineDef {
            pointed_caps: true,
            start_offset: 2_000.0,
            end_offset: 2_000.0,
            width: 1_000.0,
            ..plain(0.0)
        };
        let p = draw(def, ten_mm());
        assert!(p.iter().all(|p| p.priority == 2));
        let main = strokes(&p);
        assert_eq!(main.len(), 1);
        assert_eq!(*main[0], vec![(2.0, 0.0), (8.0, 0.0)]);
        assert!(
            p.iter()
                .any(|p| matches!(p.shape, Shape::Stroke { cap: Cap::Flat, .. }))
        );
        let caps = fills(&p);
        assert_eq!(caps.len(), 2);
        // The start cap's polygon begins and ends at the tip, zero wide.
        assert_eq!(caps[0].first(), Some(&(0.0, 0.0)));
        assert_eq!(caps[0].last(), Some(&(0.0, 0.0)));
        assert_eq!(half_width_at(caps[0], 0.0), Some(0.0));
        assert!(near(half_width_at(caps[0], 2.0).unwrap(), 0.5));
        // The end cap ends at (10, 0), zero wide.
        assert_eq!(half_width_at(caps[1], 10.0), Some(0.0));
        assert!(near(half_width_at(caps[1], 8.0).unwrap(), 0.5));
    }

    #[test]
    fn start_end_offsets_shorten_line() {
        let def = LineDef {
            start_offset: 1_000.0,
            end_offset: 1_000.0,
            ..plain(300.0)
        };
        let p = draw(def, ten_mm());
        let main = strokes(&p);
        assert_eq!(main.len(), 1);
        assert!(near(geometry::length(main[0]), 8.0));
        assert_eq!(main[0][0], (1.0, 0.0));
        assert!(fills(&p).is_empty(), "no pointed caps");
    }

    #[test]
    fn offsets_longer_than_the_line_leave_only_the_caps() {
        // 6 + 6 mm on 10 mm: scaled to 5 + 5, no main line [LS `createSinglePathRenderables`].
        let def = LineDef {
            pointed_caps: true,
            start_offset: 6_000.0,
            end_offset: 6_000.0,
            ..plain(1_000.0)
        };
        let p = draw(def.clone(), ten_mm());
        assert!(strokes(&p).is_empty());
        let caps = fills(&p);
        assert_eq!(caps.len(), 2);
        // Half width at the middle: 0.5 · 5 / 6.
        assert!(near(half_width_at(caps[0], 5.0).unwrap(), 0.5 * 5.0 / 6.0));
        // Closed lines take no offsets.
        let ring = crate::omap::closed_ring(&[(0, 0), (10_000, 0), (10_000, 10_000)]);
        let p = draw(def, ring);
        assert!(fills(&p).is_empty());
        assert_eq!(strokes(&p).len(), 1);
    }

    /// A rotatable bar along local y, 1 mm long, 0.1 mm wide.
    fn bar() -> Box<PointDef> {
        Box::new(PointDef {
            rotatable: true,
            dot: None,
            ring: None,
            elements: vec![Element {
                symbol: Symbol::Line(LineDef {
                    color: 3,
                    ..plain(100.0)
                }),
                coords: vec![MapCoord::new(0, -500), MapCoord::new(0, 500)],
            }],
        })
    }

    #[test]
    fn dash_symbol_at_dash_points() {
        // East 5 mm, then south 5 mm; the corner and the end are dash points.
        let mut coords = vec![
            MapCoord::new(0, 0),
            MapCoord::new(5_000, 0),
            MapCoord::new(5_000, 5_000),
        ];
        coords[1].flags = FLAG_DASH;
        coords[2].flags = FLAG_DASH;
        let symbol = |suppress_at_ends, scale| LineDef {
            dash_symbol: Some(DashSymbol {
                symbol: bar(),
                suppress_at_ends,
                scale,
            }),
            ..plain(200.0)
        };
        let bars = |p: &[Primitive]| -> Vec<Vec<Pt>> {
            p.iter()
                .filter(|p| p.priority == 3)
                .filter_map(|p| match &p.shape {
                    Shape::Stroke { path, .. } => Some(path.clone()),
                    _ => None,
                })
                .collect()
        };
        let p = draw(symbol(false, true), coords.clone());
        let found = bars(&p);
        assert_eq!(found.len(), 2, "{found:?}");
        // At the corner: rotated to the bisector (south-east), the bar along local y points
        // south-west, scaled by 1 / cos 45° = √2 [LS `createDashSymbolRenderables`].
        let corner = &found[0];
        let centre = (
            (corner[0].0 + corner[1].0) / 2.0,
            (corner[0].1 + corner[1].1) / 2.0,
        );
        assert!(near(centre.0, 5.0) && near(centre.1, 0.0), "{corner:?}");
        assert!(near(geometry::length(corner), std::f64::consts::SQRT_2));
        assert!(near(corner[1].0 - corner[0].0, -1.0) && near(corner[1].1 - corner[0].1, 1.0));
        // At the end: the incoming direction (south), unscaled; the bar runs east-west.
        let end = &found[1];
        assert!(near(geometry::length(end), 1.0) && near(end[0].1, end[1].1));
        // Not scaled (element coordinates are rounded to 1 µm).
        let p = draw(symbol(false, false), coords.clone());
        assert!((geometry::length(&bars(&p)[0]) - 1.0).abs() < 0.002);
        // Suppressed at the ends: only the corner.
        let p = draw(symbol(true, true), coords.clone());
        assert_eq!(bars(&p).len(), 1);
        // No dash points, no dash symbols.
        let plain_coords: Vec<MapCoord> = coords.iter().map(|c| MapCoord::new(c.x, c.y)).collect();
        assert!(bars(&draw(symbol(false, true), plain_coords)).is_empty());
    }

    #[test]
    fn mid_symbols_on_dashed_line() {
        // 10 mm of 506 dashes with a dot in the centre of every dash (createDashGroups).
        let dot = Box::new(PointDef {
            rotatable: false,
            dot: Some((3, 100.0)),
            ring: None,
            elements: Vec::new(),
        });
        let def = LineDef {
            dash: Some(super::super::symbols::Dash {
                dash: 1500.0,
                gap: 370.0,
                group: 1,
                in_group_gap: 0.0,
                half_outer: false,
            }),
            mid: Some(Mid {
                symbol: dot,
                segment: 0.0,
                end: 0.0,
                per_spot: 1,
                distance: 0.0,
                at_least_one: false,
                placement: MidPlacement::CentreOfDash,
            }),
            ..plain(200.0)
        };
        let p = draw(def, ten_mm());
        assert_eq!(strokes(&p).len(), 6);
        let d = (10.0 - 5.0 * 0.37) / 6.0;
        let dots = fills(&p);
        assert_eq!(dots.len(), 6);
        for (i, ring) in dots.iter().enumerate() {
            let x = ring[0].0 - 0.1;
            assert!(near(x, i as f64 * (d + 0.37) + d / 2.0), "{i}: {x}");
        }
    }

    #[test]
    fn borders_follow_offsets_and_main_dashes() {
        let border = super::super::symbols::Border {
            color: 4,
            width: 100.0,
            shift: 0.0,
            dash: None,
        };
        let def = LineDef {
            start_offset: 1_000.0,
            end_offset: 1_000.0,
            border: Some(border.clone()),
            ..plain(400.0)
        };
        let p = draw(def, ten_mm());
        let borders: Vec<&Vec<Pt>> = p
            .iter()
            .filter(|p| p.priority == 4)
            .filter_map(|p| match &p.shape {
                Shape::Stroke { path, .. } => Some(path),
                _ => None,
            })
            .collect();
        assert_eq!(borders.len(), 2);
        assert!(
            borders
                .iter()
                .all(|b| near(b[0].0, 1.0) && near(b[1].0, 9.0))
        );
        // A dashed main line: the borders are dashed alike [LS `createSinglePathRenderables`].
        let def = LineDef {
            dash: Some(super::super::symbols::Dash {
                dash: 1500.0,
                gap: 370.0,
                group: 1,
                in_group_gap: 0.0,
                half_outer: false,
            }),
            border: Some(border),
            ..plain(400.0)
        };
        let p = draw(def, ten_mm());
        assert_eq!(p.iter().filter(|p| p.priority == 4).count(), 12);
    }

    fn dot(color: i32) -> Box<PointDef> {
        Box::new(PointDef {
            rotatable: false,
            dot: Some((color, 100.0)),
            ring: None,
            elements: Vec::new(),
        })
    }

    /// Ten 900 mm parts.
    fn ten_long_parts() -> Vec<MapCoord> {
        (0..10)
            .flat_map(|i| {
                [
                    MapCoord::new(0, i * 1_000),
                    MapCoord {
                        flags: crate::omap::coord::FLAG_HOLE,
                        ..MapCoord::new(900_000, i * 1_000)
                    },
                ]
            })
            .collect()
    }

    /// The primitives of `def` on `coords`, whether they were cut, and the work units spent.
    fn counted(def: LineDef, coords: Vec<MapCoord>) -> (usize, bool, usize) {
        let mut r = RenderSymbols::from_set(&SymbolSet::issprom_2019());
        r.symbols.insert(LINE, Symbol::Line(def));
        let object = MapObject {
            symbol_id: LINE,
            kind: MapObjectKind::Path,
            coords,
            tags: Vec::new(),
            pattern: None,
        };
        let before = work_units();
        let (p, cut) = primitives_counted(&object, &r);
        (p.len(), cut, work_units() - before)
    }

    #[test]
    fn hostile_mid_symbols_on_dashes_are_bounded() {
        // Four billion symbols per spot: all placed at the centre of a dash group (distance 0),
        // or spread 1 mm apart from far before the group or gap, so that nearly all are tried and
        // skipped. The budget stops each.
        let cases = [
            (MidPlacement::CentreOfDashGroup, 0.0),
            (MidPlacement::CentreOfDashGroup, 1_000.0),
            (MidPlacement::CentreOfGap, 1_000.0),
        ];
        for (placement, distance) in cases {
            let def = LineDef {
                dash: Some(super::super::symbols::Dash {
                    dash: 1500.0,
                    gap: 370.0,
                    group: 1,
                    in_group_gap: 0.0,
                    half_outer: false,
                }),
                mid: Some(Mid {
                    symbol: dot(3),
                    segment: 0.0,
                    end: 0.0,
                    per_spot: u32::MAX,
                    distance,
                    at_least_one: false,
                    placement,
                }),
                ..plain(200.0)
            };
            let (_, cut, work) = counted(def, ten_long_parts());
            assert!(cut, "{placement:?} {distance}");
            assert!(
                work <= MAX_PRIMITIVES_PER_OBJECT + BUDGET_SLACK,
                "{placement:?} {distance}: {work}"
            );
            assert!(
                work >= MAX_PRIMITIVES_PER_OBJECT,
                "skipped steps are charged: {work}"
            );
        }
    }

    #[test]
    fn hostile_dash_points_are_bounded() {
        // 300 000 dash points with a dash symbol, and the same as sections of a dashed line with
        // mid symbols: the budget stops both.
        let coords: Vec<MapCoord> = (0..300_000)
            .map(|i| MapCoord {
                flags: FLAG_DASH,
                ..MapCoord::new(i * 10, (i % 2) * 10)
            })
            .collect();
        let def = LineDef {
            dash_symbol: Some(DashSymbol {
                symbol: dot(3),
                suppress_at_ends: false,
                scale: true,
            }),
            ..plain(200.0)
        };
        let (n, cut, work) = counted(def, coords.clone());
        assert!(cut && n <= MAX_PRIMITIVES_PER_OBJECT);
        assert!(work <= MAX_PRIMITIVES_PER_OBJECT + BUDGET_SLACK, "{work}");
        let def = LineDef {
            dash: Some(super::super::symbols::Dash {
                dash: 1.0,
                gap: 1.0,
                group: 1,
                in_group_gap: 0.0,
                half_outer: false,
            }),
            mid: Some(Mid {
                symbol: dot(3),
                segment: 0.0,
                end: 0.0,
                per_spot: 1,
                distance: 0.0,
                at_least_one: true,
                placement: MidPlacement::CentreOfDash,
            }),
            ..plain(200.0)
        };
        let (_, cut, work) = counted(def, coords);
        assert!(cut);
        assert!(work <= MAX_PRIMITIVES_PER_OBJECT + BUDGET_SLACK, "{work}");
    }

    #[test]
    fn pointed_caps_on_long_curvy_lines_are_charged() {
        // Ten parts of 20 000 points each, all inside the 1 m caps.
        let coords: Vec<MapCoord> = (0..10)
            .flat_map(|p| {
                (0..20_000).map(move |i| MapCoord {
                    flags: if i == 19_999 {
                        crate::omap::coord::FLAG_HOLE
                    } else {
                        0
                    },
                    ..MapCoord::new(i * 10, p * 1_000 + (i % 2) * 10)
                })
            })
            .collect();
        let def = LineDef {
            pointed_caps: true,
            start_offset: 1_000_000.0,
            end_offset: 1_000_000.0,
            ..plain(200.0)
        };
        let (n, cut, work) = counted(def, coords);
        assert_eq!((n, cut), (20, false));
        // Each cap ring of about 20 000 points costs 20 000 / 64 units.
        assert!(work >= 20 * (20_000 / 64), "{work}");
    }
}
