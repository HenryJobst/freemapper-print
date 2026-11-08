# 0015. Course setting decides what is the same on every sheet, freemapper-print the rest

Date: 2026-11-08
Status: Accepted

## Context

Some details of a printed course could be set in course setting and in freemapper-print alike, for example
where a control number stands. Whether a number hides a leg is knowledge about the course and the same on every
sheet. Whether it collides with a mask, a logo or a neighbouring control after a scale change shows only on the
finished sheet. A sibling course-setting application that replaces Purple Pen is planned.

## Decision

- What is the same on every sheet of a course belongs to course setting and is only read here: controls, order,
  legs and bends, marked routes, crossing points, forbidden and out-of-bounds areas, control descriptions and
  control number positions (central per control, overridable per course).
- What depends on the sheet belongs to freemapper-print: masks, map frame, sheet elements and corrections of
  number positions per layout or, as an exception, per sheet.
- freemapper-print places numbers itself only when the course file has no position, checks every number on the
  sheet and proposes corrections, but never moves a number silently and never writes to the course file.
- The placement algorithm is a module without dependencies on the rest of the program, so the course-setting
  application can share it. It is extracted into a shared crate later, like the renderer (ADR 0014).

## Consequences

- Positions travel with the course data (`.ppen`, IOF XML `MapTextPosition`).
- Cost if wrong: corrections in the project file go stale when course setting moves a number; `check` reports
  each such case.

Source: brainstorming 2026-11-08; user and the course-setting session agreed on the split.
