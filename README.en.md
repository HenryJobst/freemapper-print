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

freemapper-print is being planned. There is no working program yet, only the project skeleton, the
[design](docs/superpowers/specs/2026-11-08-freemapper-print-design.md), the [glossary](docs/glossary.md) and
the [architecture decisions](docs/adr/README.md). The first version targets club training in towns and parks:
sprint maps on A4, printed at home or in a copy shop.

## Building from source (for developers)

```sh
git clone https://github.com/HenryJobst/freemapper-print
cd freemapper-print
cargo build --release
```

`rust-toolchain.toml` fixes the Rust version; `rustup` installs it on the first build. Before each commit,
[lefthook](https://github.com/evilmartians/lefthook) checks formatting and clippy (`lefthook install`).

## Licence and credit

freemapper-print is licensed [GPL-3.0-or-later](LICENSE), like freemapper. It takes over code from freemapper
and may take over code from OOM (ADR 0002).
