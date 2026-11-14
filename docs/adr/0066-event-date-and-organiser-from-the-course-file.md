# 0066. Event date and organiser from the course file

Date: 2026-11-14
Status: Accepted

## Context
`{date}` and `{organiser}` in the title block come from `[event]` in the project file. freemapper-courseplan 0.17.0
writes both into its IOF XML export (`Event/StartTime/Date`, `Event/Organiser/Name`), so a person who set them once
in the course-setting program typed them a second time for the print. freemapper-core v0.27.0 (ADR 0034) reads them
into `CourseSource::event_date` and `CourseSource::organiser`.

## Decision
- `init --courses` copies the two values into `[event]` of the new project file. The date must read as a plain
  `YYYY-MM-DD`; a value with a time or an offset, or text that is no date, is left out rather than guessed.
- Only `init` copies them. The project file stays the one place the sheet reads, so a later change in the course
  file does not alter a printed sheet by itself; the person edits `[event]` or runs `init` again in a new folder.
- A `.ppen` file has neither field, so `init` leaves both unset for it, as it does for an IOF XML file without them.
- freemapper-print pins freemapper-core v0.27.0 for this.

## Consequences
- A project made before this version keeps its `[event]` as it is.
- Cost if wrong: a date the course program wrote in another form is silently not taken; the person types it.
