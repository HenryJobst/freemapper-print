# Changes

[Deutsche Version](CHANGELOG.md) · [User manual](docs/manual/en.md)

All notable changes to freemapper-print are listed here, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Fixed

- The scale bar from freemapper v0.9.0 (tag `freemapper:source=scale-bar`) counts as part of the information block and is no longer printed as map content.
- `init` takes the title from the `freemapper.toml` next to the map before it falls back to the file name.

### Changed

- The shared crates now come from freemapper-core v0.5.1 (Purple Pen writer, object ids). Output is unchanged.
- White-out areas from Purple Pen are named `whiteout-<n>` with Purple Pen's own number instead of the place in the file, so deleting or reordering one no longer changes what another means. Check project files that name `whiteout-1` and so on with `check` (ADR 0048).

## [0.5.0] - 2026-11-10

### Added

- Masks for window and corridor orienteering in the project file (ADR 0045). A `[[masks]]` table has an id and
  polygons made of an outline and holes, the windows. The points are map mm with the y axis up, as OOM shows them
  and as `centre_map_mm` takes them. `mask = "…"` in `[[assignments]]` gives a course its mask, and several courses
  may share one. On loading, the program checks every mask and reports an error with file, line and field, such
  as a hole outside its outline or a ring that crosses itself.
- Masks print. On the sheet of a course with a mask the map is missing in the masked area; outside the outline and
  in the windows it shows, and overlapping windows make one window. The course overprint, the description box, the
  sheet elements and the footer stay visible, the legs between the windows included. Without a mask of its own, a
  course gets Purple Pen's white-out areas, each on its own, also where they overlap. A sheet applies at most 1000
  masks; when a course has more, the rest are left out and `check` warns. Sheets without a mask stay the same byte
  for byte.
- `masks window` writes a mask with round windows around the controls of a course (ADR 0047). The radius counts
  in millimetres on paper, 15 mm by default; `--radius-for` gives single controls a radius of their own,
  `--controls` picks controls by code or number, and `--assign` prints the course with it. The start and the finish
  get a window too, unless `--no-start` or `--no-finish` leaves it out. The output and the comment in the project
  file name the start and the finish as such, not by the codes `STA1` and `FIN1`. The mask keeps the windows in map
  mm; after a change of print scale, run the command again.
- `masks corridor` turns a line you give with `--points` into a corridor as wide as `--width` (10 mm on paper by
  default). There is no automatic corridor. The command refuses a bend too sharp for the width, or a line that
  crosses itself, and names the place; for a crossing, the point where the edges meet.
- Both commands append the table to the project file as text, with a comment on how it was made; everything
  before it stays unchanged. They never change a mask that exists. If the file writes its assignments as
  `assignments = [...]`, `--assign` stops with exit code 2 and leaves the file as it was.
- `check` looks at course sheets with a mask (ADR 0046). It warns when a control, the start or the finish lies
  where the mask hides the map, and counts by kind what of the course overprint lies there: marked routes,
  crossing points, forbidden routes, out-of-bounds boundaries, out-of-bounds and dangerous areas, texts. The
  overprint prints all the same. It also warns when a mask hides the whole map frame, and says for information
  when a mask hides nothing in the frame or no course uses it; white-out areas from Purple Pen that hide nothing
  share one line per course. It warns when a sheet element or the footer covers part of a window. When the project
  file names a white-out area by its number (`whiteout-<n>`), `check` says where that area lies, because the number
  follows the order in the course file and can mean another area after a change in Purple Pen. When the check
  reaches its limit, `check` says that it checked only part of the sheet.
