//! Copied from freemapper 743ce4b (ADR 0014).
//! Drawing primitives as a one-page PDF: DeviceCMYK colours, painted by colour priority, texts in
//! embedded subsets of Arimo (ADR 0017).

use std::collections::BTreeSet;
use std::sync::Arc;

use pdf_writer::types::{LineCapStyle, LineJoinStyle, TextRenderingMode};
use pdf_writer::{Content, Filter, Finish, Pdf, Rect, Ref, Str, TextStr};

use super::draw::{Primitive, Shape};
use super::fonts::{PageFonts, face, text_width};
use super::geometry::Pt;
use super::symbols::{Cap, Join, RenderColour};

/// Points per millimetre.
const PT_PER_MM: f64 = 72.0 / 25.4;
/// Margin around everything drawn (mm).
const MARGIN_MM: f64 = 5.0;
/// Page sizes tried, smallest first (mm, portrait).
const PAGES: [(f64, f64); 2] = [(210.0, 297.0), (297.0, 420.0)];
/// OOM's miter limit for PDF (reference §2.1).
const MITER_LIMIT: f32 = 2.236_068;
/// OOM's miter limit for text framing in PDF: Qt 0.5 → √(1 + 4 · 0.5²) = √2 (reference §2.1).
const TEXT_FRAMING_MITER_LIMIT: f32 = std::f32::consts::SQRT_2;

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
            italic,
            text,
            rotation,
            stroke,
        } => {
            let w = text_width(text, face(*bold, *italic)) * size;
            // The box in text coordinates, turned about the origin as the text is.
            let (sin, cos) = rotation.sin_cos();
            let corners = [(0.0, -size), (w, -size), (w, size / 3.0), (0.0, size / 3.0)];
            (
                corners
                    .iter()
                    .map(|&(x, y)| (origin.0 + x * cos + y * sin, origin.1 - x * sin + y * cos))
                    .collect(),
                stroke.unwrap_or(0.0) / 2.0,
            )
        }
        // What is painted lies inside both the clip and the inner shape.
        Shape::Clipped { rings, inner } => {
            let mut points = rings.iter().flatten();
            let first = *points.next()?;
            let (a_min, a_max) = points.fold((first, first), |(min, max), q| {
                (
                    (min.0.min(q.0), min.1.min(q.1)),
                    (max.0.max(q.0), max.1.max(q.1)),
                )
            });
            let a = PaperBox {
                min: a_min,
                max: a_max,
            };
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
    fonts: &mut PageFonts,
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
            italic,
            text,
            rotation,
            stroke,
        } => {
            let (x, y0) = t.pt(*origin);
            let font_face = face(*bold, *italic);
            let cids = fonts.encode(font_face, text);
            match stroke {
                // Text line framing: the glyph outlines stroked, miter-joined with OOM's limit
                // for framing; in its own graphics state, which holds the render mode.
                Some(width) => {
                    content.save_state();
                    content.set_stroke_cmyk(c, m, y, k);
                    content.set_line_width((width * PT_PER_MM) as f32);
                    content.set_line_join(LineJoinStyle::MiterJoin);
                    content.set_miter_limit(TEXT_FRAMING_MITER_LIMIT);
                }
                None => {
                    content.set_fill_cmyk(c, m, y, k);
                }
            }
            content.begin_text();
            content.set_font(
                PageFonts::resource_name(font_face),
                (size * PT_PER_MM) as f32,
            );
            if stroke.is_some() {
                content.set_text_rendering_mode(TextRenderingMode::Stroke);
            }
            // Counter-clockwise on paper is counter-clockwise in PDF too.
            let (sin, cos) = rotation.sin_cos();
            content.set_text_matrix([cos as f32, sin as f32, -sin as f32, cos as f32, x, y0]);
            content.show(Str(&cids));
            content.end_text();
            if stroke.is_some() {
                content.restore_state();
            }
        }
        Shape::Clipped { rings, inner } => {
            begin_clip(content, t, rings);
            paint(content, t, priority, inner, colours, fonts);
            content.restore_state();
        }
    }
}

