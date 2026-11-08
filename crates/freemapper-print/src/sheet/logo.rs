//! Logos (design §5.2): club and sponsor logos as PNG, JPEG or SVG, `width_mm` wide, the height
//! from the aspect ratio.
//!
//! PNG and JPEG become a [`RasterImage`] and keep their alpha. SVG is parsed by `usvg` (no text,
//! no fonts) and drawn as fills and strokes in CMYK; what the sheet cannot draw as written is
//! simplified with a [`SheetNote::LogoSimplified`]. Logo files come from users and may be broken
//! or hostile: the limits below bound the work (ADR 0019), and a file that breaks one is left out
//! with a [`SheetNote::LogoSkipped`].

use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::{Cursor, Read};
use std::path::Path;
use std::sync::Arc;

use image::{ImageFormat, ImageReader, Limits};
use usvg::roxmltree;
use usvg::tiny_skia_path::PathSegment;

use crate::project::ElementSpec;
use crate::render::geometry::flatten_cubic;
use crate::render::image::MAX_DIMENSION;
use crate::render::page::{Cap, DirectColour, Join, PaperBox, Pt, RasterImage, Shape, SheetItem};

use super::{PlacedElement, SheetContext, SheetNote, layout};

/// Largest logo file read (bytes).
pub const MAX_FILE_BYTES: u64 = 10_000_000;
/// Largest raster logo (pixels), checked on the header before any pixel is decoded.
pub const MAX_PIXELS: u64 = 25_000_000;
/// Most memory the image decoder may allocate: 25 megapixels of RGBA plus a margin.
const MAX_DECODE_BYTES: u64 = MAX_PIXELS * 4 + 16_000_000;
/// Most path segments of an SVG logo (moves, lines, curves and closes of all its paths).
pub const MAX_SVG_SEGMENTS: usize = 20_000;
/// Most points of an SVG logo once its curves are flattened. Curves far outside the logo, or
/// huge ones, would otherwise flatten into millions of points.
const MAX_SVG_POINTS: usize = 500_000;
/// Most work of an SVG logo once `usvg` has resolved it: one unit per element plus one per two
/// bytes of path data, every `use` and every marker copy counted again.
const MAX_SVG_WORK: u64 = 1_000_000;
/// Most XML nodes of an SVG logo as written.
const MAX_SVG_NODES: u32 = 200_000;
/// Deepest SVG nesting, `use` resolved.
const MAX_SVG_DEPTH: u64 = 256;
/// Most text that entity references may expand to (bytes), estimated from above.
const MAX_ENTITY_TEXT: u64 = 50_000_000;

const GRADIENT: &str = "gradient drawn in its first stop colour";
const PATTERN: &str = "pattern fill drawn in black";
const OPACITY: &str = "transparency ignored, drawn opaque";
const DASHES: &str = "dashes drawn as solid lines";
const IMAGE: &str = "embedded image left out";
const TEXT: &str = "text left out (convert it to paths)";
const EFFECTS: &str = "clip path, mask or filter ignored";

/// RGB to CMYK, naive: k = 1 − max(r, g, b), c = (1 − r − k) / (1 − k) and so on; no colour
/// when k = 1.
pub fn rgb_to_cmyk(r: u8, g: u8, b: u8) -> DirectColour {
    let [r, g, b] = [r, g, b].map(|v| f64::from(v) / 255.0);
    let k = 1.0 - r.max(g).max(b);
    if k >= 1.0 {
        return DirectColour::BLACK;
    }
    let part = |v: f64| (1.0 - v - k) / (1.0 - k);
    DirectColour {
        cmyk: [part(r), part(g), part(b), k],
    }
}

/// Draws a `logo` element at its anchor, `width_mm` wide. A logo that does not load gives a
/// [`SheetNote::LogoSkipped`] and draws nothing.
pub(super) fn place_logo(
    ctx: &SheetContext<'_>,
    spec: &ElementSpec,
    notes: &mut Vec<SheetNote>,
) -> Option<PlacedElement> {
    let skipped = |reason: String| SheetNote::LogoSkipped {
        id: spec.id.clone(),
        reason,
    };
    // The project check makes sure both are set; this keeps a hand-built spec from panicking.
    let (Some(file), Some(width_mm)) = (&spec.file, spec.width_mm) else {
        notes.push(skipped("a logo needs `file` and `width_mm`".into()));
        return None;
    };
    let path = ctx.project.resolve(file);
    let shown = path.display();
    let logo = match load(&path) {
        Ok(logo) => logo,
        Err(reason) => {
            notes.push(skipped(format!("{shown}: {reason}")));
            return None;
        }
    };
    let (w, h) = logo.size();
    let size = (width_mm, width_mm * h / w);
    if !(size.1.is_finite() && size.1 > 0.0) {
        notes.push(skipped(format!("{shown}: cannot size the logo")));
        return None;
    }
    let top_left = layout::anchor_point(&spec.anchor, ctx.page, ctx.frame, size);
    let rect = PaperBox {
        min: top_left,
        max: (top_left.0 + size.0, top_left.1 + size.1),
    };
    let items = match logo {
        Logo::Raster(image) => vec![SheetItem::Image {
            rect,
            image: Arc::new(image),
        }],
        Logo::Vector {
            tree,
            mut simplified,
        } => match svg_items(&tree, rect, &mut simplified) {
            Ok(items) => {
                notes.extend(simplified.into_iter().map(|r| SheetNote::LogoSimplified {
                    id: spec.id.clone(),
                    reason: r.into(),
                }));
                items
            }
            Err(reason) => {
                notes.push(skipped(format!("{shown}: {reason}")));
                return None;
            }
        },
    };
    Some(PlacedElement { items, rect })
}

