//! Logos (design §5.2): club and sponsor logos as PNG, JPEG or SVG, `width_mm` wide, the height
//! from the aspect ratio.
//!
//! PNG and JPEG become a [`RasterImage`] and keep their alpha. An SVG must pass an allow-list of
//! plain shapes ([`svg_check`], ADR 0026); then `usvg` parses it (no text, no fonts) and it is
//! drawn as fills and strokes in CMYK. What the sheet cannot draw as written is simplified with a
//! [`SheetNote::LogoSimplified`]. Logo files come from users and may be broken or hostile: the
//! limits below bound the work (ADR 0019), and a file that breaks one is left out with a
//! [`SheetNote::LogoSkipped`].

use std::cell::Cell;
use std::collections::{BTreeSet, HashMap};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use image::{ImageFormat, ImageReader, Limits};
use usvg::roxmltree;
use usvg::tiny_skia_path::PathSegment;

use crate::project::ElementSpec;
use crate::render::geometry::flatten_cubic;
use crate::render::image::MAX_DIMENSION;
use crate::render::page::{
    Cap, DirectColour, FillRule, Join, PaperBox, Pt, RasterImage, Shape, SheetItem,
};

use super::{LogoSimplification, LogoSkipReason, PlacedElement, SheetContext, SheetNote, layout};

mod svg_check;

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
/// Most XML nodes of an SVG logo as written.
const MAX_SVG_NODES: u32 = 200_000;
/// Most text that entity references may expand to (bytes), estimated from above.
const MAX_ENTITY_TEXT: u64 = 50_000_000;

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

/// Most logo pixels on one sheet, all raster logos together, each counted as often as it is
/// placed (ADR 0026). Two logos of the largest size fit.
pub const MAX_PAGE_LOGO_PIXELS: u64 = 50_000_000;
/// Most SVG work on one sheet, all SVG logos together, each counted as often as it is placed: the
/// size of the logo as [`svg_check`] counts it plus the points its curves flatten into (ADR 0026).
pub const MAX_PAGE_SVG_UNITS: u64 = 1_000_000;

/// The logos of one export or `check` run, each distinct file read and decoded once (ADR 0026).
/// Sheets share the decoded image, so the PDF embeds it once per page however often it is placed.
///
/// A file that cannot be used is remembered with its reason. Decoded logos are dropped when
/// keeping another would hold more than two sheets' worth ([`MAX_PAGE_LOGO_PIXELS`],
/// [`MAX_PAGE_SVG_UNITS`]); what a page already holds stays alive there.
#[derive(Debug)]
pub struct LogoCache {
    entries: Mutex<Entries>,
    limits: LogoLimits,
    #[cfg(any(test, feature = "test-hooks"))]
    decodes: std::sync::atomic::AtomicUsize,
}

/// The per-sheet budgets of a [`LogoCache`].
#[derive(Debug, Clone, Copy)]
struct LogoLimits {
    page_pixels: u64,
    page_svg_units: u64,
}

#[derive(Debug, Default)]
struct Entries {
    by_path: HashMap<PathBuf, Entry>,
    /// Pixels and SVG work of the decoded logos held.
    held_pixels: u64,
    held_svg: u64,
}

#[derive(Debug)]
enum Entry {
    Failed(LogoSkipReason),
    Known { cost: Cost, logo: Option<Logo> },
}

/// What placing a logo once costs against the sheet's budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cost {
    Pixels(u64),
    /// [`svg_check`]'s work count; the flattened points are added when it is placed.
    SvgWork(u64),
}

impl Default for LogoCache {
    fn default() -> Self {
        Self::new()
    }
}

impl LogoCache {
    pub fn new() -> Self {
        Self::with_limits(MAX_PAGE_LOGO_PIXELS, MAX_PAGE_SVG_UNITS)
    }

    /// A cache with other per-sheet budgets, for tests.
    fn with_limits(page_pixels: u64, page_svg_units: u64) -> Self {
        LogoCache {
            entries: Mutex::new(Entries::default()),
            limits: LogoLimits {
                page_pixels,
                page_svg_units,
            },
            #[cfg(any(test, feature = "test-hooks"))]
            decodes: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// How many files this cache has decoded (feature `test-hooks`).
    #[cfg(any(test, feature = "test-hooks"))]
    #[doc(hidden)]
    pub fn decodes(&self) -> usize {
        self.decodes.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Counts one decoded raster file or parsed SVG (feature `test-hooks`).
    fn count_decode(&self) {
        #[cfg(any(test, feature = "test-hooks"))]
        self.decodes
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn entries(&self) -> std::sync::MutexGuard<'_, Entries> {
        // A panic while the lock was held leaves the map whole: entries are inserted at once.
        self.entries.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The logo at `path` if `budget` has room for it, decoding it on first use. Spellings of one
    /// path share an entry ([`lexically_normal`]). The error is the
    /// reason the logo is left out.
    fn get(&self, path: &Path, budget: &LogoBudget) -> Result<Logo, LogoSkipReason> {
        // One key for every spelling of the path, so that each file is decoded once.
        let key = lexically_normal(path);
        let known = match self.entries().by_path.get(&key) {
            Some(Entry::Failed(reason)) => return Err(reason.clone()),
            Some(Entry::Known { cost, logo }) => Some((*cost, logo.clone())),
            None => None,
        };
        if let Some((cost, Some(logo))) = known {
            budget.take(cost, &self.limits)?;
            return Ok(logo);
        }
        if let Some((cost, None)) = known {
            budget.fits(cost, &self.limits)?;
        }
        // Read the file and find out what it costs before decoding it.
        let probe = match probe(path) {
            Ok(probe) => probe,
            Err(reason) => {
                self.entries()
                    .by_path
                    .insert(key.clone(), Entry::Failed(reason.clone()));
                return Err(reason);
            }
        };
        let cost = probe.cost();
        if matches!(probe, Probe::Svg { .. }) {
            self.count_decode();
        }
        if let Err(reason) = budget.fits(cost, &self.limits) {
            self.entries()
                .by_path
                .insert(key.clone(), Entry::Known { cost, logo: None });
            return Err(reason);
        }
        if matches!(probe, Probe::Raster { .. }) {
            self.count_decode();
        }
        let logo = match probe.decode() {
            Ok(logo) => logo,
            Err(reason) => {
                self.entries()
                    .by_path
                    .insert(key.clone(), Entry::Failed(reason.clone()));
                return Err(reason);
            }
        };
        self.keep(key, cost, logo.clone());
        budget.take(cost, &self.limits)?;
        Ok(logo)
    }

    /// Stores a decoded logo, first dropping the decoded ones held when it would not fit beside
    /// them.
    fn keep(&self, key: PathBuf, cost: Cost, logo: Logo) {
        let mut entries = self.entries();
        let (pixels, svg) = match cost {
            Cost::Pixels(n) => (n, 0),
            Cost::SvgWork(n) => (0, n),
        };
        let over = entries.held_pixels + pixels > 2 * self.limits.page_pixels
            || entries.held_svg + svg > 2 * self.limits.page_svg_units;
        if over {
            for entry in entries.by_path.values_mut() {
                if let Entry::Known { logo, .. } = entry {
                    *logo = None;
                }
            }
            entries.held_pixels = 0;
            entries.held_svg = 0;
        }
        entries.held_pixels += pixels;
        entries.held_svg += svg;
        entries.by_path.insert(
            key,
            Entry::Known {
                cost,
                logo: Some(logo),
            },
        );
    }
}

/// `path` with `.` dropped and each `..` taking away the name before it, without asking the file
/// system: `logos/../logos/a.png` and `./logos/a.png` both become `logos/a.png`. A link is not
/// followed, so a path through a symbolic link and `..` may name another file than the result;
/// the cache key only needs equal spellings of one path to agree.
fn lexically_normal(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out: Vec<Component<'_>> = Vec::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => match out.last() {
                Some(Component::Normal(_)) => {
                    out.pop();
                }
                // `..` at the root stays at the root.
                Some(Component::RootDir | Component::Prefix(_)) => {}
                _ => out.push(c),
            },
            other => out.push(other),
        }
    }
    out.iter().collect()
}

/// What the logos of one sheet have used of its budgets so far.
#[derive(Debug, Default)]
pub struct LogoBudget {
    pixels: Cell<u64>,
    svg_units: Cell<u64>,
}

impl LogoBudget {
    /// Whether `cost` still fits; the error is the reason the logo is left out.
    fn fits(&self, cost: Cost, limits: &LogoLimits) -> Result<(), LogoSkipReason> {
        match cost {
            Cost::Pixels(n) if self.pixels.get().saturating_add(n) > limits.page_pixels => {
                Err(LogoSkipReason::PixelBudget)
            }
            Cost::SvgWork(n) if self.svg_units.get().saturating_add(n) > limits.page_svg_units => {
                Err(LogoSkipReason::SvgBudget)
            }
            _ => Ok(()),
        }
    }

    /// [`fits`](Self::fits), then counts `cost` as used.
    fn take(&self, cost: Cost, limits: &LogoLimits) -> Result<(), LogoSkipReason> {
        self.fits(cost, limits)?;
        match cost {
            Cost::Pixels(n) => self.pixels.set(self.pixels.get() + n),
            Cost::SvgWork(n) => self.svg_units.set(self.svg_units.get() + n),
        }
        Ok(())
    }

