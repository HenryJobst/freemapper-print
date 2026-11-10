# 0047. Generated masks

Date: 2026-11-10
Status: Accepted

## Context

Masks were hand-written (ADR 0045). User decision R4: windows are circles around controls; a corridor has no rule.

## Decision

- `masks window` centres a circle (48 vertices or more, chord error at most 0.05 print mm) on each selected control:
  all, codes, or printed numbers (`3-6`). The start and the finish get a window by default, whatever the
  selection; `--no-start` and `--no-finish` leave them out (ruling R13). Runners orient at the start, and the
  common use must not make `check` warn. Holes may overlap and unite.
- `masks corridor` offsets the user's polyline (map mm, y up) half the width to each side: flat ends, mitre at most
  twice the width (SVG `stroke-miterlimit` 2, turns up to 120°), bevel beyond. A turn back, a segment too short for its
  inner corners, or crossing edges: refused (simplify or narrow it), never repaired. No automatic corridor.
- Radius and width are print mm, like a gap, so a window looks the same at every scale; the print scale of the
  course's layout turns them into map mm, and the mask keeps the map mm. After a change of print scale the user
  deletes the mask and runs the command again; the help, the manual and the comment above the table say the scale. Without `--course` a corridor needs one scale for all layouts. Range 1 to
  100 mm. Defaults: radius 15 mm, a window 30 mm across, about 120 m of ground at 1:4000, the detail around a
  control; width 10 mm, 40 m at 1:4000, a street with its edges. The manual has no numbers to compare. The outline is
  a rectangle 100 map mm outside the map's object points and the holes; points round to 0.001 mm.
- Messages name the start and the finish by kind ("Start", "Ziel"; "start", "finish"), not by `STA1` and `FIN1`,
  which Purple Pen does not show (ruling R13): the output, the comment in the project file and `check`. The codes
  stay where the user types them (`--controls`, `--radius-for`); the list of an unknown control gives them in
  brackets. A self-crossing corridor is reported at the point where two of its edges meet.
- `project::append_mask` shares `append_correction`'s write path (ADR 0041): the table goes to the end as text with a
  comment on how it was made; `--assign` adds `mask = "<id>"` under the course's `[[assignments]]` header, or a new
  assignment. The file must read back as written, else the old content returns (exit 1). Exit 2: a taken id, a course
  with another mask, an assignment not written as a table, or a new assignment next to an inline
  `assignments = [...]`, where a new table would not load. Ids `window-<course>`, `corridor-<n>`, made unique.

## Consequences

With `--no-start` or `--no-finish`, `check` names the stop under the mask, and the command says so. Cost if wrong:
windows a quarter smaller at 1:3000 on a 1:4000 map for a user who thinks in map mm. Source: M4 plan Task 4; R4, R5,
R13.
