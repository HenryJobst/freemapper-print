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
  within ±2 000 000 mm; the checks of all masks share 100 million steps. `project::load` reads at most 16 MiB
  (`MAX_PROJECT_FILE_BYTES`) before parsing: the largest file within these limits takes about 4 MB, and parsing a
  35 MB file took 1.6 GB before the point limit could refuse it.
- The spec follows this ADR (rulings R7, R9): its sketch of §5.5 writes y up, and it describes the mask as a clip
  of the map with circular windows around the controls and a corridor drawn by hand.
- White-outs become `whiteout-1`, `whiteout-2` … in file order, for the courses their filter names, like other
  specials (R6). `mask_for` gives a course the assignment's `mask` alone, else each of its white-outs as a mask of
  its own, unchecked and unmerged; each clips the map, so the masked areas add up (R8). A `[[masks]]` entry with a
  white-out's id stands in for it. A missing `whiteout-<n>` is `Stale::Mask`, an unused mask the Info `MaskUnused`.
- A mask clips the map (ruling R5). The sheet carries one clip per mask in page mm (`MapClip`), with the nonzero
  rule: the frame ring counter-clockwise on paper, the outlines clockwise, the holes counter-clockwise. The map shows
  in the frame outside the outlines (winding 1), not inside an outline (0), and again in a hole (1) or where holes
  overlap (2). The writer sets the orientation from the signed area of each ring on the page, so a ring written
  either way clips the same.
- All clips of a sheet go into one block inside the frame's clip, one after the other: `q`, each mask's rings and
  `W n`, the map, `Q` (ruling R10). Successive clips intersect, so the map is hidden in the union of the masked
  areas, and overlapping white-outs need no polygon union (R8). The block is never nested deeper, however many
  masks: one `q` per mask stalled Ghostscript for minutes at about 10 000 white-outs. A sheet applies at most
  1000 masks (`MAX_APPLIED_MASKS`) in the order of `mask_for`; with more, the rest are left out and the sheet
  gets the warning `MasksCapped`.
- The course overprint, the sheet elements, the frame line and the footer are not clipped. The map before and after
  the lower purple each get the block, so the lower purple keeps its place among the map (R11, ADR 0034).
- The reader's note on white-outs is information now, because white-outs print.

## Consequences

Projects without masks print as before, byte for byte. `whiteout-<n>` names a place in the course file, not an
area (ruling R16): after a white-out is deleted or the order changes in Purple Pen, an assignment or a stand-in
`[[masks]]` entry silently means another area. The manual says so, and `check` prints an Info line
(`MaskNote::WhiteOutByNumber`) for each course with such a reference, with the centre of the area's box in map mm,
y up, so that a changed target shows. Amended by ADR 0048: since freemapper-core v0.4.x, `n` is Purple Pen's special-object id; the file position is
only the fallback. North lines added without a 601 symbol are sheet items and stay visible across masked paper, while the
map's own north lines are clipped with the map (ruling R14, documented in the manual). A sheet element over a
window gets a warning (ruling R15, ADR 0046). A known gap, accepted: a white-out ring that crosses itself
is not checked and clips by the nonzero rule, so a loop wound against the rest shows the map, where Purple Pen fills
it. Cost if wrong: masks copied from
the `.omap` XML (y down) mirror.

Source: M4 plan Tasks 1 and 2 and its change of 2026-11-10 (holes may overlap, replacing ruling R2); rulings R1, R5,
R6, R7, R8, R9, R10, R11, R14, R15, R16; the final review of M4.
