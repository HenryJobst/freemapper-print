# 0047. Generated masks

Date: 2026-11-10
Status: Accepted

## Context

Masks were hand-written (ADR 0045). User decision R4: windows are circles around controls; a corridor has no rule.

## Decision

- `masks window` centres a circle (48 vertices or more, chord error at most 0.05 print mm) on each selected control:
  all, codes, or printed numbers (`3-6`); start and finish only by flag. Holes may overlap and unite.
- `masks corridor` offsets the user's polyline (map mm, y up) half the width to each side: flat ends, mitre at most
  twice the width (SVG `stroke-miterlimit` 2, turns up to 120°), bevel beyond. A turn back, a segment too short for its
  inner corners, or crossing edges: refused (simplify or narrow it), never repaired. No automatic corridor.
- Radius and width are print mm, like a gap, so a window looks the same at every scale; the print scale of the
  course's layout turns them into map mm. Without `--course` a corridor needs one scale for all layouts. Range 1 to
  100 mm. Defaults: radius 15 mm, a window 30 mm across, about 120 m of ground at 1:4000, the detail around a
  control; width 10 mm, 40 m at 1:4000, a street with its edges. The manual has no numbers to compare. The outline is
  a rectangle 100 map mm outside the map's object points and the holes; points round to 0.001 mm.
- `project::append_mask` shares `append_correction`'s write path (ADR 0041): the table goes to the end as text with a
  comment on how it was made; `--assign` adds `mask = "<id>"` under the course's `[[assignments]]` header, or a new
  assignment. The file must read back as written, else the old content returns (exit 1). Exit 2: a taken id, a course
  with another mask, an assignment not written as a table. Ids `window-<course>`, `corridor-<n>`, made unique.

## Consequences

Without the start and finish flags `check` names both under the mask, and the command says so. Cost if wrong: windows
a quarter smaller at 1:3000 on a 1:4000 map for a user who thinks in map mm. Source: M4 plan Task 4; R4, R5.
