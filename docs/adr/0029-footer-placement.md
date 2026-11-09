# 0029. Footer placement: below the frame when the whole block fits

Date: 2026-11-09
Status: Accepted

## Context

Every PDF carries a visible credit line that cannot be switched off (ADR 0025), with the copyright lines from
`print.copyright` or the map notes above it. Placed by margin width alone, a tall block grew into the map.

## Decision

- The footer is one block of 5 pt lines at the bottom left, last baseline 4 mm above the paper edge. It goes below
  the frame when the whole block fits and the margin is at least 6.5 mm; else into the frame's bottom-left corner
  on a white box (0.8 mm padding, off the frame line), noted as covering the map, and `FooterCoversElement` names
  any element the box overlaps.
- At most six copyright lines: five and one counting the rest (`CopyrightTruncated`). A line wider than the white
  box is cut with "…" (`CopyrightLineCut`); widths sum glyph advances without kerning, as the PDF sets them.
- A `copyright_line` element moves the copyright lines; a `credit_line` element (at least 5 pt) the credit line,
  if wholly on the page, else it stays (`CreditLineMoved`). The credit line is painted after every other item.
- `render` gets the same footer from the same helper, sourced from the map notes, against its 5 mm frame, so it
  sits on the white box. The map does not move; a note says when the box covers map content.

## Consequences

- With 8 mm one source fits below the frame; Lübeck's four need about 14 mm, which `init` finds. `render` PDFs
  of OSM-based maps carry the attribution ODbL and CC BY require.
- Cost if wrong: on A4 with a narrow margin and two or more sources the box covers about 12 mm of the map.

Source: M2 Task 6 and its ruling; user ruling 2026-11-08; final review I1, I2, M5; Task 11 review.
