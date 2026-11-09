# 0023. North lines belong to the map

Date: 2026-11-08
Status: Accepted

## Context

North lines are OOM symbol 601, the magnetic north line. The mapper draws them in OOM. Their colour priority
puts them under walls, fences and similar objects, and the mapper breaks them at small features such as
boulders and path ends. freemapper puts its north lines into the map part "peripheral information" with the
tag `freemapper:source=sheet`, together with its information block (991 to 994: title, scale, scale bar,
credits). freemapper-print hid every sheet object by default, so freemapper maps printed without north lines.

## Decision

- Objects of symbol 601 or a variant 601.x are map content. freemapper-print always draws them, also when
  they carry the sheet tag, and does not replace them. `--show-sheet-objects` only adds the information block.
- M1 does not generate north lines.
- Planned for M2 (layouts): when a map has no north lines, freemapper-print adds them. It spaces them as the
  standard sets for the print scale (ISSprOM 2019-2: 30 mm at 1:4000), aligns them to magnetic north from the
  map's georeferencing and draws them at the colour priority of symbol 601 in the map's own symbol set, so
  they lie under the same objects as in OOM.

## Consequences

- North lines the mapper drew print as drawn, with their breaks at small features.
- Cost if wrong: a map that uses 601 for something else prints those objects too. `print::is_north_line`
  decides by the symbol code.

Source: user decision 2026-11-08 (final fix wave of M1).

## Update 2026-11-09

Since v0.5.0 (its ADR 0043) freemapper puts the north lines at the start of "default part" with the tag
`freemapper:source=north-lines`; "peripheral information" holds only the information block. Nothing changes here:
601 objects print whatever their tag and part. One difference for layouts: hiding "peripheral information" no
longer hides the north lines of such a map. The test fixture follows v0.5.0; older maps with the lines in
"peripheral information" print as before.
