//! Copied from freemapper 743ce4b (ADR 0014).
//! Drawing primitives as a one-page PDF: DeviceCMYK colours, painted in the page's order, texts in
//! embedded subsets of Arimo (ADR 0017).

use std::collections::BTreeSet;
use std::sync::Arc;

use pdf_writer::types::{LineCapStyle, LineJoinStyle, TextRenderingMode};
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref, Str, TextStr};

use super::draw::{FillRule, Shape};
use super::fonts::{PageFonts, face};
use super::geometry::Pt;
use super::image::RasterImage;
use super::page::{RenderedPage, SheetItem};
use super::symbols::{Cap, Join, RenderColour};

/// Points per millimetre.
const PT_PER_MM: f64 = 72.0 / 25.4;
/// OOM's miter limit for PDF (reference §2.1).
const MITER_LIMIT: f32 = 2.236_068;
/// OOM's miter limit for text framing in PDF: Qt 0.5 → √(1 + 4 · 0.5²) = √2 (reference §2.1).
const TEXT_FRAMING_MITER_LIMIT: f32 = std::f32::consts::SQRT_2;

/// Page mm (y down) → page points (y up).
struct Transform {
    page_height: f64,
}

impl Transform {
    fn pt(&self, p: Pt) -> (f32, f32) {
        (
            (p.0 * PT_PER_MM) as f32,
            ((self.page_height - p.1) * PT_PER_MM) as f32,
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

/// The CMYK of the map colour with `priority` (black if the page has none).
fn map_cmyk(colours: &[RenderColour], priority: i32) -> [f64; 4] {
    colours
        .iter()
        .find(|c| c.priority == priority)
        .map_or([0.0, 0.0, 0.0, 1.0], |c| c.cmyk)
}

/// Paints `shape` in `cmyk`. A clipped shape gets its own clip block here;
/// [`write_pdf`] shares one block among consecutive shapes with the same clip instead.
fn paint(
    content: &mut Content,
    t: &Transform,
    cmyk: [f64; 4],
    shape: &Shape,
    fonts: &mut PageFonts,
) {
    let [c, m, y, k] = cmyk.map(|v| v as f32);
    match shape {
        Shape::Fill { rings, rule } => {
            content.set_fill_cmyk(c, m, y, k);
            for ring in rings {
                path(content, t, ring, true);
            }
            match rule {
                FillRule::EvenOdd => content.fill_even_odd(),
                FillRule::NonZero => content.fill_nonzero(),
            };
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
            paint(content, t, cmyk, inner, fonts);
            content.restore_state();
        }
    }
}

/// The resource name of the `index`-th image of a page.
fn image_name(index: usize) -> String {
    format!("Im{index}")
}

/// `image` as a Flate-compressed DeviceRGB XObject, with a DeviceGray soft mask for its alpha.
fn write_image(pdf: &mut Pdf, image: &RasterImage, id: Ref, mask: Option<Ref>) {
    // The whole image is held in memory; pixel limits are enforced where images are decoded (Task 8).
    let data = miniz_oxide::deflate::compress_to_vec_zlib(image.rgb(), 6);
    let mut x = pdf.image_xobject(id, &data);
    x.filter(Filter::FlateDecode);
    x.width(image.width() as i32);
    x.height(image.height() as i32);
    x.color_space().device_rgb();
    x.bits_per_component(8);
    if let Some(mask) = mask {
        x.s_mask(mask);
    }
    x.finish();
    if let (Some(mask), Some(alpha)) = (mask, image.alpha()) {
        let data = miniz_oxide::deflate::compress_to_vec_zlib(alpha, 6);
        let mut x = pdf.image_xobject(mask, &data);
        x.filter(Filter::FlateDecode);
        x.width(image.width() as i32);
        x.height(image.height() as i32);
        x.color_space().device_gray();
        x.bits_per_component(8);
        x.finish();
    }
}

/// A page that could not be written.
#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    #[error("could not embed the font {face}: {message}")]
    Font { face: &'static str, message: String },
}

/// What a written page could not show as asked.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PdfInfo {
    /// Characters of the page's texts that have no glyph in the embedded font (drawn as `.notdef`).
    pub missing_glyphs: BTreeSet<char>,
}

/// The document information a PDF carries about itself.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfMeta {
    pub title: String,
    pub creator: String,
}

impl PdfMeta {
    /// `title` with freemapper-print and its link as the creator ([`crate::CREDIT`]).
    pub fn new(title: &str) -> Self {
        PdfMeta {
            title: title.to_string(),
            creator: crate::CREDIT.to_string(),
        }
    }
}

/// The one-page PDF of `page`: the MediaBox is the page size, one page millimetre is one PDF
/// millimetre, and every primitive is clipped to `page.clip`. Also returns what the page could
/// not show.
pub fn write_pdf(page: &RenderedPage, meta: &PdfMeta) -> Result<(Vec<u8>, PdfInfo), PdfError> {
    let (page_w, page_h) = page.page_mm;
    let t = Transform {
        page_height: page_h,
    };
    let colours = &page.colours;

    // The page's primitives are in paint order already ([`RenderedPage::primitives`]).
    let items = &page.primitives;

    let mut content = Content::new();
    let mut fonts = PageFonts::new();
    // One clip block for the page, and nested in it one for each run of shapes clipped to the
    // same rings (the lines of one area pattern).
    if !items.is_empty() {
        let (cx0, cy0) = t.pt((page.clip.min.0, page.clip.max.1));
        let (cx1, cy1) = t.pt((page.clip.max.0, page.clip.min.1));
        content.save_state();
        content.rect(cx0, cy0, cx1 - cx0, cy1 - cy0);
        content.clip_nonzero();
        content.end_path();
    }
    // The clip blocks open, outermost first. A pattern inside a pattern clips its shapes twice;
    // each level stays open while the next shapes share it.
    let mut open: Vec<&Arc<[Vec<Pt>]>> = Vec::new();
    for primitive in items {
        let mut chain = Vec::new();
        let mut shape = &primitive.shape;
        while let Shape::Clipped { rings, inner } = shape {
            chain.push(rings);
            shape = inner;
        }
        // The shapes of one pattern share their rings, also after the page was moved into place:
        // compare pointers only. Equal rings in separate `Arc`s get a block each; each is paid
        // for when drawn.
        let kept = open
            .iter()
            .zip(&chain)
            .take_while(|(a, b)| Arc::ptr_eq(a, b))
            .count();
        for _ in kept..open.len() {
            content.restore_state();
        }
        open.truncate(kept);
        for rings in &chain[kept..] {
            begin_clip(&mut content, &t, rings);
            open.push(rings);
        }
        paint(
            &mut content,
            &t,
            map_cmyk(colours, primitive.priority),
            shape,
            &mut fonts,
        );
    }
    for _ in &open {
        content.restore_state();
    }
    if !items.is_empty() {
        content.restore_state();
    }

    // The sheet layer: over the whole page, in its own colours, one graphics state per item.
    // Images are embedded once per `Arc`; the content names them by position.
    let mut images: Vec<&Arc<RasterImage>> = Vec::new();
    for item in &page.sheet {
        content.save_state();
        match item {
            SheetItem::Shape { colour, shape } => {
                paint(&mut content, &t, colour.cmyk, shape, &mut fonts);
            }
            SheetItem::Image { rect, image } => {
                let index = images
                    .iter()
                    .position(|i| Arc::ptr_eq(i, image))
                    .unwrap_or_else(|| {
                        images.push(image);
                        images.len() - 1
                    });
                let (x, y) = t.pt((rect.min.0, rect.max.1));
                let w = ((rect.max.0 - rect.min.0) * PT_PER_MM) as f32;
                let h = ((rect.max.1 - rect.min.1) * PT_PER_MM) as f32;
                content.transform([w, 0.0, 0.0, h, x, y]);
                content.x_object(Name(image_name(index).as_bytes()));
            }
        }
        content.restore_state();
    }

    // References come from a counter, so a page holds only the fonts it uses.
    let mut next = Ref::new(1);
    let (catalog, pages, page, contents) = (next.bump(), next.bump(), next.bump(), next.bump());
    let fonts = fonts.finish(&mut next);
    let info = next.bump();
    // Each image takes an object, and a second for its soft mask.
    let image_refs: Vec<(Ref, Option<Ref>)> = images
        .iter()
        .map(|i| (next.bump(), i.alpha().is_some().then(|| next.bump())))
        .collect();
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
    let mut resources = p.resources();
    resources.fonts().pairs(fonts.resources());
    if !image_refs.is_empty() {
        let names: Vec<String> = (0..image_refs.len()).map(image_name).collect();
        resources.x_objects().pairs(
            names
                .iter()
                .zip(&image_refs)
                .map(|(name, (id, _))| (Name(name.as_bytes()), *id)),
        );
    }
    resources.finish();
    p.finish();
    // The content stream comes first, the font programs after it.
    let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&content.finish(), 6);
    pdf.stream(contents, &compressed)
        .filter(Filter::FlateDecode);
    fonts.write(&mut pdf)?;
    for (image, (id, mask)) in images.iter().zip(&image_refs) {
        write_image(&mut pdf, image, *id, *mask);
    }
    pdf.document_info(info)
        .title(TextStr(&meta.title))
        .creator(TextStr(&meta.creator));
    let info = PdfInfo {
        missing_glyphs: fonts.missing,
    };
    Ok((pdf.finish(), info))
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
    use super::super::draw::Primitive;
    use super::super::page::{DirectColour, PaperBox, RasterImage, SheetItem};
    use super::*;