/// A logo as read, before it is placed.
enum Logo {
    Raster(RasterImage),
    /// `simplified`: what the SVG uses that the sheet cannot draw, found while reading.
    Vector {
        tree: Box<usvg::Tree>,
        simplified: BTreeSet<&'static str>,
    },
}

impl Logo {
    /// Width and height in pixels or SVG units, both positive.
    fn size(&self) -> (f64, f64) {
        match self {
            Logo::Raster(image) => (f64::from(image.width()), f64::from(image.height())),
            Logo::Vector { tree, .. } => (
                f64::from(tree.size().width()),
                f64::from(tree.size().height()),
            ),
        }
    }
}

/// Reads and decodes the logo at `path`; the error is the reason it is left out.
fn load(path: &Path) -> Result<Logo, String> {
    let bytes = read_file(path)?;
    match image::guess_format(&bytes) {
        Ok(format @ (ImageFormat::Png | ImageFormat::Jpeg)) => {
            decode_raster(&bytes, format).map(Logo::Raster)
        }
        _ if looks_like_xml(&bytes) => parse_svg(&bytes),
        _ => Err("not a PNG, JPEG or SVG file".into()),
    }
}

/// The file's bytes, at most [`MAX_FILE_BYTES`]. Only regular files are read, so a device or a
/// pipe cannot stall the export.
fn read_file(path: &Path) -> Result<Vec<u8>, String> {
    let meta = std::fs::metadata(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => "not found".to_string(),
        _ => format!("cannot read the file ({e})"),
    })?;
    let too_large = || "larger than 10 MB".to_string();
    if !meta.is_file() {
        return Err("not a regular file".into());
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(too_large());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|f| f.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes))
        .map_err(|e| format!("cannot read the file ({e})"))?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(too_large());
    }
    Ok(bytes)
}

/// Whether the data starts like XML: `<` after an optional byte order mark and white space.
fn looks_like_xml(bytes: &[u8]) -> bool {
    let rest = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    rest.iter()
        .find(|b| !b.is_ascii_whitespace())
        .is_some_and(|&b| b == b'<')
}

/// A PNG or JPEG as RGB with its alpha channel, if it has one. The size in the header is checked
/// before any pixel is decoded, and the decoder runs with a memory limit.
fn decode_raster(bytes: &[u8], format: ImageFormat) -> Result<RasterImage, String> {
    let reader = || ImageReader::with_format(Cursor::new(bytes), format);
    let cannot = |e: image::ImageError| format!("cannot decode the image ({e})");
    let (width, height) = reader().into_dimensions().map_err(cannot)?;
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(format!(
            "{width} x {height} pixels, more than 25 megapixels"
        ));
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    let mut decoder = reader();
    decoder.limits(limits);
    let decoded = decoder.decode().map_err(cannot)?;
    let (width, height) = (decoded.width(), decoded.height());
    let (rgb, alpha) = if decoded.color().has_alpha() {
        let rgba = decoded.into_rgba8().into_raw();
        let mut rgb = Vec::with_capacity(rgba.len() / 4 * 3);
        let mut alpha = Vec::with_capacity(rgba.len() / 4);
        for px in rgba.as_chunks::<4>().0 {
            rgb.extend_from_slice(&px[..3]);
            alpha.push(px[3]);
        }
        (rgb, Some(alpha))
    } else {
        (decoded.into_rgb8().into_raw(), None)
    };
    RasterImage::new(width, height, rgb, alpha).map_err(|e| format!("cannot use the image ({e})"))
}

/// An SVG parsed into paths. Entities, `use` and nesting are checked on the XML before `usvg`
/// resolves them; embedded images are never loaded, external files never read.
fn parse_svg(bytes: &[u8]) -> Result<Logo, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "cannot read the SVG (not UTF-8)")?;
    check_entities(text)?;
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        nodes_limit: MAX_SVG_NODES,
        // No resolver: external entities are not loaded.
        ..roxmltree::ParsingOptions::default()
    };
    let doc = roxmltree::Document::parse_with_options(text, options)
        .map_err(|e| format!("cannot read the SVG ({e})"))?;
    let mut simplified = BTreeSet::new();
    check_elements(&doc, &mut simplified)?;
    let opt = usvg::Options {
        resources_dir: None,
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..usvg::Options::default()
    };
    let tree =
        usvg::Tree::from_xmltree(&doc, &opt).map_err(|e| format!("cannot read the SVG ({e})"))?;
    Ok(Logo::Vector {
        tree: Box::new(tree),
        simplified,
    })
}

/// Rejects entity declarations that could expand into more than [`MAX_ENTITY_TEXT`] bytes. The
/// XML parser expands one reference into at most 255 nested ones, each no longer than the
/// document type declaration, so every `&` stands for at most 256 times its length.
fn check_entities(text: &str) -> Result<(), String> {
    let Some(start) = text.find("<!DOCTYPE") else {
        return Ok(());
    };
    let dtd = &text[start..];
    let dtd_len = dtd.find("]>").map_or(dtd.len(), |end| end + 2);
    if !dtd[..dtd_len].contains("<!ENTITY") {
        return Ok(());
    }
    let refs = text.bytes().filter(|&b| b == b'&').count() as u64;
    if refs.saturating_mul(256).saturating_mul(dtd_len as u64) > MAX_ENTITY_TEXT {
        return Err("its entities expand too far".into());
    }
    Ok(())
}

/// Size of one SVG element as `usvg` will hold it: one plus half the length of its path data
/// (`d`, `points`), an upper bound of its points.
fn element_work(node: roxmltree::Node<'_, '_>) -> u64 {
    let data =
        node.attribute("d").map_or(0, str::len) + node.attribute("points").map_or(0, str::len);
    1 + data as u64 / 2
}

