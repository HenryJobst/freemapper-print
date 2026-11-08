# Changes

[Deutsche Version](CHANGELOG.md) · [User manual](docs/manual/en.md)

All notable changes to freemapper-print are listed here, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Sheets carry a scale check bar: a bar of round length (5 m to 1000 m, at most 50 mm when printed), labelled like "200 m = 50 mm". A ruler on the printout shows whether the printer scaled the page.
- Sheets carry a title block and texts with placeholders (`{event}`, `{date}`, `{print_scale}`, `{contour_interval}`, …), the source lines of the map and the credit line "Created with freemapper-print" at the bottom left. The credit line cannot be switched off; an element only moves it. At most six source lines are shown (the rest is counted) and long ones are cut to the frame width. A sheet that shows neither scale nor contour interval gets a note.
- The event project file `freemapper-print.toml` is read and checked: event, map source, shared sheet elements and any number of layouts. Unknown keys report file, line and name.
- Program texts come in German and English: help, summary, notes, warnings and errors. The language comes from
  `--lang de|en` (before or after the command), then `FREEMAPPER_LANG`, `LC_ALL`, `LC_MESSAGES` and `LANG`. The
  texts live in Fluent files under `crates/freemapper-print-cli/i18n/`. Error messages from the library and
  clap's headings stay English for now.

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
