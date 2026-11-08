# 0026. Logos: PNG and JPEG as images, SVG as paths

Date: 2026-11-08
Status: Accepted

## Context

Clubs put their own logo and their sponsors' logos on the sheet. Most have them as PNG or JPEG, some as SVG
from the designer. The sheet layer draws fills, strokes, text and raster images in CMYK (`SheetItem`). The
core stays pure Rust (ADR 0001). The design listed PDF logos too.

## Decision

- PNG and JPEG are decoded with `image` (features `png` and `jpeg` only) into a `RasterImage`. An image with
  an alpha channel keeps it.
- SVG is parsed with `usvg` without its default features: no text, no fonts, no svgz, no writer. Embedded
  images are never loaded and external files are never read. Each path becomes a fill and a stroke in the
  SVG's paint order, at its absolute transform, with curves flattened by the map renderer's
  `flatten_cubic`. Strokes keep their width, scaled into mm.
- Colours are converted from RGB to CMYK the naive way: k = 1 − max(r, g, b), c = (1 − r − k) / (1 − k),
  m and y alike, and no colour when k = 1. There is no colour management.
- The sheet cannot draw everything an SVG can. Each of the following gives one `SheetNote::LogoSimplified`
  per logo: a gradient prints in its first stop colour (black if it has none), a pattern prints black,
  opacity below 1 prints opaque, dashes print solid, embedded images and text are left out, clip paths,
  masks and filters have no effect.
- Fills use the even-odd rule, because `Shape::Fill` has no other. An SVG fill with the nonzero rule prints
  differently only where subpaths of the same direction overlap.
- Shapes that reach outside the SVG's picture are clipped to the logo's box, as an SVG viewer clips them.
- The logo is `width_mm` wide; the height follows the aspect ratio of the pixels or of the SVG size.
- A logo file that is missing, broken, of another format or over a limit (ADR 0019) is left out with
  `SheetNote::LogoSkipped` naming the file and the reason. The sheet still renders.
- PDF logos are not read.

## Consequences

- `image` and `usvg` add 33 crates to `Cargo.lock`, all pure Rust. The lock file holds `crc32fast`, `flate2`,
  `log` and `svgtypes` at their previous releases until the newest ones are two weeks old (ADR 0006).
- Logos print in the colours a naive conversion gives. Bright RGB blues and greens come out duller than on
  screen; a club that needs exact colours supplies a logo in the colours it wants.
- A logo that relies on transparency, gradients or text prints differently, and the notes say so. Text in
  an SVG has to be converted to paths by the designer.
- Cost if wrong: a club with only a PDF logo has to export it as SVG or PNG first.

Source: M2 plan Task 8, controller rulings 2026-11-08.
