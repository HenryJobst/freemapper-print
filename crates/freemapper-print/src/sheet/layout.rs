//! Geometry of a layout on the page: the map frame border, where an anchored element lands, and
//! the notes about an element's place.

use crate::print::Paper;
use crate::project::{Anchor, AnchorTarget, Corner};
use crate::render::page::{Cap, DirectColour, Join, PaperBox, Pt, Shape, SheetItem};

use super::SheetNote;

/// Room for rounding (mm) when an element's edge meets the page or frame edge.
const EPSILON_MM: f64 = 1e-9;

/// Where along one axis an anchor sits: the start (left, top), the middle or the end edge.
#[derive(Clone, Copy)]
enum Along {
    Start,
    Middle,
    End,
}

fn along(corner: Corner) -> (Along, Along) {
    use Along::{End, Middle, Start};
    match corner {
        Corner::TopLeft => (Start, Start),
        Corner::Top => (Middle, Start),
        Corner::TopRight => (End, Start),
        Corner::Left => (Start, Middle),
        Corner::Centre => (Middle, Middle),
        Corner::Right => (End, Middle),
        Corner::BottomLeft => (Start, End),
        Corner::Bottom => (Middle, End),
        Corner::BottomRight => (End, End),
    }
}

/// The start of an element `len` long on the axis `min..max`, `offset` inward from `at`. From the
/// middle the offset counts towards the end (right, down).
fn place(at: Along, min: f64, max: f64, len: f64, offset: f64) -> f64 {
    match at {
        Along::Start => min + offset,
        Along::Middle => (min + max) / 2.0 - len / 2.0 + offset,
        Along::End => max - offset - len,
    }
}

/// The top-left corner (page mm) of an element of `size` (width, height in mm) hung on `anchor`.
///
/// The offset is measured inward from the anchor: from a left or top edge to the right or down,
/// from a right or bottom edge to the left or up, and from the middle to the right or down. The
/// element sits on the inner side of its anchor, so `top_right` with no offset puts its top-right
/// corner on the top-right corner of the box.
pub fn anchor_point(anchor: &Anchor, page: PaperBox, frame: PaperBox, size: (f64, f64)) -> Pt {
    let b = match anchor.to {
        AnchorTarget::Page => page,
        AnchorTarget::MapFrame => frame,
    };
    let (x, y) = along(anchor.corner);
    let [dx, dy] = anchor.offset_mm;
    (
        place(x, b.min.0, b.max.0, size.0, dx),
        place(y, b.min.1, b.max.1, size.1, dy),
    )
}

/// The frame line: a black stroke `border_mm` wide centred on the edge of `frame`, so half of it
/// covers the cut edge of the map; none for 0.
pub fn frame_border(frame: PaperBox, border_mm: f64) -> Option<SheetItem> {
    (border_mm > 0.0).then(|| SheetItem::Shape {
        colour: DirectColour::BLACK,
        shape: Shape::Stroke {
            path: vec![
                frame.min,
                (frame.max.0, frame.min.1),
                frame.max,
                (frame.min.0, frame.max.1),
            ],
            closed: true,
            width: border_mm,
            cap: Cap::Flat,
            join: Join::Miter,
        },
    })
}

/// The notes for an element `id` hung on `to` that covers `rect`: [`SheetNote::ElementOutsidePage`]
/// when any of it lies outside the page, [`SheetNote::ElementOverlapsMap`] when it hangs on the
/// page and covers part of the map frame. An element hung on the map frame is meant to sit on
/// the map; that is not noted.
pub fn bounds_notes(
    id: &str,
    to: AnchorTarget,
    rect: PaperBox,
    page: PaperBox,
    frame: PaperBox,
) -> Vec<SheetNote> {
    let mut notes = Vec::new();
    let outside = rect.min.0 < page.min.0 - EPSILON_MM
        || rect.min.1 < page.min.1 - EPSILON_MM
        || rect.max.0 > page.max.0 + EPSILON_MM
        || rect.max.1 > page.max.1 + EPSILON_MM;
    if outside {
        notes.push(SheetNote::ElementOutsidePage { id: id.into() });
    }
    let overlaps = |a: (f64, f64), b: (f64, f64)| a.0.max(b.0) + EPSILON_MM < a.1.min(b.1);
    if to == AnchorTarget::Page
        && overlaps((rect.min.0, rect.max.0), (frame.min.0, frame.max.0))
        && overlaps((rect.min.1, rect.max.1), (frame.min.1, frame.max.1))
    {
        notes.push(SheetNote::ElementOverlapsMap { id: id.into() });
    }
    notes
}