/// Whether `usvg` places markers on the element's points.
fn takes_markers(node: roxmltree::Node<'_, '_>) -> bool {
    matches!(
        node.tag_name().name(),
        "path" | "line" | "polyline" | "polygon"
    )
}

/// One subtree with every `use` replaced by what it refers to.
#[derive(Debug, Clone, Copy, Default)]
struct Expanded {
    /// Sum of [`element_work`].
    work: u64,
    /// Upper bound of the points that can carry a marker.
    marker_points: u64,
    depth: u64,
}

/// Checks the SVG's size once `usvg` has resolved it: every `use` replaced by what it refers
/// to and every marker copied to the points it marks. Both are done before any limit on the
/// paths could apply, so a small file could otherwise make `usvg` build millions of paths.
/// Notes embedded images and text, which are not drawn. A `use` that refers to itself, directly
/// or not, counts as empty, as `usvg` drops it.
fn check_elements(
    doc: &roxmltree::Document<'_>,
    simplified: &mut BTreeSet<&'static str>,
) -> Result<(), String> {
    let mut ids = HashMap::new();
    for node in doc.descendants().filter(|n| n.is_element()) {
        match node.tag_name().name() {
            "image" => {
                simplified.insert(IMAGE);
            }
            "text" => {
                simplified.insert(TEXT);
            }
            _ => {}
        }
        if let Some(id) = node.attribute("id") {
            ids.entry(id).or_insert(node);
        }
    }
    let target = |node: roxmltree::Node<'_, '_>| {
        if node.tag_name().name() != "use" {
            return None;
        }
        let href = node
            .attribute(("http://www.w3.org/1999/xlink", "href"))
            .or_else(|| node.attribute("href"))?;
        ids.get(href.strip_prefix('#')?).copied()
    };
    // Each subtree worked out children first, without recursion.
    let mut done: HashMap<roxmltree::NodeId, Expanded> = HashMap::new();
    let mut open = HashSet::new();
    let mut stack = vec![(doc.root_element(), false)];
    while let Some((node, expanded)) = stack.pop() {
        if done.contains_key(&node.id()) {
            continue;
        }
        let parts = || {
            node.children()
                .filter(|c| c.is_element())
                .chain(target(node))
        };
        if !expanded {
            if !open.insert(node.id()) {
                continue;
            }
            stack.push((node, true));
            for part in parts() {
                if !done.contains_key(&part.id()) && !open.contains(&part.id()) {
                    stack.push((part, false));
                }
            }
            continue;
        }
        let own = element_work(node);
        let mut sum = Expanded {
            work: own,
            marker_points: if takes_markers(node) { own + 1 } else { 0 },
            depth: 0,
        };
        for part in parts() {
            let p = done.get(&part.id()).copied().unwrap_or_default();
            sum.work = sum.work.saturating_add(p.work);
            sum.marker_points = sum.marker_points.saturating_add(p.marker_points);
            sum.depth = sum.depth.max(p.depth);
        }
        sum.depth += 1;
        if sum.work > MAX_SVG_WORK {
            return Err("too complex once `use` is resolved".into());
        }
        if sum.depth > MAX_SVG_DEPTH {
            return Err(format!("SVG nested deeper than {MAX_SVG_DEPTH} levels"));
        }
        open.remove(&node.id());
        done.insert(node.id(), sum);
    }
    let all = done
        .get(&doc.root_element().id())
        .copied()
        .unwrap_or_default();
    let markers = doc
        .descendants()
        .filter(|n| n.has_tag_name("marker"))
        .filter_map(|m| done.get(&m.id()))
        .fold(0u64, |sum, m| sum.saturating_add(m.work));
    if markers.saturating_mul(all.marker_points) > MAX_SVG_WORK {
        return Err("too complex once its markers are placed".into());
    }
    Ok(())
}

/// Turns SVG units into page mm: the logo's box starts at `origin`, `scale` mm per unit.
#[derive(Debug, Clone, Copy)]
struct ToPage {
    origin: Pt,
    scale: f64,
}

/// What converting an SVG has used up so far.
#[derive(Debug, Default)]
struct Budget {
    segments: usize,
    points: usize,
}

impl Budget {
    fn add_segments(&mut self, n: usize) -> Result<(), String> {
        self.segments += n;
        if self.segments > MAX_SVG_SEGMENTS {
            return Err("more than 20 000 path segments".into());
        }
        Ok(())
    }

    fn add_points(&mut self, n: usize) -> Result<(), String> {
        self.points += n;
        if self.points > MAX_SVG_POINTS {
            return Err("more than 500 000 points once its curves are flattened".into());
        }
        Ok(())
    }
}

/// The SVG's paths as sheet items in `rect`, in paint order. Nothing is returned when a limit is
/// reached, so a logo is drawn whole or not at all. Shapes that reach outside `rect` are clipped
/// to it, as an SVG viewer clips to the picture.
fn svg_items(
    tree: &usvg::Tree,
    rect: PaperBox,
    simplified: &mut BTreeSet<&'static str>,
) -> Result<Vec<SheetItem>, String> {
    let to_page = ToPage {
        origin: rect.min,
        scale: (rect.max.0 - rect.min.0) / f64::from(tree.size().width()),
    };
    let mut items = Vec::new();
    let mut budget = Budget::default();
    group_items(tree.root(), to_page, &mut items, &mut budget, simplified)?;

    let tolerance = 1e-6;
    let outside = items.iter().any(|item| {
        let SheetItem::Shape { shape, .. } = item else {
            return false;
        };
        let (points, margin): (Box<dyn Iterator<Item = &Pt>>, f64) = match shape {
            Shape::Fill { rings } => (Box::new(rings.iter().flatten()), 0.0),
            Shape::Stroke { path, width, .. } => (Box::new(path.iter()), width / 2.0),
            _ => (Box::new(std::iter::empty()), 0.0),
        };
        points.into_iter().any(|p| {
            p.0 - margin < rect.min.0 - tolerance
                || p.0 + margin > rect.max.0 + tolerance
                || p.1 - margin < rect.min.1 - tolerance
                || p.1 + margin > rect.max.1 + tolerance
        })
    });
    if outside {
        let ring: Arc<[Vec<Pt>]> = Arc::from(vec![vec![
            rect.min,
            (rect.max.0, rect.min.1),
            rect.max,
            (rect.min.0, rect.max.1),
        ]]);
        for item in &mut items {
            if let SheetItem::Shape { shape, .. } = item {
                let inner = std::mem::replace(shape, Shape::Fill { rings: Vec::new() });
                *shape = Shape::Clipped {
                    rings: ring.clone(),
                    inner: Box::new(inner),
                };
            }
        }
    }
    Ok(items)
}

