# freemapper-print user manual

[Deutsch](de.md) · [README](../../README.en.md)

This manual grows with the program. Chapters 5, 6, 7, 9 and 10 are written. In the end it should lead step by
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
8. Masks for window and corridor orienteering
9. [Printing at the right scale at home and in a copy shop](#9-printing-at-the-right-scale-at-home-and-in-a-copy-shop)
10. [Messages and what to do](#10-messages-and-what-to-do)

The program speaks German or English. It takes the language from `--lang de` or `--lang en`, else from the
environment variables `FREEMAPPER_LANG`, `LC_ALL`, `LC_MESSAGES` and `LANG`. The messages quoted in this manual
are the English ones.

## 5. Setting up an event

Each event has a folder with a project file, `freemapper-print.toml`. It names the event, the map, the layouts
and the sheet elements. Courses come with a later version (M3); until then the program prints the map with
title, logos and scale check bar, but without a course overprint.

The work takes three commands:

1. `init` creates the project file.
2. You edit the file in a text editor and run `check` until the sheets look as they should.
3. `export` writes one PDF per layout.

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
| `[[shared]]` | | one shared sheet element; the section may appear many times |
| `[[layouts]]` | | one layout; at least one, any number |

### Checking: `check`

```sh
freemapper-print check citysprint
```

`check` takes the project file or its folder. It draws every layout in memory, writes no file, and prints one
line per layout with paper, orientation, scale and factors, followed by the notes. It ends with "Checked 1
layout, no notes." or "Layouts with notes: 1 of 2."

The exit code is 0 when no layout has anything to report, and 1 when one has a note or cannot be drawn. Added
north lines and an intended crop (`map_frame.crop`) do not count as a note. A script can therefore run `check` before `export`. A note never stops the
export. Some notes are expected, such as the overflow of a map larger than the sheet. Chapter 10 explains each
note.

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
first written as `<name>.pdf.part` and gets its name only when it is complete on the disk. On a full disk an
older PDF of the same name stays as it was, and no half-written PDF is left behind; the same holds for any
other error. The message says how much space is needed and how much is free, or that the disk does not say.
Do not run two exports into the same folder at the same time: they would share the `.part` file.

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

## 7. Title, logos and other sheet elements

Sheet elements stand on the sheet but are not part of the map. Shared elements go under `[[shared]]` and appear
on every sheet. A layout hides single ones with `hide_shared` and has its own under `[[layouts.elements]]`. The
ids of the shared elements and of one layout's elements must be unique together. `footer` is taken.

Every element has:

| Key | Meaning |
|---|---|
| `id` | the name notes use for the element |
| `kind` | `title_block`, `text`, `logo`, `scale_check_bar`, `copyright_line` or `credit_line` |
| `anchor.to` | `"page"` (the sheet) or `"map_frame"` |
| `anchor.corner` | one of nine positions: `top_left`, `top`, `top_right`, `left`, `centre`, `right`, `bottom_left`, `bottom`, `bottom_right` |
| `anchor.offset_mm` | `[x, y]`, distance in mm from that position inward |

Each offset lies between -2000 and 2000 mm. From a left or top edge the offset counts right or down, from a right
or bottom edge left or up, from a middle right or down. The element sits on the inner side of its position: `top_right` with `[2, 2]` puts its top-right
corner 2 mm left of and 2 mm below the top-right corner.

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
`text` sets all lines in 9 pt or in `size_pt`. Both stand on a white box with a 1 mm margin; `box_white = false` leaves it
out. The font is Arimo, the same as for map texts.

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

An unknown placeholder, such as the typo `{evnet}`, stays as written and gives a note. `{course}` for the course
name comes with courses.

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

`width_mm` is the width on paper, at most 2000; the height follows from the aspect ratio. The program takes PNG, JPEG and SVG:

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

Every sheet has a footer in 5 pt at the bottom left. At the top come the copyright lines, one per line, from
`print.copyright` or from the map notes. Below them is the credit line "Created with freemapper-print –
https://github.com/HenryJobst/freemapper-print" ("Erstellt mit …" on German sheets). The credit line cannot be
switched off.

The footer goes below the map frame when all of it fits there. Otherwise it goes inside the map frame at the
bottom left on a white box, and `check` reports that it covers part of the map, and names any element the box
covers. To fix that, widen the passe-partout (`margin_mm`) or shorten `print.copyright`. The program shows at
most six copyright lines: five and a sixth that counts the rest. A line wider than the map frame is cut with
"…", and `check` names it.

A `copyright_line` or `credit_line` element takes that part out of the footer and puts it at its own anchor, in
5 pt or `size_pt`. It only moves the lines; the credit line stays on the sheet. A `credit_line` must be at least
5 pt. If it would not lie wholly on the page, the credit line stays in the footer, and `check` says so. The
credit line is drawn after everything else on the sheet, so no element can cover it.

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
would actually print, with line widths, point symbols and texts. The map is not shrunk, because that would
falsify the scale. You can take larger paper (A3), a print scale with a larger number, or in a layout choose the
part of the map with `centre_map_mm` and mark the cut as intended with `map_frame.crop = true` (chapter 6). If
you chose the orientation yourself, the message also names the other one. The PDF is written even with an overflow, so you can see what is missing.

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

This chapter explains the notes about the sheets of a layout. The notes about printing the map itself are in
chapter 9. None of them stops the export, but each one except "lines were added" and "cut there on purpose"
makes `check` end with exit code 1.

| Note (shortened) | Meaning and fix |
|---|---|
| "the map does not fit on the page. It is … too tall/too wide …" | The map is larger than the map frame and is cut. A3, another scale, a narrower passe-partout, or `centre_map_mm` for the part you want. If the cut is intended, set `map_frame.crop = true`. |
| "the map is … for the map frame and is cut there on purpose" | For information only: the layout has `map_frame.crop = true`. |
| "the passe-partout is too narrow for the footer …" | The footer sits on a white box inside the map. Raise `margin_mm`, shorten `print.copyright`, or place the lines elsewhere with `copyright_line`/`credit_line`. |
| "the footer … covers part of the element …" | The footer's white box overlaps an element in the bottom-left corner of the map frame. Move the element, or widen `margin_mm` so the footer goes below the frame. |
| "the credit line element … would not lie wholly on the page" | The `credit_line` element would reach past the page edge, so the credit line stays in the footer. Check its anchor, offset and `size_pt`. |
| "the footer … reaches past the page edge" | The footer extends beyond the sheet. Check the passe-partout and the copyright lines. |
| "the element … reaches past the page edge" | Check the element's anchor and offset; make a logo narrower if needed. |
| "the element … hangs on the page and covers part of the map frame" | An element hung on the page (`to = "page"`) lies over the map. Move it onto the passe-partout, or hang it on the map frame if it is meant to sit on the map. |
| "the paper is larger than A3" | Only for paper larger than A3; the program knows only A4 and A3 so far. |
| "layout … lacks a text with {print_scale} …" | No sheet element shows the scale or the contour interval, or `map.contour_interval_m` is missing. Add a title block or text with the placeholder, or set the value. |
| "layout … has no scale check bar" | The layout shows no `scale_check_bar` element. Add one (shared or in the layout), so a ruler on the printout shows whether the printer scaled it. |
| "the map has no north lines, so … lines were added" | For information only, see chapter 6. `north_lines = "never"` turns it off. |
| "… neither north lines nor a usable line symbol 601 …" | The map has no symbol for north lines. The program draws plain black 0.14 mm lines over the map, on top of everything. Better: draw north lines in OOM. |
| "layout … shows … of … copyright lines" | More than six copyright lines. Merge them in `print.copyright`. |
| "the copyright line '…' is wider than the map frame and is cut short" | Shorten the line in the map notes or in `print.copyright`, or split it in two. |
| "the logo … is left out: …" | The file is missing, broken, too large, or an SVG with parts that are not allowed, or the sheet already holds 50 megapixels of logos or used up its SVG budget; the reason follows. See "Logo" in chapter 7. |
| "the logo … is drawn simplified: …" | A gradient, transparency, dashes or a clip path in the SVG. The logo prints, but not quite as in the drawing program. |
| "the text … contains {…}, which is no placeholder" | A typo in a placeholder. The list is in chapter 7. |
| "layout … hides the map part …, but the map has no part of that name" | Write names in `hide_parts` exactly as in OOM, upper and lower case included. |

Errors in the project file stop `check` and `export`. The message names file, line and column and the key, for
example ``freemapper-print.toml:24:1: field `layouts[1].elements[0].width_mm`: a logo needs a `width_mm` ``. For a
missing key the line is that of the table it belongs in. These messages are in English for now.