/// Whether `paper` is larger than A3, which most office printers do not take.
pub fn larger_than_a3(paper: Paper) -> bool {
    let (w, h) = paper.size_mm();
    let (a3w, a3h) = Paper::A3.size_mm();
    w * h > a3w * a3h
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: PaperBox = PaperBox {
        min: (0.0, 0.0),
        max: (210.0, 297.0),
    };
    const FRAME: PaperBox = PaperBox {
        min: (8.0, 8.0),
        max: (202.0, 289.0),
    };

    fn anchor(to: AnchorTarget, corner: Corner) -> Anchor {
        Anchor {
            to,
            corner,
            offset_mm: [3.0, 2.0],
        }
    }

    fn rect(min: Pt, max: Pt) -> PaperBox {
        PaperBox { min, max }
    }

    #[test]
    fn anchor_corners() {
        // A 20 × 10 mm element, offset 3 mm in x and 2 mm in y, measured inward from the corner
        // (to the right and down from the middle of an edge or of the box).
        let size = (20.0, 10.0);
        let cases = [
            (Corner::TopLeft, (3.0, 2.0), (11.0, 10.0)),
            (Corner::Top, (98.0, 2.0), (98.0, 10.0)),
            (Corner::TopRight, (187.0, 2.0), (179.0, 10.0)),
            (Corner::Left, (3.0, 145.5), (11.0, 145.5)),
            (Corner::Centre, (98.0, 145.5), (98.0, 145.5)),
            (Corner::Right, (187.0, 145.5), (179.0, 145.5)),
            (Corner::BottomLeft, (3.0, 285.0), (11.0, 277.0)),
            (Corner::Bottom, (98.0, 285.0), (98.0, 277.0)),
            (Corner::BottomRight, (187.0, 285.0), (179.0, 277.0)),
        ];
        for (corner, on_page, on_frame) in cases {
            let p = anchor_point(&anchor(AnchorTarget::Page, corner), PAGE, FRAME, size);
            assert_eq!(p, on_page, "page {corner:?}");
            let f = anchor_point(&anchor(AnchorTarget::MapFrame, corner), PAGE, FRAME, size);
            assert_eq!(f, on_frame, "frame {corner:?}");
        }
    }

    #[test]
    fn frame_border_drawn() {
        let Some(SheetItem::Shape { colour, shape }) = frame_border(FRAME, 0.25) else {
            panic!("no border")
        };
        assert_eq!(colour, DirectColour::BLACK);
        assert_eq!(
            shape,
            Shape::Stroke {
                path: vec![(8.0, 8.0), (202.0, 8.0), (202.0, 289.0), (8.0, 289.0)],
                closed: true,
                width: 0.25,
                cap: Cap::Flat,
                join: Join::Miter,
            }
        );
        assert_eq!(frame_border(FRAME, 0.0), None);
    }

    #[test]
    fn element_outside_page_noted() {
        // Inside the passe-partout: nothing to note.
        let in_margin = rect((2.0, 290.0), (40.0, 296.0));
        assert_eq!(
            bounds_notes("t", AnchorTarget::Page, in_margin, PAGE, FRAME),
            []
        );
        // Over the page edge.
        let over_edge = rect((195.0, 290.0), (215.0, 296.0));
        assert_eq!(
            bounds_notes("t", AnchorTarget::Page, over_edge, PAGE, FRAME),
            [SheetNote::ElementOutsidePage { id: "t".into() }]
        );
        let above = rect((20.0, -1.0), (40.0, 5.0));
        assert_eq!(
            bounds_notes("t", AnchorTarget::MapFrame, above, PAGE, FRAME),
            [SheetNote::ElementOutsidePage { id: "t".into() }]
        );
        // Hung on the page but reaching over the map: allowed, noted.
        let over_map = rect((2.0, 280.0), (40.0, 296.0));
        assert_eq!(
            bounds_notes("t", AnchorTarget::Page, over_map, PAGE, FRAME),
            [SheetNote::ElementOverlapsMap { id: "t".into() }]
        );
        // Hung on the map frame and sitting on the map: what it is meant for.
        assert_eq!(
            bounds_notes("t", AnchorTarget::MapFrame, over_map, PAGE, FRAME),
            []
        );
        // Touching the frame edge is not overlapping.
        let touching = rect((2.0, 289.0), (40.0, 296.0));
        assert_eq!(
            bounds_notes("t", AnchorTarget::Page, touching, PAGE, FRAME),
            []
        );
    }

    #[test]
    fn only_paper_above_a3_is_larger_than_a3() {
        assert!(!larger_than_a3(Paper::A4));
        assert!(!larger_than_a3(Paper::A3));
    }
}
