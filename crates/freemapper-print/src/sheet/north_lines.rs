//! North lines for maps that have none (ADR 0027, ADR 0023).
//!
//! Pure helpers: which symbol the lines take, how far apart they lie, and where. `render_sheet`
//! puts them together. All map positions here are millimetres of the map at its own scale; a
//! north line is vertical there, because OOM draws the map oriented to magnetic north.

use std::collections::HashMap;

use crate::omap::{MapCoord, MapDocument, MapObject, MapObjectKind, MapPart, SymbolSet};
use crate::print::is_north_line;
use crate::render::page::PaperBox;
use crate::render::symbols::{Part as SymbolPart, RenderSymbols, Symbol};
use crate::scale::{MapKind, north_line_spacing_m};

/// The most lines generated for one sheet. A frame holds a few dozen; the cap is for hostile
/// numbers.
pub(super) const MAX_LINES: usize = 10_000;

/// Width of the plain fallback line (mm at the base scale), the width of ISSprOM symbol 601.
pub(super) const FALLBACK_WIDTH_MM: f64 = 0.14;

/// Name of the part that holds generated lines. It is added after the layout hid its parts.
pub(super) const PART_NAME: &str = "north lines (generated)";

/// The lines stop this far (page mm) before the frame edge on top of half their painted width, so
/// that the painted line does not reach out of the frame and show up as overflow in the report
/// (rounding to whole µm, which the drawing does, could otherwise do it). The inset follows the
/// symbol, so a thin line leaves a gap of well under 0.1 mm.
const INSET_MARGIN_MM: f64 = 0.01;

/// Whether any object of `map`, in any part, is a north line (601 or 601.x). Asked of the whole
/// map before the layout hides parts: a layout that hides the part with the north lines gets no
/// replacements.
pub(super) fn has_north_lines(map: &MapDocument) -> bool {
    map.parts
        .iter()
        .flat_map(|p| &p.objects)
        .any(|o| is_north_line(&map.symbol_set, o))
}

/// Whether `id` can draw a line along a two-point path: a line symbol (type 2) or a combined
/// one (type 16), and not hidden in the file. An area symbol such as the "North lines pattern"
/// 601.1 draws nothing along a path, and a hidden one is skipped by the renderer.
pub(super) fn draws_a_line(set: &SymbolSet, id: u32) -> bool {
    !set.is_hidden(id) && matches!(set.symbol_type(id), Some(2 | 16))
}

/// The symbol to draw generated lines with, from the `(code, id)` pairs of the symbol set that
/// pass `usable`: 601, else 601.1, else the lowest 601.x by its number. `None` without any.
pub(super) fn symbol_id<'a>(
    codes: impl Iterator<Item = (&'a str, u32)>,
    usable: impl Fn(u32) -> bool,
) -> Option<u32> {
    let mut best: Option<((u8, u32, &str), u32)> = None;
    for (code, id) in codes {
        let key = if code == "601" {
            (0, 0, code)
        } else if let Some(n) = code.strip_prefix("601.") {
            (1, n.parse().unwrap_or(u32::MAX), code)
        } else {
            continue;
        };
        if usable(id) && best.is_none_or(|(k, _)| key < k) {
            best = Some((key, id));
        }
    }
    best.map(|(_, id)| id)
}

/// The most symbols one width walk looks at, and the deepest nesting it follows. A hostile file
/// can declare combined symbols with thousands of references to other combined symbols.
const MAX_VISITS: usize = 4096;
const MAX_DEPTH: u8 = 8;

/// State of one walk over combined symbols: each symbol id is measured once (memo), and the walk
/// stops looking at new symbols after [`MAX_VISITS`]; it then reports the widest seen so far.
struct Walk<'a> {
    all: &'a RenderSymbols,
    memo: HashMap<u32, f64>,
    visits: usize,
}

