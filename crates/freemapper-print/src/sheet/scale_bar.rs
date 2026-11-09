//! The scale check bar (design §5.4): a bar of round length in metres, printed to scale, so that a
//! ruler on the printout shows whether the printer scaled the page. Sizes are in mm.

use crate::project::{ElementSpec, SheetLang};
use crate::render::fonts::{ASCENT, LINE_SPACING, face, text_width};
use crate::render::page::{Cap, DirectColour, FillRule, Join, PaperBox, Pt, Shape, SheetItem};

use super::{PlacedElement, SheetContext, layout};

/// Bar lengths in metres, ascending.
const LENGTHS_M: [u32; 9] = [5, 10, 20, 50, 100, 200, 250, 500, 1000];
/// The bar is at most this long on paper (mm).
const MAX_PRINTED_MM: u32 = 50;
const SEGMENTS: usize = 5;
const BAR_HEIGHT_MM: f64 = 2.0;
/// Outline of the bar and width of the end ticks (mm).
const LINE_MM: f64 = 0.2;
const TICK_HEIGHT_MM: f64 = 1.0;
/// Label size, 7 pt in mm.
const LABEL_MM: f64 = 7.0 * 25.4 / 72.0;
/// White box around the whole element (mm).
const PADDING_MM: f64 = 1.0;

/// The bar length in metres for `print_scale`: the largest of the list whose printed length is at
/// most 50 mm (the smallest if none is).
fn length_m(print_scale: u32) -> u32 {
    let limit = u64::from(MAX_PRINTED_MM) * u64::from(print_scale.max(1));
    LENGTHS_M
        .iter()
        .rev()
        .copied()
        .find(|&l| u64::from(l) * 1000 <= limit)
        .unwrap_or(LENGTHS_M[0])
}

/// `200 m = 50 mm`; one decimal only when the printed length is not whole: `100 m = 33,3 mm`
/// (comma on German sheets).
fn label(length_m: u32, printed_mm: f64, lang: SheetLang) -> String {
    let rounded = (printed_mm * 10.0).round() / 10.0;
    let number = if (rounded - rounded.round()).abs() < 1e-9 {
        format!("{}", rounded.round())
    } else {
        format!("{rounded:.1}")
    };
    let number = match lang {
        SheetLang::De => number.replace('.', ","),
        SheetLang::En => number,
    };
    format!("{length_m} m = {number} mm")
}

fn rect(min: Pt, max: Pt) -> Vec<Pt> {
    vec![min, (max.0, min.1), max, (min.0, max.1)]
}

/// The scale check bar for the map printed at 1:`print_scale`, with its top-left corner (the
/// white box) at `origin`: the sheet items in paint order and the size of the element (mm).
///
/// The bar is exactly `length * 1000 / print_scale` mm long, measured between the centre lines of
/// the two end ticks, which are also the centre lines of the outline's short sides.
pub fn scale_check_bar(
    print_scale: u32,
    origin: Pt,
    lang: SheetLang,
) -> (Vec<SheetItem>, (f64, f64)) {
    let metres = length_m(print_scale);
    let bar_mm = f64::from(metres) * 1000.0 / f64::from(print_scale.max(1));
    let text = label(metres, bar_mm, lang);
    let label_w = text_width(&text, face(false, false)) * LABEL_MM;
    let label_h = LABEL_MM * LINE_SPACING;

    let content_w = label_w.max(bar_mm + LINE_MM);
    let size = (
        content_w + 2.0 * PADDING_MM,
        label_h + LINE_MM / 2.0 + BAR_HEIGHT_MM + TICK_HEIGHT_MM + 2.0 * PADDING_MM,
    );
    let x0 = origin.0 + PADDING_MM + LINE_MM / 2.0;
    let x1 = x0 + bar_mm;
    let top = origin.1 + PADDING_MM + label_h + LINE_MM / 2.0;
    let bottom = top + BAR_HEIGHT_MM;

    let fill = |colour, min, max| SheetItem::Shape {
        colour,
        shape: Shape::Fill {
            rings: vec![rect(min, max)],
            rule: FillRule::EvenOdd,
        },
    };
    let stroke = |path: Vec<Pt>, closed| SheetItem::Shape {
        colour: DirectColour::BLACK,
        shape: Shape::Stroke {
            path,
            closed,
            width: LINE_MM,
            cap: Cap::Flat,
            join: Join::Miter,
        },
    };

    let mut items = vec![fill(
        DirectColour::WHITE,
        origin,
        (origin.0 + size.0, origin.1 + size.1),
    )];
    for i in 0..SEGMENTS {
        let colour = if i % 2 == 0 {
            DirectColour::BLACK
        } else {
            DirectColour::WHITE
        };
        let a = x0 + bar_mm * i as f64 / SEGMENTS as f64;
        let b = if i + 1 == SEGMENTS {
            x1
        } else {
            x0 + bar_mm * (i + 1) as f64 / SEGMENTS as f64
        };
        items.push(fill(colour, (a, top), (b, bottom)));
    }
    items.push(stroke(rect((x0, top), (x1, bottom)), true));
    for x in [x0, x1] {
        items.push(stroke(
            vec![(x, bottom), (x, bottom + TICK_HEIGHT_MM)],
            false,
        ));
    }
    items.push(SheetItem::Shape {
        colour: DirectColour::BLACK,
        shape: Shape::Text {
            origin: (
                origin.0 + PADDING_MM,
                origin.1 + PADDING_MM + ASCENT * LABEL_MM,
            ),
            size: LABEL_MM,
            bold: false,
            italic: false,
            text,
            rotation: 0.0,
            stroke: None,
        },
    });
    (items, size)
}

