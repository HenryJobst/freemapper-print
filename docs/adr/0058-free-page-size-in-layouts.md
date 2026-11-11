# 0058. A layout can set a free page size

Date: 2026-11-11
Status: Accepted

## Context
Layouts knew A4 and A3. A map larger than A3 at map scale was cut at the frame, and the sheet could not be
made larger. freemapper-core v0.20.0 (core ADR 0029) lets the page builder take any page size.

## Decision
- A layout has an optional `page_mm = [width, height]`, in portrait, millimetres. When it is set it replaces
  `paper`; `paper` stays and keeps its default. The orientation applies to it as to a paper.
- Each side must lie between 10 and 5080 mm, the limits of the page builder. The project file is refused
  otherwise, with the key named. The passe-partout check uses the shorter side of the page.
- A page larger than A3 by area gets the existing note that an office printer will not take it.
- `set_layout` (MCP) takes `page_mm` as well; `null` goes back to the paper. The command line has no switch for
  it: `render` stays on A4 and A3, and a project file holds the layouts that need more.
- `Paper` stays a closed enum with `Eq`; the free size is a separate key, so no caller of `Paper` changes.

## Consequences
A free page is the layout's decision, not a new paper name. A layout with `page_mm` and `paper = "A3"` prints on
`page_mm`; the file holds both keys until the person removes one.
