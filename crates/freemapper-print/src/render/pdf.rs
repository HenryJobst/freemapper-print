//! Copied from freemapper 743ce4b (ADR 0014).
//! Drawing primitives as a one-page PDF: DeviceCMYK colours, painted by colour priority, texts in
//! the standard fonts Helvetica / Helvetica-Bold with WinAnsi encoding.

use pdf_writer::types::{LineCapStyle, LineJoinStyle};
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref, Str, TextStr};

use super::draw::{Primitive, Shape};
use super::geometry::Pt;
use super::metrics::win_ansi;
use super::symbols::{Cap, Join, RenderColour};

/// Points per millimetre.
const PT_PER_MM: f64 = 72.0 / 25.4;
/// Margin around everything drawn (mm).
const MARGIN_MM: f64 = 5.0;
/// Page sizes tried, smallest first (mm, portrait).
const PAGES: [(f64, f64); 2] = [(210.0, 297.0), (297.0, 420.0)];
/// OOM's miter limit for PDF (reference §2.1).
const MITER_LIMIT: f32 = 2.236_068;

/// An axis-parallel rectangle on paper (mm, y down).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaperBox {
    pub min: Pt,
    pub max: Pt,
}

/// The page (mm) for content of `width` × `height` mm plus margin: A4 or A3 in either
/// orientation, the smallest that fits, else exactly the content size.
pub fn page_size(width: f64, height: f64) -> (f64, f64) {
    let (w, h) = (width + 2.0 * MARGIN_MM, height + 2.0 * MARGIN_MM);
    PAGES
        .iter()
        .flat_map(|&(a, b)| [(a, b), (b, a)])
        .find(|&(pw, ph)| w <= pw && h <= ph)
        .unwrap_or((w, h))
}

/// The box around everything a primitive paints (mm), widened by half a stroke.
fn bounds(p: &Primitive) -> Option<PaperBox> {
    shape_bounds(&p.shape)
}

fn shape_bounds(shape: &Shape) -> Option<PaperBox> {
    let (points, extra): (Vec<Pt>, f64) = match shape {
        Shape::Fill { rings } => (rings.iter().flatten().copied().collect(), 0.0),
        Shape::Stroke { path, width, .. } => (path.clone(), width / 2.0),
        Shape::Text {
            origin,
            size,
            bold,
            text,
        } => {
            let w = super::metrics::text_width(text, *bold) * size;
            (
                vec![
                    (origin.0, origin.1 - size),
                    (origin.0 + w, origin.1 + size / 3.0),
                ],
                0.0,
            )
        }
        // What is painted lies inside both the clip and the inner shape.
        Shape::Clipped { rings, inner } => {
            let a = shape_bounds(&Shape::Fill {
                rings: rings.clone(),
            })?;
            let b = shape_bounds(inner)?;
            let min = (a.min.0.max(b.min.0), a.min.1.max(b.min.1));
            let max = (a.max.0.min(b.max.0), a.max.1.min(b.max.1));
            return (min.0 <= max.0 && min.1 <= max.1).then_some(PaperBox { min, max });
        }
    };
    let first = *points.first()?;
    let (min, max) = points.iter().fold((first, first), |(min, max), q| {
        (
            (min.0.min(q.0), min.1.min(q.1)),
            (max.0.max(q.0), max.1.max(q.1)),
        )
    });
    Some(PaperBox {
        min: (min.0 - extra, min.1 - extra),
        max: (max.0 + extra, max.1 + extra),
    })
}

fn union(a: PaperBox, b: PaperBox) -> PaperBox {
    PaperBox {
        min: (a.min.0.min(b.min.0), a.min.1.min(b.min.1)),
        max: (a.max.0.max(b.max.0), a.max.1.max(b.max.1)),
    }
}

/// Paper mm (y down) → page points (y up).
struct Transform {
    offset: Pt,
    page_height: f64,
}

impl Transform {
    fn pt(&self, p: Pt) -> (f32, f32) {
        (
            ((p.0 + self.offset.0) * PT_PER_MM) as f32,
            ((self.page_height - (p.1 + self.offset.1)) * PT_PER_MM) as f32,
        )
    }
}

fn path(content: &mut Content, t: &Transform, points: &[Pt], close: bool) {
    for (i, &p) in points.iter().enumerate() {
        let (x, y) = t.pt(p);
        if i == 0 {
            content.move_to(x, y);
        } else {
            content.line_to(x, y);
        }
    }
    if close {
        content.close_path();
    }
}