    fn colours() -> Vec<RenderColour> {
        vec![
            RenderColour {
                priority: 2,
                cmyk: [0.0, 0.0, 0.0, 1.0],
                rgb: [0.0, 0.0, 0.0],
                opacity: 1.0,
            },
            RenderColour {
                priority: 9,
                cmyk: [0.0, 0.0, 0.0, 0.6],
                rgb: [0.4, 0.4, 0.4],
                opacity: 1.0,
            },
        ]
    }

    fn square(priority: i32) -> Primitive {
        Primitive {
            priority,
            shape: Shape::Fill {
                rings: vec![vec![(10.0, 10.0), (20.0, 10.0), (20.0, 20.0), (10.0, 20.0)]],
                rule: FillRule::EvenOdd,
            },
        }
    }

    fn clip() -> PaperBox {
        PaperBox {
            min: (0.0, 0.0),
            max: (100.0, 100.0),
        }
    }

    /// An A4 portrait page of `primitives`, clipped to [`clip`].
    fn page(primitives: &[Primitive]) -> RenderedPage {
        RenderedPage {
            primitives: primitives.to_vec(),
            clip: clip(),
            page_mm: (210.0, 297.0),
            colours: colours(),
            sheet: Vec::new(),
        }
    }

    fn write_info(primitives: &[Primitive], title: &str) -> (Vec<u8>, PdfInfo) {
        write_pdf(&page(primitives), &PdfMeta::new(title)).unwrap()
    }

