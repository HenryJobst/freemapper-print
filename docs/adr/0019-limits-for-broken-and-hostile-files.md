# 0019. Limits for broken and hostile files

Date: 2026-11-08
Status: Accepted

## Context

The CLI and the planned MCP server read map files from other people. Reviews found `.omap` inputs that hung
the renderer, used 1 GB for 20 objects or overflowed the stack. The final review of M1 found more: a combined
symbol that fills one large area 1000 times (a 181 KB file peaked at 906 MB, a 507 KB file with three such
objects at 2.7 GB), a pattern inside a pattern whose outer outline was copied for every line (a 241 KB file
peaked at 1.6 GB), and an area whose clipped and unclipped patterns alternate, so the PDF writes the outline
once per clipped pattern (a 507 KB file with 2000 patterns peaked at 718 MB with a 136 MB PDF while the
outline was charged once per colour). Real maps stay far below every limit: Lübeck (2173 objects) draws
about 4400 primitives, the forest sample (539 objects) about 1500.

## Decision

- XML nested deeper than 256 levels is an error in every reader (`MAX_XML_DEPTH`), and so is a bad number.
  A float attribute of an object, the georeferencing or a pattern must be finite, and a box size must not be
  negative.
- Area patterns with a spacing or point distance below 0.05 mm or a non-finite number are dropped when the
  symbols are read (`MIN_PATTERN_STEP`).
- Each object draws within 250 000 units of work (`MAX_UNITS_PER_OBJECT`). A primitive costs one unit plus
  one per 8 points it keeps (`KEPT_POINTS_PER_UNIT`). A clip outline costs one unit per 8 points, rounded up,
  for every clip block the PDF writes: once per pattern and colour. A pattern's shapes form one run per
  colour after the page is sorted by colour, and the shapes of an unclipped pattern on the same outline can
  split the runs of two clipped ones. A step that draws nothing costs one unit, and copying or testing
  coordinates costs one per 64 points. A pattern starts only while units are left.
- Symbol nesting stops at depth 8 (`MAX_DEPTH`). A part with over 100 000 dashes is drawn solid, and one with
  as many mid symbols drops them (`MAX_REPEATS`).
- A page has 500 000 units (`MAX_UNITS_PER_PAGE`). Each object takes the units it used from the page, and
  the objects after the page runs out are not drawn. Clipped shapes keep sharing their outlines when the
  page is moved into place, and the PDF writes each outline once per run, also inside another clip.
- Logo files (ADR 0026) are read only if they are regular files of at most 10 MB. A PNG or JPEG may have at most
  25 megapixels and at most 65 535 pixels a side, checked on the header before any pixel is decoded, and the
  decoder may allocate at most 116 MB.
  An SVG is checked on its XML before `usvg` resolves it: at most 200 000 nodes, entity references that
  cannot expand beyond about 50 MB, only the elements and references of the allow-list in ADR 0026, nesting
  at most 256 levels deep with `use` resolved, and at most 1 000 000 units of work: one per element plus one
  per two bytes of path data, counted again for every `use` and for every element a clip path clips, and
  shapes times the largest number of gradient stops. Then at most 20 000 path segments and 500 000 points
  after flattening. A logo that breaks a limit is left out whole with `SheetNote::LogoSkipped`; none is drawn
  in part. An earlier count of `use` and markers alone missed copies `usvg` makes of masks, patterns and clip
  paths and resolved some `href` forms differently; the allow-list replaced it.
- The report counts what the limits changed: objects cut short (`truncated_objects`), objects not drawn for
  the page limit, objects whose symbol lost a pattern, objects of unknown type. The CLI prints a note for
  each, and map text reaches the terminal with control and format characters escaped.

## Consequences

- A page keeps at most four million points (500 000 units of at most eight points each), and every clip
  block the PDF writes counts among them. How many bytes that takes is measured, not derived. Peak memory on
  a release build on macOS:
  - 200 MB on a 134 KB file of 500 000 strokes of seven points, the most of all files tried;
  - 164 MB on the 507 KB file with three combined objects, 84 MB on the 181 KB file with one;
  - 77 MB on the 507 KB file with 2000 alternating patterns, which now writes a 14 MB PDF, and 145 MB on
    a 1.0 MB file with four such objects (28 MB PDF);
  - 6 MB on the nested patterns.
- Lübeck uses about 6500 units at every allowed scale and the forest sample at most about 6400, about 1.3 %
  of the page.
- Cost if wrong: an extreme but real map loses detail. The notes name the count, and the constants can grow.

Source: security reviews of M1 Tasks 4, 5 and 8, final review of M1 and its security review, controller
rulings 2026-11-08, M2 Task 8 (logos).
