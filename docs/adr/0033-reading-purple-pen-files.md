# 0033. Reading Purple Pen files

Date: 2026-11-09
Status: Accepted

## Context

A `.ppen` carries descriptions, circle gaps, leg bends, gaps and special objects that IOF XML lacks. Purple
Pen's code (`EventDB.cs`, `petergolde/PurplePen` at `201417f9`) defines it; not all of it fits the model.

## Decision

- The reader follows Purple Pen's code, defaults and load-time fixes. Elements Purple Pen uses only for its own
  printing and editing are skipped; anything else, and every setting the overprint cannot honour (start
  number, label kind, score points, partial flagging), is noted and the file still reads. The format details
  are in the comment of `course/ppen.rs`.
- A control without a code gets the code of Purple Pen's IOF XML export (`STA1`, `FIN1`, `XCHG1`, `CROSS1`;
  `ISSUE1` for a map issue point, read as a start), so code-keyed corrections survive a switch between the two.
- A per-course kind (a map exchange at a normal control, IOF `CourseControl/@type`) goes to `Stop::kind`.
- A course with forks or loops is left out with the note `Variations`; the IOF XML export expands them.
- Circle gaps come from the course's circle-gap scale and stay in the file frame (y up). Number locations stay
  offsets. Description blocks are layout and are ignored with a note.
- Shared legs, controls and course lists are copied into every course that uses them, so one read is bounded
  as a whole: `MAX_TOTAL_POINTS` points and `MAX_TOTAL_TEXT_BYTES` of codes and names, besides the ADR 0019
  limits and `MAX_CODE_LEN`/`MAX_NAME_LEN` per string (both readers).

## Consequences

Forked `.ppen` events need the IOF XML export. The renderer turns circle gaps with the fit's axis, draws a
one-point forbidden route as a cross and starts leg gaps at the control centre.
