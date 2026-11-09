# Changes

[Deutsche Version](CHANGELOG.md) · [User manual](docs/manual/en.md)

All notable changes to freemapper-print are listed here, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Print rules as a rule profile in freemapper-courseplan's format (ADR 0031). The embedded profile `iof-print`
  holds the symbol size table per scale, the north-line spacing and the proposed contour interval. For the
  course overprint it adds the course symbol sizes from ISSprOM 2019-2 and ISOM 2017-2, the purple 35/85/0/0
  and the cell size of control descriptions. Every value names its source. The project file can name another
  profile (`rule_profile`) and overlays (`rule_overlays`) under `[print]`; an unknown name is an error with
  line and column. Printed sheets do not change.
- The project file can link courses. `[sources] courses` names the course file (`.ppen` or IOF XML, relative
  to the project file); it is read again at every check and export. `course_y_axis` ("up" or "down") sets the
  file's y axis, and `course_georef_check = false` switches off the check against latitude and longitude.
  `[[assignments]]` gives a course a layout; a course without one takes the first layout. `[[numbers]]` corrects
  the position of a control number per layout or per course. If the course file no longer has a course or
  control that the project file names, the entry stays and `check` reports it; the other courses still load.
- One sheet per course. When the project has a course file, `export` writes one PDF per course, on the layout
  its assignment names, with the course overprint in purple. The file is named after the course; characters a
  file name cannot hold become `-`, and names that differ only in case get ` (2)`. Without `centre_map_mm`
  the map frame centres on the course. `--course <NAME>` (repeatable) exports only those courses.
  `--single-file` writes all courses into one PDF, "<event> - all courses.pdf" on English sheets, one page
  per course on its own paper. A map larger than the frame is expected on a course sheet and only noted as
  Info. `--layout` still exports a layout without courses. Texts can use `{course}`, `{length_km}`,
  `{climb_m}` and `{controls}`. `check` checks every course and reports controls outside the map frame. Projects without a course file
  export as before (ADR 0035).
- The control description box on course sheets. A layout element of kind `control_descriptions` draws the IOF
  description table of the course from a `.ppen` file: the event title, the course with length and climb, the
  start, one row per control with its number, code and symbols, rows for marked routes and map exchanges with
  their length, and the finish. A course's secondary title from Purple Pen gets its own row, a map flip its
  15.6 row, and score courses list their controls by code as Purple Pen does. Symbols and line widths follow
  Purple Pen. A cell is 6 mm by default; `cell_mm`
  sets 4 to 10 mm, and the box keeps that size at every print scale. It hangs on its anchor like any element
  and sits on a white background. Courses from IOF XML have no descriptions, so their sheets show no box and
  `check` says so as Info. `check` warns when the box would reach past the page edge (the sheet then leaves it
  out), when no control of the course has descriptions, and when the course file uses a symbol the
  description standard does not have (its cell stays empty). On every course sheet `check` also warns when a
  sheet element covers a control, the start or the finish (ADR 0037).

### Fixed

- Circle gaps of courses from files whose y axis grows upwards (Purple Pen) are now mirrored along with the
  rest when the course is fitted to the map.

## [0.2.0] - 2026-11-09

### Added

- One project file per event, `freemapper-print.toml`, with three commands. `init` creates it for a map: one
  layout at the map's scale on A4 or A3 (the first on which the map fits without notes), a title block, a scale
  check bar and a proposed contour interval to confirm (2 m for sprint, 5 m for forest maps); it says where the
  map kind came from. `check` draws every layout without writing anything,
  lists the notes and exits with code 1 if there are any. Lines that only inform (added north lines, an intended
  crop) start with "Info:" and do not count. `export` writes one PDF per layout, named after the
  layout id, into the folder `out` or into `--out-dir`; `--layout` picks single layouts. A typo in the project
  file is an error with line and column and is never ignored silently, and so is a value out of range (for
  example a text over 200 pt or a passe-partout of half the paper).
- Layouts: any number per event, each with paper (A4, A3), orientation, print scale, passe-partout, frame line
  and, if wanted, a map point in the centre of the map frame, given as OOM's status bar shows it. With
  `map_frame.crop = true` a layout cuts the map on purpose: the overflow is then a note only and `check` passes.
  A layout can hide map parts and shared sheet elements and have elements of its own.
