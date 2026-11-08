# 0019. Limits for broken and hostile files

Date: 2026-11-08
Status: Accepted

## Context

The CLI and the planned MCP server read map files from other people. Reviews found `.omap` inputs that hung
the renderer, used 1 GB for 20 objects or overflowed the stack. The final review of M1 found two more: a
combined symbol that fills one large area 1000 times (a 260 KB file peaked at 906 MB, three such objects at
2.7 GB), and a pattern inside a pattern whose outer outline was copied for every line (a 246 KB file peaked
at 1.6 GB). Real maps stay far below every limit: Lübeck (2173 objects) draws about 4400 primitives, the
forest sample (539 objects) about 1500.

## Decision

- XML nested deeper than 256 levels is an error in every reader (`MAX_XML_DEPTH`), and so is a bad number.
  A float attribute of an object, the georeferencing or a pattern must be finite, and a box size must not be
  negative.
- Area patterns with a spacing or point distance below 0.05 mm or a non-finite number are dropped when the
  symbols are read (`MIN_PATTERN_STEP`).
- Each object draws within 250 000 units of work (`MAX_UNITS_PER_OBJECT`). A primitive costs one unit plus
  one per 8 points it keeps (`KEPT_POINTS_PER_UNIT`). A clip outline costs one unit per 8 points, rounded up,
  once per colour it clips, because the PDF writes it once per run of that colour. A step that draws nothing
  costs one unit, and copying or testing coordinates costs one per 64 points.
- Symbol nesting stops at depth 8 (`MAX_DEPTH`). A part with over 100 000 dashes is drawn solid, and one with
  as many mid symbols drops them (`MAX_REPEATS`).
- A page has 500 000 units (`MAX_UNITS_PER_PAGE`). Each object takes the units it used from the page, and
  the objects after the page runs out are not drawn. Clipped shapes keep sharing their outlines when the
  page is moved into place, and the PDF writes each outline once per run, also inside another clip.
- The report counts what the limits changed: objects cut short (`truncated_objects`), objects not drawn for
  the page limit, objects whose symbol lost a pattern, objects of unknown type. The CLI prints a note for
  each, and map text reaches the terminal with control and format characters escaped.

## Consequences

- A page keeps fewer than four million points. The worst crafted file measured (500 000 strokes of seven
  points) peaks at about 200 MB and renders in under a second (release build, macOS). The review's files now
  peak at 84 MB (one object) and 164 MB (three), the nested patterns at 6 MB.
- Lübeck uses about 6500 units at every allowed scale and the forest sample at most about 6400, about 1.3 %
  of the page.
- Cost if wrong: an extreme but real map loses detail. The notes name the count, and the constants can grow.

Source: security reviews of M1 Tasks 4, 5 and 8, final review of M1 and its security review, controller
rulings 2026-11-08.
