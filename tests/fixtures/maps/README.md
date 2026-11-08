# Map fixtures

Real OpenOrienteering Mapper maps used by the reader tests (`crates/freemapper-print/tests/read_maps.rs`).

## luebeck.omap

- Source: freemapper, https://github.com/HenryJobst/freemapper, commit `a0cc8ef` (2026-11-08, after the `water` branch merge),
  `examples/schleswig-holstein-luebeck/schleswig-holstein-luebeck.omap`.
- Licence: GPL-3.0-or-later (freemapper). The data sources of the map content are named in the
  map's `<notes>` element.
- The map now has water (symbols 301.1 and 301.6). The previous copy (`743ce4b`) had none.

## forest-sample.omap

- Source: OpenOrienteering Mapper, https://github.com/OpenOrienteering/mapper, tag `v0.9.6`,
  `examples/forest sample.omap`.
- Licence: GPL-3 (OpenOrienteering Mapper).
