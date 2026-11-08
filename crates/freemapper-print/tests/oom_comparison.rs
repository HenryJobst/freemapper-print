//! Compares our PDF with the vector PDF that OpenOrienteering Mapper 0.9.6 exports for the same
//! map (ADR 0009: conformant, not pixel-identical). The OOM files are made by hand, see
//! `tests/fixtures/oom/README.md`. Needs `pdftoppm` (poppler). Run with
//! `cargo test -p freemapper-print --test oom_comparison -- --ignored --nocapture`.

use std::path::{Path, PathBuf};
use std::process::Command;

use freemapper_print::omap::read::read_map_file;
use freemapper_print::print::{Orientation, Paper, PrintOptions, print_map, render_page};
use freemapper_print::render::RenderedPage;
use freemapper_print::render::draw::Shape;
use freemapper_print::scale::infer_kind;

/// Rasterisation resolution (dpi).
const DPI: &str = "150";
/// Pixels at or above this grey level count as white when cropping.
const WHITE_THRESHOLD: u8 = 250;
/// Allowed difference of the cropped sizes (px).
const SIZE_TOLERANCE_PX: usize = 2;
/// A pixel differs when its grey levels are further apart than this.
const GREY_TOLERANCE: u8 = 96;
/// Share of differing pixels that still passes.
const MAX_DIFFERING_SHARE: f64 = 0.03;

const FIXTURES: [&str; 2] = ["luebeck", "forest-sample"];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A grey image, one byte per pixel.
struct Grey {
    width: usize,
    height: usize,
    data: Vec<u8>,
}

/// Parses a binary PGM (P5, maxval 255) as `pdftoppm -gray` writes it.
fn parse_pgm(bytes: &[u8]) -> Grey {
    let mut pos = 0;
    let mut token = || -> String {
        loop {
            while bytes[pos].is_ascii_whitespace() {
                pos += 1;
            }
            if bytes[pos] == b'#' {
                while bytes[pos] != b'\n' {
                    pos += 1;
                }
            } else {
                break;
            }
        }
        let start = pos;
        while !bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        String::from_utf8(bytes[start..pos].to_vec()).unwrap()
    };
    assert_eq!(token(), "P5", "not a binary PGM");
    let width: usize = token().parse().unwrap();
    let height: usize = token().parse().unwrap();
    assert_eq!(token(), "255", "only 8-bit PGM is supported");
    pos += 1; // the single whitespace byte before the data
    let data = bytes[pos..pos + width * height].to_vec();
    Grey {
        width,
        height,
        data,
    }
}

