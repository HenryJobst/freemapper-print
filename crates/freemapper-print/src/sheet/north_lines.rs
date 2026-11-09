//! North lines for maps that have none (ADR 0027, ADR 0023).
//!
//! Pure helpers: which symbol the lines take, how far apart they lie, and where. `render_sheet`
//! puts them together. All map positions here are millimetres of the map at its own scale; a
//! north line is vertical there, because OOM draws the map oriented to magnetic north.

use crate::omap::{MapCoord, MapDocument, MapObject, MapObjectKind, MapPart};
use crate::print::is_north_line;
use crate::render::page::PaperBox;
use crate::scale::{MapKind, north_line_spacing_m};

/// The most lines generated for one sheet. A frame holds a few dozen; the cap is for hostile
/// numbers.
pub(super) const MAX_LINES: usize = 10_000;

/// Width of the plain fallback line (mm at the base scale), the width of ISSprOM symbol 601.
pub(super) const FALLBACK_WIDTH_MM: f64 = 0.14;

/// Name of the part that holds generated lines. It is added after the layout hid its parts.
pub(super) const PART_NAME: &str = "north lines (generated)";

/// The lines stop this far (page mm) inside the frame, so that the painted line, with its width
/// and its round or square end caps, does not reach out of the frame and show up as overflow in
/// the report. 0.5 mm covers a line up to 1 mm wide.
pub(super) const INSET_MM: f64 = 0.5;

/// Whether any object of `map`, in any part, is a north line (601 or 601.x). Asked of the whole
/// map before the layout hides parts: a layout that hides the part with the north lines gets no
/// replacements.
pub(super) fn has_north_lines(map: &MapDocument) -> bool {
    map.parts
        .iter()
        .flat_map(|p| &p.objects)
        .any(|o| is_north_line(&map.symbol_set, o))
}

/// The symbol to draw generated lines with, from the `(code, id)` pairs of the symbol set: 601,
/// else 601.1, else the lowest 601.x by its number. `None` without any.
pub(super) fn symbol_id<'a>(codes: impl Iterator<Item = (&'a str, u32)>) -> Option<u32> {
    let mut best: Option<((u8, u32, &str), u32)> = None;
    for (code, id) in codes {
        let key = if code == "601" {
            (0, 0, code)
        } else if let Some(n) = code.strip_prefix("601.") {
            (1, n.parse().unwrap_or(u32::MAX), code)
        } else {
            continue;
        };
        if best.is_none_or(|(k, _)| key < k) {
            best = Some((key, id));
        }
    }
    best.map(|(_, id)| id)
}

/// Distance between two lines on the map (mm at the map scale): the ground spacing of the kind
/// at the map scale.
pub(super) fn spacing_mm(kind: MapKind, map_scale: u32) -> f64 {
    north_line_spacing_m(kind) * 1000.0 / f64::from(map_scale)
}

/// The map frame in map mm (at the map scale): the `clip` of a page whose page centre shows
/// `centre_mm` (print mm), at geometry factor `gf`, with the line inset taken off.
pub(super) fn frame_in_map_mm(
    clip: PaperBox,
    page_mm: (f64, f64),
    centre_mm: (f64, f64),
    gf: f64,
) -> PaperBox {
    let to_map = |x: f64, y: f64| {
        (
            (x - page_mm.0 / 2.0 + centre_mm.0) / gf,
            (y - page_mm.1 / 2.0 + centre_mm.1) / gf,
        )
    };
    PaperBox {
        min: to_map(clip.min.0 + INSET_MM, clip.min.1 + INSET_MM),
        max: to_map(clip.max.0 - INSET_MM, clip.max.1 - INSET_MM),
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
    fn spacing_is_ground_distance_on_the_map() {
        assert_eq!(spacing_mm(MapKind::Sprint, 4000), 30.0);
        assert_eq!(spacing_mm(MapKind::Forest, 15000), 20.0);
        assert_eq!(spacing_mm(MapKind::Schoolyard, 1000), 37.5);
    }
}
