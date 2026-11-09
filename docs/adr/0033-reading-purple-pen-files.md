# 0033. Reading Purple Pen files

Date: 2026-11-09
Status: Accepted

## Context

Purple Pen's own `.ppen` file carries more than its IOF XML export: control descriptions, circle gaps, leg
bends and gaps, flagged legs and special objects. freemapper-print reads it as a second course source next to
IOF XML 3.0 (ADR 0032 covers how both fit the map). The format has no published specification. Purple Pen's
reader and writer (`EventDB.cs` and its helpers on GitHub `petergolde/PurplePen`, read at commit
`201417f97bb69302e3e0db123f428870e0914515`) define it. Several things in it do not map one to one onto the
course model.

## Decision

- **The reader follows Purple Pen's code.** It reads the elements Purple Pen reads, applies Purple Pen's
  defaults (description standard 2004, print scale 1:15000) and its load-time fixes for old circle gaps. It
  skips the elements Purple Pen writes for its own printing and editing (punch patterns, print areas, labels,
  relay settings). Any other element is noted once by name and the file still reads.
- **Codes for controls without one.** Only normal controls have a code in Purple Pen. A start, finish, map
  exchange or crossing point gets the code Purple Pen's IOF XML export gives it, counted per kind in file
  order: `STA1`, `FIN1`, `XCHG1`, `CROSS1`. A map issue point reads as a start with the code `ISSUE1` and a
  note. The export leaves map issue points out. Number corrections are keyed by code, so a project keeps them
  when it switches between the `.ppen` and its IOF XML export.
- **The kind of a control can change per course.** `Stop::kind` holds it: a map exchange at a normal control
  (`map-exchange="true"`) or an IOF XML `CourseControl/@type`. The control keeps its own kind, so a control
  can be the finish of one course and a plain control of another.
- **Courses with variations are left out.** A Purple Pen course with forks or loops would need every
  variation expanded into its own course. The reader skips such a course and notes `Variations`; Purple
  Pen's IOF XML export writes each variation as its own course, which freemapper-print reads as a fork.
- **Circle gaps follow the course's print scale.** Purple Pen stores them per control and per scale. A course
  takes the entry for its print scale, or for the map scale or 1:15000 when the event scales course symbols
  that way. Angles stay as Purple Pen writes them: degrees counter-clockwise from east with y up.
- **Flagging.** A leg is a marked route when Purple Pen flags all of it: `flagging kind="all"`, a leg from a
  map issue point, a leg into a map exchange control and a leg into a finish with symbol 14.1. The model has
  no partly flagged leg, so such a leg is drawn as a normal leg and noted.
- **Number positions stay offsets.** `number-location` is the text centre relative to the control in map mm,
  like IOF XML `MapTextPosition`. The number placement step (M3 Task 10) turns it into a direction and a gap
  once circle and text sizes are known. A control's `all-controls-code-angle` places its code in the all-controls view only and is not read.
- **Special objects.** First aid, water, forbidden route (a single cross, read as a path of one point),
  boundary, line, out of bounds, dangerous area, white-out and text become `Special`s. A description block
  is sheet layout, which freemapper-print decides, so it is noted as ignored. The kinds the model has no
  variant for (optional crossing point, registration mark, temporary construction, rectangle, ellipse,
  image) are noted by name. Course id 0 in a special's course list is Purple Pen's all-controls view and
  names no course.
- **Limits.** The reader applies the limits of the IOF reader (ADR 0019): file size, nesting depth,
  controls, courses, stops per course, points per path, special objects and notes. Course-controls and legs
  are capped at courses × stops per course. A `next` chain that loops is an error.

## Consequences

- The model's `Description` gains `column_f_text` for the dimensions Purple Pen writes as text in column F.
- Circle gap angles are in the file's axis frame. The renderer has to read them in the orientation the fit
  chose for the file.
- A forked event printed from the `.ppen` misses its variation courses until the reader expands variations.
  The note names each such course, and the IOF XML export is the way around it.