/// The image cut to the bounding box of its non-white pixels. An all-white image stays 0 x 0.
fn crop_to_content(img: &Grey) -> Grey {
    let (mut x0, mut y0, mut x1, mut y1) = (img.width, img.height, 0, 0);
    for y in 0..img.height {
        for x in 0..img.width {
            if img.data[y * img.width + x] < WHITE_THRESHOLD {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    if x1 <= x0 || y1 <= y0 {
        return Grey {
            width: 0,
            height: 0,
            data: Vec::new(),
        };
    }
    let (width, height) = (x1 - x0, y1 - y0);
    let mut data = Vec::with_capacity(width * height);
    for y in y0..y1 {
        data.extend_from_slice(&img.data[y * img.width + x0..y * img.width + x1]);
    }
    Grey {
        width,
        height,
        data,
    }
}

/// Difference image of the overlap (white = equal, darker = further apart) and the share of
/// pixels whose grey levels differ by more than [`GREY_TOLERANCE`].
fn compare(a: &Grey, b: &Grey) -> (Grey, f64) {
    let width = a.width.min(b.width);
    let height = a.height.min(b.height);
    let mut data = Vec::with_capacity(width * height);
    let mut differing = 0usize;
    for y in 0..height {
        for x in 0..width {
            let p = a.data[y * a.width + x];
            let q = b.data[y * b.width + x];
            let d = p.abs_diff(q);
            if d > GREY_TOLERANCE {
                differing += 1;
            }
            data.push(255 - d);
        }
    }
    let share = if width * height == 0 {
        1.0
    } else {
        differing as f64 / (width * height) as f64
    };
    (
        Grey {
            width,
            height,
            data,
        },
        share,
    )
}

fn write_pgm(path: &Path, img: &Grey) {
    let mut out = format!("P5\n{} {}\n255\n", img.width, img.height).into_bytes();
    out.extend_from_slice(&img.data);
    std::fs::write(path, out).unwrap();
}

/// The `pdftoppm` to use: on the PATH, or Homebrew's location. `None` when neither runs.
fn find_pdftoppm() -> Option<&'static str> {
    [
        "pdftoppm",
        "/opt/homebrew/bin/pdftoppm",
        "/usr/local/bin/pdftoppm",
    ]
    .into_iter()
    .find(|c| Command::new(c).arg("-v").output().is_ok())
}

/// Rasterises page 1 of `pdf` to grey at [`DPI`] and returns the image.
fn rasterise(tool: &str, pdf: &Path, work: &Path, stem: &str) -> Grey {
    let prefix = work.join(stem);
    let status = Command::new(tool)
        .args(["-r", DPI, "-gray", "-singlefile"])
        .arg(pdf)
        .arg(&prefix)
        .status()
        .expect("run pdftoppm");
    assert!(status.success(), "pdftoppm failed on {}", pdf.display());
    parse_pgm(&std::fs::read(prefix.with_extension("pgm")).unwrap())
}

/// Width and height (mm) of the bounding box of the drawn fills and strokes.
fn drawn_extent(page: &RenderedPage) -> (f64, f64) {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    let mut add = |p: &(f64, f64)| {
        x0 = x0.min(p.0);
        y0 = y0.min(p.1);
        x1 = x1.max(p.0);
        y1 = y1.max(p.1);
    };
    for primitive in &page.primitives {
        match &primitive.shape {
            Shape::Fill { rings } => rings.iter().flatten().for_each(&mut add),
            Shape::Stroke { path, .. } => path.iter().for_each(&mut add),
            _ => {}
        }
    }
    (x1 - x0, y1 - y0)
}

/// The smallest of A4 and A3 on which the whole map fits at map scale, trying the orientation that
/// matches the map's shape first (landscape for a map wider than tall). Returns the file name part
/// and the options for it.
fn paper_for(map: &Path) -> (String, PrintOptions) {
    let doc = read_map_file(map).unwrap();
    let kind = infer_kind(&doc, map).expect("the fixture's kind must be inferable");
    for (paper, name) in [(Paper::A4, "a4"), (Paper::A3, "a3")] {
        let mut options = PrintOptions {
            print_scale: None,
            kind,
            paper,
            orientation: Orientation::Auto,
            centre: None,
            show_sheet_objects: false,
        };
        let (page, _) = render_page(&doc, &options).unwrap();
        let (w, h) = drawn_extent(&page);
        let order = if w >= h {
            [Orientation::Landscape, Orientation::Portrait]
        } else {
            [Orientation::Portrait, Orientation::Landscape]
        };
        for orientation in order {
            options.orientation = orientation;
            let (_, report) = print_map(&doc, &options, "t").unwrap();
            if report.overflow_mm.is_none() {
                let label = if orientation == Orientation::Landscape {
                    "landscape"
                } else {
                    "portrait"
                };
                return (
                    format!("{}-{name}-{label}", doc.georef.scale_denominator),
                    options,
                );
            }
        }
    }
    panic!("{} fits on neither A4 nor A3 at map scale", map.display());
}

#[test]
#[ignore = "needs hand-made OOM exports (tests/fixtures/oom/README.md) and pdftoppm"]
fn matches_oom_within_tolerance() {
    let tool = find_pdftoppm();
    let out_dir = root().join("target/oom-comparison");
    std::fs::create_dir_all(&out_dir).unwrap();
    let work = tempfile::tempdir().unwrap();
    let mut failures = Vec::new();

    for fixture in FIXTURES {
        let map = root().join(format!("tests/fixtures/maps/{fixture}.omap"));
        let (suffix, options) = paper_for(&map);
        let name = format!("{fixture}-{suffix}");
        let reference = root().join(format!("tests/fixtures/oom/{name}.pdf"));
        if !reference.exists() {
            println!(
                "SKIP {name}: tests/fixtures/oom/{name}.pdf is missing; export it from OOM as described in tests/fixtures/oom/README.md"
            );
            continue;
        }
        let Some(tool) = tool else {
            println!("SKIP {name}: pdftoppm (poppler) is not installed, nothing to compare");
            continue;
        };

        let doc = read_map_file(&map).unwrap();
        let (pdf, _) = print_map(&doc, &options, fixture).unwrap();
        let ours_pdf = work.path().join(format!("{name}-ours.pdf"));
        std::fs::write(&ours_pdf, pdf).unwrap();

        let ours = crop_to_content(&rasterise(tool, &ours_pdf, work.path(), "ours"));
        let theirs = crop_to_content(&rasterise(tool, &reference, work.path(), "theirs"));
        println!(
            "{name}: ours {}x{} px, OOM {}x{} px (cropped, {DPI} dpi)",
            ours.width, ours.height, theirs.width, theirs.height
        );
        let size_ok = ours.width.abs_diff(theirs.width) <= SIZE_TOLERANCE_PX
            && ours.height.abs_diff(theirs.height) <= SIZE_TOLERANCE_PX
            && ours.width > 0;
        let (diff, share) = compare(&ours, &theirs);
        println!(
            "{name}: {:.2} % of pixels differ (limit {:.0} %)",
            share * 100.0,
            MAX_DIFFERING_SHARE * 100.0
        );
        if !size_ok || share >= MAX_DIFFERING_SHARE {
            let diff_path = out_dir.join(format!("{name}-diff.pgm"));
            write_pgm(&diff_path, &diff);
            std::fs::copy(&ours_pdf, out_dir.join(format!("{name}-ours.pdf"))).unwrap();
            println!(
                "{name}: difference image written to {}",
                diff_path.display()
            );
            failures.push(format!(
                "{name}: size ok = {size_ok}, {:.2} % differ, see {}",
                share * 100.0,
                diff_path.display()
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn pgm_helpers_work() {
    let img =
        parse_pgm(b"P5\n# comment\n4 3\n255\n\xff\xff\xff\xff\xff\x00\x10\xff\xff\xff\xff\xff");
    assert_eq!((img.width, img.height), (4, 3));
    let c = crop_to_content(&img);
    assert_eq!((c.width, c.height), (2, 1));
    let (_, share) = compare(
        &c,
        &Grey {
            width: 2,
            height: 1,
            data: vec![0, 255],
        },
    );
    assert!((share - 0.5).abs() < 1e-9);
}

/// Where the first vertex of the building that starts at `corner` (map units) lies on the page (mm).
/// Found through the building's own first edge, which is unique enough among the page's fills.
fn corner_on_page(
    doc: &freemapper_print::omap::MapDocument,
    page: &RenderedPage,
    corner: (i32, i32),
) -> (f64, f64) {
    let c = |x: i32| f64::from(x) / 1000.0;
    let coords = doc
        .parts
        .iter()
        .flat_map(|p| &p.objects)
        .map(|o| &o.coords)
        .find(|k| k.len() >= 2 && (k[0].x, k[0].y) == corner)
        .expect("a building starts at this corner");
    let edge = (c(coords[1].x - coords[0].x), c(coords[1].y - coords[0].y));
    let mut found = Vec::new();
    for primitive in &page.primitives {
        if let Shape::Fill { rings } = &primitive.shape {
            for r in rings.iter().filter(|r| (4..=5).contains(&r.len())) {
                let d = (r[1].0 - r[0].0, r[1].1 - r[0].1);
                if (d.0 - edge.0).abs() < 0.002 && (d.1 - edge.1).abs() < 0.002 {
                    found.push(r[0]);
                }
            }
        }
    }
    found.dedup();
    assert_eq!(
        found.len(),
        1,
        "the building is not unique on the page: {found:?}"
    );
    found[0]
}

/// The distances quoted in `tests/fixtures/oom/README.md` (Probedruck) follow from the fixture and
/// the layout, and both corners lie inside the clip rectangle.
#[test]
fn probedruck_distances_match_readme() {
    let map = root().join("tests/fixtures/maps/luebeck.omap");
    let doc = read_map_file(&map).unwrap();
    let readme = std::fs::read_to_string(root().join("tests/fixtures/oom/README.md")).unwrap();
    let kind = infer_kind(&doc, &map).unwrap();
    let cases = [
        (
            Paper::A4,
            Orientation::Auto,
            (-114_203, -65_073),
            (116_555, 3_974),
        ),
        (
            Paper::A3,
            Orientation::Portrait,
            (-80_780, 20_866),
            (-107_548, -71_778),
        ),
    ];
    for (paper, orientation, a, b) in cases {
        let options = PrintOptions {
            print_scale: None,
            kind,
            paper,
            orientation,
            centre: None,
            show_sheet_objects: false,
        };
        let (page, _) = render_page(&doc, &options).unwrap();
        let pa = corner_on_page(&doc, &page, a);
        let pb = corner_on_page(&doc, &page, b);
        for p in [pa, pb] {
            assert!(
                p.0 > page.clip.min.0
                    && p.0 < page.clip.max.0
                    && p.1 > page.clip.min.1
                    && p.1 < page.clip.max.1,
                "{paper:?}: {p:?} lies outside the clip {:?}",
                page.clip
            );
        }
        let mm = (pa.0 - pb.0).hypot(pa.1 - pb.1);
        // Print scale equals map scale, so paper millimetres times the scale are metres.
        let metres = mm * f64::from(doc.georef.scale_denominator) / 1000.0;
        let german = |v: f64| format!("{v:.1}").replace('.', ",");
        let a_mm = f64::from(a.0 - b.0).hypot(f64::from(a.1 - b.1)) / 1000.0;
        assert!(
            (mm - a_mm).abs() < 1e-6,
            "page distance {mm} vs map distance {a_mm}"
        );
        for needle in [
            format!("{} mm", german(mm)),
            format!("{} m", german(metres)),
        ] {
            assert!(
                readme.contains(&needle),
                "{paper:?}: README lacks \"{needle}\""
            );
        }
        let pos = |p: (f64, f64)| format!("{} mm | {} mm", german(p.0), german(p.1));
        for p in [pa, pb] {
            assert!(
                readme.contains(&pos(p)),
                "{paper:?}: README lacks page position {}",
                pos(p)
            );
        }
    }
}
