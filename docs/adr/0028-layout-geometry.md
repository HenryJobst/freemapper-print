# 0028. Layout geometry: map frame, passe-partout and anchors

Date: 2026-11-09
Status: Accepted

## Context

A layout puts the map and its sheet elements on one page (ADR 0025). The `render` command keeps a fixed 5 mm
margin (ADR 0020). A layout needed its own rules for where the map ends, how the frame line sits on that edge,
how elements find their place, and which overlaps the program reports.

## Decision

- The map frame is the page less the passe-partout, `margin_mm` (8 mm by default), the same on every side. The
  layout passes it to the renderer as `PrintOptions::frame_margin_mm` and does not compute a fixed box itself:
  `auto` orientation is decided inside `render_page`, so a box fixed beforehand would fit only one orientation.
  The map is clipped to the frame, the overflow is measured against it, and the frame centre is the page centre.
  `centre_map_mm` puts that map point there; without it the middle of what is drawn goes there.
- The frame line is `border_mm` wide (0.25 mm by default, 0 draws none), black, on the sheet layer and centred on
  the frame edge. Half of it covers the cut edge of the map, so no white hairline shows between map and line.
  The other half lies on the passe-partout.
- An anchor names the page or the map frame, one of nine positions, and `offset_mm = [x, y]` measured inward.
  From a left or top edge the offset counts right or down, from a right or bottom edge left or up, and from a
  middle position right or down. The element sits on the inner side of its anchor.
- `hide_parts` compares part names exactly, case included. A name that matches no part gives a note.
- An element that reaches past the page edge gets a note. An element hung on the page that covers part of the
  map frame gets a note too. An element hung on the map frame is meant to lie on the map and gets none.
- Paper larger than A3 gets a note, because most office printers stop at A3 and the IOF advises against it.

## Consequences

- The same project prints the same page in portrait and landscape without recomputing boxes.
- Cost if wrong: the frame line covers 0.125 mm of map at the edge with the default width. A margin wider than
  half the page leaves a frame of zero size and only a degenerate frame line.

Source: M2 plan, Task 5; ledger rulings of Task 5 (margin-based frame, frame line centred on the edge).
