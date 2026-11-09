# 0029. Footer placement: below the frame when the whole block fits

Date: 2026-11-09
Status: Accepted

## Context

Every sheet carries the credit line, and it cannot be switched off (ADR 0025). Above it come the copyright
lines, one source per line, from `print.copyright` or the map notes. The first rule placed this footer below the
map frame when the bottom passe-partout was at least 6.5 mm wide. With two or more source lines the block then
grew upward into the frame line and the map.

## Decision

- The footer is one block of 5 pt black lines at the bottom left. Its last baseline lies 4 mm above the paper
  edge, its left edge on the frame edge.
- The block goes below the frame only when the whole block fits between the frame and that baseline, and the
  bottom margin is at least 6.5 mm. Otherwise it moves inside the frame to the bottom-left corner, on a white box
  with 0.8 mm padding that stays off the inner half of the frame line, and the sheet gets a note saying it covers
  part of the map.
- At most six copyright lines are shown: five sources and a sixth that counts the rest. A line wider than the
  frame is cut with "…". The program notes both.
- A `copyright_line` or `credit_line` element moves that part out of the footer to its own anchor. Neither
  element can remove the credit line.

## Consequences

- With the default 8 mm passe-partout, a map with one source keeps the footer below the frame. A map whose notes
  name four sources needs about 14 mm (Lübeck); `init` widens the passe-partout until the starter fits.
- Cost if wrong: on A4 with a narrow margin and two or more sources, the white box covers about 12 mm of the map's
  bottom-left corner.

Source: M2 plan, Task 6 and its fix round; ledger ruling of Task 6 (footer block inside the frame when it does
not fit below); user ruling 2026-11-08 (credit line on every sheet).