impl Walk<'_> {
    fn widest(&mut self, symbol: &Symbol, depth: u8) -> f64 {
        self.visits += 1;
        if self.visits > MAX_VISITS {
            return 0.0;
        }
        match symbol {
            Symbol::Line(def) => {
                let border = def.border.as_ref().map_or(0.0, |b| b.width + b.shift.abs());
                def.width.max(border)
            }
            Symbol::Combined(parts) if depth < MAX_DEPTH => {
                let mut widest = 0.0_f64;
                for part in parts {
                    let w = match part {
                        SymbolPart::Ref(id) => self.widest_of(*id, depth + 1),
                        SymbolPart::Inline(s) => self.widest(s, depth + 1),
                    };
                    widest = widest.max(w);
                    if self.visits > MAX_VISITS {
                        break;
                    }
                }
                widest
            }
            _ => 0.0,
        }
    }

    fn widest_of(&mut self, id: u32, depth: u8) -> f64 {
        if let Some(w) = self.memo.get(&id) {
            return *w;
        }
        let Some(symbol) = self.all.symbols.get(&id) else {
            return 0.0;
        };
        // 0 while it is being measured, so a reference cycle ends.
        self.memo.insert(id, 0.0);
        let w = self.widest(symbol, depth);
        self.memo.insert(id, w);
        w
    }
}

/// The width (µm) of the widest line `id` paints, and how many symbols the walk looked at.
fn widest_um(all: &RenderSymbols, id: u32) -> (f64, usize) {
    let mut walk = Walk {
        all,
        memo: HashMap::new(),
        visits: 0,
    };
    let w = walk.widest_of(id, 0);
    (w, walk.visits)
}

/// Half the widest line `id` paints (mm), at symbol factor `factor`. A combined symbol takes
/// its widest part; a symbol that cannot be read gives 0.
pub(super) fn half_width_mm(set: &SymbolSet, id: u32, factor: f64) -> f64 {
    let Ok(all) = RenderSymbols::from_set(set) else {
        return 0.0;
    };
    let (width_um, _) = widest_um(&all, id);
    // Width in µm as the symbol file has it.
    (width_um / 1000.0 * factor / 2.0).max(0.0)
}

/// The inset (page mm) for lines of a symbol whose half width is `half_width_mm`.
pub(super) fn inset_mm(half_width_mm: f64) -> f64 {
    half_width_mm + INSET_MARGIN_MM
}

/// Distance between two lines on the map (mm at the map scale): the ground spacing of the kind
/// at the map scale.
pub(super) fn spacing_mm(kind: MapKind, map_scale: u32) -> f64 {
    north_line_spacing_m(kind) * 1000.0 / f64::from(map_scale)
}

/// The map frame in map mm (at the map scale): the `clip` of a page whose page centre shows
/// `centre_mm` (print mm), at geometry factor `gf`, with `inset` (page mm) taken off every side.
/// The result is clamped to the range in which a coordinate survives scaling to print µm without
/// saturating (`i32`), so lines never pile up at the end of that range.
pub(super) fn frame_in_map_mm(
    clip: PaperBox,
    page_mm: (f64, f64),
    centre_mm: (f64, f64),
    gf: f64,
    inset: f64,
) -> PaperBox {
    let to_map = |x: f64, y: f64| {
        (
            (x - page_mm.0 / 2.0 + centre_mm.0) / gf,
            (y - page_mm.1 / 2.0 + centre_mm.1) / gf,
        )
    };
    // Conservative: a geometry factor below 1 does not widen the range.
    let limit = f64::from(i32::MAX) / 1000.0 / gf.max(1.0);
    let clamp = |(x, y): (f64, f64)| (x.clamp(-limit, limit), y.clamp(-limit, limit));
    PaperBox {
        min: clamp(to_map(clip.min.0 + inset, clip.min.1 + inset)),
        max: clamp(to_map(clip.max.0 - inset, clip.max.1 - inset)),
    }
}

/// Map x (mm) of the lines between `lo` and `hi`: every multiple of `spacing`, counted from map
/// x = 0, at most [`MAX_LINES`]. Empty for a range or spacing that is not finite and positive.
pub(super) fn line_xs_mm(spacing: f64, lo: f64, hi: f64) -> Vec<f64> {
    if !(spacing.is_finite() && spacing > 0.0 && lo.is_finite() && hi.is_finite() && lo <= hi) {
        return Vec::new();
    }
    let first = (lo / spacing).ceil() as i64;
    let last = (hi / spacing).floor() as i64;
    (first..=last)
        .take(MAX_LINES)
        .map(|k| k as f64 * spacing)
        .collect()
}

