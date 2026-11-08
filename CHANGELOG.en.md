# Changes

[Deutsche Version](CHANGELOG.md) · [User manual](docs/manual/en.md)

All notable changes to freemapper-print are listed here, newest first. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Project skeleton modelled on freemapper: Rust workspace with a library and the `freemapper-print` program
  (only `--version` and `--help` so far), CI for Linux, macOS and Windows, release workflow.
- Design, glossary and the first 15 architecture decisions.
- `freemapper-print render <map> --out <pdf>` prints an OOM map as an exact-scale PDF. You choose the paper
  (A4, A3), the orientation and the print scale, `--kind` sets the map kind, and `--show-sheet-objects` also
  prints freemapper's information block. North lines belong to the map and are always in the PDF. The program says how many millimetres of the map do not
  fit on the page. It also reports objects it did not draw, simplified or cut short, substituted fonts and
  missing glyphs, and it reminds you to print at actual size. Text from the map reaches the terminal with
  control and format characters escaped, and so do error messages.
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
