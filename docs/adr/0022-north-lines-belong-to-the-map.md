# 0022. North lines belong to the map

Date: 2026-11-08
Status: Accepted

## Context

North lines are OOM symbol 601, the magnetic north line. The mapper draws them in OOM. Their colour priority
puts them under walls, fences and similar objects, and the mapper breaks them at small features such as
boulders and path ends. ISSprOM 2019-2 asks for a spacing of 30 mm at 1:4000; ISOM sets its own. freemapper
puts its north lines into the map part "peripheral information" with the tag `freemapper:source=sheet`,
together with its information block (991 to 994: title, scale, scale bar, credits). freemapper-print hid
every sheet object by default, so freemapper maps printed without north lines.

## Decision

- Objects of symbol 601 or a variant 601.x are map content. freemapper-print draws them by default, also when
  they carry the sheet tag. `--show-sheet-objects` only adds the information block.
- freemapper-print never generates north lines of its own and never hides the map's north lines. Placing
  them, their spacing and their breaks are the mapper's job in OOM.
- A later milestone (M2) warns when a map has no north lines instead of generating them.

## Consequences

- North lines print as the mapper drew them, under the objects their colour priority puts on top.
- Cost if wrong: a map that uses 601 for something else prints those objects too. A code check in
  `print::is_north_line` decides it.

Source: user decision 2026-11-08 (final fix wave of M1).
