# freemapper-print

[Deutsche Version](README.md) · [User manual](docs/manual/en.md) · [Glossary](docs/glossary.md) · [Changes](CHANGELOG.en.md)

freemapper-print turns a finished orienteering map and the courses from a course-setting program into
ready-to-print sheets, one sheet per course. It is the counterpart to
[freemapper](https://github.com/HenryJobst/freemapper): freemapper produces the starter map, the mapper draws in
OpenOrienteering Mapper (OOM), and freemapper-print puts the finished map on paper.

Planned:

- the map from OOM (`.omap`) at the exact print scale as a vector PDF, OCAD maps later;
- courses from Purple Pen (`.ppen`) and from any program that writes IOF XML 3.0 (Purple Pen, Condes, OCAD
  course setting), with course overprint and control descriptions;
- passe-partout, title, north lines, club and sponsor logos, and a scale check bar to measure against;
- masks for window and corridor orienteering, in variants assigned to single courses;
- several layouts per event, for example 1:4000 for the older classes and 1:5000 for the younger ones, with
  symbols kept at the size the standard sets;
- use through an editor in the browser, the command line or an AI assistant (MCP).

The program needs no OCAD and runs on macOS, Linux and Windows.

## Status

Since milestone M1, freemapper-print prints a finished OOM map at the exact scale as a vector PDF
(`freemapper-print render`). Layouts with title and logos follow in M2, courses from Purple Pen in M3, and masks
for window and corridor orienteering in M4. The rest is in the
[design](docs/superpowers/specs/2026-11-08-freemapper-print-design.md), the [glossary](docs/glossary.md) and the
[architecture decisions](docs/adr/README.md). The first version targets club training in towns and parks: sprint
maps on A4, printed at home or in a copy shop.

## Building from source (for developers)

```sh
git clone https://github.com/HenryJobst/freemapper-print
cd freemapper-print
cargo build --release
```

`rust-toolchain.toml` fixes the Rust version; `rustup` installs it on the first build. Before each commit,
[lefthook](https://github.com/evilmartians/lefthook) checks formatting and clippy (`lefthook install`).

## Usage

```sh
freemapper-print render luebeck.omap --out luebeck-5000-a4.pdf --paper a4 --scale 5000
```

This writes the map as a PDF on an A4 page at 1:5000. The main options:

- `--paper a4|a3` picks the paper (default A4), `--orientation auto|portrait|landscape` the orientation. With
  `auto` the program takes the orientation that cuts off less of the map.
- `--scale <number>` sets the print scale, for example 4000 for 1:4000. Without it the scale of the map applies.
- `--kind sprint|schoolyard|forest` sets the map kind if the program can tell it neither from `freemapper.toml`
  nor from the map file.
- `--show-sheet-objects` also draws the north lines and the information block of the map.

If the map does not fit on the page, the program writes the PDF anyway and says how many millimetres are
missing. It also lists objects it did not draw, or drew in simplified form.

Print the PDF at actual size (100 %) and switch "fit to page" off. Otherwise the scale is wrong. A finished
example with the same pages at 1:4000 and 1:5000 is in
[examples/luebeck-scales](examples/luebeck-scales/README.en.md).
The manual explains printing in the chapter
[Printing](docs/manual/en.md#9-printing-at-the-right-scale-at-home-and-in-a-copy-shop).

## With an AI agent

An MCP server that lets an AI agent print the maps comes in a later milestone. Until then everything runs on the
command line.

## Licence and credit

freemapper-print is licensed [GPL-3.0-or-later](LICENSE), like freemapper. It takes over code from freemapper
and may take over code from OOM (ADR 0002).

The program embeds the Arimo font (Arial-compatible, SIL Open Font License 1.1). Its licence is in the release
archives as `FONT-LICENSE.txt`.
