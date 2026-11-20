# 0075. A forest example on a generated map

Date: 2026-11-20
Status: Accepted, amends [0064](0064-examples-on-two-released-maps.md)

## Context
The examples stand on Remo Madella's two sprint maps (ADR 0064). Training views (ADR 0071) are most useful on a
forest map: contour orienteering needs relief, ultimate orienteering needs paths to leave out. No finished forest map
from OCAD is at hand. freemapper generates a forest map for each Bundesland (ISOM 2017-2, 1:10 000), from
OpenStreetMap and the open terrain model of the state.

## Decision
- The example `examples/siebengebirge-views` uses the generated map "Siebengebirge" (freemapper example
  `nordrhein-westfalen-siebengebirge`, commit 02275869, data fetched 2026-11-16), copied unchanged. Its licences are
  ODbL (OpenStreetMap) and dl-de/zero-2-0 (Geobasis NRW). The example README and `ATTRIBUTION.txt` say that the map is
  generated and has not been checked in the field.
- The area was chosen by counting, on the 15 generated forest maps, contour objects and path objects (502 to 507).
  Siebengebirge has 296 and 164; flat maps (Bürgerpark 62 and 300, Dammheide 18 and 196) leave nothing to read from
  contours, Cloef (232 and 43) has too few paths, and Bastei (299 and 210) is dominated by rock and the Elbe.
- Three layouts on one extract: `vollstaendig`, `isohypsen` (`only:101,102,103,601.1`) and `ultimate`
  (`no-paths`), A4 landscape at 1:7500, a scale of the forest standard, so the map grows to 133 % with its symbols.
  The list for contour orienteering names `601.1`, because a list names exact codes and `601` does not take in the
  north line symbols 601.1 to 601.4 (the maps of freemapper use `601.1`).
- The manual describes training views in chapter 6 with this example as the figure.

## Consequences
- The examples now also show a forest map, until a finished one from OCAD replaces it.
- Cost if wrong: a reader may take the generated paths for surveyed ones; the README says where they come from. The
  example adds about 1.3 MB (map 0.7 MB, PDFs 0.34 MB, preview 0.42 MB).
