# Changes

[Deutsche Version](CHANGELOG.md) · [User manual](docs/manual/en.md)

All notable changes to freemapper-print are listed here, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- One project file per event, `freemapper-print.toml`, with three commands. `init` creates it for a map: one
  layout at the map's scale on A4 or A3 (the first on which the map fits without notes), a title block, a scale
  check bar and a proposed contour interval to confirm. `check` draws every layout without writing anything,
  lists the notes and exits with code 1 if there are any. `export` writes one PDF per layout, named after the
  layout id, into the folder `out` or into `--out-dir`; `--layout` picks single layouts. A typo in the project
  file is an error with line and column and is never ignored silently.
- Layouts: any number per event, each with paper (A4, A3), orientation, print scale, passe-partout, frame line
  and, if wanted, a map point in the centre of the map frame. A layout can hide map parts and shared sheet
  elements and have elements of its own.
- Sheet elements, each anchored to one of nine positions of the page or the map frame: title block and texts
  with placeholders (`{event}`, `{date}`, `{organiser}`, `{print_scale}`, `{map_scale}`, `{contour_interval}`,
  `{layout}`), logos and a scale check bar. A sheet that shows neither scale nor contour interval gets a note,
  because the IOF requires both.
- A footer on every sheet: the copyright lines of the map (or from `print.copyright`), one per line, and below
  them the credit line "Created with freemapper-print" with the project address. The credit line cannot be
  switched off; an element only moves it. The footer goes below the map frame when all of it fits there,
  otherwise on a white box inside the map, and `check` reports that. At most six copyright lines appear, and
  long ones are cut.
- Logos as PNG, JPEG or SVG, `width_mm` wide. PNG and JPEG keep their transparency. An SVG prints in CMYK and
  stays sharp at any size; gradients print in their first colour, transparency prints opaque, and the program
  reports each simplification. To guard against files that compute for minutes or demand gigabytes of memory,
  the program takes only plain SVGs: shapes, gradients, `use` and simple clip paths. A logo with text, masks,
  filters, patterns, embedded images or style sheets, with ambiguous attributes or an elaborate DTD is left out,
  and the note suggests exporting it as a plain SVG. So is a missing, broken or oversized file (over 10 MB,
  25 megapixels or 20 000 path segments). The rest of the sheet still prints.
- Scale check bar: a bar of round length (5 m to 1000 m, at most 50 mm when printed), labelled like
  "200 m = 50 mm". A ruler on the printout shows whether the printer scaled the page.
- North lines for maps without any: if the map has no symbol 601 objects and the layout has
  `north_lines = "auto"`, the program adds vertical lines at the ground spacing of the map kind (sprint 120 m,
  forest 300 m, schoolyard 37.5 m, still unchecked), drawn with the map's own 601 symbol so they lie under walls
  and fences like drawn ones. Gaps at small features remain the mapper's job in OOM. A map without a north-line
  symbol gets plain black 0.14 mm lines and a note.
- Messages and help in German and English. The language comes from `--lang de|en`, else from
  `FREEMAPPER_LANG` and the system's language settings. Errors about the project file and clap's help headings
  stay English for now. `event.lang` sets the language on the sheet (date, decimal separator, credit line).
- Safe writing: before each PDF the program checks that the size of the PDF plus a 50 MB reserve is free. The
  PDF is written as a `.part` file and gets its name only once it is complete. A full disk leaves no
  half-written PDF, and the message names the space needed and the space free.
- Example `examples/luebeck-competition-layouts`: a city sprint with an A4 sheet at 1:4000 for the older
  classes and an A3 sheet at 1:3000 for the youngest, with title block, two made-up logos, scale check bar and
  footer, built with `init`, `check` and `export` alone.

### Changed

- The Lübeck map in the tests and in the example `luebeck-scales` now comes from freemapper 0.5.0. It has the
  IOF colours of Rev. 4 (for example brown 25/75/100/0 instead of 0/56/100/18) and buildings in black 50 %
  instead of 60 %. Its north lines now lie in the map part "default part"; "peripheral information" holds only
  the information block. The PDFs of `luebeck-scales` are regenerated; sizes and overflow (4.0 mm on A4) are
  unchanged.

## [0.1.0] - 2026-11-08

### Added

- Project skeleton modelled on freemapper: Rust workspace with a library and the `freemapper-print` program
  (only `--version` and `--help` so far), CI for Linux, macOS and Windows, release workflow.
- Design, glossary and architecture decisions.
- `freemapper-print render <map> --out <pdf>` prints an OOM map as an exact-scale PDF. You choose the paper
  (A4, A3), the orientation and the print scale, `--kind` sets the map kind, and `--show-sheet-objects` also
  prints freemapper's information block. North lines belong to the map and are always in the PDF. The program
  says how many millimetres of the map do not fit on the page. It also reports objects it did not draw,
  simplified or cut short, substituted fonts and missing glyphs, and it reminds you to print at actual size.
  Text from the map reaches the terminal with control and format characters escaped, and so do error
  messages.
- The OOM map (`.omap`) is read, with all its parts. Background images (templates) are ignored.
- Area patterns, pointed line ends, dash symbols, text framing and rotated text are drawn as in OOM. Map text
  uses the embedded Arimo font, including umlauts and other Unicode characters.
- Symbol sizes follow the IOF standards. A larger print scale enlarges everything by the same ratio: a sprint
  map at 1:3000 to 133 %, a forest map to 150 % at 1:10000, 200 % at 1:7500 and 300 % at 1:5000. A scale
  smaller than the base scale (sprint 1:5000, forest 1:20000) keeps the symbols at their standard size, and the
  program warns that the print is not standard and meant for training only.
- Example `examples/luebeck-scales`: the same sprint map at 1:4000 on A4 and at 1:3000 on A3, with
  instructions for measuring.
- A test compares the output with the PDF export of OOM.
- The release archives include the licence of the Arimo font as `FONT-LICENSE.txt`.
- Text in a text box wraps at the box width and aligns to the box edges, as in OOM. The paragraph spacing of
  the text symbol applies after each paragraph.
- The overflow measures what would actually print, with line widths, point symbols and texts. The page is
  centred on it too.
- Broken or overloaded map files no longer stall the program: XML nested too deep is an error, and a page has
  a drawing limit. The program reports objects it leaves out or draws only in part because of that, objects of
  unknown type, dropped area patterns and a broken `freemapper.toml`. The limit counts every point a page
  keeps and every outline the PDF writes. The worst crafted file measured, 500 000 short strokes in 134 KB,
  needed about 200 MB of memory.
- A box size or rotation that is not a finite number, and a negative box size, are read errors. Old box texts
  whose coordinates are stored as `<coord>` elements keep their box.