/// Draws a `scale_check_bar` element at its anchor, at the layout's print scale (not the map
/// scale).
pub(super) fn place_scale_bar(ctx: &SheetContext<'_>, spec: &ElementSpec) -> PlacedElement {
    let lang = ctx.project.project.event.lang;
    let scale = ctx.print.print_scale;
    let (_, size) = scale_check_bar(scale, (0.0, 0.0), lang);
    let top_left = layout::anchor_point(&spec.anchor, ctx.page, ctx.frame, size);
    let (items, _) = scale_check_bar(scale, top_left, lang);
    PlacedElement {
        items,
        rect: PaperBox {
            min: top_left,
            max: (top_left.0 + size.0, top_left.1 + size.1),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::page::RenderedPage;
    use crate::render::pdf::{PdfMeta, content_of, write_pdf};

    /// x of the two end ticks in the PDF content (pt): the open two-point strokes.
    fn tick_xs(items: Vec<SheetItem>) -> Vec<f64> {
        let page = RenderedPage {
            primitives: Vec::new(),
            clip: PaperBox {
                min: (0.0, 0.0),
                max: (210.0, 297.0),
            },
            page_mm: (210.0, 297.0),
            centre_mm: (0.0, 0.0),
            colours: Vec::new(),
            sheet: items,
        };
        let (pdf, _) = write_pdf(&page, &PdfMeta::new("t")).unwrap();
        let content = String::from_utf8(content_of(&pdf)).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        let mut xs = Vec::new();
        for (i, l) in lines.iter().enumerate() {
            if l.ends_with(" m") && lines[i + 1].ends_with(" l") && lines[i + 2] == "S" {
                xs.push(l.split(' ').next().unwrap().parse().unwrap());
            }
        }
        xs
    }

    fn bar_pt(scale: u32) -> f64 {
        let xs = tick_xs(scale_check_bar(scale, (20.0, 30.0), SheetLang::De).0);
        assert_eq!(xs.len(), 2, "{xs:?}");
        xs[1] - xs[0]
    }

    fn label_of(items: &[SheetItem]) -> String {
        items
            .iter()
            .find_map(|i| match i {
                SheetItem::Shape {
                    shape: Shape::Text { text, .. },
                    ..
                } => Some(text.clone()),
                _ => None,
            })
            .unwrap()
    }

    #[test]
    fn bar_length_1_4000() {
        assert_eq!(length_m(4000), 200);
        let pt = bar_pt(4000);
        assert!((pt - 141.732).abs() < 0.001, "{pt}");
        let items = scale_check_bar(4000, (0.0, 0.0), SheetLang::En).0;
        assert_eq!(label_of(&items), "200 m = 50 mm");
    }

    #[test]
    fn bar_length_1_3000() {
        assert_eq!(length_m(3000), 100);
        let pt = bar_pt(3000);
        assert!(
            (pt - 100.0 / 3000.0 * 1000.0 * 72.0 / 25.4).abs() < 0.001,
            "{pt}"
        );
    }

    #[test]
    fn bar_length_1_10000() {
        assert_eq!(length_m(10000), 500);
        assert!((bar_pt(10000) - 141.732).abs() < 0.001);
    }

    #[test]
    fn bar_length_1_200() {
        assert_eq!(length_m(200), 10);
        assert!((bar_pt(200) - 141.732).abs() < 0.001);
    }

    #[test]
    fn label_german_comma() {
        let items = scale_check_bar(3000, (0.0, 0.0), SheetLang::De).0;
        assert_eq!(label_of(&items), "100 m = 33,3 mm");
        let items = scale_check_bar(3000, (0.0, 0.0), SheetLang::En).0;
        assert_eq!(label_of(&items), "100 m = 33.3 mm");
    }

    #[test]
    fn element_has_white_box_and_five_segments() {
        let (items, size) = scale_check_bar(4000, (10.0, 20.0), SheetLang::De);
        let SheetItem::Shape {
            colour,
            shape: Shape::Fill { rings, .. },
        } = &items[0]
        else {
            panic!("white box first");
        };
        assert_eq!(*colour, DirectColour::WHITE);
        assert_eq!(rings[0][0], (10.0, 20.0));
        assert_eq!(rings[0][2], (10.0 + size.0, 20.0 + size.1));
        let blacks = items
            .iter()
            .skip(1)
            .filter(|i| {
                matches!(
                    i,
                    SheetItem::Shape {
                        colour: DirectColour::BLACK,
                        shape: Shape::Fill { .. }
                    }
                )
            })
            .count();
        assert_eq!(blacks, 3);
    }
}