/// What a written page could not show as asked.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PdfInfo {
    /// Characters of the page's texts that have no glyph in the embedded font (drawn as `.notdef`).
    pub missing_glyphs: BTreeSet<char>,
}

/// The PDF of `map` primitives (clipped to `clip`) and `sheet` primitives (not clipped).
pub fn write_pdf(
    map: &[Primitive],
    sheet: &[Primitive],
    clip: PaperBox,
    colours: &[RenderColour],
    title: &str,
) -> Vec<u8> {
    write_pdf_info(map, sheet, clip, colours, title).0
}

/// [`write_pdf`] plus what the page could not show.
pub fn write_pdf_info(
    map: &[Primitive],
    sheet: &[Primitive],
    clip: PaperBox,
    colours: &[RenderColour],
    title: &str,
) -> (Vec<u8>, PdfInfo) {
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
    let mut fonts = PageFonts::new();
    let (cx0, cy0) = t.pt((clip.min.0, clip.max.1));
    let (cx1, cy1) = t.pt((clip.max.0, clip.min.1));
    // One clip block for each run of consecutive map primitives, and nested in it one for each
    // run of shapes clipped to the same rings (the lines of one area pattern).
    let mut clipping = false;
    let mut pattern_clip: Option<&Arc<[Vec<Pt>]>> = None;
    for (primitive, clipped) in items {
        let (rings, shape) = match &primitive.shape {
            Shape::Clipped { rings, inner } => (Some(rings), &**inner),
            shape => (None, shape),
        };
        // The pattern shapes of one area share their rings: compare pointers first.
        let same_clip = match (pattern_clip, rings) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b) || a == b,
            _ => false,
        };
        if pattern_clip.is_some() && (clipped != clipping || !same_clip) {
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
        paint(
            &mut content,
            &t,
            primitive.priority,
            shape,
            colours,
            &mut fonts,
        );
    }
    if pattern_clip.is_some() {
        content.restore_state();
    }
    if clipping {
        content.restore_state();
    }

    // References come from a counter, so a page holds only the fonts it uses.
    let mut next = Ref::new(1);
    let (catalog, pages, page, contents) = (next.bump(), next.bump(), next.bump(), next.bump());
    let fonts = fonts.finish(&mut next);
    let info = next.bump();
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
    p.resources().fonts().pairs(fonts.resources());
    p.finish();
    // The content stream comes first, the font programs after it.
    let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&content.finish(), 6);
    pdf.stream(contents, &compressed)
        .filter(Filter::FlateDecode);
    fonts.write(&mut pdf);
    pdf.document_info(info)
        .title(TextStr(title))
        .creator(TextStr(
            "freemapper – https://github.com/HenryJobst/freemapper",
        ));
    let info = PdfInfo {
        missing_glyphs: fonts.missing,
    };
    (pdf.finish(), info)
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
                rings: vec![vec![(10.0, 10.0), (20.0, 10.0), (20.0, 20.0), (10.0, 20.0)]].into(),
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

    fn text(rotation: f64, stroke: Option<f64>) -> Primitive {
        Primitive {
            priority: 2,
            shape: Shape::Text {
                origin: (10.0, 10.0),
                size: 2.5,
                bold: false,
                italic: false,
                text: "Wald".into(),
                rotation,
                stroke,
            },
        }
    }

    #[test]
    fn line_framing_strokes_the_glyphs() {
        let pdf = content_of(&write_pdf(
            &[],
            &[text(0.0, Some(0.3)), text(0.0, None)],
            clip(),
            &colours(),
            "Test",
        ));
        let at = |needle: &[u8]| {
            find(&pdf, needle).unwrap_or_else(|| panic!("{}", String::from_utf8_lossy(needle)))
        };
        // Stroke only (render mode 1), framing colour, 2 × half width, miter join with OOM's
        // text framing limit √2 [RI `TextFramingRenderable`], inside q … Q so that the fill text
        // after it is not stroked.
        let (save, mode, restore) = (at(b"q\n"), at(b"1 Tr"), at(b"Q\n"));
        assert!(save < mode && mode < restore);
        assert!(find(&pdf, b"0 0 0 1 K").is_some());
        assert!(find(&pdf, b"0.8503937 w").is_some(), "0.3 mm");
        assert!(find(&pdf, b"1.4142135 M").is_some());
        assert_eq!(pdf.windows(2).filter(|w| w == b"Tj").count(), 2);
        assert!(
            find(&pdf[restore..], b"Tr").is_none(),
            "fill text in the default mode"
        );
    }

    #[test]
    fn rotated_text_turns_its_text_matrix() {
        let pdf = content_of(&write_pdf(
            &[],
            &[text(std::f64::consts::FRAC_PI_6, None)],
            clip(),
            &colours(),
            "Test",
        ));
        // Counter-clockwise on paper is counter-clockwise in PDF (y up).
        let tm = String::from_utf8_lossy(&pdf);
        let line = tm.lines().find(|l| l.ends_with("Tm")).unwrap();
        let v: Vec<f64> = line
            .split_whitespace()
            .take(4)
            .map(|x| x.parse().unwrap())
            .collect();
        let (s, c) = std::f64::consts::FRAC_PI_6.sin_cos();
        for (a, b) in v.iter().zip([c, s, -s, c]) {
            assert!((a - b).abs() < 1e-6, "{line}");
        }
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

    fn plain_text(text: &str, bold: bool, italic: bool) -> Primitive {
        Primitive {
            priority: 2,
            shape: Shape::Text {
                origin: (10.0, 10.0),
                size: 2.5,
                bold,
                italic,
                text: text.into(),
                rotation: 0.0,
                stroke: None,
            },
        }
    }

    /// Every stream of `pdf`, decompressed (all streams written here are FlateDecode).
    fn streams(pdf: &[u8]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        let mut from = 0;
        while let Some(start) = find(&pdf[from..], b"stream\n").map(|i| from + i + 7) {
            let end = start + find(&pdf[start..], b"\nendstream").expect("end of stream");
            out.push(
                miniz_oxide::inflate::decompress_to_vec_zlib(&pdf[start..end])
                    .expect("FlateDecode"),
            );
            from = end + 10;
        }
        out
    }

    /// The string operands of a content stream, in order (literal and hex strings).
    fn shown_strings(content: &[u8]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < content.len() {
            match content[i] {
                b'(' => {
                    let (mut s, mut depth) = (Vec::new(), 1);
                    i += 1;
                    loop {
                        match content[i] {
                            b'\\' => {
                                i += 1;
                                match content[i] {
                                    b'n' => s.push(b'\n'),
                                    b'r' => s.push(b'\r'),
                                    b't' => s.push(b'\t'),
                                    b'b' => s.push(8),
                                    b'f' => s.push(12),
                                    d @ b'0'..=b'7' => {
                                        let mut v = u32::from(d - b'0');
                                        for _ in 0..2 {
                                            if matches!(content[i + 1], b'0'..=b'7') {
                                                i += 1;
                                                v = v * 8 + u32::from(content[i] - b'0');
                                            }
                                        }
                                        s.push(v as u8);
                                    }
                                    c => s.push(c),
                                }
                            }
                            b'(' => {
                                depth += 1;
                                s.push(b'(');
                            }
                            b')' => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                                s.push(b')');
                            }
                            c => s.push(c),
                        }
                        i += 1;
                    }
                    out.push(s);
                }
                b'<' if content.get(i + 1) != Some(&b'<') => {
                    let end = i + find(&content[i..], b">").unwrap();
                    let hex: Vec<u8> = content[i + 1..end]
                        .iter()
                        .copied()
                        .filter(u8::is_ascii_hexdigit)
                        .collect();
                    out.push(
                        hex.chunks(2)
                            .map(|h| {
                                u8::from_str_radix(std::str::from_utf8(h).unwrap(), 16).unwrap()
                            })
                            .collect(),
                    );
                    i = end;
                }
                _ => {}
            }
            i += 1;
        }
        out
    }

    /// The `bfchar` entries of a ToUnicode CMap: CID → text.
    fn bf_chars(cmap: &[u8]) -> std::collections::HashMap<u16, String> {
        let text = String::from_utf8_lossy(cmap);
        let mut map = std::collections::HashMap::new();
        for block in text.split("beginbfchar").skip(1) {
            let block = block.split("endbfchar").next().unwrap();
            for line in block.lines().filter(|l| l.starts_with('<')) {
                let mut parts = line.split(['<', '>']).filter(|p| !p.trim().is_empty());
                let cid = u16::from_str_radix(parts.next().unwrap(), 16).unwrap();
                let units: Vec<u16> = parts
                    .next()
                    .unwrap()
                    .as_bytes()
                    .chunks(4)
                    .map(|h| u16::from_str_radix(std::str::from_utf8(h).unwrap(), 16).unwrap())
                    .collect();
                map.insert(cid, String::from_utf16(&units).unwrap());
            }
        }
        map
    }

    #[test]
    fn pdf_embeds_subset_fonts() {
        let pdf = write_pdf(
            &[],
            &[plain_text("Wald", false, false)],
            clip(),
            &colours(),
            "Test",
        );
        for key in [
            &b"/FontFile2"[..],
            b"/ToUnicode",
            b"/Type0",
            b"/CIDFontType2",
            b"/Identity-H",
            b"/CIDToGIDMap",
            b"/W [",
            b"+Arimo-Regular",
        ] {
            assert!(
                find(&pdf, key).is_some(),
                "{}",
                String::from_utf8_lossy(key)
            );
        }
        assert!(find(&pdf, b"/Helvetica").is_none());
        assert!(find(&pdf, b"Arimo-Bold").is_none(), "only used faces");
        assert!(pdf.len() < 60_000, "{} bytes", pdf.len());
    }

    #[test]
    fn faces_follow_bold_and_italic() {
        let pdf = write_pdf(
            &[],
            &[
                plain_text("Wald", true, false),
                plain_text("Wald", false, true),
                plain_text("Wald", true, true),
            ],
            clip(),
            &colours(),
            "Test",
        );
        for name in [&b"Arimo-Bold"[..], b"Arimo-Italic", b"Arimo-BoldItalic"] {
            assert!(
                find(&pdf, name).is_some(),
                "{}",
                String::from_utf8_lossy(name)
            );
        }
        assert!(find(&pdf, b"Arimo-Regular").is_none());
        assert_eq!(pdf.windows(10).filter(|w| w == b"/FontFile2").count(), 3);
    }

    #[test]
    fn non_ascii_text_round_trips() {
        let texts = ["„Gänsefüßchen“", "Řeka"];
        let (pdf, info) = write_pdf_info(
            &[],
            &texts.map(|t| plain_text(t, false, false)),
            clip(),
            &colours(),
            "Test",
        );
        assert!(info.missing_glyphs.is_empty());
        let streams = streams(&pdf);
        let cmap = streams
            .iter()
            .find(|s| find(s, b"beginbfchar").is_some())
            .expect("ToUnicode CMap");
        let to_unicode = bf_chars(cmap);
        let shown: Vec<String> = shown_strings(&content_of(&pdf))
            .iter()
            .map(|s| {
                s.chunks(2)
                    .map(|c| to_unicode[&u16::from_be_bytes([c[0], c[1]])].as_str())
                    .collect()
            })
            .collect();
        assert_eq!(shown, texts);
        assert!(shown.iter().all(|s| !s.contains('?')));
    }

    #[test]
    fn missing_glyphs_are_reported() {
        let (pdf, info) = write_pdf_info(
            &[],
            &[plain_text("Wald ✈ 中", false, false)],
            clip(),
            &colours(),
            "Test",
        );
        assert_eq!(info.missing_glyphs, ['✈', '中'].into_iter().collect());
        assert!(pdf.ends_with(b"%%EOF") || pdf.ends_with(b"%%EOF\n"));
        // .notdef stands in: CID 0 is shown.
        let shown = shown_strings(&content_of(&pdf));
        assert!(shown[0].chunks(2).any(|c| c == [0, 0]));
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
