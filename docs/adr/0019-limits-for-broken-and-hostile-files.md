# 0019. Limits for broken and hostile files

Date: 2026-11-08
Status: Accepted

## Context

The CLI and the planned MCP server read map files from other people. Reviews found `.omap` inputs that hung
the renderer, used 1 GB for 20 objects or overflowed the stack. Real maps stay far below these limits:
Lübeck (2173 objects) draws about 4400 primitives, the forest sample (539 objects) about 1500.

## Decision

- XML nested deeper than 256 levels is an error in every reader (`MAX_XML_DEPTH`), and so is a bad number.
- Area patterns with a spacing or point distance below 0.05 mm or a non-finite number are dropped when the
  symbols are read (`MIN_PATTERN_STEP`).
- Each object draws within 250 000 units (`MAX_PRIMITIVES_PER_OBJECT`): one per primitive, one per step that
  draws nothing, one per 64 points copied or tested. Symbol nesting stops at depth 8 (`MAX_DEPTH`). A part with
  over 100 000 dashes is drawn solid, one with as many mid symbols drops them (`MAX_REPEATS`). A page holds at most 2 000 000 primitives
  (`MAX_PRIMITIVES_PER_PAGE`); the objects after that are not drawn.
- The report counts what the limits changed: objects cut short (`truncated_objects`), objects not drawn for the
  page limit, objects whose symbol lost a pattern, objects of unknown type. The CLI prints a note for each, and
  map text reaches the terminal with control and format characters escaped.

## Consequences

- A hostile file costs seconds and a few hundred megabytes at most, and the user learns what is missing.
- Cost if wrong: an extreme but real map loses detail. The notes name the count, and the constants can grow.

Source: security reviews of M1 Tasks 4, 5 and 8, final review of M1, controller rulings 2026-11-08.
