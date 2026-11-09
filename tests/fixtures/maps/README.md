# Map fixtures

Real OpenOrienteering Mapper maps used by the reader tests (`crates/freemapper-print/tests/read_maps.rs`).

## luebeck.omap

- Source: freemapper, https://github.com/HenryJobst/freemapper, tag `v0.8.0`, commit `a2b41b4` (2026-11-09),
  `examples/schleswig-holstein-luebeck/schleswig-holstein-luebeck.omap`, copied unchanged. The map names its
  candidates as the template `templates/candidates.omap`; that file is not copied.
- Licence: the map data come from © OpenStreetMap contributors (ODbL) and © GeoBasis-DE/LVermGeo SH
  (CC BY 4.0, https://creativecommons.org/licenses/by/4.0/, source modified), as the example's `ATTRIBUTION.txt`
  and the map's `<notes>` say. The file was written by freemapper (GPL-3.0-or-later).
- 2281 objects: the base area alone in the part `base`, 2267 in "default part" (with the nine north lines) and
  13 in "peripheral information" (the information block).

### What changed from v0.5.0 to v0.8.0

The previous copy came from v0.5.0 (`e8b6db9`). 2160 of its 2173 objects are in the new file unchanged, with
the same symbol and coordinates, the nine north lines and the 13 sheet objects among them. Notes,
georeferencing and templates are unchanged.

- New: the base area (freemapper ADR 0069), one area of the new symbol 990 "Walkable area (base fill)" in the
  new colour "Lower brown 30% for walkable area" at priority 32, behind all others, in a new first part `base`
  with the tag `freemapper:source=base`. 33 colours (was 32) and 190 symbols (was 189).
- New: 27 open land areas (401, Yellow 100%, priority 30): parks and green spaces from ALKIS are yellow
  (freemapper ADR 0069). 183 objects carry `freemapper:source=alkis-nutzung` (was 156).
- New: 93 canopies (522, Black 20% at priority 11 with a Black 100% outline) from ALKIS, tag
  `freemapper:alkis` (freemapper v0.7.0, ADR 0061).
- Gone: 13 of the 16 unpaved footpaths (505.1). Compacted and fine gravel is no longer an unpaved path but a
  501.6 candidate (freemapper ADR 0073); 3 remain.
- The candidates of v0.8.0, among them 180 control sites (531 with `freemapper:control`) and 16 unreachable
  passages (522 with `freemapper:hidden=unreachable`), are only in `templates/candidates.omap`. The map file
  holds none of them.

Tests that changed with it:

| Test | Old | New | Why |
|---|---|---|---|
| `luebeck_object_count` (`tests/read_maps.rs`) | 2173 objects | 2281; parts `base` 1, "default part" 2267, "peripheral information" 13 | +1 base area, +27 401, +93 522, −13 505.1 |
| `luebeck_lower_purple_under_black_over_every_fill` (`src/overprint/tests.rs`) | painted priorities 2, 10, 14, 15, 16 | 2, 10, 11, 14, 15, 16, 30, 32 | 11 canopies, 30 open land, 32 base area; none has a canonical name, so the lower purple stays right under Black 100% (2) |

New: `tests/candidates.rs` checks that the map names `templates/candidates.omap` and holds no candidate tag and
no 531, and that two real candidates from freemapper's `candidates.omap` (a control site and an unreachable
passage) are drawable on their own but leave the page of the map unchanged when they lie next to it.

Unchanged: the overflow of 4.0 mm on A4 (the base area does not count, ADR 0044, and lies inside the map's
extent anyway), the Probedruck distances in `tests/fixtures/oom/README.md` (the buildings P and Q are the same),
and the OOM reference file name `luebeck-4000-a3-landscape.pdf` the ignored `oom_comparison` test asks for.

### Earlier: v0.5.0

Against `a0cc8ef`, v0.5.0 brought the colours after IOF Printing and Colour Definitions Rev. 4 (freemapper
ADR 0044), building fills 521.3 and 521.1 in "Black 50%" (priority 10) instead of "Black 60%" (priority 9), and
moved the nine north lines (601) from "peripheral information" (tag `freemapper:source=sheet`) to the start of
"default part" (tag `freemapper:source=north-lines`, freemapper ADR 0043).

## forest-sample.omap

- Source: OpenOrienteering Mapper, https://github.com/OpenOrienteering/mapper, tag `v0.9.6`,
  `examples/forest sample.omap`.
- Licence: GPL-3 (OpenOrienteering Mapper).

## Renderer provenance

The render code in `crates/freemapper-print/src/render/` matches freemapper `743ce4b`. freemapper `a0cc8ef`
changed only tests there; the paint-order test `training_pdf_paints_water_under_paved_areas_and_buildings` in
`render/draw.rs` came from `a0cc8ef`.
