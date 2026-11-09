# 0028. Layout geometry: map frame, passe-partout and anchors

Date: 2026-11-09
Status: Accepted

## Context

A layout puts the map and its elements on one page (ADR 0025); `render` keeps a 5 mm margin (ADR 0020).

## Decision

- The map frame is the page less `margin_mm` (8 mm by default, below half the shorter paper side) on every side,
  passed as `PrintOptions::frame_margin_mm`, because `auto` orientation is decided inside `render_page`. The map
  is clipped to the frame, overflow is measured against it, and its centre is the page centre.
- `centre_map_mm = [x, y]` puts that map point there, in the coordinates OOM's status bar shows: mm, y pointing up
  (`MapWidget::updateCursorposLabel` prints `x` and `-y`). The sheet negates y for the file's y-down coordinates.
- `map_frame.crop = true` marks a cut as intended: the overflow becomes the note `MapCropped`, which only informs.
- The frame line (`border_mm`, 0.25 mm by default, at most 20, 0 for none) is centred on the frame edge: half
  covers the map's cut edge, so no white hairline shows.
- An anchor is the page or the map frame, one of nine positions, and `offset_mm` measured inward: from a left or
  top edge right or down, from a right or bottom edge left or up, from a middle right or down.
- `hide_parts` matches names exactly, case included. Notes: an unknown part there; an element past the page
  edge; one hung on the page over the frame; paper above A3; no `scale_check_bar` (the program adds none).

## Consequences

- One project prints the same page in portrait and landscape, and a point read off OOM centres the frame.
- Cost if wrong: the frame line covers 0.125 mm of map at the edge with the default width.

Source: M2 plan Task 5 and its ledger rulings; final review M4 and M6; Task 11 review (intended crop).
