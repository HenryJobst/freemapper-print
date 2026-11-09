# 0045. Mask variants

Date: 2026-11-10
Status: Accepted

## Context

Window and corridor orienteering hide most of the map (spec §4). Purple Pen white-outs wait for M4 (ADR 0033).

## Decision

- `[[masks]]` has `id` and `polygons`, each a list of rings of `[x, y]` points, the outline first. Points are map
  mm with y up, as OOM shows them and `centre_map_mm` takes them (ADR 0028, ruling R1), in either orientation. An
  id is unique, 1 to 64 characters, no control characters. An assignment's `mask` names one or `whiteout-<n>`.
- The masked area is the outline minus the union of its holes: the map is hidden there, a hole is a window. Holes
  may overlap. Refused with file:line: a hole not wholly inside its outline or touching it; outlines of one
  variant that overlap, touch or nest (also in a window, where overlapping windows would show the map through it);
  a ring that crosses or touches itself or lies on one line; values that are not finite. Limits (ADR 0019): 64
  variants, 256 polygons each, 4096 points per ring, 100 000 per file, 3 different points per ring, coordinates
  within ±2 000 000 mm; the checks of all masks share 100 million steps.
- White-outs become `whiteout-1`, `whiteout-2` … in file order, for the courses their filter names, like other
  specials (R6). `mask_for` gives a course the assignment's `mask` alone, else each of its white-outs as a mask of
  its own, unchecked and unmerged; each clips the map, so the masked areas add up (R8). A `[[masks]]` entry with a
  white-out's id stands in for it. A missing `whiteout-<n>` is `Stale::Mask`, an unused mask the Info `MaskUnused`.

## Consequences

Projects without masks print as before. Cost if wrong: masks copied from the `.omap` XML (y down) mirror.

Source: M4 plan Task 1 and its change of 2026-11-10 (holes may overlap, replacing ruling R2); rulings R1, R6, R8.
