# 0015. Course content stays in course setting, presentation belongs here

Date: 2026-11-08
Status: Accepted

## Context

Some details of a printed course can be set in Purple Pen and in freemapper-print alike, for example where a
control number stands. A number's best position depends on the print scale, the masks and the sheet elements,
which only freemapper-print knows. A marked route, on the other hand, changes the course length and the control
descriptions and is the same on every sheet.

## Decision

- What changes what the runner must do belongs to course setting and is only read here: controls, order, legs
  and bends, marked routes, crossing points, forbidden and out-of-bounds areas, control descriptions.
- What only changes how a sheet looks belongs to freemapper-print: control number placement, masks, map frame,
  sheet elements, and later gaps in circles and legs.
- Control number placement has five levels, the more specific winning: automatic, course file, per control,
  per layout, per course. Placements are stored as direction and gap in print millimetres.
- Automatic placement and `check` look for numbers that collide with the overprint, masks, sheet elements or
  dense map detail.
- freemapper-print never writes to the course file.

## Consequences

- A course setter can leave number placement to freemapper-print and fix only the cases `check` reports.
- Cost if wrong: a club that tunes numbers in Purple Pen for one scale sees them moved where a level 3 to 5
  entry or automatic placement overrides them.

Source: brainstorming 2026-11-08 (user: number placement in freemapper-print, per layout, check and automatic
placement; marked routes discussed).