    /// Counts `points` flattened SVG points as used; the error when they break the budget.
    fn take_points(&self, points: usize, limits: &LogoLimits) -> Result<(), LogoSkipReason> {
        let used = self.svg_units.get().saturating_add(points as u64);
        if used > limits.page_svg_units {
            return Err(LogoSkipReason::SvgBudget);
        }
        self.svg_units.set(used);
        Ok(())
    }
}

/// Draws a `logo` element at its anchor, `width_mm` wide. A logo that does not load, or does not
/// fit into the sheet's budget, gives a [`SheetNote::LogoSkipped`] and draws nothing.
pub(super) fn place_logo(
    ctx: &SheetContext<'_>,
    spec: &ElementSpec,
    notes: &mut Vec<SheetNote>,
) -> Option<PlacedElement> {
    let skipped = |file: Option<&Path>, reason| SheetNote::LogoSkipped {
        id: spec.id.clone(),
        file: file.map(Path::to_path_buf),
        reason,
    };
    // The project check makes sure both are set; this keeps a hand-built spec from panicking.
    let (Some(file), Some(width_mm)) = (&spec.file, spec.width_mm) else {
        notes.push(skipped(None, LogoSkipReason::NeedsFileAndWidth));
        return None;
    };
    let path = ctx.project.resolve(file);
    let skipped = |reason| skipped(Some(&path), reason);
    let logo = match ctx.logos.get(&path, ctx.logo_budget) {
        Ok(logo) => logo,
        Err(reason) => {
            notes.push(skipped(reason));
            return None;
        }
    };
    let (w, h) = logo.size();
    let size = (width_mm, width_mm * h / w);
    if !(size.1.is_finite() && size.1 > 0.0) {
        notes.push(skipped(LogoSkipReason::CannotSize));
        return None;
    }
    let top_left = layout::anchor_point(&spec.anchor, ctx.page, ctx.frame, size);
    let rect = PaperBox {
        min: top_left,
        max: (top_left.0 + size.0, top_left.1 + size.1),
    };
    let items = match logo {
        Logo::Raster(image) => vec![SheetItem::Image { rect, image }],
        Logo::Vector(tree) => match svg_items(&tree, rect) {
            Ok((items, simplified, points)) => {
                if let Err(reason) = ctx.logo_budget.take_points(points, &ctx.logos.limits) {
                    notes.push(skipped(reason));
                    return None;
                }
                notes.extend(
                    simplified
                        .into_iter()
                        .map(|reason| SheetNote::LogoSimplified {
                            id: spec.id.clone(),
                            reason,
                        }),
                );
                items
            }
            Err(reason) => {
                notes.push(skipped(reason));
                return None;
            }
        },
    };
    Some(PlacedElement { items, rect })
}

/// A logo as decoded, shared by every place that draws it.
#[derive(Debug, Clone)]
enum Logo {
    Raster(Arc<RasterImage>),
    Vector(Arc<usvg::Tree>),
}

impl Logo {
    /// Width and height in pixels or SVG units, both positive.
    fn size(&self) -> (f64, f64) {
        match self {
            Logo::Raster(image) => (f64::from(image.width()), f64::from(image.height())),
            Logo::Vector(tree) => (
                f64::from(tree.size().width()),
                f64::from(tree.size().height()),
            ),
        }
    }
}

/// A logo file read and checked. A PNG or JPEG is not decoded yet; an SVG is parsed already,
/// since its work count needs the parsed XML.
enum Probe {
    Raster {
        bytes: Vec<u8>,
        format: ImageFormat,
        pixels: u64,
    },
    Svg {
        tree: Arc<usvg::Tree>,
        work: u64,
    },
}

impl Probe {
    fn cost(&self) -> Cost {
        match self {
            Probe::Raster { pixels, .. } => Cost::Pixels(*pixels),
            Probe::Svg { work, .. } => Cost::SvgWork(*work),
        }
    }

    fn decode(self) -> Result<Logo, LogoSkipReason> {
        match self {
            Probe::Raster { bytes, format, .. } => {
                decode_raster(&bytes, format).map(|i| Logo::Raster(Arc::new(i)))
            }
            Probe::Svg { tree, .. } => Ok(Logo::Vector(tree)),
        }
    }
}

/// Reads the logo at `path` and checks what it costs: the size in the header of a PNG or JPEG
/// (no pixel decoded), the work count of an SVG (parsed). The error is the reason it is left out.
fn probe(path: &Path) -> Result<Probe, LogoSkipReason> {
    let bytes = read_file(path)?;
    match image::guess_format(&bytes) {
        Ok(format @ (ImageFormat::Png | ImageFormat::Jpeg)) => {
            let (width, height) = raster_size(&bytes, format)?;
            Ok(Probe::Raster {
                bytes,
                format,
                pixels: u64::from(width) * u64::from(height),
            })
        }
        _ if looks_like_xml(&bytes) => {
            let (tree, work) = parse_svg(&bytes)?;
            Ok(Probe::Svg {
                tree: Arc::new(tree),
                work,
            })
        }
        _ => Err(LogoSkipReason::UnknownFormat),
    }
}

/// The file's bytes, at most [`MAX_FILE_BYTES`]. Only regular files are read, so a device or a
/// pipe cannot stall the export.
fn read_file(path: &Path) -> Result<Vec<u8>, LogoSkipReason> {
    let meta = std::fs::metadata(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => LogoSkipReason::NotFound,
        _ => LogoSkipReason::Unreadable {
            error: e.to_string(),
        },
    })?;
    let too_large = || LogoSkipReason::FileTooLarge;
    if !meta.is_file() {
        return Err(LogoSkipReason::NotAFile);
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(too_large());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|f| f.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes))
        .map_err(|e| LogoSkipReason::Unreadable {
            error: e.to_string(),
        })?;
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

/// The size in the header of a PNG or JPEG, refused when it is over [`MAX_PIXELS`] or
/// [`MAX_DIMENSION`]. No pixel is decoded.
fn raster_size(bytes: &[u8], format: ImageFormat) -> Result<(u32, u32), LogoSkipReason> {
    let (width, height) = ImageReader::with_format(Cursor::new(bytes), format)
        .into_dimensions()
        .map_err(|e| LogoSkipReason::Undecodable {
            error: e.to_string(),
        })?;
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(LogoSkipReason::TooManyPixels { width, height });
    }
    if width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(LogoSkipReason::TooWideOrHigh { width, height });
    }
    Ok((width, height))
}

/// A PNG or JPEG as RGB with its alpha channel, if it has one. The size in the header is checked
/// before any pixel is decoded, and the decoder runs with a memory limit.
fn decode_raster(bytes: &[u8], format: ImageFormat) -> Result<RasterImage, LogoSkipReason> {
    let reader = || ImageReader::with_format(Cursor::new(bytes), format);
    let cannot = |e: image::ImageError| LogoSkipReason::Undecodable {
        error: e.to_string(),
    };
    raster_size(bytes, format)?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    let mut decoder = reader();
    decoder.limits(limits);
    let decoded = decoder.decode().map_err(|e| match e {
        image::ImageError::Limits(_) => LogoSkipReason::DecoderLimit,
        e => cannot(e),
    })?;
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
    RasterImage::new(width, height, rgb, alpha).map_err(|e| LogoSkipReason::Undecodable {
        error: e.to_string(),
    })
}

/// An SVG parsed into paths, with its work count ([`svg_check::check_svg`]). Entities and the allow-list ([`svg_check`]) are checked on the XML
/// before `usvg` resolves it, so a file that is refused never reaches `usvg`. Embedded images are
/// never loaded, external files never read.
fn parse_svg(bytes: &[u8]) -> Result<(usvg::Tree, u64), LogoSkipReason> {
    let text = std::str::from_utf8(bytes).map_err(|_| LogoSkipReason::SvgNotUtf8)?;
    let scan = svg_check::pre_scan(bytes)?;
    check_entities(&scan)?;
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        nodes_limit: MAX_SVG_NODES,
        // No resolver: external entities are not loaded.
        ..roxmltree::ParsingOptions::default()
    };
    let doc = roxmltree::Document::parse_with_options(text, options).map_err(|e| {
        LogoSkipReason::SvgUnreadable {
            error: e.to_string(),
        }
    })?;
    let work = svg_check::check_svg(&doc)?;
    let opt = usvg::Options {
        resources_dir: None,
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_xmltree(&doc, &opt).map_err(|e| LogoSkipReason::SvgUnreadable {
        error: e.to_string(),
    })?;
    Ok((tree, work))
}

/// Rejects entity references that expand into more than [`MAX_ENTITY_TEXT`] bytes in all. The
/// pre-scan has read the entities and counted their uses; an entity holds no references, so its
/// text is brought in once per use.
fn check_entities(scan: &svg_check::Scan<'_>) -> Result<(), LogoSkipReason> {
    if scan.entity_text() > MAX_ENTITY_TEXT {
        return Err(LogoSkipReason::SvgTooComplex {
            what: "its entities expand too far".into(),
        });
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
    fn add_segments(&mut self, n: usize) -> Result<(), LogoSkipReason> {
        self.segments += n;
        if self.segments > MAX_SVG_SEGMENTS {
            return Err(LogoSkipReason::SvgTooComplex {
                what: "more than 20 000 path segments".into(),
            });
        }
        Ok(())
    }

    fn add_points(&mut self, n: usize) -> Result<(), LogoSkipReason> {
        self.points += n;
        if self.points > MAX_SVG_POINTS {
            return Err(LogoSkipReason::SvgTooComplex {
                what: "more than 500 000 points once its curves are flattened".into(),
            });
        }
        Ok(())
    }
}

/// The SVG's paths as sheet items in `rect`, in paint order, and the count of points its curves
/// flattened into. Nothing is returned when a limit is reached, so a logo is drawn whole or not
/// at all. Shapes that reach outside `rect` are clipped to it, as an SVG viewer clips to the
/// picture.
fn svg_items(
    tree: &usvg::Tree,
    rect: PaperBox,
) -> Result<(Vec<SheetItem>, BTreeSet<LogoSimplification>, usize), LogoSkipReason> {
    let mut simplified = BTreeSet::new();
    let to_page = ToPage {
        origin: rect.min,
        scale: (rect.max.0 - rect.min.0) / f64::from(tree.size().width()),
    };
    let mut items = Vec::new();
    let mut budget = Budget::default();
    group_items(
        tree.root(),
        to_page,
        &mut items,
        &mut budget,
        &mut simplified,
    )?;

    let tolerance = 1e-6;
    let outside = items.iter().any(|item| {
        let SheetItem::Shape { shape, .. } = item else {
            return false;
        };
        let (points, margin): (Box<dyn Iterator<Item = &Pt>>, f64) = match shape {
            Shape::Fill { rings, .. } => (Box::new(rings.iter().flatten()), 0.0),
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
                let inner = std::mem::replace(
                    shape,
                    Shape::Fill {
                        rings: Vec::new(),
                        rule: FillRule::EvenOdd,
                    },
                );
                *shape = Shape::Clipped {
                    rings: ring.clone(),
                    inner: Box::new(inner),
                };
            }
        }
    }
    Ok((items, simplified, budget.points))
}

fn group_items(
    group: &usvg::Group,
    to_page: ToPage,
    items: &mut Vec<SheetItem>,
    budget: &mut Budget,
    simplified: &mut BTreeSet<LogoSimplification>,
) -> Result<(), LogoSkipReason> {
    if group.opacity().get() < 1.0 {
        simplified.insert(LogoSimplification::Opacity);
    }
    // The allow-list lets only clip paths through; masks and filters cannot occur.
    if group.clip_path().is_some() || group.mask().is_some() || !group.filters().is_empty() {
        simplified.insert(LogoSimplification::ClipPath);
    }
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => group_items(g, to_page, items, budget, simplified)?,
            usvg::Node::Path(p) => path_items(p, to_page, items, budget, simplified)?,
            // The allow-list keeps images and text out; if `usvg` still makes one, it is noted.
            usvg::Node::Image(_) => {
                simplified.insert(LogoSimplification::Image);
            }
            usvg::Node::Text(_) => {
                simplified.insert(LogoSimplification::Text);
            }
        }
    }
    Ok(())
}