    fn write(primitives: &[Primitive]) -> Vec<u8> {
        write_info(primitives, "Test").0
    }

    fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack.windows(needle.len()).position(|w| w == needle)
    }

    #[test]
    fn primitives_are_painted_in_the_given_order() {
        // `render_page` sorts by priority; the writer keeps the order it gets.
        let pdf = content_of(&write(&[square(2), square(9)]));
        let black = find(&pdf, b"0 0 0 1 k").expect("black fill");
        let grey = find(&pdf, b"0 0 0 0.6 k").expect("grey fill");
        assert!(black < grey, "the first primitive is painted first");
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
        let pdf = content_of(&write(&[line]));
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
        let pdf = content_of(&write(&[clipped_line(15.0)]));
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
        let first = clipped_line(11.0);
        let Shape::Clipped { rings, .. } = &first.shape else {
            panic!()
        };
        let shared = Arc::clone(rings);
        let mut lines: Vec<Primitive> = (1..5)
            .map(|i| {
                let Shape::Clipped { inner, .. } = clipped_line(11.0 + f64::from(i)).shape else {
                    panic!()
                };
                Primitive {
                    priority: 2,
                    shape: Shape::Clipped {
                        rings: Arc::clone(&shared),
                        inner,
                    },
                }
            })
            .collect();
        lines.insert(0, first);
        let pdf = content_of(&write(&lines));
        let clips = pdf.windows(4).filter(|w| w == b"W*\nn").count();
        let strokes = pdf.windows(2).filter(|w| w == b"S\n").count();
        assert_eq!((clips, strokes), (1, 5));
        // Equal rings in separate `Arc`s are not compared point by point: a block each.
        let separate: Vec<Primitive> = (0..2).map(|i| clipped_line(11.0 + f64::from(i))).collect();
        let pdf = content_of(&write(&separate));
        assert_eq!(pdf.windows(4).filter(|w| w == b"W*\nn").count(), 2);
    }

    #[test]
    fn nested_clips_share_their_blocks_too() {
        // Lines of a pattern inside a pattern: clipped to the outer outline and to their own.
        // Each outline is written once, though both clips wrap every line.
        let outer: Arc<[Vec<Pt>]> = vec![vec![(0.0, 0.0), (50.0, 0.0), (50.0, 50.0)]].into();
        let inner: Arc<[Vec<Pt>]> = vec![vec![(10.0, 10.0), (20.0, 10.0), (20.0, 20.0)]].into();
        let lines: Vec<Primitive> = (0..5)
            .map(|i| {
                let Shape::Clipped { inner: line, .. } = clipped_line(11.0 + f64::from(i)).shape
                else {
                    panic!()
                };
                Primitive {
                    priority: 2,
                    shape: Shape::Clipped {
                        rings: Arc::clone(&outer),
                        inner: Box::new(Shape::Clipped {
                            rings: Arc::clone(&inner),
                            inner: line,
                        }),
                    },
                }
            })
            .collect();
        let pdf = content_of(&write(&lines));
        let clips = pdf.windows(4).filter(|w| w == b"W*\nn").count();
        let strokes = pdf.windows(2).filter(|w| w == b"S\n").count();
        let ops = |op: &[u8]| pdf.split(|&b| b == b'\n').filter(|l| *l == op).count();
        let (saves, restores) = (ops(b"q"), ops(b"Q"));
        assert_eq!((clips, strokes), (2, 5));
        assert_eq!((saves, restores), (3, 3), "the page and both outlines");
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
        let pdf = content_of(&write(&[text(0.0, Some(0.3)), text(0.0, None)]));
        // After the page clip block `q … re W n`.
        let body = find(&pdf, b"re\nW\nn\n").expect("page clip");
        let at = |needle: &[u8]| {
            body + find(&pdf[body..], needle)
                .unwrap_or_else(|| panic!("{}", String::from_utf8_lossy(needle)))
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
        let pdf = content_of(&write(&[text(std::f64::consts::FRAC_PI_6, None)]));
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
        let pdf = write(&many);
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
        let pdf = write(&[plain_text("Wald", false, false)]);
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
        let pdf = write(&[
            plain_text("Wald", true, false),
            plain_text("Wald", false, true),
            plain_text("Wald", true, true),
        ]);
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
        let (pdf, info) = write_info(&texts.map(|t| plain_text(t, false, false)), "Test");
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
        let (pdf, info) = write_info(&[plain_text("Wald ✈ 中", false, false)], "Test");
        assert_eq!(info.missing_glyphs, ['✈', '中'].into_iter().collect());
        assert!(pdf.ends_with(b"%%EOF") || pdf.ends_with(b"%%EOF\n"));
        // .notdef stands in: CID 0 is shown.
        let shown = shown_strings(&content_of(&pdf));
        assert!(shown[0].chunks(2).any(|c| c == [0, 0]));
    }

    #[test]
    fn pdf_structure_is_valid() {
        let pdf = write_info(&[square(9)], "Strausberg Vorstadt").0;
        assert!(pdf.starts_with(b"%PDF-"));
        assert!(find(&pdf, b"xref").is_some() && find(&pdf, b"trailer").is_some());
        assert!(pdf.ends_with(b"%%EOF") || pdf.ends_with(b"%%EOF\n"));
        assert!(find(&pdf, b"(Strausberg Vorstadt)").is_some(), "title");
        // UTF-16BE hex (the en dash is not ASCII).
        let creator: String = crate::CREDIT
            .encode_utf16()
            .map(|u| format!("{u:04X}"))
            .collect();
        assert!(find(&pdf, creator.as_bytes()).is_some(), "creator");
        assert!(
            find(&pdf, b"/MediaBox [0 0 595.2756 841.8898]").is_some(),
            "A4"
        );
    }

    fn sheet_page(sheet: Vec<SheetItem>, primitives: &[Primitive]) -> Vec<u8> {
        let mut page = page(primitives);
        page.sheet = sheet;
        write_pdf(&page, &PdfMeta::new("Test")).unwrap().0
    }

    fn sheet_stroke(colour: DirectColour, y: f64) -> SheetItem {
        SheetItem::Shape {
            colour,
            shape: Shape::Stroke {
                path: vec![(150.0, y), (180.0, y)],
                closed: false,
                width: 0.5,
                cap: Cap::Flat,
                join: Join::Miter,
            },
        }
    }

    #[test]
    fn sheet_items_paint_after_map() {
        let sheet = vec![
            sheet_stroke(
                DirectColour {
                    cmyk: [0.1, 0.2, 0.3, 0.4],
                },
                200.0,
            ),
            sheet_stroke(
                DirectColour {
                    cmyk: [0.5, 0.6, 0.7, 0.8],
                },
                210.0,
            ),
        ];
        let pdf = content_of(&sheet_page(sheet, &[square(2), square(9)]));
        let pos =
            |n: &[u8]| find(&pdf, n).unwrap_or_else(|| panic!("{}", String::from_utf8_lossy(n)));
        let (map1, map2) = (pos(b"0 0 0 1 k"), pos(b"0 0 0 0.6 k"));
        let (s1, s2) = (pos(b"0.1 0.2 0.3 0.4 K"), pos(b"0.5 0.6 0.7 0.8 K"));
        assert!(map1 < map2 && map2 < s1 && s1 < s2);
        // The map's clip block is closed before the first sheet item: q (page clip) ... Q, then
        // one q ... Q per sheet item.
        let ops = |op: &[u8]| pdf.split(|&b| b == b'\n').filter(|l| *l == op).count();
        assert_eq!((ops(b"q"), ops(b"Q")), (3, 3));
        let page_restore = find(&pdf[map2..], b"Q\n").unwrap() + map2;
        assert!(page_restore < s1, "map clip closed before the sheet");
    }

    #[test]
    fn direct_cmyk_colour() {
        let fill = SheetItem::Shape {
            colour: DirectColour {
                cmyk: [0.25, 0.5, 0.75, 0.125],
            },
            shape: Shape::Fill {
                rings: vec![vec![(10.0, 10.0), (20.0, 10.0), (20.0, 20.0)]],
                rule: FillRule::EvenOdd,
            },
        };
        let pdf = content_of(&sheet_page(vec![fill], &[]));
        assert!(find(&pdf, b"0.25 0.5 0.75 0.125 k").is_some());
        let pdf = content_of(&sheet_page(
            vec![sheet_stroke(DirectColour::WHITE, 5.0)],
            &[],
        ));
        assert!(find(&pdf, b"0 0 0 0 K").is_some());
        assert_eq!(DirectColour::BLACK.cmyk, [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn map_clip_does_not_apply_to_sheet() {
        // The map clip is (0,0)-(100,100); the sheet stroke at x 150..180 lies outside it.
        let pdf = content_of(&sheet_page(
            vec![sheet_stroke(DirectColour::BLACK, 50.0)],
            &[square(2)],
        ));
        let clip_end = find(&pdf, b"Q\n").expect("map clip closed");
        let stroke = find(&pdf, b"S\n").expect("sheet stroke");
        assert!(clip_end < stroke);
        // Only the map's page clip exists; the sheet adds no clip.
        assert_eq!(pdf.windows(3).filter(|w| *w == b"W\nn").count(), 1);
        // A sheet without map primitives has no clip block at all.
        let pdf = content_of(&sheet_page(
            vec![sheet_stroke(DirectColour::BLACK, 50.0)],
            &[],
        ));
        assert!(find(&pdf, b"W\nn").is_none());
    }

    fn rgba_image() -> Arc<RasterImage> {
        Arc::new(RasterImage::new(2, 1, vec![255, 0, 0, 0, 255, 0], Some(vec![255, 128])).unwrap())
    }

    fn logo(image: &Arc<RasterImage>) -> SheetItem {
        SheetItem::Image {
            rect: PaperBox {
                min: (10.0, 20.0),
                max: (30.0, 25.0),
            },
            image: Arc::clone(image),
        }
    }

    #[test]
    fn image_xobject_with_smask() {
        let image = rgba_image();
        let pdf = sheet_page(vec![logo(&image)], &[]);
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("/Subtype /Image"), "image XObject");
        assert!(text.contains("/SMask"), "soft mask");
        assert!(text.contains("/ColorSpace /DeviceRGB"));
        assert!(text.contains("/ColorSpace /DeviceGray"));
        assert!(text.contains("/XObject"));
        // 20 x 5 mm at (10, 297 - 25) mm, in points.
        let content = content_of(&pdf);
        let pt = |mm: f64| format!("{}", (mm * PT_PER_MM) as f32);
        let cm = format!("{} 0 0 {} {} {} cm", pt(20.0), pt(5.0), pt(10.0), pt(272.0));
        assert!(
            find(&content, cm.as_bytes()).is_some(),
            "{cm} in {}",
            String::from_utf8_lossy(&content)
        );
        assert!(find(&content, b"/Im0 Do").is_some());
    }

    #[test]
    fn image_without_alpha_has_no_smask() {
        let image = Arc::new(RasterImage::new(1, 1, vec![1, 2, 3], None).unwrap());
        let pdf = sheet_page(vec![logo(&image)], &[]);
        assert!(!String::from_utf8_lossy(&pdf).contains("/SMask"));
    }

    #[test]
    fn identical_images_are_embedded_once() {
        let image = rgba_image();
        let pdf = sheet_page(vec![logo(&image), logo(&image)], &[]);
        let content = content_of(&pdf);
        assert_eq!(content.windows(7).filter(|w| *w == b"/Im0 Do").count(), 2);
        assert!(find(&content, b"/Im1").is_none());
        let text = String::from_utf8_lossy(&pdf);
        assert_eq!(
            text.matches("/Subtype /Image").count(),
            2,
            "image and mask once"
        );
        // A different Arc with equal pixels is a separate image.
        let other = Arc::new((*image).clone());
        let pdf = sheet_page(vec![logo(&image), logo(&other)], &[]);
        assert_eq!(
            String::from_utf8_lossy(&pdf)
                .matches("/Subtype /Image")
                .count(),
            4
        );
    }

    #[test]
    fn sheet_text_without_map_text_embeds_one_font() {
        let item = SheetItem::Shape {
            colour: DirectColour::BLACK,
            shape: Shape::Text {
                origin: (10.0, 10.0),
                size: 3.0,
                bold: false,
                italic: false,
                text: "A\u{4e2d}".into(),
                rotation: 0.0,
                stroke: None,
            },
        };
        let mut p = page(&[square(2)]);
        p.sheet = vec![item];
        let (pdf, info) = write_pdf(&p, &PdfMeta::new("T")).unwrap();
        assert!(info.missing_glyphs.contains(&'\u{4e2d}'));
        assert_eq!(
            String::from_utf8_lossy(&pdf)
                .matches("/Subtype /Type0")
                .count(),
            1
        );
    }

    #[test]
    fn sheet_text_shares_fonts_and_reports_missing_glyphs() {
        let item = SheetItem::Shape {
            colour: DirectColour::BLACK,
            shape: Shape::Text {
                origin: (10.0, 10.0),
                size: 3.0,
                bold: false,
                italic: false,
                text: "A\u{4e2d}".into(),
                rotation: 0.0,
                stroke: None,
            },
        };
        let mut p = page(&[text(0.0, None)]);
        p.sheet = vec![item];
        let (pdf, info) = write_pdf(&p, &PdfMeta::new("T")).unwrap();
        assert!(info.missing_glyphs.contains(&'\u{4e2d}'));
        assert_eq!(
            String::from_utf8_lossy(&pdf)
                .matches("/Subtype /Type0")
                .count(),
            1
        );
    }
}
