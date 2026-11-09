# 0037. Control description box

Date: 2026-11-09
Status: Accepted

## Context

A course sheet needs the IOF control descriptions of its course (spec §5.3). ADR 0036 gives the symbols. Purple
Pen users know how Purple Pen lays out and draws the table, and the IOF specification leaves line widths and
fonts open. IOF XML has no descriptions. The box sits on the sheet like a title block, so it follows the M2
element rules (ADR 0028).

## Decision

- A layout element of kind `control_descriptions` draws the box. `cell_mm` sets the printed cell size from 4 to
  10 mm; without it the rule profile decides (6 mm in `iof-print`). The size does not change with the print
  scale.
- Rows and drawing follow Purple Pen's `DescriptionFormatter.cs` and `DescriptionRenderer.cs`, with the line
  widths and font sizes of `DescriptionAppearance` in `Appearance.cs`, all at commit `201417f9` (the commit of
  ADR 0036). Rows: event title (the project's), course row, start, one row per control (A number, B code, C to
  H symbols, column F text as text), finish. Score courses show only start and controls, with the code in B,
  an empty A and the number of controls in the course row. Instruction rows: finish 14.3 (or the file's 14.x)
  with the length of the leg into it, 14.1 when that leg is marked; crossing point 13.3; marked leg 13.2; leg
  into a map exchange control 13.5; map exchange at a control 13.5control with "0 m". Lengths are the leg
  through its bends, map mm × map scale ÷ 1000, rounded to 10 m with ties to even, as .NET rounds. Climb is
  rounded to 5 m.
- Lines are 5 % (thick) and 2.5 % (thin) of a cell with flat ends. A thick line runs above every row that is not a
  control row, after the start and after every third control row, and at the outer edges and after columns B
  and E.
- Text is Arimo in black. Purple Pen's fonts fall back to Arial at fixed sizes (em per cell): title, course row
  and lengths 0.56 bold, number 0.63 bold, code 0.56, column F 0.50 and split column F 0.42. Arimo has Arial's
  metrics, so these sizes apply unchanged. Too wide text shrinks to 95 % of the size that fits.
- The box has a white background and hangs on its anchor. A box that would reach past the page edge is left
  out with a warning, because a cut description could hide a control. Over the map frame, an element hung on
  the page gets the usual M2 note.
- An unknown symbol id leaves its cell empty and gives a warning that names the id.
- `NoDescriptionBox` informs only when the course comes from IOF XML or the layout has no box element. It warns
  when the box would leave the page or no control of the course has descriptions (ruling R25).

## Consequences

Sheets show the same table Purple Pen prints, and a Purple Pen update to the layout needs a check against this
ADR. Courses from IOF XML stay without descriptions until freemapper-print can enter them. The box has no text
column (Purple Pen's "symbols and text") and no key for custom symbols yet.

Source: M3 plan Task 9 and the controller rulings for it (cell size, layout, text, placement, unknown ids).
