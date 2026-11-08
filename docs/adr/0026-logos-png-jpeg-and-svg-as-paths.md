# 0026. Logos: PNG and JPEG as images, SVG as paths from an allow-list

Date: 2026-11-08
Status: Accepted

## Context

Clubs put their own logo and their sponsors' logos on the sheet. Most have them as PNG or JPEG, some as SVG
from the designer. The sheet layer draws fills, strokes, text and raster images in CMYK (`SheetItem`). The
core stays pure Rust (ADR 0001). The design listed PDF logos too.

`usvg` copies subtrees while it resolves an SVG: every `use`, every marker, patterns per path, masks and clip
paths in bounding-box units per user, `feImage` targets. The first version of this decision counted `use`
and markers before calling `usvg`. The review of Task 8 broke that count twice. A `use` whose `href` had a
space before the `#`, or that carried both `href` and `xlink:href`, was resolved differently from `usvg` (2.4
GB in 20 s). A 4 KB file of masks nested seven deep took 3 GB, because masks were not counted at all.
Counting every copy `usvg` makes means following its internals release by release.

## Decision

- PNG and JPEG are decoded with `image` (features `png` and `jpeg` only) into a `RasterImage`. An image with
  an alpha channel keeps it.
- An SVG logo must pass an allow-list on its XML before `usvg` sees it (`sheet::logo::svg_check`). Anything
  else leaves the logo out with `SheetNote::LogoSkipped` and the advice to export it as a plain SVG.
  - Elements: `svg`, `g`, `path`, `rect`, `circle`, `ellipse`, `line`, `polyline`, `polygon`, `defs`,
    `linearGradient`, `radialGradient`, `stop`, `title`, `desc`, `metadata`, `use`, `symbol` (only through
    `use`) and `clipPath`. A `style` element in any namespace is refused, because `usvg` reads it as a
    style sheet. Elements of other namespaces (Inkscape's `sodipodi:namedview`, RDF in `metadata`) are
    passed over, as `usvg` skips them with their content.
  - `filter`, `mask` and the marker properties must be `none`. `fill` and `stroke` may point to a gradient,
    `clip-path` to a clip path; `url()` in any other property is refused. A gradient may take its stops
    from another gradient, at most 16 links deep.
  - A clip path must be in user space (`clipPathUnits="userSpaceOnUse"`, the default). Its children are
    shapes or `use` of a shape, and nothing inside it, or used by it, has a clip path of its own.
  - Every reference resolves as in `usvg`: for `use` and gradient links the unprefixed `href` first, then
    `xlink:href`, parsed by `svgtypes::IRI` (spaces around `#id` allowed); `url()` by `svgtypes::FuncIRI` or
    `svgtypes::Paint`; `style` attributes split by `simplecss`. Both crates are the versions `usvg` uses. An
    id that occurs twice is refused, because `usvg` resolves it differently for `use` and for other links.
  - What `usvg` still copies is counted: one unit per element plus one per two bytes of path data, again for
    every `use` and for every element a clip path clips, and gradient stops once per shape (ADR 0019 has the
    limits).
- `usvg` runs without its default features: no text, no fonts, no svgz, no writer. Embedded images are never
  loaded and external files are never read. Each path becomes a fill and a stroke in the SVG's paint order,
  at its absolute transform, with curves flattened by the map renderer's `flatten_cubic`. Strokes keep their
  width, scaled into mm.
- `Shape::Fill` has a fill rule (`FillRule`). Map areas fill even-odd as in OOM, and so do the other sheet
  elements; an SVG fill takes `nonzero` or `evenodd` from the SVG. The PDF writes `f` or `f*`. Clip outlines
  stay even-odd.
- Colours are converted from RGB to CMYK the naive way: k = 1 − max(r, g, b), c = (1 − r − k) / (1 − k),
  m and y alike, and no colour when k = 1. There is no colour management.
- The sheet cannot draw everything the allow-list lets through. Each of the following gives one
  `SheetNote::LogoSimplified` per logo: a gradient prints in its first stop colour, opacity below 1 prints
  opaque, dashes print solid, and clip paths have no effect.
- Shapes that reach outside the SVG's picture are clipped to the logo's box, as an SVG viewer clips them.
- The logo is `width_mm` wide; the height follows the aspect ratio of the pixels or of the SVG size.
- A logo file that is missing, broken, of another format or over a limit (ADR 0019) is left out with
  `SheetNote::LogoSkipped` naming the file and the reason. The sheet still renders.
- PDF logos are not read.

## Consequences

- `image` and `usvg` add 33 crates to `Cargo.lock`, all pure Rust. The lock file holds `crc32fast`, `flate2`,
  `log` and `svgtypes` at their previous releases until the newest ones are two weeks old (ADR 0006). `png`
  and `flate2` bring `miniz_oxide` 0.8, so the lock has it twice: 0.8.9 for them, 0.9.1 for the PDF writer.
- A logo with masks, patterns, filters, markers, embedded images or text is refused even where `usvg` could
  have drawn it safely. The note tells the club to export a plain SVG; most editors have such an export, and
  text has to be converted to paths anyway.
- The checks follow `usvg`'s reference rules but none of its copying rules, so a new `usvg` release needs a
  look at its reference parsing only.
- Logos print in the colours a naive conversion gives. Bright RGB blues and greens come out duller than on
  screen; a club that needs exact colours supplies a logo in the colours it wants.
- `Shape::Stroke` has no miter limit, so an SVG's `stroke-miterlimit` (default 4) is not carried over and the
  PDF default of 10 applies: sharp corners of mitred strokes can stick out further than in a browser.
- Cost if wrong: a club with only a PDF logo has to export it as SVG or PNG first.

Source: M2 plan Task 8, controller rulings 2026-11-08, Task 8 review and fix round 1 (2026-11-09).