fn group_items(
    group: &usvg::Group,
    to_page: ToPage,
    items: &mut Vec<SheetItem>,
    budget: &mut Budget,
    simplified: &mut BTreeSet<&'static str>,
) -> Result<(), String> {
    if group.opacity().get() < 1.0 {
        simplified.insert(OPACITY);
    }
    if group.clip_path().is_some() || group.mask().is_some() || !group.filters().is_empty() {
        simplified.insert(EFFECTS);
    }
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => group_items(g, to_page, items, budget, simplified)?,
            usvg::Node::Path(p) => path_items(p, to_page, items, budget, simplified)?,
            usvg::Node::Image(_) => {
                simplified.insert(IMAGE);
            }
            usvg::Node::Text(_) => {
                simplified.insert(TEXT);
            }
        }
    }
    Ok(())
}

/// The colour of `paint`: a gradient gives its first stop colour, a pattern black.
fn paint_colour(paint: &usvg::Paint, simplified: &mut BTreeSet<&'static str>) -> DirectColour {
    let first_stop = |stops: &[usvg::Stop]| {
        stops.first().map_or(DirectColour::BLACK, |s| {
            let c = s.color();
            rgb_to_cmyk(c.red, c.green, c.blue)
        })
    };
    match paint {
        usvg::Paint::Color(c) => rgb_to_cmyk(c.red, c.green, c.blue),
        usvg::Paint::LinearGradient(g) => {
            simplified.insert(GRADIENT);
            first_stop(g.stops())
        }
        usvg::Paint::RadialGradient(g) => {
            simplified.insert(GRADIENT);
            first_stop(g.stops())
        }
        usvg::Paint::Pattern(_) => {
            simplified.insert(PATTERN);
            DirectColour::BLACK
        }
    }
}

/// One SVG path as a fill and a stroke, in the order the SVG paints them.
fn path_items(
    path: &usvg::Path,
    to_page: ToPage,
    items: &mut Vec<SheetItem>,
    budget: &mut Budget,
    simplified: &mut BTreeSet<&'static str>,
) -> Result<(), String> {
    budget.add_segments(path.data().len())?;
    if !path.is_visible() || (path.fill().is_none() && path.stroke().is_none()) {
        return Ok(());
    }
    let t = path.abs_transform();
    let map = |p: usvg::tiny_skia_path::Point| -> Pt {
        let (x, y) = (f64::from(p.x), f64::from(p.y));
        let (sx, kx, ky, sy) = (
            f64::from(t.sx),
            f64::from(t.kx),
            f64::from(t.ky),
            f64::from(t.sy),
        );
        (
            to_page.origin.0 + to_page.scale * (sx * x + kx * y + f64::from(t.tx)),
            to_page.origin.1 + to_page.scale * (ky * x + sy * y + f64::from(t.ty)),
        )
    };
    let subpaths = flatten(path.data(), map, budget)?;

    let fill = path.fill().map(|f| {
        if f.opacity().get() < 1.0 {
            simplified.insert(OPACITY);
        }
        // `Shape::Fill` fills even-odd; a nonzero SVG fill differs only where subpaths of the
        // same direction overlap.
        SheetItem::Shape {
            colour: paint_colour(f.paint(), simplified),
            shape: Shape::Fill {
                rings: subpaths
                    .iter()
                    .filter(|(ring, _)| ring.len() >= 3)
                    .map(|(ring, _)| ring.clone())
                    .collect(),
            },
        }
    });
    let mut strokes = Vec::new();
    if let Some(s) = path.stroke() {
        if s.opacity().get() < 1.0 {
            simplified.insert(OPACITY);
        }
        if s.dasharray().is_some() {
            simplified.insert(DASHES);
        }
        let colour = paint_colour(s.paint(), simplified);
        // The width scales with the transform's area factor, exact for uniform scaling.
        let factor = f64::from(t.sx * t.sy - t.kx * t.ky).abs().sqrt();
        let width = f64::from(s.width().get()) * factor * to_page.scale;
        let cap = match s.linecap() {
            usvg::LineCap::Butt => Cap::Flat,
            usvg::LineCap::Round => Cap::Round,
            usvg::LineCap::Square => Cap::Square,
        };
        let join = match s.linejoin() {
            usvg::LineJoin::Miter | usvg::LineJoin::MiterClip => Join::Miter,
            usvg::LineJoin::Round => Join::Round,
            usvg::LineJoin::Bevel => Join::Bevel,
        };
        if width > 0.0 && width.is_finite() {
            strokes.extend(subpaths.iter().filter(|(line, _)| line.len() >= 2).map(
                |(line, closed)| SheetItem::Shape {
                    colour,
                    shape: Shape::Stroke {
                        path: line.clone(),
                        closed: *closed,
                        width,
                        cap,
                        join,
                    },
                },
            ));
        }
    }
    let fill = fill.filter(
        |f| !matches!(f, SheetItem::Shape { shape: Shape::Fill { rings }, .. } if rings.is_empty()),
    );
    match path.paint_order() {
        usvg::PaintOrder::FillAndStroke => {
            items.extend(fill);
            items.extend(strokes);
        }
        usvg::PaintOrder::StrokeAndFill => {
            items.extend(strokes);
            items.extend(fill);
        }
    }
    Ok(())
}