/// Opens a clip block: `q`, the rings, `W* n` (even-odd, as the area fill).
fn begin_clip(content: &mut Content, t: &Transform, rings: &[Vec<Pt>]) {
    content.save_state();
    for ring in rings {
        path(content, t, ring, true);
    }
    content.clip_even_odd();
    content.end_path();
}

/// Paints `shape` in the colour of `priority`. A clipped shape gets its own clip block here;
/// [`write_pdf`] shares one block among consecutive shapes with the same clip instead.
fn paint(
    content: &mut Content,
    t: &Transform,
    priority: i32,
    shape: &Shape,
    colours: &[RenderColour],
) {
    let [c, m, y, k] = colours
        .iter()
        .find(|c| c.priority == priority)
        .map_or([0.0, 0.0, 0.0, 1.0], |c| c.cmyk)
        .map(|v| v as f32);
    match shape {
        Shape::Fill { rings } => {
            content.set_fill_cmyk(c, m, y, k);
            for ring in rings {
                path(content, t, ring, true);
            }
            content.fill_even_odd();
        }
        Shape::Stroke {
            path: points,
            closed,
            width,
            cap,
            join,
        } => {
            content.set_stroke_cmyk(c, m, y, k);
            content.set_line_width((width * PT_PER_MM) as f32);
            content.set_line_cap(match cap {
                Cap::Flat => LineCapStyle::ButtCap,
                Cap::Round => LineCapStyle::RoundCap,
                Cap::Square => LineCapStyle::ProjectingSquareCap,
            });
            content.set_line_join(match join {
                Join::Miter => LineJoinStyle::MiterJoin,
                Join::Round => LineJoinStyle::RoundJoin,
                Join::Bevel => LineJoinStyle::BevelJoin,
            });
            content.set_miter_limit(MITER_LIMIT);
            path(content, t, points, *closed);
            content.stroke();
        }
        Shape::Text {
            origin,
            size,
            bold,
            text,
        } => {
            let (x, y0) = t.pt(*origin);
            let bytes: Vec<u8> = text.chars().map(win_ansi).collect();
            content.set_fill_cmyk(c, m, y, k);
            content.begin_text();
            content.set_font(
                Name(if *bold { b"F2" } else { b"F1" }),
                (size * PT_PER_MM) as f32,
            );
            content.set_text_matrix([1.0, 0.0, 0.0, 1.0, x, y0]);
            content.show(Str(&bytes));
            content.end_text();
        }
        Shape::Clipped { rings, inner } => {
            begin_clip(content, t, rings);
            paint(content, t, priority, inner, colours);
            content.restore_state();
        }
    }
}

/// The PDF of `map` primitives (clipped to `clip`) and `sheet` primitives (not clipped).
pub fn write_pdf(
    map: &[Primitive],
    sheet: &[Primitive],
    clip: PaperBox,
    colours: &[RenderColour],
    title: &str,
) -> Vec<u8> {
    let content_box = sheet.iter().filter_map(bounds).fold(clip, union);
    let (width, height) = (
        content_box.max.0 - content_box.min.0,
        content_box.max.1 - content_box.min.1,
    );
    let (page_w, page_h) = page_size(width, height);
    let t = Transform {
        offset: (
            (page_w - width) / 2.0 - content_box.min.0,
            (page_h - height) / 2.0 - content_box.min.1,
        ),
        page_height: page_h,
    };

    // Highest priority first: priority 0 is painted last, on top. Stable, so objects keep their
    // order within a colour.
    let mut items: Vec<(&Primitive, bool)> = map
        .iter()
        .map(|p| (p, true))
        .chain(sheet.iter().map(|p| (p, false)))
        .collect();
    items.sort_by_key(|(p, _)| std::cmp::Reverse(p.priority));

    let mut content = Content::new();
    let (cx0, cy0) = t.pt((clip.min.0, clip.max.1));
    let (cx1, cy1) = t.pt((clip.max.0, clip.min.1));
    // One clip block for each run of consecutive map primitives, and nested in it one for each
    // run of shapes clipped to the same rings (the lines of one area pattern).
    let mut clipping = false;
    let mut pattern_clip: Option<&[Vec<Pt>]> = None;
    for (primitive, clipped) in items {
        let (rings, shape) = match &primitive.shape {
            Shape::Clipped { rings, inner } => (Some(rings.as_slice()), &**inner),
            shape => (None, shape),
        };
        if pattern_clip.is_some() && (clipped != clipping || pattern_clip != rings) {
            content.restore_state();
            pattern_clip = None;
        }
        if clipped && !clipping {
            content.save_state();
            content.rect(cx0, cy0, cx1 - cx0, cy1 - cy0);
            content.clip_nonzero();
            content.end_path();
        } else if !clipped && clipping {
            content.restore_state();
        }
        clipping = clipped;
        if let (None, Some(rings)) = (pattern_clip, rings) {
            begin_clip(&mut content, &t, rings);
            pattern_clip = Some(rings);
        }
        paint(&mut content, &t, primitive.priority, shape, colours);
    }
    if pattern_clip.is_some() {
        content.restore_state();
    }
    if clipping {
        content.restore_state();
    }

    let (catalog, pages, page, contents, regular, bold, info) = (
        Ref::new(1),
        Ref::new(2),
        Ref::new(3),
        Ref::new(4),
        Ref::new(5),
        Ref::new(6),
        Ref::new(7),
    );
    let mut pdf = Pdf::new();
    pdf.catalog(catalog).pages(pages);
    pdf.pages(pages).kids([page]).count(1);
    let mut p = pdf.page(page);
    p.media_box(Rect::new(
        0.0,
        0.0,
        (page_w * PT_PER_MM) as f32,
        (page_h * PT_PER_MM) as f32,
    ));
    p.parent(pages);
    p.contents(contents);
    p.resources()
        .fonts()
        .pair(Name(b"F1"), regular)
        .pair(Name(b"F2"), bold);
    p.finish();
    for (id, name) in [(regular, &b"Helvetica"[..]), (bold, &b"Helvetica-Bold"[..])] {
        pdf.type1_font(id)
            .base_font(Name(name))
            .encoding_predefined(Name(b"WinAnsiEncoding"));
    }
    let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&content.finish(), 6);
    pdf.stream(contents, &compressed)
        .filter(Filter::FlateDecode);
    pdf.document_info(info)
        .title(TextStr(title))
        .creator(TextStr(
            "freemapper – https://github.com/HenryJobst/freemapper",
        ));
    pdf.finish()
}