/// A part of vertical objects of `symbol` at map x `xs_mm`, from map y `y_lo` to `y_hi` (mm).
pub(super) fn part(symbol: u32, xs_mm: &[f64], y_lo: f64, y_hi: f64) -> MapPart {
    let um = |mm: f64| (mm * 1000.0).round() as i32;
    MapPart {
        name: PART_NAME.into(),
        objects: xs_mm
            .iter()
            .map(|x| MapObject {
                symbol_id: symbol,
                kind: MapObjectKind::Path,
                coords: vec![
                    MapCoord::new(um(*x), um(y_lo)),
                    MapCoord::new(um(*x), um(y_hi)),
                ],
                tags: Vec::new(),
                pattern: None,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_lie_on_multiples_of_the_spacing_from_zero() {
        assert_eq!(line_xs_mm(30.0, -50.0, 70.0), [-30.0, 0.0, 30.0, 60.0]);
        assert_eq!(line_xs_mm(30.0, 0.0, 0.0), [0.0]);
        assert!(line_xs_mm(30.0, 1.0, 29.0).is_empty());
    }

    #[test]
    fn count_is_capped_and_bad_numbers_give_none() {
        assert_eq!(line_xs_mm(1e-6, 0.0, 1e9).len(), MAX_LINES);
        assert!(line_xs_mm(0.0, 0.0, 10.0).is_empty());
        assert!(line_xs_mm(30.0, f64::NAN, 10.0).is_empty());
        assert!(line_xs_mm(30.0, 0.0, f64::INFINITY).is_empty());
        assert!(line_xs_mm(30.0, 10.0, 0.0).is_empty());
    }

    #[test]
    fn frame_is_clamped_to_the_coordinate_range() {
        let clip = PaperBox {
            min: (8.0, 8.0),
            max: (202.0, 289.0),
        };
        let f = frame_in_map_mm(clip, (210.0, 297.0), (1e15, -1e15), 1.0, 0.1);
        assert!(f.max.0 <= 2_147_483.7 && f.min.1 >= -2_147_483.7, "{f:?}");
        assert!(line_xs_mm(30.0, f.min.0, f.max.0).len() <= 1);
    }

    #[test]
    fn half_width_follows_the_symbol_and_the_factor() {
        let set = SymbolSet::issprom_2019();
        let id = set.id_for_code("601").unwrap();
        let w = half_width_mm(&set, id, 1.0);
        assert!(w > 0.03 && w < 0.2, "{w}");
        assert!((half_width_mm(&set, id, 2.0) - 2.0 * w).abs() < 1e-12);
        assert!(draws_a_line(&set, id));
    }

    #[test]
    fn combined_symbol_walk_is_bounded() {
        use std::collections::HashSet;
        // Id 0 refers to 1000 combined symbols (1..=1000), each referring to the same 1000
        // other combined symbols (1001..=2000), each referring to 1000 more, and so on: about
        // 1e12 visits without a memo.
        let layers = 6_u32;
        let width = 1000_u32;
        let mut symbols = HashMap::new();
        for layer in 0..layers {
            let ids: Vec<u32> = if layer == 0 {
                vec![0]
            } else {
                (0..width).map(|i| layer * width + i).collect()
            };
            for id in ids {
                let next = (layer + 1) * width;
                let parts = (0..width).map(|i| SymbolPart::Ref(next + i)).collect();
                symbols.insert(id, Symbol::Combined(parts));
            }
        }
        let all = RenderSymbols {
            colours: Vec::new(),
            symbols,
            dropped_patterns: HashSet::new(),
        };
        let (w, visits) = widest_um(&all, 0);
        assert_eq!(w, 0.0);
        assert!(visits <= MAX_VISITS + 1, "{visits}");
        // A cycle ends too.
        let mut cyc = HashMap::new();
        cyc.insert(
            1,
            Symbol::Combined(vec![SymbolPart::Ref(1), SymbolPart::Ref(2)]),
        );
        cyc.insert(2, Symbol::Combined(vec![SymbolPart::Ref(1)]));
        let all = RenderSymbols {
            colours: Vec::new(),
            symbols: cyc,
            dropped_patterns: HashSet::new(),
        };
        assert!(widest_um(&all, 1).1 < 10);
    }

    #[test]
    fn spacing_is_ground_distance_on_the_map() {
        assert_eq!(spacing_mm(MapKind::Sprint, 4000), 30.0);
        assert_eq!(spacing_mm(MapKind::Forest, 15000), 20.0);
        assert_eq!(spacing_mm(MapKind::Schoolyard, 1000), 37.5);
    }
}
