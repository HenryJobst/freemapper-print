# 0010. Courses from IOF XML 3.0 and Purple Pen, masks owned by freemapper-print

Date: 2026-11-08
Status: Accepted

## Context

Courses come from Purple Pen, Condes or OCAD course setting. All three export IOF XML 3.0. Research on
2026-11-08 showed that IOF XML 3.0 has no control description columns C to H, no masks, no forbidden areas and
no leg bends, and that all three programs write the y axis upward against the standard. Purple Pen's `.ppen`
file is open XML and contains all of it.

## Decision

- IOF XML 3.0 is the common course import. The importer accepts both y directions.
- Purple Pen's `.ppen` is a second import with descriptions, special objects and bends. Its white-out objects
  become mask variants.
- Mask variants belong to freemapper-print. They are drawn or edited in the editor and assigned to courses,
  whatever the course source.

## Consequences

- Courses from IOF XML need their control descriptions from somewhere else (see the design, open question).
- The `.ppen` reader follows Purple Pen's format, which can change between Purple Pen versions.
- Cost if wrong: if Condes or OCAD users need descriptions from their own files, a third reader is needed.

Source: brainstorming 2026-11-08 (user: IOF XML 3.0 for Condes and OCAD; IOF XML research).