- Sheet elements, each anchored to one of nine positions of the page or the map frame: title block and texts
  with placeholders (`{event}`, `{date}`, `{organiser}`, `{print_scale}`, `{map_scale}`, `{contour_interval}`,
  `{layout}`), logos and a scale check bar. A sheet that lacks either the scale or the contour interval gets a
  note, because the IOF requires both, and so does a layout without a scale check bar.
- A footer on every sheet: the copyright lines of the map (or from `print.copyright`), one per line, and below
  them the credit line "Created with freemapper-print" with the project address. The credit line cannot be
  switched off: a `credit_line` element moves it only when it is at least 5 pt and lies wholly on the page, and
  the line is drawn over everything else, on a white box. The whole footer lies on a white box too, so a dark
  element under it cannot hide it. The footer goes below the map frame when all of it fits there, otherwise
  inside the map, and `check` reports that and any element it covers, naming what the footer holds. At most six
  copyright lines appear, and a line that is too long is cut and named in a note.
- Logos as PNG, JPEG or SVG, `width_mm` wide. PNG and JPEG keep their transparency. An SVG prints in CMYK and
  stays sharp at any size; gradients print in their first colour, transparency prints opaque, and the program
  reports each simplification. To guard against files that compute for minutes or demand gigabytes of memory,
  the program takes only plain SVGs: shapes, gradients, `use` and simple clip paths. A logo with text, masks,
  filters, patterns, embedded images or style sheets, with ambiguous attributes or an elaborate DTD is left out,
  and the note suggests exporting it as a plain SVG. So is a missing, broken or oversized file (over 10 MB,
  25 megapixels or 20 000 path segments). The rest of the sheet still prints. Each logo file is decoded once per
  `check` or `export` and embedded once per PDF; a sheet holds at most 50 megapixels of logos and 1 000 000
  units of SVG work.
- Scale check bar: a bar of round length (5 m to 1000 m, at most 50 mm when printed), labelled like
  "200 m = 50 mm". A ruler on the printout shows whether the printer scaled the page.
- North lines for maps without any: if the map has no symbol 601 objects and the layout has
  `north_lines = "auto"`, the program adds vertical lines at the ground spacing of the map kind (sprint 120 m,
  forest 300 m, schoolyard 37.5 m, still unchecked), drawn with the map's own 601 symbol so they lie under walls
  and fences like drawn ones. Gaps at small features remain the mapper's job in OOM. A map without a north-line
  symbol gets plain black 0.14 mm lines and a note.
- Messages and help in German and English. The language comes from `--lang de|en`, else from
  `FREEMAPPER_LANG` and the system's language settings. Notes about logos come in the program language too.
  Errors about the project file, details from the system or an image decoder inside a note (such as the name of
  an SVG element) and clap's help headings stay English for now. `event.lang` sets the language on the sheet (date, decimal separator, credit line).
- Safe writing: before each PDF the program checks that the size of the PDF plus a 50 MB reserve is free. The
  PDF is written as a `.part` file and gets its name only once it is complete. A full disk leaves no
  half-written PDF, and the message names the space needed and the space free. Any other error, and a crash
  while writing, leaves no `.part` file behind either.
- Example `examples/luebeck-competition-layouts`: a city sprint with an A4 sheet at 1:4000 for the older
  classes and an A3 sheet at 1:3000 for the youngest, with title block, two made-up logos, scale check bar and
  footer, built with `init`, `check` and `export` alone.

### Changed

- `render` puts the footer on its PDF as well: the copyright lines from the map notes and the credit line
  "Created with freemapper-print", in the program language, on a white box in the bottom-left corner. An
  OSM-based map thus carries the attribution its licence asks for. The map itself does not move; the PDFs of
  `luebeck-scales` are regenerated.
- The Lübeck map in the tests and in the example `luebeck-scales` now comes from freemapper 0.5.0. It has the
  IOF colours of Rev. 4 (for example brown 25/75/100/0 instead of 0/56/100/18) and buildings in black 50 %
  instead of 60 %. Its north lines now lie in the map part "default part"; "peripheral information" holds only
  the information block. The PDFs of `luebeck-scales` are regenerated; sizes and overflow (4.0 mm on A4) are
  unchanged.

### Fixed

- An error no longer repeats the system's message, as in "No such file or directory (os error 2): No such file
  or directory (os error 2)".
- The PDF writer refuses a page with a number that is infinite or not a number instead of writing an invalid
  PDF.

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