/// The colour of `paint`: a gradient gives its first stop colour, a pattern black.
fn paint_colour(
    paint: &usvg::Paint,
    simplified: &mut BTreeSet<LogoSimplification>,
) -> DirectColour {
    let first_stop = |stops: &[usvg::Stop]| {
        stops.first().map_or(DirectColour::BLACK, |s| {
            let c = s.color();
            rgb_to_cmyk(c.red, c.green, c.blue)
        })
    };
    match paint {
        usvg::Paint::Color(c) => rgb_to_cmyk(c.red, c.green, c.blue),
        usvg::Paint::LinearGradient(g) => {
            simplified.insert(LogoSimplification::Gradient);
            first_stop(g.stops())
        }
        usvg::Paint::RadialGradient(g) => {
            simplified.insert(LogoSimplification::Gradient);
            first_stop(g.stops())
        }
        usvg::Paint::Pattern(_) => {
            simplified.insert(LogoSimplification::Pattern);
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
    simplified: &mut BTreeSet<LogoSimplification>,
) -> Result<(), LogoSkipReason> {
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
            simplified.insert(LogoSimplification::Opacity);
        }
        SheetItem::Shape {
            colour: paint_colour(f.paint(), simplified),
            shape: Shape::Fill {
                rings: subpaths
                    .iter()
                    .filter(|(ring, _)| ring.len() >= 3)
                    .map(|(ring, _)| ring.clone())
                    .collect(),
                rule: match f.rule() {
                    usvg::FillRule::NonZero => FillRule::NonZero,
                    usvg::FillRule::EvenOdd => FillRule::EvenOdd,
                },
            },
        }
    });
    let mut strokes = Vec::new();
    if let Some(s) = path.stroke() {
        if s.opacity().get() < 1.0 {
            simplified.insert(LogoSimplification::Opacity);
        }
        if s.dasharray().is_some() {
            simplified.insert(LogoSimplification::Dashes);
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
        |f| !matches!(f, SheetItem::Shape { shape: Shape::Fill { rings, .. }, .. } if rings.is_empty()),
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
) -> Result<Vec<(Vec<Pt>, bool)>, LogoSkipReason> {
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
    use crate::render::page::{DirectColour, FillRule, PaperBox, RenderedPage, Shape, SheetItem};
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

    /// The sheet items after the frame line and before the footer: what the logo drew. The
    /// footer is the last items: its white box, then its lines of text.
    fn logo_items(page: &RenderedPage) -> Vec<SheetItem> {
        let is_text = |i: &SheetItem| {
            matches!(
                i,
                SheetItem::Shape {
                    shape: Shape::Text { .. },
                    ..
                }
            )
        };
        let mut end = page.sheet.len();
        while end > 0 && is_text(&page.sheet[end - 1]) {
            end -= 1;
        }
        // The footer's white box.
        end = end.saturating_sub(1);
        let start = usize::from(matches!(
            page.sheet.first(),
            Some(SheetItem::Shape {
                shape: Shape::Stroke { .. },
                colour: DirectColour::BLACK,
            })
        ));
        page.sheet[start..end.max(start)].to_vec()
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
            [SheetNote::LogoSkipped { id, file, reason }] => {
                assert_eq!(id, "logo");
                // As the notes read before the reasons became values: the file, then why.
                match file {
                    Some(file) => format!("{}: {reason}", file.display()),
                    None => reason.to_string(),
                }
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

    /// A project whose layout `a4` shows `n` logos of `file`, each 10 mm wide.
    fn many_logos(file: &Path, n: usize) -> crate::project::LoadedProject {
        let mut layout = a4();
        layout.elements = (0..n)
            .map(|i| ElementSpec {
                kind: ElementKind::Logo,
                anchor: Anchor {
                    to: AnchorTarget::Page,
                    corner: Corner::TopLeft,
                    offset_mm: [10.0, 10.0 + 5.0 * i as f64],
                },
                file: Some(file.to_path_buf()),
                width_mm: Some(10.0),
                ..element(&format!("logo{i}"))
            })
            .collect();
        project(MapKind::Sprint, Vec::new(), &layout)
    }

    fn images(page: &RenderedPage) -> Vec<Arc<RasterImage>> {
        page.sheet
            .iter()
            .filter_map(|i| match i {
                SheetItem::Image { image, .. } => Some(image.clone()),
                _ => None,
            })
            .collect()
    }

    fn budget_skips(report: &SheetReport) -> usize {
        report
            .notes
            .iter()
            .filter(|n| {
                matches!(
                    n,
                    SheetNote::LogoSkipped {
                        reason: LogoSkipReason::PixelBudget | LogoSkipReason::SvgBudget,
                        ..
                    }
                )
            })
            .count()
    }

    #[test]
    fn one_file_is_decoded_once_per_cache() {
        let lp = many_logos(&fixture("club.png"), 30);
        let layout = &lp.project.layouts[0];
        let cache = LogoCache::new();
        let map = empty_map();
        let (page, report) = crate::sheet::render_sheet(&lp, &map, layout, &cache).unwrap();
        assert_eq!(logo_notes(&report), []);
        let drawn = images(&page);
        assert_eq!(drawn.len(), 30);
        // One decoded image, shared: the PDF embeds it once.
        assert!(drawn.iter().all(|i| Arc::ptr_eq(i, &drawn[0])));
        assert_eq!(cache.decodes(), 1);
        // A second sheet with the same cache decodes nothing.
        let (page, _) = crate::sheet::render_sheet(&lp, &map, layout, &cache).unwrap();
        assert!(Arc::ptr_eq(&images(&page)[0], &drawn[0]));
        assert_eq!(cache.decodes(), 1);
        // A failure is remembered too.
        let missing = many_logos(Path::new("/nowhere/nope.png"), 3);
        let (_, report) =
            crate::sheet::render_sheet(&missing, &map, &missing.project.layouts[0], &cache)
                .unwrap();
        assert_eq!(logo_notes(&report).len(), 3);
    }

    #[test]
    fn spellings_of_one_path_share_one_decode() {
        // `./logos/club.png` and `logos/../logos/club.png` name one file.
        let plain = fixture("club.png");
        let dir = plain.parent().unwrap();
        let dotted = dir.join(".").join("club.png");
        let detour = dir.join("..").join("logos").join("club.png");
        let mut lp = many_logos(&plain, 3);
        let layout = &mut lp.project.layouts[0];
        layout.elements[1].file = Some(dotted);
        layout.elements[2].file = Some(detour.clone());
        let cache = LogoCache::new();
        let (page, report) =
            crate::sheet::render_sheet(&lp, &empty_map(), &lp.project.layouts[0], &cache).unwrap();
        assert_eq!(logo_notes(&report), []);
        let drawn = images(&page);
        assert_eq!(drawn.len(), 3);
        assert!(drawn.iter().all(|i| Arc::ptr_eq(i, &drawn[0])));
        assert_eq!(cache.decodes(), 1);
        // Notes keep the user's spelling.
        let mut missing = many_logos(&dir.join("..").join("logos").join("nope.png"), 1);
        missing.project.layouts[0].elements[0].file =
            Some(dir.join("..").join("logos").join("nope.png"));
        let (_, report) =
            crate::sheet::render_sheet(&missing, &empty_map(), &missing.project.layouts[0], &cache)
                .unwrap();
        assert!(matches!(&logo_notes(&report)[..],
            [SheetNote::LogoSkipped { file: Some(f), .. }] if f.to_string_lossy().contains("..")));
    }

    #[test]
    fn lexical_normalisation() {
        let n = |p: &str| lexically_normal(Path::new(p));
        assert_eq!(n("/a/./b/../c.png"), PathBuf::from("/a/c.png"));
        assert_eq!(n("a/../../b"), PathBuf::from("../b"));
        assert_eq!(n("/.."), PathBuf::from("/"));
        assert_eq!(n("./x"), PathBuf::from("x"));
    }

    #[test]
    fn pixel_budget_skips_logos_past_it() {
        // club.png has 8 x 4 = 32 pixels: three fit into 100.
        let lp = many_logos(&fixture("club.png"), 30);
        let cache = LogoCache::with_limits(100, MAX_PAGE_SVG_UNITS);
        let (page, report) =
            crate::sheet::render_sheet(&lp, &empty_map(), &lp.project.layouts[0], &cache).unwrap();
        assert_eq!(images(&page).len(), 3);
        assert_eq!(budget_skips(&report), 27);
        assert_eq!(cache.decodes(), 1);
        // The budget is per sheet: the next sheet draws three again.
        let (page, _) =
            crate::sheet::render_sheet(&lp, &empty_map(), &lp.project.layouts[0], &cache).unwrap();
        assert_eq!(images(&page).len(), 3);
    }

    #[test]
    fn svg_budget_skips_logos_past_it() {
        let lp = many_logos(&fixture("sponsor.svg"), 5);
        let layout = &lp.project.layouts[0];
        let (_, report) =
            crate::sheet::render_sheet(&lp, &empty_map(), layout, &LogoCache::new()).unwrap();
        assert_eq!(budget_skips(&report), 0);
        let cache = LogoCache::with_limits(MAX_PAGE_LOGO_PIXELS, 1);
        let (page, report) = crate::sheet::render_sheet(&lp, &empty_map(), layout, &cache).unwrap();
        assert_eq!(budget_skips(&report), 5);
        assert_eq!(logo_items(&page), []);
        assert_eq!(cache.decodes(), 1);
    }

    #[test]
    fn page_budgets_are_50_megapixels_and_a_million_svg_units() {
        assert_eq!(MAX_PAGE_LOGO_PIXELS, 50_000_000);
        assert_eq!(MAX_PAGE_SVG_UNITS, 1_000_000);
    }

    #[test]
    fn cache_drops_decoded_logos_past_its_limit() {
        // Page budget 40 pixels: the cache keeps at most 80, two copies of club.png's 32.
        let dir = tempfile::tempdir().unwrap();
        let png = std::fs::read(fixture("club.png")).unwrap();
        let files: Vec<PathBuf> = ["a.png", "b.png", "c.png"]
            .iter()
            .map(|n| write_temp(&dir, n, &png))
            .collect();
        let cache = LogoCache::with_limits(40, MAX_PAGE_SVG_UNITS);
        let draw = |file: &Path| {
            let lp = many_logos(file, 1);
            let (page, _) =
                crate::sheet::render_sheet(&lp, &empty_map(), &lp.project.layouts[0], &cache)
                    .unwrap();
            assert_eq!(images(&page).len(), 1);
        };
        draw(&files[0]);
        draw(&files[1]);
        draw(&files[0]);
        assert_eq!(cache.decodes(), 2);
        // The third file does not fit beside them: the decoded ones are dropped first.
        draw(&files[2]);
        draw(&files[0]);
        assert_eq!(cache.decodes(), 4);
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
                    shape: Shape::Fill { rings, .. },
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
                reason: LogoSimplification::Gradient
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
        png_claiming_depth(width, height, 8)
    }

    fn png_claiming_depth(width: u32, height: u32, depth: u8) -> Vec<u8> {
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
        ihdr.extend_from_slice(&[depth, 6, 0, 0, 0]);
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
        assert!(reason.ends_with(TOO_COMPLEX), "{reason}");
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
        // Entities that refer to entities are refused by the byte scan, before the XML parser.
        assert!(
            reason.ends_with(&plain("an entity holding markup or references")),
            "{reason}"
        );
        // An entity longer than 1024 bytes is refused by the byte scan too.
        let svg = format!(
            r#"<!DOCTYPE svg [<!ENTITY e "{}">]><svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"/>"#,
            "a".repeat(1025)
        );
        let path = write_temp(&dir, "long.svg", svg.as_bytes());
        let (_, report) = sheet_with(&path, 40.0);
        let reason = skipped_reason(&report);
        assert!(
            reason.ends_with(&plain("an entity longer than 1024 bytes")),
            "{reason}"
        );
        // A plain entity of 1024 bytes used 50 000 times would expand to 51 MB.
        let entity = "a".repeat(1024);
        let body = "&e;".repeat(50_000);
        let svg = format!(
            r#"<?xml version="1.0"?><!DOCTYPE svg [<!ENTITY e "{entity}">]><svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><title>{body}</title></svg>"#
        );
        let path = write_temp(&dir, "wide.svg", svg.as_bytes());
        let (_, report) = sheet_with(&path, 40.0);
        let reason = skipped_reason(&report);
        assert!(reason.ends_with("its entities expand too far"), "{reason}");
    }

    #[test]
    fn svg_entity_with_markup_skipped() {
        // An entity that brings in a mask, and one that hides `<` as a character reference.
        for value in ["<mask id='m'/>", "&#60;mask id='m'/>"] {
            let svg = format!(
                r#"<!DOCTYPE svg [<!ENTITY e "{value}">]><svg xmlns="http://www.w3.org/2000/svg" width="10" height="10">&e;<rect width="9" height="9"/></svg>"#
            );
            let dir = tempfile::tempdir().unwrap();
            let path = write_temp(&dir, "markup.svg", svg.as_bytes());
            let (_, report) = sheet_with(&path, 40.0);
            let reason = skipped_reason(&report);
            assert!(
                reason.ends_with(&plain("an entity holding markup or references")),
                "{value}: {reason}"
            );
        }
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
            <defs><clipPath id="c"><rect width="90" height="90"/></clipPath></defs>
            <rect width="50" height="50" fill="#00ff00" fill-opacity="0.5"/>
            <path d="M5 60 L95 60" stroke="#000" stroke-dasharray="4 2"/>
            <rect x="60" width="40" height="40" fill="#000" clip-path="url(#c)"/>
        </svg>"##;
        let dir = tempfile::tempdir().unwrap();
        let path = write_temp(&dir, "mixed.svg", svg.as_bytes());
        let (page, report) = sheet_with(&path, 40.0);
        let reasons: Vec<LogoSimplification> = logo_notes(&report)
            .into_iter()
            .map(|n| match n {
                SheetNote::LogoSimplified { reason, .. } => reason,
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(
            reasons,
            [
                LogoSimplification::Opacity,
                LogoSimplification::Dashes,
                LogoSimplification::ClipPath,
            ]
        );
        assert_eq!(fills(&logo_items(&page)).len(), 2);
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
        assert!(reason.ends_with(&plain("the element <marker>")), "{reason}");
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
        // Refused by the byte scan, before the XML parser.
        assert!(
            reason.ends_with(&plain("nesting more than 256 levels deep")),
            "{reason}"
        );
        // 256 levels pass: the root, 254 groups and the rectangle.
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10">{}<rect width="1" height="1" fill="#000"/>{}</svg>"##,
            "<g>".repeat(254),
            "</g>".repeat(254)
        );
        let path = write_temp(&dir, "deep.svg", svg.as_bytes());
        let (page, report) = sheet_with(&path, 40.0);
        assert_eq!(logo_notes(&report), []);
        assert_eq!(fills(&logo_items(&page)).len(), 1);
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

    const TOO_COMPLEX: &str = "too complex once `use` and clip paths are resolved";

    fn plain(what: &str) -> String {
        format!(
            "{what} is not supported in logos; export the logo as a plain SVG (shapes, paths, gradients)"
        )
    }

    const SVG_OPEN: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="100" height="100" viewBox="0 0 100 100">"#;

    fn sheet_with_svg(body: &str) -> (RenderedPage, SheetReport) {
        let dir = tempfile::tempdir().unwrap();
        let svg = format!("{SVG_OPEN}{body}</svg>");
        let path = write_temp(&dir, "logo.svg", svg.as_bytes());
        sheet_with(&path, 40.0)
    }

    /// The reason an SVG of `body` is skipped. Every reason below comes from the allow-list or
    /// the count before `usvg` runs, so `usvg` never sees these files.
    fn svg_skipped(body: &str) -> String {
        let (page, report) = sheet_with_svg(body);
        assert_eq!(logo_items(&page), []);
        skipped_reason(&report)
    }

    /// What an SVG of `body` draws, and its notes.
    fn svg_drawn(body: &str) -> (Vec<SheetItem>, Vec<SheetNote>) {
        let (page, report) = sheet_with_svg(body);
        (logo_items(&page), logo_notes(&report))
    }

    /// A `use` bomb: `levels` groups, each using the one below ten times, the lowest a path with
    /// `data` as its `d`. `link` writes the reference to the level below.
    fn use_bomb(levels: usize, data: &str, link: impl Fn(usize) -> String) -> String {
        let mut b = format!(r#"<defs><path id="a0" d="{data}"/><g id="t"/>"#);
        for i in 1..=levels {
            b.push_str(&format!(r#"<g id="a{i}">"#));
            for _ in 0..10 {
                b.push_str(&format!("<use {}/>", link(i - 1)));
            }
            b.push_str("</g>");
        }
        b.push_str(&format!("</defs><use {}/>", link(levels)));
        b
    }

    #[test]
    fn svg_use_bomb_every_href_form_skipped() {
        type Link = fn(usize) -> String;
        let forms: [(&str, Link); 4] = [
            ("href", |i| format!(r##"href="#a{i}""##)),
            ("xlink:href", |i| format!(r##"xlink:href="#a{i}""##)),
            ("leading space", |i| format!(r##"href=" #a{i}""##)),
            ("trailing space", |i| format!(r##"href="#a{i} ""##)),
        ];
        for (form, link) in forms {
            let reason = svg_skipped(&use_bomb(8, "M0 0h1", link));
            assert!(reason.ends_with(TOO_COMPLEX), "{form}: {reason}");
        }
    }

    #[test]
    fn svg_use_of_long_path_data_skipped() {
        // 10^4 copies of a path with 4000 bytes of data.
        let data = format!("M0 0{}", "h1v1".repeat(1000));
        let reason = svg_skipped(&use_bomb(4, &data, |i| format!(r##"href="#a{i}""##)));
        assert!(reason.ends_with(TOO_COMPLEX), "{reason}");
    }

    #[test]
    fn svg_xlink_dummy_does_not_hide_a_use() {
        // `xlink:href` points to an empty group, `href` to the bomb: usvg would follow `href`.
        // Two attributes of one local name are refused before anything is counted.
        let reason = svg_skipped(&use_bomb(8, "M0 0h1", |i| {
            format!(r##"href="#a{i}" xlink:href="#t""##)
        }));
        assert!(
            reason.ends_with(&plain("an element with two \"href\" attributes")),
            "{reason}"
        );
    }

    /// `body` inside a `<g>` that declares the foreign namespace `x` (`urn:x`).
    fn with_x(body: &str) -> String {
        format!(r#"<g xmlns:x="urn:x">{body}</g>"#)
    }

    #[test]
    fn svg_foreign_id_skipped() {
        // usvg finds `use` targets by `id` in any namespace: a target with only `x:id`.
        let mut b = String::from("<defs>");
        for i in 0..4 {
            b.push_str(&format!(r#"<g x:id="u{i}">"#));
            b.push_str(&format!(r##"<use href="#u{}"/>"##, i + 1).repeat(10));
            b.push_str("</g>");
        }
        b.push_str(r##"<path x:id="u4" d="M0 0h1"/></defs><use href="#u0"/>"##);
        let reason = svg_skipped(&with_x(&b));
        assert!(
            reason.ends_with(&plain("the attribute id in the namespace urn:x")),
            "{reason}"
        );
        // `x:id` before `id`: usvg's `use` map takes the first.
        let reason = svg_skipped(&with_x(
            r##"<path x:id="a" id="b" d="M0 0h1"/><use href="#a"/>"##,
        ));
        assert!(
            reason.ends_with(&plain("an element with two ids")),
            "{reason}"
        );
        // Drawing programs' namespaces may not carry an id either.
        let reason = svg_skipped(
            r##"<path xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" inkscape:id="a" d="M0 0h1"/><use href="#a"/>"##,
        );
        assert!(
            reason.ends_with(&plain(
                "the attribute id in the namespace http://www.inkscape.org/namespaces/inkscape"
            )),
            "{reason}"
        );
    }

    #[test]
    fn svg_foreign_path_data_skipped() {
        // A short `x:d` in front of the real `d`, which usvg reads.
        let data = format!("M0 0{}", "h1v1".repeat(100_000));
        let mut b = format!(r#"<defs><path id="p" x:d="M0 0" d="{data}"/></defs>"#);
        b.push_str(&r##"<use href="#p"/>"##.repeat(100));
        let reason = svg_skipped(&with_x(&b));
        assert!(
            reason.ends_with(&plain("an element with two \"d\" attributes")),
            "{reason}"
        );
        // `x:d` alone is not read by usvg, but is refused as well.
        let reason = svg_skipped(&with_x(r#"<path x:d="M0 0h1"/>"#));
        assert!(
            reason.ends_with(&plain("the attribute d in the namespace urn:x")),
            "{reason}"
        );
    }

    #[test]
    fn svg_clip_units_and_inheritance_skipped() {
        let clip = |units: &str| {
            format!(
                r#"<defs><clipPath id="c" {units}><path d="M0 0h1v1z"/></clipPath></defs><rect width="1" height="1" clip-path="url(#c)"/>"#
            )
        };
        // A foreign `userSpaceOnUse` in front of the `xml:` one that usvg reads.
        let reason = svg_skipped(&with_x(&clip(
            r#"x:clipPathUnits="userSpaceOnUse" xml:clipPathUnits="objectBoundingBox""#,
        )));
        assert!(
            reason.ends_with(&plain("an element with two \"clipPathUnits\" attributes")),
            "{reason}"
        );
        // usvg reads `xlink:` and `xml:` attributes by their local name.
        for units in [
            r#"xml:clipPathUnits="objectBoundingBox""#,
            r#"xlink:clipPathUnits="objectBoundingBox""#,
        ] {
            let reason = svg_skipped(&clip(units));
            assert!(
                reason.ends_with(&plain("a clip path in objectBoundingBox units")),
                "{units}: {reason}"
            );
        }
        // `inherit` copies the parent's clip path, in the attribute or in `style`.
        let user = r#"<defs><clipPath id="c"><path d="M0 0h1v1z"/></clipPath></defs>"#;
        for child in [
            r#"<rect width="1" height="1" clip-path="inherit"/>"#,
            r#"<rect width="1" height="1" style="clip-path: inherit"/>"#,
        ] {
            let reason = svg_skipped(&format!(r#"{user}<g clip-path="url(#c)">{child}</g>"#));
            assert!(
                reason.ends_with(&plain("clip-path: inherit")),
                "{child}: {reason}"
            );
        }
        // `style` wins over the attribute and the last declaration over the first in usvg.
        for rect in [
            r#"<rect width="1" height="1" clip-path="url(#c)" style="clip-path:url(#d)"/>"#,
            r#"<rect width="1" height="1" style="clip-path:url(#c);clip-path:url(#d)"/>"#,
        ] {
            let reason = svg_skipped(&format!(
                r#"<defs><clipPath id="c"><rect width="1" height="1"/></clipPath><clipPath id="d"><rect width="1" height="1"/></clipPath></defs>{rect}"#
            ));
            assert!(
                reason.ends_with(&plain("clip-path set more than once on one element")),
                "{rect}: {reason}"
            );
        }
    }

    #[test]
    fn svg_use_loops_skipped() {
        for body in [
            // Two groups that use each other four times each: usvg misses this loop.
            r##"<defs><g id="a"><use href="#b"/><use href="#b"/><use href="#b"/><use href="#b"/></g><g id="b"><use href="#a"/><use href="#a"/><use href="#a"/><use href="#a"/></g></defs><use href="#a"/>"##,
            // A group that uses itself.
            r##"<g id="a"><rect width="1" height="1"/><use href="#a"/></g>"##,
        ] {
            let reason = svg_skipped(body);
            assert!(
                reason.ends_with(&plain("a `use` that refers to itself")),
                "{body}: {reason}"
            );
        }
    }

    #[test]
    fn svg_deep_clip_paths_refused_before_parsing() {
        // Q1: groups with a clip path nested 300 deep. The byte scan refuses them, so no check
        // walks the ancestors of each.
        let body = format!(
            r#"{}<path d="M0 0L1 1"/>{}"#,
            r#"<g clip-path="x">"#.repeat(300),
            "</g>".repeat(300)
        );
        let reason = svg_skipped(&body);
        assert!(
            reason.ends_with(&plain("nesting more than 256 levels deep")),
            "{reason}"
        );
    }

    #[test]
    fn svg_many_attributes_refused_before_parsing() {
        // Q2: one element with 1001 attributes; `>` inside a quoted value does not end the tag.
        let attributes: String = (0..1001).map(|i| format!(r#" a{i}=">""#)).collect();
        let reason = svg_skipped(&format!(r#"<path d="M0 0L1 1"{attributes}/>"#));
        assert!(
            reason.ends_with(&plain("an element with more than 1000 attributes")),
            "{reason}"
        );
        // 1000 attributes each on 60 elements: 60 million pairs to compare.
        let attributes: String = (0..999).map(|i| format!(r#" a{i}="""#)).collect();
        let body = format!(r#"<path d="M0 0L1 1"{attributes}/>"#).repeat(60);
        let reason = svg_skipped(&body);
        assert!(
            reason.ends_with(&plain("this many attributes in all")),
            "{reason}"
        );
        // Attributes in comments and CDATA do not count.
        let comment = format!("<!-- <g {attributes}> -->").repeat(60);
        let (items, notes) = svg_drawn(&format!(
            r##"{comment}<![CDATA[<g {attributes}>]]><rect width="5" height="5" fill="#000"/>"##
        ));
        assert_eq!(notes, []);
        assert_eq!(fills(&items).len(), 1);
    }

    fn gradient(id: &str, stops: usize, link: &str) -> String {
        format!(
            r#"<linearGradient id="{id}" {link}>{}</linearGradient>"#,
            r##"<stop offset="0" stop-color="#ff0000"/>"##.repeat(stops)
        )
    }

    #[test]
    fn svg_gradient_stops_bounded() {
        // Q9: a gradient with more than 256 stops, used directly or through links.
        let reason = svg_skipped(&format!(
            r#"<defs>{}{}</defs><rect width="9" height="9" fill="url(#b)"/>"#,
            gradient("a", 257, ""),
            gradient("b", 0, r##"href="#a""##)
        ));
        assert!(
            reason.ends_with(&plain("a gradient with more than 256 stops")),
            "{reason}"
        );
        // 256 stops pass, but every gradient that takes them is a copy: 4000 copies are too many.
        let mut defs = gradient("a", 256, "");
        for i in 0..4000 {
            defs.push_str(&gradient(&format!("l{i}"), 0, r##"href="#a""##));
        }
        let reason = svg_skipped(&format!(
            r#"<defs>{defs}</defs><rect width="9" height="9" fill="url(#l0)"/>"#
        ));
        assert!(reason.ends_with(TOO_COMPLEX), "{reason}");
        let (items, notes) = svg_drawn(&format!(
            r#"<defs>{}{}</defs><rect width="9" height="9" fill="url(#b)"/>"#,
            gradient("a", 256, ""),
            gradient("b", 0, r##"xlink:href="#a""##)
        ));
        assert_eq!(
            notes,
            [SheetNote::LogoSimplified {
                id: "logo".into(),
                reason: LogoSimplification::Gradient
            }]
        );
        assert_eq!(fills(&items).len(), 1);
    }

    #[test]
    fn svg_equal_href_and_xlink_href_accepted() {
        // Both point to the same path, written slightly differently.
        for hrefs in [
            r##"href="#p" xlink:href="#p""##,
            r##"xlink:href=" #p" href="#p ""##,
        ] {
            let (items, notes) = svg_drawn(&format!(
                r##"<defs><path id="p" d="M0 0H50V50Z" fill="#000"/></defs><use {hrefs}/>"##
            ));
            assert_eq!(notes, [], "{hrefs}");
            assert_eq!(fills(&items).len(), 1, "{hrefs}");
        }
        let reason = svg_skipped(
            r##"<defs><path id="p" d="M0 0H50V50Z"/><path id="q" d="M0 0H5V5Z"/></defs><use href="#p" xlink:href="#q"/>"##,
        );
        assert!(
            reason.ends_with(&plain("an element with two \"href\" attributes")),
            "{reason}"
        );
    }

    #[test]
    fn svg_drawing_program_attributes_accepted() {
        let (items, notes) = svg_drawn(
            r##"<path xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" xmlns:i="http://ns.adobe.com/AdobeIllustrator/10.0/" sodipodi:nodetypes="ccc" i:knockout="Off" d="M0 0H50V50Z" fill="#000"/>"##,
        );
        assert_eq!(notes, []);
        assert_eq!(fills(&items).len(), 1);
    }

    #[test]
    fn svg_patterns_skipped() {
        let reason = svg_skipped(
            r##"<defs><pattern id="p" width="0.5" height="0.5"><rect width="1" height="1"/></pattern></defs><rect width="1" height="1" fill="url(#p)"/>"##,
        );
        assert!(
            reason.ends_with(&plain("the element <pattern>")),
            "{reason}"
        );
        // Patterns nested in patterns.
        let mut b = String::from("<defs>");
        for i in 0..6 {
            let fill = if i < 5 {
                format!(r##" fill="url(#p{})""##, i + 1)
            } else {
                String::new()
            };
            b.push_str(&format!(r#"<pattern id="p{i}" width="0.5" height="0.5">"#));
            b.push_str(&format!(r#"<rect width="1" height="1"{fill}/>"#).repeat(10));
            b.push_str("</pattern>");
        }
        b.push_str(r##"</defs><rect width="10" height="10" fill="url(#p0)"/>"##);
        let reason = svg_skipped(&b);
        assert!(
            reason.ends_with(&plain("the element <pattern>")),
            "{reason}"
        );
    }

    #[test]
    fn svg_masks_skipped() {
        let reason = svg_skipped(
            r##"<defs><mask id="m"><rect width="1" height="1" fill="white"/></mask></defs><rect width="1" height="1" mask="url(#m)"/>"##,
        );
        assert!(reason.ends_with(&plain("the element <mask>")), "{reason}");
        // The 4 KB file of masks nested seven deep that took 3 GB in usvg.
        let mut b = String::from("<defs>");
        for i in 0..7 {
            let mask = if i < 6 {
                format!(r##" mask="url(#m{})""##, i + 1)
            } else {
                String::new()
            };
            b.push_str(&format!(r#"<mask id="m{i}">"#));
            b.push_str(&format!(r#"<rect width="1" height="1" fill="white"{mask}/>"#).repeat(10));
            b.push_str("</mask>");
        }
        b.push_str(r##"</defs><rect width="10" height="10" mask="url(#m0)"/>"##);
        let reason = svg_skipped(&b);
        assert!(reason.ends_with(&plain("the element <mask>")), "{reason}");
        // A mask property without a mask element.
        let reason = svg_skipped(r#"<rect width="1" height="1" style="mask: url(#nowhere)"/>"#);
        assert!(reason.ends_with(&plain("the property mask")), "{reason}");
    }

    #[test]
    fn svg_clip_paths_checked() {
        let bbox = svg_skipped(
            r##"<defs><clipPath id="c" clipPathUnits="objectBoundingBox"><path d="M0 0h1v1z"/></clipPath></defs><rect width="1" height="1" clip-path="url(#c)"/>"##,
        );
        assert!(
            bbox.ends_with(&plain("a clip path in objectBoundingBox units")),
            "{bbox}"
        );
        let nested = svg_skipped(
            r##"<defs><clipPath id="c0"><rect width="1" height="1" clip-path="url(#c1)"/></clipPath><clipPath id="c1"><rect width="1" height="1"/></clipPath></defs><rect width="10" height="10" clip-path="url(#c0)"/>"##,
        );
        assert!(
            nested.ends_with(&plain("a clip path inside a clip path")),
            "{nested}"
        );
        let clipped_clip = svg_skipped(
            r##"<defs><clipPath id="c0" style="clip-path: url( #c1 )"><rect width="1" height="1"/></clipPath><clipPath id="c1"><rect width="1" height="1"/></clipPath></defs><rect width="10" height="10" clip-path="url(#c0)"/>"##,
        );
        assert!(
            clipped_clip.ends_with(&plain("a clip path inside a clip path")),
            "{clipped_clip}"
        );
        let group = svg_skipped(
            r##"<defs><g id="g"><rect width="1" height="1"/></g><clipPath id="c"><use xlink:href="#g"/></clipPath></defs><rect width="10" height="10" clip-path="url(#c)"/>"##,
        );
        assert!(
            group.ends_with(&plain("a clip path that uses <g>")),
            "{group}"
        );
        let not_a_clip = svg_skipped(
            r##"<defs><rect id="r" width="1" height="1"/></defs><rect width="10" height="10" clip-path="url( #r )"/>"##,
        );
        assert!(
            not_a_clip.ends_with(&plain("clip-path pointing to <rect>")),
            "{not_a_clip}"
        );
        // Illustrator's way: a clip path that uses a shape, in user space.
        let (items, notes) = svg_drawn(
            r##"<defs><rect id="r" width="90" height="90"/><clipPath id="c"><use xlink:href="#r" overflow="visible"/></clipPath></defs><g clip-path="url(#c)"><rect width="50" height="50" fill="#000"/></g>"##,
        );
        assert_eq!(
            notes,
            [SheetNote::LogoSimplified {
                id: "logo".into(),
                reason: LogoSimplification::ClipPath
            }]
        );
        assert_eq!(fills(&items).len(), 1);
    }

    #[test]
    fn svg_clip_path_copies_counted() {
        // A user-space clip path of 400 000 bytes of path data, used by 20 shapes.
        let data = format!("M0 0{}z", "h1v1".repeat(100_000));
        let mut b = format!(r#"<defs><clipPath id="c"><path d="{data}"/></clipPath></defs>"#);
        b.push_str(&r##"<rect width="1" height="1" clip-path="url(#c)"/>"##.repeat(20));
        let reason = svg_skipped(&b);
        assert!(reason.ends_with(TOO_COMPLEX), "{reason}");
    }

    #[test]
    fn svg_filters_images_text_and_styles_skipped() {
        let cases = [
            (
                r##"<defs><g id="big"><rect width="1" height="1"/></g><filter id="f"><feImage href="#big"/></filter></defs><rect width="1" height="1" filter="url(#f)"/>"##,
                plain("the element <filter>"),
            ),
            (
                r#"<rect width="1" height="1" style="filter: blur(2px)"/>"#,
                plain("the property filter"),
            ),
            (
                r#"<image href="file:///etc/passwd" width="10" height="10"/>"#,
                plain("the element <image>"),
            ),
            (
                r#"<text x="0" y="9">Club</text>"#,
                plain("the element <text>"),
            ),
            (
                r#"<style>@import url("https://example.com/a.css");</style><rect width="5" height="5"/>"#,
                plain("the element <style>"),
            ),
            (
                r#"<x:style xmlns:x="urn:x">rect { fill: red }</x:style>"#,
                plain("the element <style>"),
            ),
            (
                r#"<switch><rect width="5" height="5"/></switch>"#,
                plain("the element <switch>"),
            ),
            (
                r#"<rect width="5" height="5" cursor="url(#c)"/>"#,
                plain("url() in the property cursor"),
            ),
            (
                r#"<rect id="a" width="5" height="5"/><rect id="a" width="5" height="5"/>"#,
                plain("the same id \"a\" on two elements"),
            ),
        ];
        for (body, expected) in cases {
            let reason = svg_skipped(body);
            assert!(reason.ends_with(&expected), "{body}: {reason}");
        }
    }

    #[test]
    fn svg_paint_links_resolved_as_usvg_does() {
        // Gradients in every form usvg takes.
        for fill in [
            r#"fill="url(#g)""#,
            r#"fill="url( #g )""#,
            r#"fill="url('#g') red""#,
            r#"style="fill: url( &quot;#g&quot; )""#,
        ] {
            let (items, notes) = svg_drawn(&format!(
                r##"<defs><linearGradient id="g"><stop offset="0" stop-color="#ff0000"/><stop offset="1" stop-color="#0000ff"/></linearGradient></defs><rect width="10" height="10" {fill}/>"##
            ));
            assert_eq!(
                notes,
                [SheetNote::LogoSimplified {
                    id: "logo".into(),
                    reason: LogoSimplification::Gradient
                }],
                "{fill}"
            );
            assert!(close_cmyk(fills(&items)[0].0.cmyk, [0.0, 1.0, 1.0, 0.0]));
        }
        // A paint link to anything else, in every form.
        for fill in [
            r#"fill="url(#grp)""#,
            r#"stroke="url( #grp )""#,
            r#"style="fill:url('#grp')""#,
        ] {
            let reason = svg_skipped(&format!(
                r##"<defs><g id="grp"/></defs><rect width="10" height="10" {fill}/>"##
            ));
            assert!(reason.contains("with a <g>"), "{fill}: {reason}");
        }
        let linked = svg_skipped(
            r##"<defs><rect id="r"/><linearGradient id="g" xlink:href=" #r"/></defs><rect width="10" height="10" fill="url(#g)"/>"##,
        );
        assert!(
            linked.ends_with(&plain("a gradient linked to <rect>")),
            "{linked}"
        );
        let looped = svg_skipped(
            r##"<defs><linearGradient id="a" href="#b"/><linearGradient id="b" href="#a"/></defs><rect width="10" height="10" fill="url(#a)"/>"##,
        );
        assert!(
            looped.ends_with(&plain("a chain of gradient links this long")),
            "{looped}"
        );
    }

    #[test]
    fn svg_editor_elements_ignored() {
        let (items, notes) = svg_drawn(
            r##"<sodipodi:namedview xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" pagecolor="#ffffff"/>
            <metadata><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"/></metadata>
            <g xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" inkscape:label="Layer 1"><rect width="10" height="10" fill="#000"/></g>"##,
        );
        assert_eq!(notes, []);
        assert_eq!(fills(&items).len(), 1);
    }

    #[test]
    fn svg_external_use_ignored() {
        // usvg does not load other files; the link resolves to nothing.
        let (items, notes) =
            svg_drawn(r##"<use href="other.svg#a"/><rect width="5" height="5" fill="#000"/>"##);
        assert_eq!(notes, []);
        assert_eq!(fills(&items).len(), 1);
    }

    #[test]
    fn svg_nonzero_fill_keeps_the_overlap() {
        // Two squares drawn the same way round, overlapping in the middle: nonzero fills the
        // overlap, even-odd leaves it empty.
        let d = "M10 10 H60 V60 H10 Z M40 40 H90 V90 H40 Z";
        let (page, report) = sheet_with_svg(&format!(
            r##"<path d="{d}" fill="#000"/><path d="{d}" fill="#000" fill-rule="evenodd"/>"##
        ));
        assert_eq!(logo_notes(&report), []);
        let rules: Vec<FillRule> = logo_items(&page)
            .iter()
            .filter_map(|i| match i {
                SheetItem::Shape {
                    shape: Shape::Fill { rule, rings },
                    ..
                } => {
                    assert_eq!(rings.len(), 2);
                    Some(*rule)
                }
                _ => None,
            })
            .collect();
        assert_eq!(rules, [FillRule::NonZero, FillRule::EvenOdd]);
        let (pdf, _) =
            crate::render::pdf::write_pdf(&page, &crate::render::pdf::PdfMeta::new("t")).unwrap();
        let content = String::from_utf8(crate::render::pdf::content_of(&pdf)).unwrap();
        let ops: Vec<&str> = content
            .lines()
            .filter(|l| *l == "f" || *l == "f*")
            .collect();
        // The logo's two fills, then the footer's white box (even-odd).
        assert_eq!(ops, ["f", "f*", "f*"]);
    }

    #[test]
    fn wide_png_skipped() {
        let dir = tempfile::tempdir().unwrap();
        // 20 megapixels, but wider than a PDF image may be.
        let path = write_temp(&dir, "wide.png", &png_claiming(1_000_000, 20));
        let (_, report) = sheet_with(&path, 40.0);
        let reason = skipped_reason(&report);
        assert!(
            reason.ends_with("1000000 x 20 pixels, wider or higher than 65 535"),
            "{reason}"
        );
    }

    #[test]
    fn deep_png_too_large_to_decode() {
        let dir = tempfile::tempdir().unwrap();
        // 25 megapixels of 16-bit RGBA: 200 MB, more than the decoder may allocate.
        let path = write_temp(&dir, "deep.png", &png_claiming_depth(5000, 5000, 16));
        let (_, report) = sheet_with(&path, 40.0);
        let reason = skipped_reason(&report);
        assert!(
            reason.ends_with("too large to decode; save it as an 8-bit image or with fewer pixels"),
            "{reason}"
        );
    }

    /// An SVG with the document type declaration `doctype` before a black square, the square
    /// preceded by `body`.
    fn svg_with_doctype(doctype: &str, body: &str) -> String {
        format!(
            r##"<?xml version="1.0"?>{doctype}<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10">{body}<rect width="5" height="5" fill="#000"/></svg>"##
        )
    }

    fn skipped_svg(svg: &str) -> String {
        let dir = tempfile::tempdir().unwrap();
        let path = write_temp(&dir, "logo.svg", svg.as_bytes());
        let (page, report) = sheet_with(&path, 40.0);
        assert_eq!(logo_items(&page), []);
        skipped_reason(&report)
    }

    fn drawn_svg(svg: &str) -> (Vec<SheetItem>, Vec<SheetNote>) {
        let dir = tempfile::tempdir().unwrap();
        let path = write_temp(&dir, "logo.svg", svg.as_bytes());
        let (page, report) = sheet_with(&path, 40.0);
        (logo_items(&page), logo_notes(&report))
    }

    #[test]
    fn svg_doctype_beyond_entities_refused_before_parsing() {
        // rr10: declarations the XML parser skips up to `>` or reads to `?>` may hold `]>`; a
        // scan that ended the subset there would read the entity after it as text and miss the
        // markup inside. Every reason below comes from the byte scan, before the XML parser runs.
        let markup = r#"<!ENTITY e "<g><g></g></g>">"#;
        let other = plain("a document type declaration with more than entities and comments");
        for (subset, reason) in [
            (format!("<?x ]>?>{markup}"), other.clone()),
            (format!("<!ELEMENT x ]>{markup}"), other.clone()),
            (
                format!("<!ATTLIST rect fill CDATA \"#f00\">{markup}"),
                other.clone(),
            ),
            (format!("<!NOTATION n SYSTEM \"x\">{markup}"), other.clone()),
            ("%p;".to_string(), other.clone()),
            ("<![CDATA[x]]>".to_string(), other.clone()),
            (
                r#"<!ENTITY % p "x">"#.to_string(),
                plain("a parameter entity"),
            ),
            (
                r#"<!ENTITY e "a%b">"#.to_string(),
                plain("an entity holding markup or references"),
            ),
            (
                r#"<!ENTITY e SYSTEM "file:///etc/passwd">"#.to_string(),
                plain("an external entity"),
            ),
            (
                r#"<!ENTITY e PUBLIC "-//x//EN" "x.ent">"#.to_string(),
                plain("an external entity"),
            ),
            // E1: `]>` inside a comment does not end the subset.
            (
                format!("<!-- ]> -->{markup}"),
                plain("an entity holding markup or references"),
            ),
            (
                format!(r#"<!-- ]> --><!ENTITY w "{}">"#, "a".repeat(2000)),
                plain("an entity longer than 1024 bytes"),
            ),
            // Q1 inside an entity: deep clipped groups that the scan of elements would not see.
            (
                format!(
                    r#"<!ENTITY e "{}{}">"#,
                    "<g clip-path='url(#c)'>".repeat(300),
                    "</g>".repeat(300)
                ),
                plain("an entity holding markup or references"),
            ),
        ] {
            let svg = svg_with_doctype(&format!("<!DOCTYPE svg [{subset}]>"), "&e;");
            let got = skipped_svg(&svg);
            assert!(got.ends_with(&format!(": {reason}")), "{subset}: {got}");
        }
        // A subset that never ends: the SVG after it is not a declaration, or the file ends.
        let svg = svg_with_doctype(r#"<!DOCTYPE svg [<!ENTITY e "x">"#, "");
        assert!(skipped_svg(&svg).ends_with(&other), "unterminated");
        assert!(
            skipped_svg(r#"<!DOCTYPE svg [<!ENTITY e "x">"#)
                .ends_with(&plain("an unterminated document type declaration")),
            "end of file"
        );
        let svg = svg_with_doctype("<!DOCTYPE svg [<!-- ]>", "");
        assert!(
            skipped_svg(&svg).ends_with(&plain("an unterminated document type declaration")),
            "unterminated comment"
        );
    }

    #[test]
    fn svg_second_or_late_doctype_refused() {
        let first = r#"<!DOCTYPE svg [<!ENTITY a "x">]>"#;
        let svg = svg_with_doctype(&format!("{first}<!DOCTYPE svg []>"), "");
        assert!(
            skipped_svg(&svg).ends_with(&plain("a second document type declaration")),
            "second"
        );
        let svg = svg_with_doctype("", "<!DOCTYPE svg []>");
        assert!(
            skipped_svg(&svg).ends_with(&plain("a document type declaration inside the SVG")),
            "late"
        );
    }

    #[test]
    fn svg_doctype_in_comments_and_cdata_ignored() {
        // E2: a document type declaration inside a comment is not the real one. The real one's
        // entity of 1000 bytes is used 60 000 times, 60 MB once expanded; the counts come from
        // the entities the scan read, not from the text of the comment.
        let svg = svg_with_doctype(
            &format!(
                r#"<!-- <!DOCTYPE x []> --><!DOCTYPE svg [<!ENTITY e "{}">]>"#,
                "a".repeat(1000)
            ),
            &format!("<title>{}</title>", "&e;".repeat(60_000)),
        );
        assert!(
            skipped_svg(&svg).ends_with("its entities expand too far"),
            "E2"
        );
        // Declarations in comments and CDATA of the body are ignored, as the XML parser ignores
        // them; the square is drawn.
        let (items, notes) = drawn_svg(&svg_with_doctype(
            "",
            r#"<!-- <!DOCTYPE x [<!ELEMENT y ANY>]> --><title><![CDATA[<!DOCTYPE z []>]]></title>"#,
        ));
        assert_eq!(notes, []);
        assert_eq!(fills(&items).len(), 1);
    }

    #[test]
    fn svg_namespace_declarations_bounded() {
        // N1: namespaces declared on the root and on each child; the XML parser copies all of
        // them for every child that declares one more.
        let root: String = (0..60).map(|i| format!(r#" xmlns:p{i}="u""#)).collect();
        let children: String = (0..10).map(|i| format!(r#"<g xmlns:q{i}="u"/>"#)).collect();
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg"{root} width="10" height="10">{children}<rect width="5" height="5" fill="#000"/></svg>"##
        );
        assert!(
            skipped_svg(&svg).ends_with(&plain("more than 64 namespace declarations")),
            "N1"
        );
        // 64 in all are fine.
        let root: String = (0..53).map(|i| format!(r#" xmlns:p{i}="u""#)).collect();
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg"{root} width="10" height="10">{children}<rect width="5" height="5" fill="#000"/></svg>"##
        );
        let (items, notes) = drawn_svg(&svg);
        assert_eq!(notes, []);
        assert_eq!(fills(&items).len(), 1);
    }

    #[test]
    fn svg_inkscape_file_drawn() {
        // L1: a plain Inkscape save, `version` and `inkscape:version` on the root.
        let svg = r##"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<!-- Created with Inkscape (http://www.inkscape.org/) -->

<svg
   width="50mm"
   height="50mm"
   viewBox="0 0 50 50"
   version="1.1"
   id="svg1"
   inkscape:version="1.3.2 (091e20e, 2023-11-25)"
   sodipodi:docname="logo.svg"
   xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape"
   xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd"
   xmlns:xlink="http://www.w3.org/1999/xlink"
   xmlns="http://www.w3.org/2000/svg"
   xmlns:svg="http://www.w3.org/2000/svg"
   xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
   xmlns:cc="http://creativecommons.org/ns#"
   xmlns:dc="http://purl.org/dc/elements/1.1/">
  <sodipodi:namedview
     id="namedview1"
     pagecolor="#ffffff"
     inkscape:document-units="mm"
     inkscape:zoom="2.1" />
  <defs
     id="defs1" />
  <g
     inkscape:label="Ebene 1"
     inkscape:groupmode="layer"
     id="layer1">
    <path
       style="fill:#008000;fill-opacity:1;stroke:#000000;stroke-width:0.5"
       d="M 5,5 H 45 V 45 H 5 Z"
       id="path1"
       sodipodi:nodetypes="ccccc" />
    <circle
       style="fill:#ffffff"
       id="circle1"
       cx="25"
       cy="25"
       r="10" />
  </g>
  <metadata
     id="metadata1">
    <rdf:RDF>
      <cc:Work
         rdf:about="">
        <dc:title>Logo</dc:title>
      </cc:Work>
    </rdf:RDF>
  </metadata>
</svg>
"##;
        let (items, notes) = drawn_svg(svg);
        assert_eq!(notes, []);
        assert_eq!(fills(&items).len(), 2);
        // A name that `usvg` reads stays refused twice, in any namespace.
        let reason = svg_skipped(
            r#"<path xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" d="M0 0H5V5Z" inkscape:d="M0 0"/>"#,
        );
        assert!(
            reason.ends_with(&plain("an element with two \"d\" attributes")),
            "{reason}"
        );
    }

    #[test]
    fn svg_illustrator_save_as_drawn() {
        // L2: Illustrator's "Save As" writes the SVG 1.1 public identifier and entities for its
        // namespaces, used in `xmlns` attributes.
        let svg = r##"<?xml version="1.0" encoding="utf-8"?>
<!-- Generator: Adobe Illustrator 24.0.0, SVG Export Plug-In . SVG Version: 6.00 Build 0)  -->
<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd" [
	<!ENTITY ns_extend "http://ns.adobe.com/Extensibility/1.0/">
	<!ENTITY ns_ai "http://ns.adobe.com/AdobeIllustrator/10.0/">
	<!ENTITY ns_graphs "http://ns.adobe.com/Graphs/1.0/">
	<!ENTITY ns_vars "http://ns.adobe.com/Variables/1.0/">
	<!ENTITY ns_imrep "http://ns.adobe.com/ImageReplacement/1.0/">
	<!ENTITY ns_sfw "http://ns.adobe.com/SaveForWeb/1.0/">
	<!ENTITY ns_custom "http://ns.adobe.com/GenericCustomNamespace/1.0/">
	<!ENTITY ns_adobe_xpath "http://ns.adobe.com/XPath/1.0/">
]>
<svg version="1.1" id="Ebene_1" xmlns:x="&ns_extend;" xmlns:i="&ns_ai;" xmlns:graph="&ns_graphs;"
	 xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" x="0px" y="0px" viewBox="0 0 100 100"
	 style="enable-background:new 0 0 100 100;" xml:space="preserve">
<metadata>
	<sfw  xmlns="&ns_sfw;">
		<slices></slices>
		<sliceSourceBounds  bottomLeftOrigin="true" height="100" width="100" x="0" y="0"></sliceSourceBounds>
	</sfw>
</metadata>
<g>
	<polygon style="fill:#E30613;" points="50,20 60,45 85,45 65,60 72,85 50,70 28,85 35,60 15,45 40,45 	"/>
	<path i:knockout="Off" style="fill:none;stroke:#1D1D1B;stroke-width:3;stroke-miterlimit:10;" d="M20,80c10-10,50-10,60,0"/>
</g>
</svg>
"##;
        let (items, notes) = drawn_svg(svg);
        assert_eq!(notes, []);
        assert_eq!(fills(&items).len(), 1);
    }

    #[test]
    fn svg_entity_declarations_bounded() {
        // R1: the XML parser looks every reference up among all entities in turn. Many empty
        // entities expand to nothing, so only their number bounds the time.
        let decls =
            |n: usize| -> String { (0..n).map(|i| format!(r#"<!ENTITY a{i} "">"#)).collect() };
        let refs = "&a64;".repeat(1000);
        let svg = svg_with_doctype(
            &format!("<!DOCTYPE svg [{}]>", decls(65)),
            &format!("<title>{refs}</title>"),
        );
        assert!(
            skipped_svg(&svg).ends_with(&plain("more than 64 entities")),
            "65 entities"
        );
        let refs = "&a63;".repeat(1000);
        let (items, notes) = drawn_svg(&svg_with_doctype(
            &format!("<!DOCTYPE svg [{}]>", decls(64)),
            &format!(r#"<title>{refs}</title><g data-x="{refs}"/>"#),
        ));
        assert_eq!(notes, []);
        assert_eq!(fills(&items).len(), 1);
    }

    #[test]
    fn svg_style_bounded() {
        // R3: `simplecss` works out a line and column for every declaration from the start of
        // the value, so a long style takes time that grows with its square.
        let rect = |style: &str| format!(r##"<rect width="5" height="5" style="{style}"/>"##);
        let reason = svg_skipped(&rect(&"fill:red;".repeat(8000)));
        assert!(
            reason.ends_with(&plain("a style attribute longer than 2048 bytes")),
            "{reason}"
        );
        let reason = svg_skipped(&rect(&"a:b;".repeat(65)));
        assert!(
            reason.ends_with(&plain("a style attribute with more than 64 declarations")),
            "{reason}"
        );
        // The same built from entities: short in the file, long once expanded.
        let built = |entity: &str, uses: usize| {
            svg_with_doctype(
                &format!(r#"<!DOCTYPE svg [<!ENTITY e "{entity}">]>"#),
                &rect(&"&e;".repeat(uses)),
            )
        };
        assert!(
            skipped_svg(&built(&"fill:red;".repeat(113), 1200))
                .ends_with(&plain("a style attribute longer than 2048 bytes"))
                && skipped_svg(&built(&"fill:red;".repeat(113), 3))
                    .ends_with(&plain("a style attribute longer than 2048 bytes")),
            "entity length"
        );
        assert!(
            skipped_svg(&built(&"fill:red;".repeat(30), 3))
                .ends_with(&plain("a style attribute with more than 64 declarations")),
            "entity declarations"
        );
        // 2040 bytes of style on each of 129 shapes: more than 256 KiB in all.
        let long = format!("stroke-dasharray:{}", "1 ".repeat(1011));
        let reason = svg_skipped(&rect(&long).repeat(129));
        assert!(
            reason.ends_with(&plain("this much style in all")),
            "{reason}"
        );
        // A style as editors write it is drawn.
        let (items, notes) = svg_drawn(&rect(
            "fill:#008000;fill-opacity:1;fill-rule:nonzero;stroke:none;stroke-width:0.5;\
             stroke-linecap:round;stroke-linejoin:round;stroke-miterlimit:4;\
             stroke-dasharray:none;stroke-opacity:1;paint-order:normal",
        ));
        assert_eq!(notes, []);
        assert_eq!(fills(&items).len(), 1);
    }

    #[test]
    fn svg_long_attribute_values_bounded() {
        // Entities can blow any attribute up to megabytes; outside path data, which the count of
        // work bounds, a value over 1 MB is refused before the XML parser runs.
        for (attribute, unit) in [
            ("transform", "translate(1 1) ".repeat(66)),
            ("stroke-dasharray", "1 2 ".repeat(250)),
        ] {
            let svg = svg_with_doctype(
                &format!(r#"<!DOCTYPE svg [<!ENTITY e "{unit}">]>"#),
                &format!(r#"<g {attribute}="{}"/>"#, "&e;".repeat(1100)),
            );
            let reason = skipped_svg(&svg);
            assert!(
                reason.ends_with(&plain("an attribute value longer than 1 MB")),
                "{attribute}: {reason}"
            );
        }
    }

    #[test]
    fn svg_attribute_bytes_counted_per_copy() {
        // `usvg` parses the attributes of a used element again for every `use`. A 2 KB style
        // used 200 times through nested `use` is 400 KB of CSS, though the file holds it once.
        let style = format!("stroke-dasharray:{}", "1 ".repeat(1000));
        let reason = svg_skipped(&format!(
            r##"<defs><rect id="r" width="1" height="1" style="{style}"/><g id="g">{}</g></defs>{}"##,
            r##"<use href="#r"/>"##.repeat(10),
            r##"<use href="#g"/>"##.repeat(20)
        ));
        assert!(
            reason.ends_with(&plain(
                "this much style once `use` and clip paths are resolved"
            )),
            "{reason}"
        );
        // A dash array of 1 MB used five times: 6 MB of values to parse.
        let dashes = "1 ".repeat(500_000);
        let reason = svg_skipped(&format!(
            r##"<defs><rect id="r" width="1" height="1" stroke="#000" stroke-dasharray="{dashes}"/></defs>{}"##,
            r##"<use href="#r"/>"##.repeat(5)
        ));
        assert!(reason.ends_with(TOO_COMPLEX), "{reason}");
    }

    #[test]
    fn svg_symbol_used_a_few_times_drawn() {
        let (items, notes) = svg_drawn(
            r##"<defs><symbol id="s"><path d="M0 0H10V10Z" style="fill:#008000;stroke:none;fill-opacity:1"/></symbol></defs>
            <use href="#s" x="0"/><use href="#s" x="20"/><use xlink:href="#s" x="40"/><use href="#s" x="60"/>"##,
        );
        // `usvg` clips a symbol to its viewport; the sheet draws it unclipped and says so.
        assert_eq!(
            notes,
            [SheetNote::LogoSimplified {
                id: "logo".into(),
                reason: LogoSimplification::ClipPath,
            }]
        );
        assert_eq!(fills(&items).len(), 4);
    }
}
