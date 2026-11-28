# 0080. Paper sizes A6 to A2

Date: 2026-11-28
Status: Accepted

## Context
`Paper` knew A4 and A3, the formats of an office printer. ADR 0079 printed the paper frames A6, A5 and A2 as free
pages (`page_mm`), because the core had no such paper. The project file then said `page_mm = [148.0, 210.0]` where
the person means A5, and the program had to find the name again from the size to show it.

## Decision
- `Paper` has A6, A5, A4, A3 and A2 (freemapper-core ADR 0047, core v0.34.0). The project file takes
  `paper = "A6"` to `"A2"`, the command `render` takes `--paper a6` to `a2`, and the MCP tool `set_layout` and the
  editor offer the same five.
- A paper frame becomes a layout with its paper, never with `page_mm`. A layout that prints a frame already is found
  by the size of its page, so a project made with 0.14.0 (free page) and one made now (paper) count as the same.
- `page_mm` stays for every size that is none of the five. The starter project still tries A4, then A3.
- The note that the paper is larger than A3 now also holds for A2.
- The PDFs do not change.

## Consequences
- Projects written by 0.14.0 keep working; their `page_mm` is read as before.
- The pin of the core moves to v0.34.0.
