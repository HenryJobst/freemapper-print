# freemapper-print

[Deutsche Version](README.md) · [User manual](docs/manual/en.md) · [Glossary](docs/glossary.md) · [Changes](CHANGELOG.en.md)

freemapper-print turns a finished orienteering map and the courses from a course-setting program into
ready-to-print sheets, one sheet per course. It is the counterpart to
[freemapper](https://github.com/HenryJobst/freemapper): freemapper produces the starter map, the mapper draws in
OpenOrienteering Mapper (OOM), and freemapper-print puts the finished map on paper.

What it does already:

- print a finished OOM map (`.omap`) as a vector PDF at the exact print scale, on A4 or A3;
- print at another scale with the symbol sizes the standard sets (for example 1:3000 for the youngest classes,
  symbols at 133 %);
- always print the map's north lines.

New in 0.2.0:

- one project file per event with any number of layouts, created with `init`, checked with `check` and written
  as PDFs with `export`;
- on every sheet a passe-partout, a frame line and a footer with the copyright and credit lines; as elements you
  place, a title block and texts with placeholders, club and sponsor logos (PNG, JPEG, SVG) and a scale check bar
  to measure against;
- north lines added when the map has none;
- `render` prints the copyright lines from the map notes and the credit line too;
- messages and help in German and English;
- a full disk leaves no half-written PDF.

Planned:

- courses from Purple Pen (`.ppen`) and from any program that writes IOF XML 3.0 (Purple Pen, Condes, OCAD course
  setting), with course overprint and control descriptions (M3);
- masks for window and corridor orienteering, in variants assigned to single courses (M4);
- use through an AI assistant (MCP, M5) and an editor in the browser (M6), OCAD maps later.

The program needs no OCAD and runs on macOS, Linux and Windows.

## Examples

Finished examples are in [`examples/`](examples/README.en.md). Click a picture to open the example with its PDFs.

<a href="examples/luebeck-competition-layouts/README.en.md"><img src="examples/luebeck-competition-layouts/preview.jpg" width="640"
alt="A city sprint in Lübeck on A4 at 1:4000 and on A3 at 1:3000, with title, logos and scale check bar"></a>

**One competition, two layouts**
([`luebeck-competition-layouts`](examples/luebeck-competition-layouts/README.en.md)): a city sprint with an A4
sheet at 1:4000 for the older classes and an A3 sheet at 1:3000 for the youngest, both with title block, club
and sponsor logo, scale check bar and footer. Built with `init`, `check` and `export` alone.

<a href="examples/luebeck-scales/README.en.md"><img src="examples/luebeck-scales/preview.jpg" width="640"
alt="The same sprint map of Lübeck old town at 1:4000 and 1:3000"></a>

**Lübeck at two scales** ([`luebeck-scales`](examples/luebeck-scales/README.en.md)): the same sprint map at 1:4000
on A4 and at 1:3000 on A3, printed with `render`. At 1:3000 every symbol grows to 133 % as ISSprOM requires.
Each milestone adds examples: courses from Purple Pen (M3), window and corridor orienteering (M4).

## Status

The current version is [v0.2.0](https://github.com/HenryJobst/freemapper-print/releases/tag/v0.2.0) of
9 November 2026 (see the [changes](CHANGELOG.en.md)). It prints a finished OOM map at the exact scale
(`freemapper-print render`) and turns a project file into finished sheets with layouts, title block, logos and
scale check bar (`init`, `check`, `export`). Courses from Purple Pen (M3) and masks for window and corridor orienteering (M4)
follow. The first version targets club training in towns and parks: sprint maps on A4, printed at home or in a
copy shop. More in the [design](docs/superpowers/specs/2026-11-08-freemapper-print-design.md), the
[glossary](docs/glossary.md) and the [architecture decisions](docs/adr/README.md).

## Installation

freemapper-print is a single program without an installer:

1. Download the archive for your system from the
   [release page](https://github.com/HenryJobst/freemapper-print/releases/latest):
   - Windows: `freemapper-print-…-windows-x86_64.zip`
   - macOS with Apple chip (M1 and newer): `freemapper-print-…-macos-arm64.tar.gz`
   - macOS with Intel processor: `freemapper-print-…-macos-x86_64.tar.gz`
   - Linux: `freemapper-print-…-linux-x86_64.tar.gz` (ARM computers: `linux-arm64`)
2. Unpack the archive. The folder contains `freemapper-print` or `freemapper-print.exe`.
3. On the first start the system warns because the program is not signed. On Windows choose "More info", then
   "Run anyway". On macOS click "Open Anyway" in "System Settings › Privacy & Security", or run
   `xattr -d com.apple.quarantine freemapper-print` in Terminal in the unpacked folder. On Linux, run
   `chmod +x freemapper-print` if needed.
4. Check in a terminal in the unpacked folder: `./freemapper-print --version` (Windows:
   `.\freemapper-print.exe --version`).

## Building from source (for developers)

```sh
git clone https://github.com/HenryJobst/freemapper-print
cd freemapper-print
cargo build --release
```

`rust-toolchain.toml` fixes the Rust version; `rustup` installs it on the first build. Before each commit,
[lefthook](https://github.com/evilmartians/lefthook) checks formatting and clippy (`lefthook install`).

## Usage

### An event

```sh
freemapper-print init citysprint --map luebeck.omap --title "Stadtsprint Lübeck"
freemapper-print check citysprint
freemapper-print export citysprint
```

`init` creates the project file `freemapper-print.toml` in the folder `citysprint`: one layout at the map's
scale on A4 or A3, a title block, a scale check bar and a proposed contour interval for you to check against the
map. In the file you add more layouts, logos and texts. `check` draws every layout without writing anything and
lists the notes: map too large for the map frame, passe-partout too narrow for the footer, logo unreadable, and so
on. `export` writes one PDF per layout into the folder `out` next to the project file, named after the layout id.
The [manual](docs/manual/en.md#5-setting-up-an-event) describes the project file key by key.

### The map alone

```sh
freemapper-print render luebeck.omap --out luebeck-3000-a3.pdf --paper a3 --scale 3000
```

This writes the sprint map as a PDF on an A3 page at 1:3000, without title or scale check bar. The main options:

- `--paper a4|a3` picks the paper (default A4), `--orientation auto|portrait|landscape` the orientation. With
  `auto` the program takes the orientation that cuts off less of the map.
- `--scale <number>` sets the print scale, for example 4000 for 1:4000. Without it the scale of the map applies.
  At a larger scale the symbols grow as the standard sets: a sprint map at 1:3000 to 133 %, a forest map at
  1:10000 to 150 %. A scale smaller than the standard's base (sprint 1:5000) keeps the symbols at their
  standard size, and the program warns that the print is for training only.
- `--kind sprint|schoolyard|forest` sets the map kind if the program can tell it neither from `freemapper.toml`
  nor from the map file.
- `--show-sheet-objects` also prints freemapper's information block: title, scale, scale bar and credits.
  North lines belong to the map and are always in the PDF.
- `--lang de|en` picks the language of the messages; without it `FREEMAPPER_LANG` and the system's
  language settings decide.

If the map does not fit on the page, the program writes the PDF anyway and says how many millimetres are
missing. It also lists objects it did not draw, or drew in simplified form.

Print the PDF at actual size (100 %) and switch "fit to page" off. Otherwise the scale is wrong. The manual
explains printing in the chapter
[Printing](docs/manual/en.md#9-printing-at-the-right-scale-at-home-and-in-a-copy-shop).

## With an AI agent

An MCP server that lets an AI agent print the maps comes in a later milestone. Until then everything runs on the
command line.

## Licence and credit

freemapper-print is licensed [GPL-3.0-or-later](LICENSE), like freemapper. It takes over code from freemapper
and may take over code from OOM (ADR 0002).

The program embeds the Arimo font (Arial-compatible, SIL Open Font License 1.1). Its licence is in the release
archives as `FONT-LICENSE.txt`.