/// The subpaths of `data` in page mm, curves flattened as the map renderer does, each with
/// whether it is closed.
fn flatten(
    data: &usvg::tiny_skia_path::Path,
    map: impl Fn(usvg::tiny_skia_path::Point) -> Pt,
    budget: &mut Budget,
) -> Result<Vec<(Vec<Pt>, bool)>, String> {
    let mut subpaths: Vec<(Vec<Pt>, bool)> = Vec::new();
    let mut current: Vec<Pt> = Vec::new();
    let finish = |current: &mut Vec<Pt>, closed: bool, subpaths: &mut Vec<(Vec<Pt>, bool)>| {
        if !current.is_empty() {
            subpaths.push((std::mem::take(current), closed));
        }
    };
    for segment in data.segments() {
        let before = current.len();
        match segment {
            PathSegment::MoveTo(p) => {
                finish(&mut current, false, &mut subpaths);
                current.push(map(p));
            }
            PathSegment::LineTo(p) => current.push(map(p)),
            PathSegment::QuadTo(q, p) => {
                let p0 = current.last().copied().unwrap_or_else(|| map(q));
                let (q, p3) = (map(q), map(p));
                let c1 = (
                    p0.0 + 2.0 / 3.0 * (q.0 - p0.0),
                    p0.1 + 2.0 / 3.0 * (q.1 - p0.1),
                );
                let c2 = (
                    p3.0 + 2.0 / 3.0 * (q.0 - p3.0),
                    p3.1 + 2.0 / 3.0 * (q.1 - p3.1),
                );
                flatten_cubic(p0, c1, c2, p3, &mut current, 0);
            }
            PathSegment::CubicTo(c1, c2, p) => {
                let (c1, c2, p3) = (map(c1), map(c2), map(p));
                let p0 = current.last().copied().unwrap_or(c1);
                flatten_cubic(p0, c1, c2, p3, &mut current, 0);
            }
            PathSegment::Close => {
                // A closed ring repeats no point: the fill and the stroke close it.
                if current.len() > 1 && current.first() == current.last() {
                    current.pop();
                }
                finish(&mut current, true, &mut subpaths);
            }
        }
        budget.add_points(current.len().saturating_sub(before))?;
    }
    finish(&mut current, false, &mut subpaths);
    Ok(subpaths)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::super::tests::{a4, element, empty_map, project, sheet_of};
    use super::*;
    use crate::project::{Anchor, AnchorTarget, Corner, ElementKind};
    use crate::render::page::{DirectColour, PaperBox, RenderedPage, Shape, SheetItem};
    use crate::scale::MapKind;
    use crate::sheet::SheetReport;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/logos")
            .join(name)
    }

    /// A sheet with one logo `width_mm` wide at the page's top-left corner, offset 10/20 mm.
    fn sheet_with(file: &Path, width_mm: f64) -> (RenderedPage, SheetReport) {
        let mut layout = a4();
        layout.elements = vec![ElementSpec {
            kind: ElementKind::Logo,
            anchor: Anchor {
                to: AnchorTarget::Page,
                corner: Corner::TopLeft,
                offset_mm: [10.0, 20.0],
            },
            file: Some(file.to_path_buf()),
            width_mm: Some(width_mm),
            ..element("logo")
        }];
        let lp = project(MapKind::Sprint, Vec::new(), &layout);
        sheet_of(&lp, &empty_map())
    }

    /// The sheet items after the frame line: what the logo drew.
    fn logo_items(page: &RenderedPage) -> Vec<SheetItem> {
        page.sheet
            .iter()
            .skip_while(|i| {
                matches!(
                    i,
                    SheetItem::Shape {
                        shape: Shape::Stroke { .. },
                        colour: DirectColour::BLACK,
                    }
                )
            })
            .take_while(|i| match i {
                // The credit footer starts with its text or white box; the logos come before.
                SheetItem::Shape {
                    shape: Shape::Text { .. },
                    ..
                } => false,
                _ => true,
            })
            .cloned()
            .collect()
    }

    fn logo_notes(report: &SheetReport) -> Vec<SheetNote> {
        report
            .notes
            .iter()
            .filter(|n| {
                matches!(
                    n,
                    SheetNote::LogoSkipped { .. } | SheetNote::LogoSimplified { .. }
                )
            })
            .cloned()
            .collect()
    }

    fn skipped_reason(report: &SheetReport) -> String {
        match logo_notes(report).as_slice() {
            [SheetNote::LogoSkipped { id, reason }] => {
                assert_eq!(id, "logo");
                reason.clone()
            }
            other => panic!("expected one LogoSkipped, got {other:?}"),
        }
    }

    fn image_of(
        items: &[SheetItem],
    ) -> (PaperBox, std::sync::Arc<crate::render::page::RasterImage>) {
        match items {
            [SheetItem::Image { rect, image }] => (*rect, image.clone()),
            other => panic!("expected one image, got {other:?}"),
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn close_cmyk(a: [f64; 4], b: [f64; 4]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6)
    }

    #[test]
    fn png_logo_placed() {
        let (page, report) = sheet_with(&fixture("club.png"), 40.0);
        assert_eq!(logo_notes(&report), []);
        let (rect, image) = image_of(&logo_items(&page));
        // 8 x 4 pixels, 40 mm wide: 20 mm high, top-left corner at the offset.
        assert_eq!(rect.min, (10.0, 20.0));
        assert!(
            close(rect.max.0, 50.0) && close(rect.max.1, 40.0),
            "{rect:?}"
        );
        assert_eq!((image.width(), image.height()), (8, 4));
        assert_eq!(&image.rgb()[..3], &[204, 0, 0]);
        assert_eq!(&image.rgb()[12..15], &[0, 0, 204]);
        let alpha = image.alpha().expect("the PNG keeps its alpha");
        assert_eq!(alpha[0], 255);
        // Bottom-right pixel: transparent.
        assert_eq!(alpha[31], 0);
    }

    #[test]
    fn jpeg_logo() {
        let (page, report) = sheet_with(&fixture("club.jpg"), 32.0);
        assert_eq!(logo_notes(&report), []);
        let (rect, image) = image_of(&logo_items(&page));
        assert!(
            close(rect.max.0, 42.0) && close(rect.max.1, 36.0),
            "{rect:?}"
        );
        assert_eq!((image.width(), image.height()), (16, 8));
        assert!(image.alpha().is_none());
        let [r, g, b] = [image.rgb()[0], image.rgb()[1], image.rgb()[2]];
        assert!(r < 40 && g > 120 && b < 40, "green, got {r} {g} {b}");
    }

    fn write_temp(dir: &tempfile::TempDir, name: &str, bytes: &[u8]) -> PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    /// A fill as drawn: its colour and rings.
    type Fill = (DirectColour, Vec<Vec<(f64, f64)>>);

    fn fills(items: &[SheetItem]) -> Vec<Fill> {
        items
            .iter()
            .filter_map(|i| match i {
                SheetItem::Shape {
                    colour,
                    shape: Shape::Fill { rings },
                } => Some((*colour, rings.clone())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn svg_logo_paths() {
        let dir = tempfile::tempdir().unwrap();
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50" viewBox="0 0 200 100">
            <rect x="0" y="0" width="100" height="100" fill="#ff0000"/>
            <circle cx="150" cy="50" r="40" fill="#0000ff"/>
        </svg>"##;
        let path = write_temp(&dir, "two.svg", svg.as_bytes());
        let (page, report) = sheet_with(&path, 50.0);
        assert_eq!(logo_notes(&report), []);
        let items = logo_items(&page);
        let fills = fills(&items);
        assert_eq!(fills.len(), 2, "{items:?}");
        assert!(close_cmyk(fills[0].0.cmyk, [0.0, 1.0, 1.0, 0.0]));
        assert!(close_cmyk(fills[1].0.cmyk, [1.0, 1.0, 0.0, 0.0]));
        // 200 user units over 50 mm: the red square spans 25 mm from the logo's corner.
        let xs: Vec<f64> = fills[0].1[0].iter().map(|p| p.0).collect();
        let ys: Vec<f64> = fills[0].1[0].iter().map(|p| p.1).collect();
        let min = |v: &[f64]| v.iter().copied().fold(f64::INFINITY, f64::min);
        let max = |v: &[f64]| v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        assert!(close(min(&xs), 10.0) && close(max(&xs), 35.0), "{xs:?}");
        assert!(close(min(&ys), 20.0) && close(max(&ys), 45.0), "{ys:?}");
        // The circle is flattened to a polygon within the logo.
        let circle = &fills[1].1[0];
        assert!(circle.len() > 8, "{circle:?}");
        for p in circle {
            let d = ((p.0 - 47.5).powi(2) + (p.1 - 32.5).powi(2)).sqrt();
            assert!((d - 10.0).abs() < 0.01, "{p:?} at {d}");
        }
    }

    #[test]
    fn svg_stroke_and_gradient() {
        let (page, report) = sheet_with(&fixture("sponsor.svg"), 50.0);
        assert_eq!(
            logo_notes(&report),
            [SheetNote::LogoSimplified {
                id: "logo".into(),
                reason: "gradient drawn in its first stop colour".into()
            }]
        );
        let items = logo_items(&page);
        let fills = fills(&items);
        assert_eq!(fills.len(), 3, "{items:?}");
        // #ffcc00: no cyan, 20 % magenta, full yellow.
        assert!(
            close_cmyk(fills[2].0.cmyk, [0.0, 0.2, 1.0, 0.0]),
            "{:?}",
            fills[2].0
        );
        let strokes: Vec<_> = items
            .iter()
            .filter_map(|i| match i {
                SheetItem::Shape {
                    colour,
                    shape: Shape::Stroke { path, width, .. },
                } => Some((*colour, path.clone(), *width)),
                _ => None,
            })
            .collect();
        assert_eq!(strokes.len(), 1, "{items:?}");
        let (colour, path, width) = &strokes[0];
        assert_eq!(*colour, DirectColour::BLACK);
        // 4 user units at 0.25 mm per unit.
        assert!(close(*width, 1.0), "{width}");
        assert!(close(path[0].0, 12.5) && close(path[0].1, 42.5), "{path:?}");
    }

    #[test]
    fn rgb_to_cmyk_naive() {
        assert_eq!(rgb_to_cmyk(0, 0, 0), DirectColour::BLACK);
        assert_eq!(rgb_to_cmyk(255, 255, 255), DirectColour::WHITE);
        assert!(close_cmyk(
            rgb_to_cmyk(255, 0, 0).cmyk,
            [0.0, 1.0, 1.0, 0.0]
        ));
        // 50 % grey: k = 0.498, no colour.
        let grey = rgb_to_cmyk(128, 128, 128).cmyk;
        assert!(close_cmyk(grey, [0.0, 0.0, 0.0, 1.0 - 128.0 / 255.0]));
        // (0, 102, 204): k = 0.2, c = 1, m = 0.5, y = 0.
        assert!(close_cmyk(
            rgb_to_cmyk(0, 102, 204).cmyk,
            [1.0, 0.5, 0.0, 0.2]
        ));
    }

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = !0u32;
        for &b in bytes {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }

    /// A PNG that claims `width` x `height` RGBA pixels but holds only a few bytes of data.
    fn png_claiming(width: u32, height: u32) -> Vec<u8> {
        let chunk = |kind: &[u8], data: &[u8]| {
            let mut c = (data.len() as u32).to_be_bytes().to_vec();
            let mut body = kind.to_vec();
            body.extend_from_slice(data);
            c.extend_from_slice(&body);
            c.extend_from_slice(&crc32(&body).to_be_bytes());
            c
        };
        let mut ihdr = width.to_be_bytes().to_vec();
        ihdr.extend_from_slice(&height.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        let idat = miniz_oxide::deflate::compress_to_vec_zlib(&[0; 64], 6);
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.extend(chunk(b"IHDR", &ihdr));
        png.extend(chunk(b"IDAT", &idat));
        png.extend(chunk(b"IEND", &[]));
        png
    }

    #[test]
    fn huge_raster_skipped() {
        let dir = tempfile::tempdir().unwrap();
        // 6000 x 5000 = 30 megapixels in a file of about 100 bytes.
        let path = write_temp(&dir, "huge.png", &png_claiming(6000, 5000));
        let (page, report) = sheet_with(&path, 40.0);
        let reason = skipped_reason(&report);
        assert!(
            reason.contains("6000 x 5000") && reason.contains("25 megapixels"),
            "{reason}"
        );
        assert_eq!(logo_items(&page), []);
    }

    #[test]
    fn corrupt_file_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let png = std::fs::read(fixture("club.png")).unwrap();
        let cut = write_temp(&dir, "cut.png", &png[..50]);
        let (page, report) = sheet_with(&cut, 40.0);
        let reason = skipped_reason(&report);
        assert!(
            reason.contains("cut.png") && reason.contains("cannot decode"),
            "{reason}"
        );
        assert_eq!(logo_items(&page), []);

        let text = write_temp(&dir, "logo.png", b"just some text");
        let (_, report) = sheet_with(&text, 40.0);
        let reason = skipped_reason(&report);
        assert!(reason.contains("not a PNG, JPEG or SVG file"), "{reason}");

        let broken = write_temp(
            &dir,
            "broken.svg",
            b"<svg xmlns='http://www.w3.org/2000/svg'><rect",
        );
        let (_, report) = sheet_with(&broken, 40.0);
        let reason = skipped_reason(&report);
        assert!(reason.contains("cannot read the SVG"), "{reason}");
    }

    #[test]
    fn oversized_file_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.png");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(MAX_FILE_BYTES + 1).unwrap();
        let (_, report) = sheet_with(&path, 40.0);
        let reason = skipped_reason(&report);
        assert!(reason.contains("larger than 10 MB"), "{reason}");
    }

    /// An SVG with one path of `segments` segments (a move and the lines after it).
    fn svg_with_segments(segments: usize) -> String {
        let mut d = String::from("M0 0");
        for i in 1..segments {
            d.push_str(&format!(" L{} {}", i % 100, (i / 100) % 100));
        }
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><path d="{d}" fill="none" stroke="black"/></svg>"#
        )
    }

    #[test]
    fn svg_too_complex_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let ok = write_temp(
            &dir,
            "ok.svg",
            svg_with_segments(MAX_SVG_SEGMENTS).as_bytes(),
        );
        let (page, report) = sheet_with(&ok, 40.0);
        assert_eq!(logo_notes(&report), []);
        assert_eq!(logo_items(&page).len(), 1);

        let path = write_temp(
            &dir,
            "busy.svg",
            svg_with_segments(MAX_SVG_SEGMENTS + 1).as_bytes(),
        );
        let (page, report) = sheet_with(&path, 40.0);
        let reason = skipped_reason(&report);
        assert!(
            reason.contains("more than 20 000 path segments"),
            "{reason}"
        );
        assert_eq!(logo_items(&page), []);
    }

    #[test]
    fn svg_use_bomb_skipped() {
        // Each level uses the one below ten times: 10^8 rectangles once resolved.
        let mut defs = String::from(r#"<rect id="l0" width="1" height="1"/>"#);
        for level in 1..=8 {
            defs.push_str(&format!(r#"<g id="l{level}">"#));
            for _ in 0..10 {
                defs.push_str(&format!(r##"<use href="#l{}"/>"##, level - 1));
            }
            defs.push_str("</g>");
        }
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><defs>{defs}</defs><use href="#l8"/></svg>"##
        );
        let dir = tempfile::tempdir().unwrap();
        let path = write_temp(&dir, "bomb.svg", svg.as_bytes());
        let (_, report) = sheet_with(&path, 40.0);
        let reason = skipped_reason(&report);
        assert!(
            reason.contains("too complex once `use` is resolved"),
            "{reason}"
        );
    }

    #[test]
    fn svg_entity_bomb_skipped() {
        let mut dtd = String::from(r#"<!ENTITY e0 "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa">"#);
        for level in 1..=9 {
            let refs = format!("&e{};", level - 1).repeat(10);
            dtd.push_str(&format!(r#"<!ENTITY e{level} "{refs}">"#));
        }
        let body = "&e9;".repeat(10_000);
        let svg = format!(
            r#"<?xml version="1.0"?><!DOCTYPE svg [{dtd}]><svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><title>{body}</title></svg>"#
        );
        let dir = tempfile::tempdir().unwrap();
        let path = write_temp(&dir, "laughs.svg", svg.as_bytes());
        let (_, report) = sheet_with(&path, 40.0);
        let reason = skipped_reason(&report);
        assert!(reason.contains("entities"), "{reason}");
    }

    #[test]
    fn svg_illustrator_entities_accepted() {
        let svg = r##"<?xml version="1.0"?>
<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd" [
  <!ENTITY ns_svg "http://www.w3.org/2000/svg">
]>
<svg xmlns="&ns_svg;" width="10" height="10"><rect width="10" height="10" fill="#000"/></svg>"##;
        let dir = tempfile::tempdir().unwrap();
        let path = write_temp(&dir, "ai.svg", svg.as_bytes());
        let (page, report) = sheet_with(&path, 10.0);
        assert_eq!(logo_notes(&report), []);
        assert_eq!(fills(&logo_items(&page)).len(), 1);
    }

    #[test]
    fn svg_simplifications_noted() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <rect width="50" height="50" fill="#00ff00" fill-opacity="0.5"/>
            <path d="M5 60 L95 60" stroke="#000" stroke-dasharray="4 2"/>
            <image href="data:image/png;base64,iVBORw0KGgo=" width="10" height="10"/>
            <text x="0" y="90">Sponsor</text>
            <rect x="60" width="40" height="40" fill="#000" style="filter: blur(2px)"/>
        </svg>"##;
        let dir = tempfile::tempdir().unwrap();
        let path = write_temp(&dir, "mixed.svg", svg.as_bytes());
        let (page, report) = sheet_with(&path, 40.0);
        let reasons: Vec<String> = logo_notes(&report)
            .into_iter()
            .map(|n| match n {
                SheetNote::LogoSimplified { reason, .. } => reason,
                other => panic!("{other:?}"),
            })
            .collect();
        for expected in [
            "transparency ignored, drawn opaque",
            "dashes drawn as solid lines",
            "embedded image left out",
            "text left out (convert it to paths)",
            "clip path, mask or filter ignored",
        ] {
            assert!(
                reasons.iter().any(|r| r == expected),
                "{expected}: {reasons:?}"
            );
        }
        assert!(!fills(&logo_items(&page)).is_empty());
    }

    #[test]
    fn svg_outside_its_box_clipped() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10">
            <rect x="-50" y="-50" width="200" height="200" fill="#000"/>
        </svg>"##;
        let dir = tempfile::tempdir().unwrap();
        let path = write_temp(&dir, "big.svg", svg.as_bytes());
        let (page, _) = sheet_with(&path, 10.0);
        let items = logo_items(&page);
        let [
            SheetItem::Shape {
                shape: Shape::Clipped { rings, inner },
                ..
            },
        ] = items.as_slice()
        else {
            panic!("{items:?}");
        };
        assert!(matches!(**inner, Shape::Fill { .. }));
        assert_eq!(
            rings[0],
            [(10.0, 20.0), (20.0, 20.0), (20.0, 30.0), (10.0, 30.0)]
        );
    }

    #[test]
    fn svg_marker_bomb_skipped() {
        // A marker of 2000 points on each of 2000 points.
        let mut d = String::from("M0 0");
        for i in 0..2000 {
            d.push_str(&format!(" L{} {}", i % 50, i % 7));
        }
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><defs><marker id="m"><path d="{d}"/></marker></defs><path d="{d}" marker-mid="url(#m)" fill="none" stroke="black"/></svg>"##
        );
        let dir = tempfile::tempdir().unwrap();
        let path = write_temp(&dir, "markers.svg", svg.as_bytes());
        let (_, report) = sheet_with(&path, 40.0);
        let reason = skipped_reason(&report);
        assert!(reason.contains("markers"), "{reason}");
    }

    #[test]
    fn svg_too_deep_skipped() {
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10">{}<rect width="1" height="1"/>{}</svg>"#,
            "<g>".repeat(300),
            "</g>".repeat(300)
        );
        let dir = tempfile::tempdir().unwrap();
        let path = write_temp(&dir, "deep.svg", svg.as_bytes());
        let (_, report) = sheet_with(&path, 40.0);
        let reason = skipped_reason(&report);
        assert!(reason.contains("nested deeper than 256"), "{reason}");
    }

    #[test]
    fn svg_far_curves_skipped() {
        // Control points far outside the picture: every curve flattens into many points.
        let curve = " C 1e30 1e30 -1e30 1e30 5 5".repeat(10);
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><path d="M0 0{curve}"/></svg>"#
        );
        let dir = tempfile::tempdir().unwrap();
        let path = write_temp(&dir, "far.svg", svg.as_bytes());
        let (_, report) = sheet_with(&path, 40.0);
        let reason = skipped_reason(&report);
        assert!(
            reason.contains("points once its curves are flattened"),
            "{reason}"
        );
    }

    #[test]
    fn missing_logo_file_noted() {
        let (page, report) = sheet_with(Path::new("nope/logo.png"), 40.0);
        let reason = skipped_reason(&report);
        assert!(reason.contains("/nowhere/nope/logo.png"), "{reason}");
        assert!(reason.contains("not found"), "{reason}");
        assert_eq!(logo_items(&page), []);
        // The rest of the sheet still renders: the credit footer is there.
        assert!(page.sheet.iter().any(|i| matches!(
            i,
            SheetItem::Shape {
                shape: Shape::Text { .. },
                ..
            }
        )));
    }

    #[test]
    fn directory_as_logo_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let (_, report) = sheet_with(dir.path(), 40.0);
        let reason = skipped_reason(&report);
        assert!(reason.contains("not a regular file"), "{reason}");
    }
}
