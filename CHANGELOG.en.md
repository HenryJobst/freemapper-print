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
  draws the north lines and the information block. The program says how many millimetres of the map do not
  fit on the page. It also reports objects it did not draw, simplified or cut short, substituted fonts and
  missing glyphs, and it reminds you to print at actual size. Text from the map reaches the terminal with
  control characters escaped.