- Two examples: `luebeck-training-window` (window orienteering with eight controls) and
  `luebeck-training-corridor` (corridor orienteering along three streets of Lübeck's old town). The manual has a
  chapter 11 on masks.

### Changed

- The note on white-out areas in the course file now starts with "Info:" and no longer counts toward the exit
  code of `check`, because white-outs print.
- The program reads a project file of at most 16 MB and refuses a larger one before reading it. A file within the
  limits for masks needs about 4 MB at most.
- The last line of `check` counts in the right number: "With notes: 1 of 1 course, 0 of 1 layout sheet."

## [0.4.0] - 2026-11-10

### Changed

- Program texts, rule profiles and file writes now come from freemapper-core (`freemapper-text`,
  `freemapper-rules`, `freemapper-fs`; ADR 0042). The printed sheets and all other output stay the same, and the
  example PDFs are byte for byte identical.
- The map reader, the course file readers, the fit of courses to the map and the placement of control numbers
  now come from freemapper-core too (`freemapper-omap`, `freemapper-course`, all crates at v0.3.0; ADR 0043).
  This is an internal change. Users see no difference, and the example PDFs are byte for byte identical.
- When the project file is a symbolic link, `numbers accept` replaces it with a regular file as it writes. It used
  to write through the link into the target file. The file the link pointed to keeps its content, and the project
  file keeps its permissions (ADR 0041).
- `export` and also `init` refuse an `out` folder, or an `--out-dir` inside the project, that is a link leading
  out of the project folder. Nothing is written there, and the message says: "...: a link leads out of the
  project folder, so nothing is written there; replace the link with a real folder, or give the folder it leads
  to with --out-dir". An `--out-dir` to a folder outside the project, also on another drive, still works.
- The partial file while a PDF is written is now named `<name>.pdf.<process id>.<counter>.part` instead of
  `<name>.pdf.part`. Two runs into one folder no longer share a partial file. `export` removes partial files older
  than an hour from the project's `out` folder.
- A missing `--out-dir` is still reported as before (the folder must exist).
- The maps in the tests and in all four examples now come from freemapper 0.8.0 (commit `a2b41b4`). They lie on the
  base area of the sprint map, and parks are yellow. The examples' PDFs and previews are rebuilt and show both.
  freemapper no longer draws paths of compacted gravel as paths of the map, so six controls moved: control 36 of
  the Lübeck course A to a building corner (the course is now 2.5 km instead of 2.6 km), and five controls of the
  Viktoriapark training to building corners and a corner of the open land. freemapper's candidates, among them
  control sites and unreachable passages, lie in a template of their own and never reach the sheet. A new test
  checks this.

### Added

- The base area of newer freemapper sprint maps: one area in the colour "walkable area" in the map part `base`,
  under the whole map. freemapper-print always prints it, cut at the map frame. It does not count for the centre
  of the map frame, the overflow, the added north lines or the placement of control numbers, because it reaches
  beyond the map. The program refuses `hide_parts = ["base"]` with a message in German or English (ADR 0044).
  Print such maps in colour: in greyscale the light brown is hard to tell from paper white and paved areas.

## [0.3.0] - 2026-11-09

### Added

- Print rules as a rule profile in freemapper-courseplan's format (ADR 0031). The embedded profile `iof-print`
  holds the symbol size table per scale, the north-line spacing and the proposed contour interval. For the course
  overprint it adds the course symbol sizes from ISSprOM 2019-2 and ISOM 2017-2, the purple 35/85/0/0 and the cell
  size of control descriptions. Every value names its source. The project file can name another profile
  (`rule_profile`) and overlays (`rule_overlays`) under `[print]`; an unknown name is an error with line and
  column. Printed sheets do not change.
- Courses from Purple Pen and IOF XML 3.0 (ADR 0033). The program reads `.ppen` files (controls, courses, bends and
  gaps of legs, circle gaps per scale, descriptions, out-of-bounds areas, forbidden routes, marked routes, number
  positions, print scales) and IOF XML 3.0 `CourseData` (controls, courses, positions, number positions, length and
  climb). IOF XML has no descriptions, special objects, bends or print scales, so its sheets show controls, legs
  and numbers only. The reader is chosen by the file extension. What the program cannot print yet is left out and
  reported: courses with variations (forks and loops) or alternative controls, and unknown elements. Purple Pen's
  white-out is read and reported but not printed; it waits for the masks (M4).
- A course must fit the map (ADR 0032). A Purple Pen file is read with its y axis up, as Purple Pen writes it; one
  that does not fit that way is an error and never prints mirrored. For IOF XML the program tries both readings of
  the y axis and takes the one that puts all controls on the map. With latitude and longitude in the file and a
  georeferenced UTM map, those decide. If both fit and nothing decides, it assumes "up" and warns until
  `course_y_axis` confirms it. A `course_y_axis = "down"` on a Purple Pen file gets a warning. A map scale in the
  course file must equal the map's. A control more than 20 m from where its latitude and longitude put it stops the
  fit. Each failure is an error that says what to check; nothing is printed from a course that does not fit.
- The course overprint (ADR 0034). The program draws start, controls, legs, finish, out-of-bounds areas, forbidden
  routes, marked routes, crossing points, map exchanges, texts and numbers in the course purple, in two layers. The
  lower purple lies below black, brown and blue 100 %, the upper over the whole map. Both are painted in `Multiply`
  blend mode. Sizes come from the rule profile and take the standard's size at the print scale, the course factor:
  a forest map printed at 1:10000 gets 7.5 mm circles, at 1:15000 5.0 mm, whatever scale the map file is drawn at.
  A forbidden route is a row of crosses, as ISOM 2017-2 draws symbol 711.
- The project file can link courses. `[sources] courses` names the course file (`.ppen` or IOF XML, relative to the
  project file); it is read again at every check and export. `course_y_axis` ("up" or "down") sets the file's y
  axis (a Purple Pen project needs none), and `course_georef_check = false` switches off the check against latitude
  and longitude. `[[assignments]]` gives a course a layout; a course without one takes the first layout.
  `[[numbers]]` corrects the position of a control number per layout or per course. If the course file no longer
  has a course or control that the project file names, the entry stays and `check` reports it; the other courses
  still load.
- One sheet per course. When the project has a course file, `export` writes one PDF per course, on the layout its
  assignment names, with the course overprint in purple. The file is named after the course; characters a file name
  cannot hold become `-`, and names that differ only in case get ` (2)`. Without `centre_map_mm` the map frame
  centres on the course. `--course <NAME>` (repeatable) exports only those courses. `--single-file` writes all
  courses into one PDF, "<event> - all courses.pdf" on English sheets, one page per course on its own paper. A map
  larger than the frame is expected on a course sheet and only noted as Info. `--layout` still exports a layout
  without courses. Texts can use `{course}`, `{length_km}`, `{climb_m}` and `{controls}`. `check` checks every
  course and reports controls outside the map frame, then checks the layout sheets that `export --layout` writes.
  Projects without a course file export as before (ADR 0035).
- The control description box on course sheets. A layout element of kind `control_descriptions` draws the IOF
  description table of the course from a `.ppen` file: the event title, the course with length and climb, the
  start, one row per control with its number, code and symbols, rows for marked routes and map exchanges with their
  length, and the finish. A course's secondary title from Purple Pen gets its own row, a map flip its 15.6 row, and
  score courses list their controls by code as Purple Pen does. Symbols and line widths follow Purple Pen. A cell
  is 6 mm by default; `cell_mm` sets 4 to 10 mm, and the box keeps that size at every print scale. It hangs on its
  anchor like any element and sits on a white background. Courses from IOF XML have no descriptions, so their
  sheets show no box and `check` says so as Info. `check` warns when the box would reach past the page edge (the
  sheet then leaves it out), when no control of the course has descriptions, and when the course file uses a symbol
  the description standard does not have (its cell stays empty). On every course sheet `check` also warns when a
  sheet element covers a control, the start or the finish (ADR 0037).
- Control numbers on course sheets. A number sits where the course file puts it: for Purple Pen at the position per
  course, converted with the course's print scale and item scaling, for IOF XML at `MapTextPosition`. A correction
  in `[[numbers]]` for the layout or the course comes first; its `gap_mm` is above 0 and at most 10 mm. Where the
  file has no position, freemapper-print places the number itself: behind the control, in line with the incoming
  leg, and off circles, legs, other numbers, sheet elements and the map's point and line symbols. `check` checks
  every number. When one lies on the course overprint, on a sheet element, over the edge of the map frame, or on a
  point or line symbol of the map, the warning names what it covers, map symbols by their code, and proposes a
  direction and gap. Without a free place it says so and proposes the least covered one. The number still prints in
  its place; nothing moves on its own. The rule profile's `number_gap` rule sets a gap of 0.5 to 5 mm, rounded to
  0.01 mm as in freemapper-courseplan, and `check` warns about gaps outside it. The search for places has a budget
  per sheet; a sheet that reaches it says so, and its remaining numbers stay unsearched. When the course file has
  changed a position that a correction with `base_seen` overrides, `check` reports it. Scoring and search live in a
  module of their own, which freemapper-courseplan will take over (ADR 0038).
- `init --courses <FILE>` builds a project from a course file. It reads and fits the file first, so a file that
  does not fit the map stops `init` before it writes anything. A `.ppen` gets one layout per print scale in the
  file, an assignment for every course, a control description box in the top-right corner of the map frame and
  `map_frame.crop = true`; a course without a print scale takes the map scale. IOF XML gets one layout at the map
  scale and no description box. Every layout id names its print scale, such as `a4-4000`. The title block starts
  with `{event} · {course}`, so every course sheet names its course, including one from IOF XML without a
  description box. `init` says where each value came from.
- `numbers accept <PROJECT> --course <NAME> --control <CODE> (--layout-wide | --sheet)` writes a number correction
  into the project file. Without `--direction` and `--gap` it takes the proposal that `check` shows. It appends a
  `[[numbers]]` table and leaves everything before it unchanged, comments included, then loads the file again; if
  that fails, it restores the old content. A link to the project file stays a link, the file keeps its permissions
  and line endings. A correction for the same control and scope is refused. `--layout-wide` takes the proposal only
  when it is free on every course of that layout that prints the number (ADR 0025, ADR 0038).
- Example `examples/luebeck-competition-courses`: the Lübeck sprint with three courses from a hand-written Purple
  Pen file, A and B on A4 at 1:4000 and C on A3 at 1:3000, with control descriptions, an out-of-bounds area, a
  forbidden crossing, a marked route to the finish and one number moved with `numbers accept`. The folder `iof/`
  prints the same courses from IOF XML 3.0, without descriptions. The start stands on the street west of the old
  town.
- Example `examples/berlin-viktoriapark-training`: a club training of 1.7 km on freemapper's Viktoriapark map
  (version 0.6.0), one course from Purple Pen on A4 at 1:4000 with control descriptions.
- Manual chapter 8 "Courses" and the course notes and errors in chapter 10, in German and English. The READMEs list
  what M3 adds.

### Fixed

- Generated north lines without a north-line symbol in the map (the plain 0.14 mm black lines) took the symbol
  factor, so on a map drawn at an enlarged scale they printed too thin. They now take the standard's size at the
  print scale, as the course symbols do (ADR 0018).

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
- Messages and help in German and English. The language comes from `--lang de|en`, else from `FREEMAPPER_LANG`
  and the system's language settings. Notes about logos come in the program language too. Errors about the
  project file, details from the system or an image decoder inside a note (such as the name of an SVG element)
  and clap's help headings stay English for now. `event.lang` sets the language on the sheet (date, decimal
  separator, credit line).
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