/// The decompressed page content of a PDF written by [`write_pdf`] (for tests).
#[cfg(test)]
pub(crate) fn content_of(pdf: &[u8]) -> Vec<u8> {
    let find = |needle: &[u8]| pdf.windows(needle.len()).position(|w| w == needle);
    let start = find(b"stream\n").expect("content stream") + 7;
    let end = find(b"\nendstream").expect("end of stream");
    miniz_oxide::inflate::decompress_to_vec_zlib(&pdf[start..end]).expect("FlateDecode")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colours() -> Vec<RenderColour> {
        vec![
            RenderColour {
                priority: 2,
                cmyk: [0.0, 0.0, 0.0, 1.0],
            },
            RenderColour {
                priority: 9,
                cmyk: [0.0, 0.0, 0.0, 0.6],
            },
        ]
    }

    fn square(priority: i32) -> Primitive {
        Primitive {
            priority,
            shape: Shape::Fill {
                rings: vec![vec![(10.0, 10.0), (20.0, 10.0), (20.0, 20.0), (10.0, 20.0)]],
            },
        }
    }

    fn clip() -> PaperBox {
        PaperBox {
            min: (0.0, 0.0),
            max: (100.0, 100.0),
        }
    }

    fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack.windows(needle.len()).position(|w| w == needle)
    }

    #[test]
    fn page_size_is_a4_or_a3() {
        assert_eq!(page_size(150.0, 200.0), (210.0, 297.0));
        assert_eq!(page_size(250.0, 150.0), (297.0, 210.0));
        assert_eq!(page_size(350.0, 250.0), (420.0, 297.0));
        assert_eq!(page_size(500.0, 100.0), (510.0, 110.0));
    }

    #[test]
    fn primitives_are_painted_by_priority() {
        let pdf = content_of(&write_pdf(
            &[square(2), square(9)],
            &[],
            clip(),
            &colours(),
            "Test",
        ));
        let black = find(&pdf, b"0 0 0 1 k").expect("black fill");
        let grey = find(&pdf, b"0 0 0 0.6 k").expect("grey fill");
        assert!(
            grey < black,
            "priority 9 is painted before 2, so 2 is on top"
        );
        assert!(find(&pdf, b"f*").is_some(), "even-odd fill");
        assert!(
            find(&pdf, b"re\nW\nn").is_some(),
            "map primitives are clipped"
        );
    }

    #[test]
    fn strokes_carry_width_caps_and_miter_limit() {
        let line = Primitive {
            priority: 2,
            shape: Shape::Stroke {
                path: vec![(0.0, 0.0), (10.0, 0.0)],
                closed: false,
                width: 0.35,
                cap: Cap::Round,
                join: Join::Miter,
            },
        };
        let pdf = content_of(&write_pdf(&[line], &[], clip(), &colours(), "Test"));
        assert!(find(&pdf, b"0 0 0 1 K").is_some(), "stroke colour");
        assert!(find(&pdf, b"1 J").is_some(), "round cap");
        assert!(find(&pdf, b"2.236068 M").is_some(), "miter limit");
        assert!(find(&pdf, b"0.992126 w").is_some(), "0.35 mm in points");
    }

    fn clipped_line(y: f64) -> Primitive {
        Primitive {
            priority: 2,
            shape: Shape::Clipped {
                rings: vec![vec![(10.0, 10.0), (20.0, 10.0), (20.0, 20.0), (10.0, 20.0)]],
                inner: Box::new(Shape::Stroke {
                    path: vec![(5.0, y), (25.0, y)],
                    closed: false,
                    width: 0.1,
                    cap: Cap::Flat,
                    join: Join::Miter,
                }),
            },
        }
    }

    #[test]
    fn clipped_shape_uses_clip_path() {
        let pdf = content_of(&write_pdf(
            &[clipped_line(15.0)],
            &[],
            clip(),
            &colours(),
            "Test",
        ));
        let after = |from: usize, needle: &[u8]| {
            find(&pdf[from..], needle)
                .map(|i| from + i)
                .unwrap_or_else(|| panic!("{} after {from}", String::from_utf8_lossy(needle)))
        };
        // The page clip block comes first, then the pattern clip block nested in it.
        let page_clip = after(0, b"re\nW\nn\n");
        let save = after(page_clip, b"q\n");
        let ring = after(save, b" m\n");
        let clip = after(ring, b"h\nW*\nn\n");
        let stroke = after(clip, b"S\n");
        let restore = after(stroke, b"Q\n");
        assert!(save < ring && ring < clip && clip < stroke && stroke < restore);
    }

    #[test]
    fn clipped_shapes_with_one_outline_share_a_clip_block() {
        let lines: Vec<Primitive> = (0..5).map(|i| clipped_line(11.0 + f64::from(i))).collect();
        let pdf = content_of(&write_pdf(&lines, &[], clip(), &colours(), "Test"));
        let clips = pdf.windows(4).filter(|w| w == b"W*\nn").count();
        let strokes = pdf.windows(2).filter(|w| w == b"S\n").count();
        assert_eq!((clips, strokes), (1, 5));
    }

    #[test]
    fn text_is_win_ansi_with_helvetica() {
        let text = Primitive {
            priority: 2,
            shape: Shape::Text {
                origin: (10.0, 10.0),
                size: 2.5,
                bold: true,
                text: "Maßstab ©".into(),
            },
        };
        let pdf = write_pdf(&[], &[text], clip(), &colours(), "Test");
        assert!(find(&pdf, b"/Helvetica-Bold").is_some());
        assert!(find(&pdf, b"/WinAnsiEncoding").is_some());
        let pdf = content_of(&pdf);
        // ß = 0xDF, © = 0xA9 in WinAnsi.
        assert!(find(&pdf, b"<4D61DF7374616220A9> Tj").is_some());
    }

    #[test]
    fn content_is_compressed_and_clip_blocks_are_shared() {
        let many: Vec<Primitive> = (0..200).map(|_| square(9)).collect();
        let pdf = write_pdf(&many, &[], clip(), &colours(), "Test");
        assert!(find(&pdf, b"/FlateDecode").is_some());
        let start = find(&pdf, b"stream\n").unwrap() + 7;
        let end = find(&pdf, b"\nendstream").unwrap();
        let content = miniz_oxide::inflate::decompress_to_vec_zlib(&pdf[start..end]).unwrap();
        let clips = content.windows(5).filter(|w| w == b"re\nW\n").count();
        assert_eq!(clips, 1, "one clip block for a run of map primitives");
        assert!(find(&content, b"0 0 0 0.6 k").is_some());
    }

    #[test]
    fn pdf_structure_is_valid() {
        let pdf = write_pdf(&[square(9)], &[], clip(), &colours(), "Strausberg Vorstadt");
        assert!(pdf.starts_with(b"%PDF-"));
        assert!(find(&pdf, b"xref").is_some() && find(&pdf, b"trailer").is_some());
        assert!(pdf.ends_with(b"%%EOF") || pdf.ends_with(b"%%EOF\n"));
        assert!(find(&pdf, b"(Strausberg Vorstadt)").is_some(), "title");
        // UTF-16BE (the en dash is not ASCII): "HenryJobst".
        assert!(
            find(&pdf, b"00480065006E00720079004A006F00620073007400").is_some(),
            "creator"
        );
        assert!(
            find(&pdf, b"/MediaBox [0 0 595.2756 841.8898]").is_some(),
            "A4"
        );
    }
}
