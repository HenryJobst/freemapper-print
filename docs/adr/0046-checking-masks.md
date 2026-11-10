# 0046. Checking masks

Date: 2026-11-10
Status: Accepted

## Context

A mask clips the map and never the course overprint (ADR 0045). A runner who meets a marked route or a control
where the mask hides the map sees purple on blank paper. On a window course a control without a window is a real
mistake. The M4 plan asks `check` to say so (review focus 1).

## Decision

- `render_course_sheet` checks the masks it applies (at most `MAX_APPLIED_MASKS`) and adds sheet notes:
  - `ControlInMaskedArea` (warning): the start, controls, map exchanges and finish whose centre lies where a mask
    hides the map, each code once, in course order.
  - `OverprintInMaskedArea` (warning), one note per kind with the count of objects: marked routes (a leg marked as
    one, sampled where it is drawn outside its circles, and marked-route specials), crossing points, forbidden
    routes, out-of-bounds boundaries, out-of-bounds areas, dangerous areas, and text specials.
  - `MaskLeavesNoWindow` (warning): one mask hides the whole map frame of a course with legs.
  - `MaskCoversNothing` (info): the mask hides nothing in the frame.
  - `MaskCheckLimited` (warning): the check reached its budget, so part of the overprint was not checked.
- A point counts when it lies inside the map frame and any applied mask hides it (`mask::polygon_contains`, the
  test of `mask::contains`, in the internal frame). Paths are sampled at most every 1 mm of paper; an area counts by
  its outline and by a grid of points 2 mm apart inside it, so an area around a masked spot counts too. A text counts
  by the corner its box hangs on, a control by its centre.
- Not counted: plain legs, control numbers, first aid, refreshment points and plain lines. Legs that cross masked
  areas are the point of window orienteering, and numbers often stand on blank paper. Crossing points and
  boundaries are lower purple (ADR 0034), but the runner needs the map under them, so they count.
- One sheet takes at most 200 000 samples (`MAX_MASK_SAMPLES`) and 50 million steps (`MAX_MASK_CHECK_STEPS`): box
  tests, ring edges, points of an area. Controls come first, then legs, then specials in file order. A hostile route
  of 10 000 points over kilometres stops at the budget with `MaskCheckLimited`.
- The frame tests look at each mask on its own. A polygon whose outline crosses the frame hides part of it; a frame
  inside an outline is wholly hidden unless a hole reaches it, and hides nothing when it lies wholly in one hole. A
  frame covered only by the union of several overlapping holes counts as partly hidden, and a frame hidden only by
  several masks together gets no `MaskLeavesNoWindow`; `ControlInMaskedArea` still names every control.
- `MaskUnused` prints once per run with the notes of the course file (`SheetNote::Mask`), as information.

## Consequences

Sheets without a mask get none of these notes, and the examples still pass `check`. The notes never change the
PDF. Cost if wrong: one function (`sheet::mask_check::mask_notes`) decides what counts, and the CLI only prints it.

Source: M4 plan Task 3 and its review focus 1; ruling R8 (several masks per sheet).
