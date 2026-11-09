# 0029. Footer placement: below the frame when the whole block fits

Date: 2026-11-09
Status: Accepted

## Context

Every PDF carries a visible credit line that cannot be switched off (ADR 0025), with the copyright lines from
`print.copyright` or the map notes above it. Placed by margin width alone, a tall block grew into the map.

## Decision

- The footer is one block of 5 pt lines at the bottom left on a white box, last baseline 4 mm above the paper
  edge. It goes below the frame when the whole block fits and the margin is at least 6.5 mm (the box is white on
  white there, kept off the frame line); else into the frame's bottom-left corner (0.8 mm padding), noted as
  covering the map. Footer notes name what it holds (`FooterContent`) and any element it overlaps.
- At most six copyright lines: five and one counting the rest (`CopyrightTruncated`). A line wider than the white
  box is cut with "…" (`CopyrightLineCut`); widths sum glyph advances without kerning, as the PDF sets them.
- A `copyright_line` element moves the copyright lines; a `credit_line` element (at least 5 pt) the credit line,
  on its own white box, if wholly on the page, else it stays (`CreditLineMoved`). The credit line is painted
  after every other item, so no element painted before it can hide it.
- `render` gets the same footer from the same helper and the map notes, inside its 5 mm frame; the map does not
  move, and a note says when the box covers map content.

## Consequences

- With 8 mm one source fits below the frame; Lübeck's four need about 14 mm, which `init` finds. `render` PDFs
  carry the attribution ODbL and CC BY require. Cost if wrong: a narrow A4 margin costs ~12 mm of map corner.

Source: M2 Task 6 and its ruling; user ruling 2026-11-08; final review I1, I2, M5; Task 11 review.
