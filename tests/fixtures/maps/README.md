# Map fixtures

Real OpenOrienteering Mapper maps used by the reader tests (`crates/freemapper-print/tests/read_maps.rs`).

## luebeck.omap

- Source: freemapper, https://github.com/HenryJobst/freemapper, tag `v0.5.0`, commit `e8b6db9` (2026-11-09),
  `examples/schleswig-holstein-luebeck/schleswig-holstein-luebeck.omap`, copied unchanged.
- Licence: the map data come from © OpenStreetMap contributors (ODbL) and © GeoBasis-DE/LVermGeo SH
  (CC BY 4.0, https://creativecommons.org/licenses/by/4.0/, source modified), as the example's `ATTRIBUTION.txt`
  and the map's `<notes>` say. The file was written by freemapper (GPL-3.0-or-later).
- Changes against the previous copy (`a0cc8ef`): colours after IOF Printing and Colour Definitions Rev. 4
  (freemapper ADR 0044); building fills 521.3 and 521.1 in "Black 50%" (priority 10) instead of "Black 60%"
  (priority 9); the nine north lines (601) moved from "peripheral information" (tag `freemapper:source=sheet`)
  to the start of "default part" (tag `freemapper:source=north-lines`, freemapper ADR 0043). Object
  coordinates are unchanged: 2173 objects, 2160 in "default part" (was 2151) and 13 in "peripheral
  information" (was 22). The map notes are unchanged.

## forest-sample.omap

- Source: OpenOrienteering Mapper, https://github.com/OpenOrienteering/mapper, tag `v0.9.6`,
  `examples/forest sample.omap`.
- Licence: GPL-3 (OpenOrienteering Mapper).

## Renderer provenance

The render code in `crates/freemapper-print/src/render/` matches freemapper `743ce4b`. freemapper `a0cc8ef`
changed only tests there; the paint-order test `training_pdf_paints_water_under_paved_areas_and_buildings` in
`render/draw.rs` came from `a0cc8ef`.
