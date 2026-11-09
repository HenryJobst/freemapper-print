# freemapper-print user manual

[Deutsch](de.md) · [README](../../README.en.md)

This manual grows with the program. Chapters 5 to 10 are written. In the end it should lead step by
step from the finished OOM map and the Purple Pen file to the printed sheets, without needing experience with
the command line.

Chapters:

1. What freemapper-print does
2. Installation
3. With an AI assistant
4. Without an assistant: editor and command line
5. [Setting up an event](#5-setting-up-an-event)
6. [Layouts: paper size, scale, map frame](#6-layouts-paper-size-scale-map-frame)
7. [Title, logos and other sheet elements](#7-title-logos-and-other-sheet-elements)
8. [Courses](#8-courses)
9. [Printing at the right scale at home and in a copy shop](#9-printing-at-the-right-scale-at-home-and-in-a-copy-shop)
10. [Messages and what to do](#10-messages-and-what-to-do)
11. Masks for window and corridor orienteering (planned, M4)

The program speaks German or English. It takes the language from `--lang de` or `--lang en`, else from the
environment variables `FREEMAPPER_LANG`, `LC_ALL`, `LC_MESSAGES` and `LANG`. The messages quoted in this manual
are the English ones.

## 5. Setting up an event

Each event has a folder with a project file, `freemapper-print.toml`. It names the event, the map, the layouts
and the sheet elements. Without a course file the program prints the map with title, logos and scale check bar.
With one, it prints a sheet for every course, with the course overprint (chapter 8).

The work takes three commands:

1. `init` creates the project file.
2. You edit the file in a text editor and run `check` until the sheets look as they should.
3. `export` writes one PDF per layout, or one per course when the project has a course file.

The example [examples/luebeck-competition-layouts](../../examples/luebeck-competition-layouts/README.en.md)
shows the whole way with every command and its output.

### Creating the project file: `init`

```sh
freemapper-print init citysprint --map maps/luebeck.omap --title "Stadtsprint Lübeck"
```

This creates the folder `citysprint` if it does not exist yet, and `freemapper-print.toml` in it. `init` never
overwrites an existing project file. The file stores the path to the map relative to itself, so you can move the
folder as long as the map moves with it.

The new file holds:

- the title from `--title`, or the file name of the map without the option;
- the map kind. It comes from `--kind`, else from the `freemapper.toml` next to the map, else from the symbol
  set and the scale of the map, and `init` says which. If none of these gives one, the program stops and asks
  for `--kind sprint`, `--kind schoolyard` or `--kind forest`;
- a proposed contour interval: 2 m for sprint maps, 5 m for forest maps, none for schoolyard maps. A comment
  in the file marks the value. Look up the map's contour interval and correct it;
- a title block with `{event}` and `1:{print_scale} · {contour_interval}` at the top left of the map frame and a
  scale check bar at the bottom right;
- one layout at the scale of the map. `init` tries A4 with an 8 mm passe-partout, then A4 with a wider one (in
  2 mm steps up to 24 mm), then A3, and takes the first sheet `check` has nothing to report about. For the
  Lübeck sprint map that is A3 with 14 mm. If no sheet fits, the file keeps A4 with 8 mm, and `init` starts its
  message with "Warning:" and prints what `check` will report.

`init` sets the sheet language, used for the date, the decimal separator and the credit line on the sheet, to
the program language. It is stored in `event.lang`.

### The project file

The file is [TOML](https://toml.io/en/). Each line has the form `key = value`, and sections stand in square
brackets. A key the program does not know is an error that names file, line and column, so a typo is never
ignored silently. A value out of range is an error with the line and column of the value. Paths are relative
to the project file.

```toml
format = 1

[event]
title = "Stadtsprint Lübeck"
date = 2027-05-15                  # optional, for {date}
organiser = "OL-Verein Musterstadt" # optional, for {organiser}
lang = "de"                        # sheet language: de or en, default de

[sources]
map = "luebeck.omap"

[map]
kind = "sprint"                    # sprint, schoolyard or forest
contour_interval_m = 2.0           # contour interval in metres, for {contour_interval}

[print]
copyright = """
© OpenStreetMap contributors, ODbL
© GeoBasis-DE/LVermGeo SH/CC BY 4.0 (Quelle verändert)
"""                                # optional, replaces the sources from the map

[[shared]]                         # shared sheet elements, chapter 7
id = "title"
kind = "title_block"
lines = ["{event}", "1:{print_scale} · {contour_interval}"]
anchor = { to = "map_frame", corner = "top_left", offset_mm = [2.0, 2.0] }

[[layouts]]                        # one or more layouts, chapter 6
id = "a4-4000"
paper = "A4"
print_scale = 4000
```

| Section | Key | Meaning |
|---|---|---|
| top | `format` | always `1`. The program refuses any other number and names the version it knows. |
| `[event]` | `title` | name of the event, for `{event}` |
| | `date` | date as `YYYY-MM-DD` without quotes, for `{date}` |
| | `organiser` | organiser, for `{organiser}` |
| | `lang` | sheet language `de` or `en`: "15.05.2027" or "15 May 2027", "2,5 m" or "2.5 m", "Erstellt mit" or "Created with" |
| `[sources]` | `map` | path to the OOM map (`.omap`) |
| `[map]` | `kind` | map kind: `sprint`, `schoolyard` or `forest`. It decides how the symbols grow at another print scale (chapter 9) and how far apart north lines lie. |
| | `contour_interval_m` | contour interval in metres, larger than 0 and at most 100 |
| `[print]` | `copyright` | copyright lines, one per line. Without the key the program takes the map notes. |
| | `rule_profile` | rule profile with the print rules, default `iof-print`, the only one so far. `crates/freemapper-print/rules/README.md` describes the profiles. |
| | `rule_overlays` | overlays on the rule profile, as a list in the order they apply. Default: none. |
| `[[shared]]` | | one shared sheet element; the section may appear many times |
| `[[layouts]]` | | one layout; at least one, any number |

### Checking: `check`

```sh
freemapper-print check citysprint
```

`check` takes the project file or its folder. It draws every layout in memory, writes no file, and prints one
line per layout with paper, orientation, scale and factors, followed by the notes. It ends with "Checked 1
layout, no notes." or "Layouts with notes: 1 of 2."

The exit code is 0 when no layout has anything to report, and 1 when one has a note or cannot be drawn. Lines that
start with "Info:", for added north lines and an intended crop (`map_frame.crop`), do not count. A script can
therefore run `check` before `export`. A note never stops the export. Some notes are expected, such as the overflow
of a map larger than the sheet. Chapter 10 explains each note.

### Writing the PDFs: `export`

```sh
freemapper-print export citysprint
freemapper-print export citysprint --layout a4-4000
freemapper-print export citysprint --out-dir ~/Desktop/print
```

`export` writes one PDF per layout and names it after the layout id, for example `a4-4000.pdf`. Without
`--out-dir` the PDFs go into the folder `out` next to the project file, which the program creates when needed.
A folder given with `--out-dir` must exist. `--layout` exports only that layout and can be repeated. A PDF of the
same name is overwritten.

For each layout `export` prints the same lines as `check`, with the path of the PDF in the first one. The
layouts are drawn one after another, so only one sheet is in memory at a time. The exit code is 0 once all PDFs
are written, also when there were notes.

Before writing, the program checks the free space: it needs the size of the PDF plus a 50 MB reserve. The PDF is
first written as a partial file (`<name>.pdf.….part`) and gets its name only when it is complete on the disk. On a
full disk an older PDF of the same name stays as it was, and no half-written PDF is left behind; the same holds
for any other error. The message says how much space is needed and how much is free, or that the disk does not say.
The partial file is named `<name>.pdf.<process id>.<counter>.part`, so two exports into the same folder do not
share it. `export` removes partial files older than an hour from the folder `out` of the project.

If `out`, or a folder inside the project that you give with `--out-dir`, is a link that leads out of the project
folder, the program writes nothing there and says "a link leads out of the project folder". Replace the link with
a real folder. An `--out-dir` outside the project, also on another drive, works.

## 6. Layouts: paper size, scale, map frame

A layout is a template for a map sheet. An event usually has one to five, for example A4 at 1:4000 for the
older classes and A3 at 1:3000 for the youngest. Each layout becomes one PDF.

```toml
[[layouts]]
id = "a3-3000"
paper = "A3"
orientation = "landscape"
print_scale = 3000
north_lines = "auto"
hide_parts = []
hide_shared = []
map_frame = { margin_mm = 10.0, border_mm = 0.25 }
```

| Key | Default | Meaning |
|---|---|---|
| `id` | | name of the layout and of its PDF (`a3-3000.pdf`), required |
| `paper` | `"A4"` | `"A4"` or `"A3"` |
| `orientation` | `"auto"` | `"portrait"`, `"landscape"` or `"auto"`: the orientation in which less of the map is cut off |
| `print_scale` | scale of the map | denominator of the print scale, `3000` for 1:3000 |
| `north_lines` | `"auto"` | `"auto"` adds north lines when the map has none; `"never"` never does |
| `hide_parts` | empty | map parts the sheet leaves out, by their exact name in OOM |
| `hide_shared` | empty | ids of shared elements this layout does not show |
| `elements` | empty | this layout's own sheet elements, chapter 7 |
| `map_frame.margin_mm` | 8 | width of the passe-partout in mm, the same on every side; less than half the shorter paper side |
| `map_frame.border_mm` | 0.25 | width of the frame line in mm, at most 20; 0 draws none |
| `map_frame.centre_map_mm` | middle of the map | map point `[x, y]` in the centre of the map frame, as OOM shows it |
| `map_frame.crop` | `false` | `true`: the frame cuts the map on purpose; the overflow is a note only |

The layout id becomes a file name. It must therefore not contain any of `/ \ : * ? " < > |`, must not start or
end with a dot, and must not be a Windows device name (`con`, `nul`, `com1` …). The program refuses two ids that
differ only in upper and lower case, because their PDFs would be one file on macOS and Windows.

### Map frame and passe-partout

The map frame is the sheet without the passe-partout. The program cuts the map to it and never shrinks it,
because that would falsify the scale. If the map reaches beyond it, `check` says by how many millimetres
(chapter 9). The frame line is centred on the edge of the map frame: one half covers the cut edge of the map,
the other lies on the passe-partout.

Without `centre_map_mm`, the middle of what is printed lands in the middle of the frame. With
`centre_map_mm = [x, y]` that map point lands there. The numbers are millimetres on the map at map scale, exactly
as OOM shows them in its status bar when the coordinate display is set to map coordinates: point at the spot in
OOM and copy both numbers. OOM shows y pointing up (its source prints `x` and `-y` in
`MapWidget::updateCursorposLabel`); the program converts that to the y-down coordinates of the `.omap` file
itself. Each number may be at most 2 000 000 mm from the map origin. For a map larger than the sheet, this is
how you put the frame on the competition area.

If the sheet is meant to show only part of the map, set `crop = true` as well
(`map_frame = { centre_map_mm = [120.0, -80.0], crop = true }`). The overflow is then a note for information,
and `check` still ends with 0.

How wide the passe-partout must be depends mostly on the footer (chapter 7): it goes below the map frame only
when all of it fits there. Two lines fit into 8 mm, five need about 14 mm.

### Print scale

Without `print_scale` the layout prints at the scale of the map. Another scale changes the symbol sizes as the
standard says (chapter 9). A sprint map prints at 1:3000, 1:4000 and 1:5000, a forest map at 1:5000 to 1:20000.
Sheet elements keep their size in millimetres: a 14 pt title is as large on the A3 sheet at 1:3000 as on the A4
sheet at 1:4000.

### North lines

North lines (symbol 601) belong to the map. If the map has them, the program prints them as drawn, with their
gaps at small features. If it has none and `north_lines` is `"auto"`, the program adds vertical lines across the
whole map frame at the ground spacing of the map kind: 120 m for sprint maps (30 mm at 1:4000, 40 mm at
1:3000), 300 m for forest maps, 37.5 m for schoolyard maps. They use the map's own 601 symbol and so lie under
walls and fences like drawn ones. They still run without gaps; the mapper sets those in OOM. A layout that hides
the map part holding the map's own north lines with `hide_parts` gets no added ones.

### Hiding map parts

`hide_parts = ["peripheral information"]` leaves out the map part of that name. Upper and lower case count. A
name the map does not have gives a note. freemapper's information block (title, scale, scale bar) never prints
on a layout's sheet; the sheet elements take its place.

### The base area of sprint maps

Newer sprint maps from freemapper lie on a base area: one area in the colour "walkable area", a light brown like
that of paved areas, in the map part `base`. It belongs to the map and always prints, cut at the map frame. It
cannot be switched off, and the program refuses `hide_parts = ["base"]` with an error. As it reaches a little beyond
the map, it does not count for the centre of the map frame, the overflow, the added north lines or the placement
of control numbers. Print such maps in colour: in greyscale the light brown is hard to tell from paper white and
from paved areas. Schoolyard maps have no base area.

## 7. Title, logos and other sheet elements

Sheet elements stand on the sheet but are not part of the map. Shared elements go under `[[shared]]` and appear
on every sheet. A layout hides single ones with `hide_shared` and has its own under `[[layouts.elements]]`. The
ids of the shared elements and of one layout's elements must be unique together. `footer` is taken.

Every element has:

| Key | Meaning |
|---|---|
| `id` | the name notes use for the element |
| `kind` | `title_block`, `text`, `logo`, `scale_check_bar`, `copyright_line`, `credit_line` or `control_descriptions` (chapter 8) |
| `anchor.to` | `"page"` (the sheet) or `"map_frame"` |
| `anchor.corner` | one of nine positions: `top_left`, `top`, `top_right`, `left`, `centre`, `right`, `bottom_left`, `bottom`, `bottom_right` |
| `anchor.offset_mm` | `[x, y]`, distance in mm from that position inward |

Each offset lies between -2000 and 2000 mm. From a left or top edge the offset counts right or down, from a right
or bottom edge left or up, from a middle right or down. The element sits on the inner side of its position:
`top_right` with `[2, 2]` puts its top-right corner 2 mm left of and 2 mm below the top-right corner.

An element hung on the map frame lies on the map and covers it, as intended. An element hung on the page that
reaches into the map frame gets a note, and so does an element that reaches past the page edge.

Other keys apply only to some kinds. A key on the wrong kind is an error.

### Title block and text

```toml
[[shared]]
id = "title"
kind = "title_block"
lines = ["{event}", "{date} · {organiser}", "1:{print_scale} · {contour_interval}"]
anchor = { to = "map_frame", corner = "top_left", offset_mm = [2.0, 2.0] }
```

A title block sets its first line in 14 pt bold (`size_pt` changes that, up to 200) and the others in 9 pt. A
`text` sets all lines in 9 pt or in `size_pt`. Both stand on a white box with a 1 mm margin; `box_white = false`
leaves it out. The font is Arimo, the same as for map texts.

The program fills in these placeholders:

| Placeholder | becomes |
|---|---|
| `{event}` | `event.title` |
| `{date}` | `event.date`, "15.05.2027" on German sheets, "15 May 2027" on English ones |
| `{organiser}` | `event.organiser` |
| `{print_scale}` | the layout's print scale, from five digits on with a thin space: "4000", "10 000" |
| `{map_scale}` | the scale of the map |
| `{contour_interval}` | `map.contour_interval_m` with its unit: "2 m", "2.5 m" |
| `{layout}` | the layout id |
| `{course}` | on a course sheet, the name of the course |
| `{length_km}` | on a course sheet, the length of the course in kilometres, without unit |
| `{climb_m}` | on a course sheet, the climb in metres, without unit |
| `{controls}` | on a course sheet, the number of controls |

An unknown placeholder, such as the typo `{evnet}`, stays as written and gives a note. The four course
placeholders are known everywhere, but only course sheets fill them (chapter 8). On a layout sheet they stay empty,
and `check` says so as Info.

The IOF requires the scale and the contour interval on every sheet. If no text of a layout has `{print_scale}`
or `{contour_interval}`, or `map.contour_interval_m` is missing, `check` says so.

### Logo

```toml
[[shared]]
id = "club_logo"
kind = "logo"
file = "logos/club.svg"
width_mm = 35.0
anchor = { to = "map_frame", corner = "top_right", offset_mm = [2.0, 2.0] }
```

`width_mm` is the width on paper, at most 2000; the height follows from the aspect ratio. The program takes PNG,
JPEG and SVG:

- **PNG and JPEG** stay pixel images, and a PNG keeps its transparency. For print the image should have about
  300 pixels per inch, so around 400 pixels for 35 mm.
- **SVG** becomes fills and lines in CMYK and stays sharp at any size. It is the best choice when the logo
  consists of fills, lines and gradients only.

Not every SVG can be printed. An SVG can hold much more than shapes: masks, filters, patterns, embedded images,
text, style sheets. Some files use that to demand minutes of computing time or gigabytes of memory from a few
kilobytes. So the program accepts only a fixed list of plain elements: `svg`, `g`, `path`, `rect`, `circle`,
`ellipse`, `line`, `polyline`, `polygon`, `defs`, `linearGradient`, `radialGradient`, `stop`, `use`, `title`,
`desc`, `metadata` and `clipPath` (in user coordinates only). If a logo holds anything else, the program leaves
it out and says so. Typical reasons:

- Text as `<text>`: convert the text to paths (Inkscape: "Path › Object to Path", Illustrator: "Type › Create
  Outlines").
- Masks, filters, shadows, patterns, embedded pixel images, style sheets in `<style>`: save the logo without
  these effects.
- A document type declaration (DTD) holding more than comments and short text entities as Illustrator writes
  them.
- Too large or nested too deep: over 10 MB, over 20 000 path segments, over 256 levels.

The note then suggests exporting the logo as a plain SVG, in Inkscape for example with "Save As › Plain SVG".
The rest of the sheet still prints.

What the sheet cannot draw exactly as the SVG describes it, it draws simplified and reports: a gradient prints in
its first colour, transparency prints opaque, dashes print solid, and clip paths have no effect. A PNG or JPEG
over 25 megapixels or over 65 535 pixels on a side, a file over 10 MB, and a missing or broken file are left out
with a note. The exact limits are in the [glossary](../glossary.md) under "Logo" and in ADR 0026.

The program reads and decodes each logo file once per `check` or `export`, however many elements and layouts
use it, and the PDF holds it once. One sheet takes at most 50 megapixels of PNG and JPEG logos and 1 000 000
units of SVG work, each logo counted as often as it is placed. A logo past that is left out with a note. Two
logos at the largest allowed size fit; a club logo of a few hundred pixels uses almost nothing.

### Scale check bar

```toml
[[shared]]
id = "scale_check_bar"
kind = "scale_check_bar"
anchor = { to = "map_frame", corner = "bottom_right", offset_mm = [2.0, 2.0] }
```

A black and white bar on a white box, labelled for example "200 m = 50 mm". Its length is the largest round
distance (5, 10, 20, 50, 100, 200, 250, 500 or 1000 m) that is at most 50 mm long at the print scale: 200 m at
1:4000, 100 m (33.3 mm) at 1:3000. A ruler on the printout shows whether the printer scaled the page
(chapter 9).

### Footer: copyright and credit lines

Every sheet has a footer in 5 pt at the bottom left, on a white box, so that nothing drawn under it hides
it; below the map frame the box is white on white paper. At the top come the copyright lines, one per line, from
`print.copyright` or from the map notes. Below them is the credit line "Created with freemapper-print –
https://github.com/HenryJobst/freemapper-print" ("Erstellt mit …" on German sheets). The credit line cannot be
switched off.

The footer goes below the map frame when all of it fits there. Otherwise it goes inside the map frame at the bottom
left, and `check` reports that it covers part of the map. In either place `check` names any element the footer
covers. To fix that, widen the passe-partout (`margin_mm`) or shorten `print.copyright`. The program shows at most
six copyright lines: five and a sixth that counts the rest. A line wider than the map frame is cut with "…", and
`check` names it.

A `copyright_line` or `credit_line` element takes that part out of the footer and puts it at its own anchor, in
5 pt or `size_pt`. It only moves the lines; the credit line stays on the sheet, on its own white box. A `credit_line`
must be at least 5 pt. If it would not lie wholly on the page, the credit line stays in the footer, and `check`
says so. The credit line is drawn after everything else on the sheet, so no element can cover it.

## 8. Courses

A course sheet is a layout with the course drawn on the map in purple. The project file links to the file from
your course-setting program, and `check` and `export` do the rest: one sheet per course, with control numbers and,
for Purple Pen files, the control descriptions. The example
[examples/luebeck-competition-courses](../../examples/luebeck-competition-courses/README.en.md) goes through all of
it with every command and its output. [examples/berlin-viktoriapark-training](../../examples/berlin-viktoriapark-training/README.en.md)
is a smaller one, a club training with a single course.

### Where courses come from

The program reads two kinds of files:

- A Purple Pen project, `.ppen`. It holds the controls, the courses, bends and gaps of the legs, circle gaps per
  scale, the control descriptions, special objects such as out-of-bounds areas, forbidden routes and marked
  routes, the position of each control number, and the print scale of each course.
- IOF XML 3.0 with `CourseData`, from Purple Pen, Condes, the OCAD course setting or any other program that
  writes it. It holds controls, courses, the positions of the controls and, if the writer sets them, the position
  of each number, the course length and the climb.

IOF XML has no control descriptions, special objects, bends or print scales. A sheet from an IOF XML file shows the
controls, the legs and the numbers, nothing else. If you want the description box or an out-of-bounds area on the
sheet, export the project from Purple Pen and use the `.ppen`. The program picks the reader from the file extension, `.ppen` or `.xml`; any other extension is an error.
If your program offers both, the `.ppen` gives the fuller sheet.

Not everything in a file is printed yet:

- A course with variations (forks and loops in Purple Pen) is left out, and so is a course with alternative
  controls at one stop. `check` says which. Purple Pen's IOF XML export writes each variation as a course of its
  own, which prints as a sheet of its own.
- An element the program does not know is skipped, with a note.

White-out areas print as masks of the courses they belong to, each on its own, also where they overlap (ADR 0045).
The map is missing there; the course overprint, the description box and the footer stay. When a course's
assignment names a mask of its own, only that one applies. `check` lists the white-out areas for information.

### Linking the file: the live link

In the project file, `[sources]` gets the key `courses`:

```toml
[sources]
map = "luebeck.omap"
courses = "courses.ppen"
```

The path is relative to the project file. The program reads the course file again on every `check` and every
`export`. If you move a control in Purple Pen, add a course or change a description, save the file and run
`export`. Nothing needs refreshing in the project.

The project file keeps only what the course file does not know. Two kinds of entries refer to it:

- `[[assignments]]` says which layout a course prints on. It names the course as the course file does:

  ```toml
  [[assignments]]
  course = "A"
  layout = "a4-4000"
  ```

  A course without an assignment takes the first layout, and `check` says so as Info.
- `[[numbers]]` corrects the position of a control number, either for a layout or for one course (see "Number
  positions" below). It names the control by its code.

When the course file changes, these entries can name something that no longer exists, for example a renamed course
or a deleted control. The program does not drop them. It reports each one with a note and keeps it in the project
file, so a renamed course does not cost you its settings. The other courses load as usual. Fix the name in the
project file, or delete the entry. If a position in the course file has moved since you made a correction, `check`
tells you that too. The correction still applies, and you decide whether it still fits.

### Coordinates and the fit

Before it prints, the program fits the course to the map, and it stops when the fit looks wrong. A course on the
wrong map or upside down would send runners to the wrong place. These checks run:

- **The y axis.** Purple Pen, Condes and OCAD write the y axis upwards, OOM counts downwards. Purple Pen always
  writes it upwards, so the program reads a `.ppen` that way. If the course does not fit the map like that, the
  fit stops; it never prints a Purple Pen course mirrored. An IOF XML file does not say which axis it uses. The
  program tries both readings and takes the one that puts every control on the map. With latitude and longitude
  in the file and a georeferenced map, the coordinates decide. If both readings fit and nothing decides, the
  program takes "up" and warns, because a course near the middle of the map fits both ways. Look at a sheet, then
  confirm the axis in the project file:

  ```toml
  [sources]
  course_y_axis = "up"      # or "down"
  ```

  An IOF XML file that was written downwards and fits both ways prints upside down until you set it. That is the
  one case where the program asks you to look. A Purple Pen project needs no `course_y_axis`. If you set
  `"down"` for a `.ppen` anyway, the program follows it and warns, because the course then prints mirrored.
- **The map scale.** If the course file names a map scale, it must equal the scale of the map. If it differs, the
  fit stops with "the course file is drawn for 1:…, the map is 1:…". Export the courses from the course-setting
  program for the map's scale. The program never rescales positions. A file without a map scale gets an Info line.
  A map that states no scale of its own stops the fit with "the course file is drawn for 1:…, and the map states
  no scale"; set the scale in OOM.
- **The georeference.** If the map is georeferenced in UTM and a control has both a map position and latitude and
  longitude, the program compares them. A control more than 20 m from where its latitude and longitude put it
  stops the fit with "control … lies … m from where its lat/lng put it". Latitude and longitude from a GNSS reading
  in the field are good to 5 or 10 m. A wrong map, zone or datum shifts every control by hundreds of metres. What to
  do depends on the cause:
  - The map's georeferencing is wrong or belongs to another map. Fix it in OOM.
  - The course file belongs to another map. Take the right file.
  - Only the latitude and longitude are wrong or unwanted. Set `course_georef_check = false` under `[sources]`.
    The map positions then place the course alone, and `check` tells you with an Info line that it ignored the
    coordinates.

  Other coordinate systems than UTM are not checked yet; `check` says so as Info.
- **Outside the map.** If no reading of the y axis puts all controls on the map, the fit stops with "only … of …
  controls lie on the map; the course file belongs to another map, uses another origin or another y axis". Check
  that you linked the right course file and the right map. If you set `course_y_axis`, check that it is right.
  Controls that sit up to 10 mm outside the drawn map still count, so a start or finish at the edge of a cut-out
  works.
- **Latitude and longitude only.** A control with latitude and longitude but no map position is placed through the
  map's georeferencing. If the map cannot do that, or you switched the check off, the fit stops with "controls …
  have only lat/lng and cannot be placed", followed by the reason.

When a check fails, `check` and `export` print the error with the file and stop for the whole course file, so a
bad file never produces a sheet. The exit code is 1.

### Course sheets and the overprint

With a course file, `export` writes one PDF per course instead of one per layout (see "Export" below). A course
sheet is the layout the course is assigned to, with these changes:

- The map frame centres on the course, unless the layout sets `map_frame.centre_map_mm`. The program centres on
  the controls and bends and widens the frame for a special object only if the object still fits. Course sheets are
  usually cut from a larger map, so set `map_frame.crop = true`, as `init --courses` does. A map larger than the
  frame is expected on a course sheet and is only an Info line. A control that lies outside the frame or across its
  edge is a note.
- The program draws the course over the map in purple (35/85/0/0 CMYK in the rule profile) and with the symbol sizes
  that the rule profile sets for the map kind, from ISSprOM 2019-2 and ISOM 2017-2. These are the sizes at the base
  scale (1:4000 sprint, 1:15000 forest). On the sheet they take the size the standard sets for the print scale,
  the course factor: 133 % at 1:3000 on a sprint map, 150 % at 1:10000 on a forest map. That does not depend on the
  scale the map file is drawn at. A forest map drawn at 1:10000 gets 7.5 mm circles at 1:10000 and 5.0 mm circles
  at 1:15000. The summary line names the symbol factor, which is for the map's own symbols.
- The purple has two layers, as the IOF's "Printing and Colour Definitions" (Rev. 4) require. The lower purple
  holds the start, the controls, the legs, the finish, forbidden lines and crossing points. It lies below black,
  brown and blue 100 %, so the map's black detail stays readable over the circle. The upper purple holds the control
  numbers, map exchanges, out-of-bounds areas, forbidden routes, marked routes and texts. It lies over the whole map.
- Both layers are painted in `Multiply` blend mode. The map shows through the purple everywhere, and black under
  the purple stays black. An office printer has no spot colour and no overprint flag, so the PDF has to show the
  result itself.
- The text elements know four more placeholders: `{course}` (the course name), `{length_km}` (the length in
  kilometres, without unit), `{climb_m}` (the climb in metres, without unit) and `{controls}` (the number of
  controls). A placeholder stays empty, with an Info line, if the file has no value for the course or if the sheet
  has no course. For example `"{event} · Course {course}"` names the course in the title block.
  `init --courses` writes `{event} · {course}`.

The program also checks that no sheet element hides part of the course. If a title block, logo, text, scale check
bar, footer line or the description box covers a control, the start or the finish, the check warns and names the
element and the controls. Move the element with its anchor.

### The control description box

A layout element of kind `control_descriptions` shows the control descriptions of the course as the IOF defines
them:

```toml
[[layouts.elements]]
id = "descriptions"
kind = "control_descriptions"
cell_mm = 6.0                        # optional, 4 to 10, default from the rule profile
anchor = { to = "map_frame", corner = "top_right", offset_mm = [2.0, 2.0] }
```

The box is laid out the way Purple Pen lays it out. It shows the event title, the course with its length and
climb, the start, one row per control with number, code and the symbols for the columns C to H, the rows for marked
routes and map exchanges with their length, and the finish. The secondary title of a course from Purple Pen gets
its own row, and a map flip the row 15.6. A score course lists its controls by code, the way Purple Pen does.

The box has a white background and is drawn in Arimo. A cell is 6 mm by default. `cell_mm` sets it from 4 to 10 mm.
The box keeps its size at every print scale, as an element on paper should. If a 1:3000 sheet needs more room for
the map, make the cells smaller.

The box does not appear if it would reach past the page edge, because a cut-off description could hide a control.
`check` warns about that, and about a course where no control has descriptions. A layout without the element
simply has no box, and `check` says so as Info. The same holds for IOF XML. A symbol that the course file's
description standard does not know leaves its cell empty, and `check` names it. The description standards 2004,
2018 and 2024 are read.

Not done yet:

- A map issue point is read as a start. Its own row, with the length of the marked route to the start, is missing.
- A leg that is only partly flagged is drawn and described as unflagged.
- A long course is not split into several columns.
- The box has no column for text next to the symbols, and no key for custom symbols.

### Number positions

Where a control number sits matters on a crowded sprint map, and several people may want to decide. The program
looks at four levels, and the more specific one wins:

1. Automatic placement. It applies only to a numbered control for which the course file gives no position.
2. The course file. Purple Pen stores the position per course, relative to the control. For a `.ppen`, the
   program converts the offset at the course's print scale and with Purple Pen's setting for scaling course
   objects ("Scale sizes of items"). For IOF XML, it takes `MapTextPosition`.
3. A correction for the layout: a `[[numbers]]` table with `layout`.
4. A correction for the sheet of one course: a `[[numbers]]` table with `course`.

A correction names the control, a direction in degrees (0 is east, counter-clockwise) and optionally a gap:

```toml
[[numbers]]
control = "49"
course = "A"
direction = 161.0
gap_mm = 0.5
```

The gap is the shortest distance in mm at the base scale between the outer edge of the circle line and the
number's box. It grows with the course factor like the circle, so a number keeps its look at every print scale. A `gap_mm` lies
above 0 and at most 10 mm. Without it, the correction keeps the gap of the level below. `numbers accept` also writes
a `[numbers.base_seen]` table with the position the file had when you made the correction.

The rule profile sets the range in which a gap should lie. In `iof-print` it is 0.5 to 5 mm. A number outside the
range prints where it is, and `check` warns. A profile rule with only a minimum makes automatic placement keep to
that gap, as freemapper-courseplan does; `check` then accepts any larger gap up to 10 mm.

When the program places a number itself, it follows four rules. The user rules of orienteering come first:

1. The number covers no point symbol and no line symbol of the map. This rule is hard.
2. The number goes behind the control, in line with the leg that comes in. For the first control that leg comes
   from the start.
3. If the box behind the control would come within half a line width of the outgoing leg, the number goes on the
   outer bisector, between the two legs on their outer side.
4. If the preferred place is blocked, the free place that deviates least from it wins.

The program never moves a number on its own. It prints where the level puts it, and `check` scores every number
against the overprint, the sheet elements, the edge of the map frame and the map's own symbols. If a number lies
on one of them, the note names what it covers (map symbols by their code) and proposes a direction and a gap.
Where no place is free, the proposal is the least covered one.

The search has a limit per sheet: 2 million candidate positions, and 200 million for all sheets of one course
file, shared equally. Real courses stay far below it. A sheet that reaches it says so in a note; the numbers the
search did not get to stay where their level puts them, and their conflicts come without a proposal.

To take a proposal:

```sh
freemapper-print numbers accept citysprint --course A --control 49 --sheet
```

`--sheet` makes the correction for this course's sheet (level 4). `--layout-wide` makes it for every course that
prints on the course's layout (level 3). Exactly one of the two is needed. Without `--direction` and `--gap`, the
command takes the proposal from `check`. With both, it writes your values. If `check` has no proposal because the
number has no conflict, the command stops and asks for the two values.

The proposal was found on the sheet of the course you name. With `--layout-wide` and no values of your own, the
command draws every other course on that layout that prints the number with the correction added, and writes it
only when none of them gets a conflict there. Otherwise it names those courses and stops with exit code 2. Use
`--sheet` for the one course, or give `--direction` and `--gap` for all of them.

`numbers accept` appends a `[[numbers]]` table to the end of the project file as text. Everything before it stays
as it is, comments included. The new lines end like the file's (CRLF or LF), a project file that is a link is
replaced by a regular file (the link's target keeps its content), and the file keeps its permissions. It then
loads the file again, and if that fails, it restores the old content. It
refuses a second correction for the same control and scope. To change one, edit the file. Afterwards run `check`
to see the result.

### Export

```sh
freemapper-print export citysprint
freemapper-print export citysprint --course A --course B
freemapper-print export citysprint --single-file
freemapper-print export citysprint --layout a4-4000
```

With a course file, `export` writes one PDF per course into the folder `out`, named after the course: `A.pdf`.
Characters that a file name cannot hold become `-`. Names that differ only in case get " (2)".

- `--course <NAME>` exports only the named course. It can be repeated. A name the file does not have is an error
  that lists the courses it does have, with exit code 2.
- `--single-file` writes all courses into one PDF, one page per course, each on the paper of its layout. The file
  is named after the event title with " - all courses" ("- alle Bahnen" on German sheets). It cannot be combined
  with `--layout`.
- `--layout <ID>` exports a layout as before, without any course, the map alone with title and scale check bar.
  With a course file it cannot be combined with `--course` or `--single-file`.
- `--out-dir` works as before.

A project without a course file exports exactly as in chapter 5.

### Starting from a course file: `init --courses`

```sh
freemapper-print init citysprint --map luebeck.omap --title "Stadtsprint Lübeck" --courses courses.ppen
```

`init` reads and fits the course file first. A file that does not fit the map stops `init` before it writes
anything. For a `.ppen` it then creates one layout per print scale in the file, an assignment for every course, a
description box in the top-right corner of the map frame, and `map_frame.crop = true`. A course without a print
scale takes the map scale. For IOF XML it creates one layout at the map scale and no description box, because the
format has none. Each layout id names its print scale, such as `a4-4000`. In both cases the title block starts
with `{event} · {course}`, so that each sheet names its course. `init` says where each value came from and names
the course file as the project file does, relative to the project folder.

Treat the file as a start. `init` picks paper and passe-partout for the layout as a sheet of its own; whether
each course fits the frame shows in `check`. Change paper and passe-partout to your liking, then run `check`. For
an IOF XML file the first `check` may report the assumed y axis; confirm it as described above.

### Checking courses

`check` goes through every course. For each it prints one line with the layout, the paper, the print scale, the
map scale and the factors, followed by the notes. Then it checks each layout as a sheet without course, as
`export --layout` writes it. Notes about the course file itself, from reading and fitting it, come once before the
first course. The last line reads "Checked 3 courses and 2 layout sheets, no notes." or "With notes: 1 of 3
courses, 0 of 2 layout sheets." The exit code is 0 when nothing needs your attention (Info lines do not count) and
1 otherwise. A script can run `check` before `export`.

The notes and what to do are in chapter 10.

## 9. Printing at the right scale at home and in a copy shop

An orienteering map must have the scale printed on it. The PDF from freemapper-print has it exactly: 1 mm on
the paper is the ground distance the print scale sets. Whether the printout keeps it is decided in the print
dialog.

### Creating the PDF

For an event, `export` writes the PDFs (chapter 5). For a quick print of the bare map without a project file
there is `render`:

```sh
freemapper-print render luebeck.omap --out luebeck-4000-a4.pdf --paper a4
```

Without `--scale` the program prints at the scale of the map. `--scale 3000` prints at 1:3000. Choose A4 or A3
(`--paper`) and, if needed, the orientation (`--orientation`). With `auto` the program takes the orientation
that cuts off less of the map. `render` leaves 5 mm free on every side and prints neither title nor scale check
bar. It does print the footer: the copyright lines from the map notes and the credit line, in the program
language, on a white box in the bottom-left corner of the printable area. The 5 mm margin is too narrow for
them below the map. If the box covers part of the map, `render` says so; a project with a wider passe-partout
puts the footer below the map. First the program writes a line with the paper, the print scale and the factors,
then come notes and a reminder to print at actual size.

A course sheet comes from `export` too (chapter 8). It has the same scale rules as a layout sheet. In a PDF from
`--single-file`, each page has the paper of its own layout, and the print dialog must print every page at actual
size.

### Printing at actual size

Open the PDF and choose 100 % or "Actual size" in the print dialog. Switch off "Fit to page" and "Shrink to
printable area". Any other setting changes the scale, and the map is no longer right. In a copy shop, say the
same: no scaling, 100 %, paper as in the PDF.

### A4 or A3

If the map is larger than the map frame (for `render`, the page less 5 mm on each side), the program cuts it
off and tells you, for example:

```
Warning: the map does not fit on the page. It is 4.0 mm too tall for the printable area. Try A3 paper, or ...
```

The number says by how many millimetres the map extends beyond it, both sides together. It is measured on what
would actually print, with line widths, point symbols and texts. The map is not shrunk, because that would falsify
the scale. You can take larger paper (A3), a print scale with a larger number, or in a layout choose the part of
the map with `centre_map_mm` and mark the cut as intended with `map_frame.crop = true` (chapter 6). If you chose
the orientation yourself, the message also names the other one. The PDF is written even with an overflow, so you
can see what is missing.

### How large the symbols are

The specification for the map kind sets the symbol sizes for a base scale: 1:4000 for sprint maps (ISSprOM),
1:15000 for forest maps (ISOM). A larger print enlarges everything by the same ratio, the terrain and every
symbol. A sprint map at 1:3000, which the standard recommends for the youngest age groups, has its symbols at
133 %: a small footpath is 0.36 mm wide instead of 0.27 mm. A forest map grows to 150 % at 1:10000, 200 % at
1:7500 and 300 % at 1:5000. The map needs correspondingly more paper.

The standards foresee no print smaller than the base scale. The program prints it anyway, for example a sprint
map at 1:5000. The symbols then keep their 1:4000 size and only the terrain shrinks. The program warns:

```
Warning: 1:5000 is not a standard scale for sprint maps. The map is reduced and the symbols keep their 1:4000
size. Use this print for training only.
```

For this the program needs the map kind (sprint, schoolyard, forest). `render` tells it from `freemapper.toml`
or from the map file, or you give it with `--kind`. In a project it is `map.kind`.

### Checking the scale with a ruler

A layout shows a scale check bar when it has a `scale_check_bar` element (chapter 7). `init` adds one, and
`check` reports a layout without it. Put the ruler on the bar: with "200 m = 50 mm" it
must be exactly 50 mm long. `render` prints no bar. Then measure between two points whose distance you know,
such as two building corners or the ends of a straight stretch of street. For 100 m your ruler should show:

| Print scale | 100 m on the map |
|---|---|
| 1:4000 | 25 mm |
| 1:3000 | 33.3 mm |

If the value is off, the printer has scaled. Check the print dialog. The examples
[examples/luebeck-scales](../../examples/luebeck-scales/README.en.md) and
[examples/luebeck-competition-layouts](../../examples/luebeck-competition-layouts/README.en.md) hold the same
map at 1:4000 on A4 and at 1:3000 on A3.

### Notes about printing the map

These notes come from `render`, `check` and `export`:

- "not drawn because the symbol is unknown or hidden": objects whose symbol the map file hides or does not
  know. They are missing in the PDF.
- "simplified or cut short": an object needs more drawing work than the program allows per object. It
  appears simplified or cut off.
- "not drawn because the page reached its drawing limit": the whole page needs more drawing work than the
  program allows per page. Only broken or deliberately overloaded files get there.
- "not drawn because the program does not know their object type": the file holds objects of a kind OOM does
  not write. They are missing in the PDF.
- "drawn without an area pattern whose spacing is below 0.05 mm or not a number": the map file asks for an
  area pattern denser than 0.05 mm or with invalid numbers. The program leaves the pattern out and prints
  the rest of the area.
- "Note: ... The map kind comes from the map file instead.": the `freemapper.toml` next to the map is broken.
  The program then takes the map kind from the map file.
- "the font Arimo replaces ...": the PDF contains only the Arimo font. The program replaces the other fonts of
  the map with it.
- "the embedded font has no glyph for these characters, so they are missing in the PDF": a character of the map
  is not in Arimo and is not printed.
- "the footer with the copyright and credit lines sits on a white box … and covers part of the map" (`render`
  only): the footer lies on the map in the bottom-left corner. Use a project to put it below the map.
- `render` prints freemapper's information block (title, scale, scale bar, credits) only with
  `--show-sheet-objects`. At another print scale it scales with the map, so a scale printed in it can be wrong.
  North lines belong to the map and are always in the PDF.

## 10. Messages and what to do

This chapter explains the notes about the sheets of a layout and of a course. The notes about printing the map
itself are in chapter 9. None of them stops the export. Each line that starts with "Note:" makes `check` end with exit code 1;
a line that starts with "Info:" does not.

| Note (shortened) | Meaning and fix |
|---|---|
| "the map does not fit on the page. It is … too tall/too wide …" | The map is larger than the map frame and is cut. A3, another scale, a narrower passe-partout, or `centre_map_mm` for the part you want. If the cut is intended, set `map_frame.crop = true`. |
| "Info: the map is … for the map frame and is cut there on purpose" | For information only: the layout has `map_frame.crop = true`. |
| "the passe-partout is too narrow for the footer …" | The footer sits on a white box inside the map. Raise `margin_mm`, shorten `print.copyright`, or place the lines elsewhere with `copyright_line`/`credit_line`. |
| "the footer … at the bottom left covers part of the element …" | The footer and its white box overlap an element in the bottom-left corner, below or inside the map frame. Move the element, or move the footer lines with a `copyright_line` or `credit_line` element. The note names what the footer holds: copyright lines, the credit line or both. |
| "the credit line element … would not lie wholly on the page" | The `credit_line` element would reach past the page edge, so the credit line stays in the footer. Check its anchor, offset and `size_pt`. |
| "the footer … reaches past the page edge" | The footer extends beyond the sheet. Check the passe-partout and the copyright lines. |
| "the element … reaches past the page edge" | Check the element's anchor and offset; make a logo narrower if needed. |
| "the element … hangs on the page and covers part of the map frame" | An element hung on the page (`to = "page"`) lies over the map. Move it onto the passe-partout, or hang it on the map frame if it is meant to sit on the map. |
| "the paper is larger than A3" | Only for paper larger than A3; the program knows only A4 and A3 so far. |
| "layout … lacks a text with {print_scale} …" | No sheet element shows the scale or the contour interval, or `map.contour_interval_m` is missing. Add a title block or text with the placeholder, or set the value. |
| "layout … has no scale check bar" | The layout shows no `scale_check_bar` element. Add one (shared or in the layout), so a ruler on the printout shows whether the printer scaled it. |
| "Info: the map has no north lines, so … lines were added" | For information only, see chapter 6. `north_lines = "never"` turns it off. |
| "… neither north lines nor a usable line symbol 601 …" | The map has no symbol for north lines. The program draws plain black 0.14 mm lines over the map, on top of everything. Better: draw north lines in OOM. |
| "layout … shows … of … copyright lines" | More than six copyright lines. Merge them in `print.copyright`. |
| "the copyright line '…' is wider than the map frame and is cut short" | Shorten the line in the map notes or in `print.copyright`, or split it in two. |
| "the logo … is left out: …" | The file is missing, broken, too large, or an SVG with parts that are not allowed, or the sheet already holds 50 megapixels of logos or used up its SVG budget; the reason follows. See "Logo" in chapter 7. |
| "the logo … is drawn simplified: …" | A gradient, transparency, dashes or a clip path in the SVG. The logo prints, but not quite as in the drawing program. |
| "the text … contains {…}, which is no placeholder" | A typo in a placeholder. The list is in chapter 7. |
| "layout … hides the map part …, but the map has no part of that name" | Write names in `hide_parts` exactly as in OOM, upper and lower case included. |

Errors in the project file stop `check` and `export`. The message names file, line and column and the key, for
example ``freemapper-print.toml:24:1: field `layouts[1].elements[0].width_mm`: a logo needs a `width_mm` ``. For a
missing key the line is that of the table it belongs in. These messages are in English for now, except the one
about `hide_parts = ["base"]` (see "The base area of sprint maps" in chapter 6), which comes in German too.

### Notes about courses

These notes appear when the project has a course file. Notes about the course file itself come once, before the
first course. The closing line "The course file itself has notes. They are listed before the first course." tells
you that some of the notes above belong to the file and not to a course. A line with "Info:" does not count for
the exit code.

| Note (shortened) | Meaning and fix |
|---|---|
| "Info: the course file's y axis points up; the program turned it to match the map." | For information only. Most course-setting programs write it that way. |
| "the course fits the map with its y axis pointing up and pointing down, and nothing in the file decides …" | Only for IOF XML. The course lies where both readings fit, so the program assumed "up". Look at a sheet. If the course is right, confirm it with `course_y_axis = "up"` under `[sources]`, and if it is upside down, set `"down"`. |
| "the project file sets course_y_axis = "down" under [sources], but Purple Pen always writes the y axis pointing up …" | The course prints mirrored. If the file comes from Purple Pen, delete the line. |
| "Info: the course file states no map scale, so it was not compared with the map's scale." | The file has no scale to compare. Nothing to do, but check that you linked the right map. |
| "Info: … a map position and latitude and longitude. They differ by … m at most." | The check against latitude and longitude passed. |
| "Info: latitude and longitude were not checked, because …" | The reason follows: the map is not georeferenced, its coordinate system is not UTM, no control has both a map position and latitude and longitude, or the project file switched the check off. |
| "Info: the project file switches the check against latitude and longitude off …" | `course_georef_check = false` is set. |
| "Info: control … only latitude and longitude. The map's georeferencing placed them." | The course file has no map position for these controls. Check one of them on the sheet. |
| "Info: the course file has no control descriptions." | The file is IOF XML, or a `.ppen` without descriptions. |
| "the course file contains the element …, which this version does not read. It is left out." | The element is missing on the sheets. Look at what it is in your course-setting program. |
| "Info: the course file has white-out areas. Each one hides the map on the courses it belongs to, unless a course's assignment names a mask of its own." | For information only. Purple Pen's white-out hides the map on the courses it belongs to in Purple Pen. Check on the sheets that this is what you want; a mask of its own in the assignment replaces it for that course. |
| "course … has forks or loops (Purple Pen variations) … the course is left out" | The course is missing on purpose. Export the courses from Purple Pen as IOF XML, which writes each variation as a course of its own. |
| "course … offers alternative controls at one stop … the course is left out" | This version cannot print them. Split the course in the course-setting program. |
| "Info: courses … are the variants of fork …. Each prints as a sheet of its own." | For information only. |
| "Info: the map issue point … is used as the start." | A map issue is read as a start, so its row in the description box is missing. |
| "Info: the course file gave more notes than are listed here." | The file is unusual or damaged. The notes shown are the first ones. |
| "Info: course … has no assignment and prints on the first layout, …" | Add an `[[assignments]]` entry if another layout is meant. |
| "the project file assigns course … to a layout, but the course file has no such course. The assignment stays …" | The course was renamed or deleted in the course file. Fix the name in the project file, or delete the entry. |
| "the project file corrects the number of control … for …, but the course file has no such control there. The correction stays …" | The control was renumbered or removed, or it is not in that course. Fix the code, or delete the entry. |
| "the course file has moved the number of control … since the project file corrected it. Check whether the correction still fits." | Someone moved the number in the course-setting program after you made the correction. Look at the sheet. If the correction still fits, the note stays until you delete the entry and write it again with `numbers accept --direction … --gap …`. If it does not fit, delete the entry. |
| "the course file has no course to export, so no PDF was written." | The file has no course, or every course was left out. See the notes above. |

Notes about one course sheet:

| Note (shortened) | Meaning and fix |
|---|---|
| "on the sheet of course …, control … lies outside the map frame or across its edge." | The frame does not show the whole course. Take larger paper or a print scale with a larger number, or set `map_frame.centre_map_mm` to move the frame. |
| "Info: the map is … for the map frame. A course sheet shows the part around the course …" | For information only. A course sheet cuts the map on purpose. |
| "Info: course … comes from IOF XML, which has no control descriptions, so its sheet has no description box." | For information only. Use the `.ppen` for descriptions. |
| "Info: the layout of course … has no control_descriptions element …" | For information only. Add the element to the layout if you want the box. |
| "the description box of course … would reach past the page edge, so the sheet leaves it out …" | Move the box with its anchor or make it smaller with `cell_mm`. |
| "no control of course … has descriptions in the course file, so its sheet has no description box. Enter them in Purple Pen." | Enter the descriptions in Purple Pen. |
| "the element … covers control …. Move the element with its anchor so that runners can see the course." | A title block, logo, text, footer line or the box sits on a control, the start or the finish. Move it. |
| "the descriptions of course … use symbol …, which the course file's description standard does not have. Its cell … stays empty." | The symbol id does not exist in the standard of the file (2004, 2018 or 2024). Pick another symbol in Purple Pen. |
| "the course overprint of course … asks for more hatch lines, dashes or crosses than one sheet draws, so some are left out." | A special object is very large or the file is damaged. Make the area smaller, or split it. |
| "Info: the text … uses …, but the course file has no value for course …, so it prints empty." | The file has no length or climb for the course. Enter them in the course-setting program, or leave the placeholder out of the text. |
| "Info: the text … uses …, which is only filled on course sheets. On this sheet it stays empty." | A shared text with `{course}` or the like prints on a sheet without course. Hide the text on that layout with `hide_shared`. |
| "on the sheet of course … the number of control … covers …. It is printed there all the same. …" | The number lies on the overprint, a sheet element, the edge of the frame or a point or line symbol of the map. The note proposes a free direction and gap. Take it with `numbers accept` (see chapter 8). |
| "… The sheet reached its search limit, so there is no proposal. …" | The number has a conflict, but the search for places had stopped. Set a position with `numbers accept --direction … --gap …`. |
| "on the sheet of course … the search for number positions reached its limit. …" | The course has hundreds of controls close together. The numbers the search did not get to stay where their level puts them. Look at the sheet and correct numbers by hand. |
| "… the number of control … is … mm from its circle. The rule profile allows … to … mm." | The gap from the file or a correction lies outside the range of the rule profile. The number is printed that way. Change it under `[[numbers]]`. |

### Errors with the course file

An error stops `check` and `export`. The line starts with "could not use the course file" or "could not read the
course file", followed by the reason. The reasons from reading the file are in English:

- "invalid course file: …": the XML is damaged, or lacks something the format requires, such as a control id. Open
  it in the course-setting program and save it again.
- "course file too large or too complex: …": the file passes one of the limits that protect the program from
  hostile files. Split the event into several files.
- "… uses unit …; only mm is supported": the IOF XML gives positions in another unit. Export it with millimetres.
- "unknown file type; use `.ppen` (Purple Pen) or `.xml` (IOF XML 3.0)": the extension of `[sources] courses` is
  neither.

The reasons from the fit are the errors of the section on coordinates in chapter 8:

- "the course file is drawn for 1:…, the map is 1:…; export the courses for the map's scale"
- "the course file is drawn for 1:…, and the map states no scale; set the map's scale in the map file"
- "only … of … controls lie on the map; the course file belongs to another map, uses another origin or another y axis"
- "control … lies … m from where its lat/lng put it (20 m allowed); the course file does not match the map's georeferencing"
- "controls … have only lat/lng and cannot be placed: …"

Errors of `export` and `numbers accept` about names:

- "the course file has no course …; its courses are …": the name in `--course` is not in the file.
- "--course and --single-file need a course file, and the project names none under [sources] courses": add the
  `courses` key.
- "the course … has no control …; its controls are …": the code for `numbers accept` is not in the course.
- "there is no proposal for control … on the sheet of course …: its number has no conflict. Give --direction and
  --gap …": nothing to correct. Give both values if you want to move the number anyway.
- "the project file has a correction for control … for … already. Edit the file to change it …": edit the entry.
- "the proposal for control … comes from the sheet of course … and is not free for every course on layout …":
  `--layout-wide` would move a conflict to the courses it names. Use `--sheet`, or give `--direction` and
  `--gap`.
- "--direction needs a finite number, and --gap must be above 0 and at most … mm".
- "the project … does not load with the new correction, so it was left as it was": the program restored the file.
  Report this, as it should not happen.
