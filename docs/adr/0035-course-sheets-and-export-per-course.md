# 0035. Course sheets and export per course

Date: 2026-11-09
Status: Accepted

## Context

M3 prints one sheet per course (spec §5.7). A course takes the layout its assignment names, or the first layout
(ADR 0025). Course names come from course-setting programs and can hold characters a file name cannot, such as
`D/H 12` or `Bahn 1: lang`, umlauts in NFD from macOS, or two names that differ only in case. Clubs print a
training on one printer run, so one PDF with every course is useful too. M2 exports and checks layouts, one page
in memory at a time.

## Decision

- `render_course_sheet` draws a course on a layout through the same body as `render_sheet`. Without
  `centre_map_mm` the frame centres on the box around the course's stops and leg bends. Each special object that
  applies to the course widens the box, in file order, only while the widened box still fits in the frame, so an
  out-of-bounds area for all courses far away does not pull the course off the sheet (white-out does not count
  until M4). The north-line second pass keeps that centre.
- On a course sheet the map reaching past the frame is expected: it is the Info note `MapCroppedToCourse`, as
  `crop = true` makes it on a layout. Controls outside the frame stay a warning.
- The overprint goes on the page after the map, with the numbers where the course file puts them, else due east
  of the circle at 1 mm. Task 10 of M3 replaces the number placement.
- Course placeholders: `{course}`, `{length_km}` (one decimal, comma on German sheets), `{climb_m}` (whole metres)
  and `{controls}` (numbered stops, so start, finish and crossing points do not count). The length is the file's
  when it gives one; otherwise the legs through their bends, marked routes included, map mm × map scale ÷ 1000.
  A score course without a length in the file has none. An empty `{climb_m}` or `{length_km}` is an Info note.
  The four are always known placeholders: a sheet without a course fills them with nothing and notes it as Info.
- Course sheet notes: controls outside the frame or across its edge, the default layout (Info), no description
  box (Info for IOF XML and for layouts without a box element, a warning where a `.ppen` box cannot be drawn;
  ADR 0037), and an overprint
  cut short by the repeat limit. The notes about the course file
  as a whole (reader, fit, stale entries) are `SheetNote`s too, but they come once per run from
  `course_event_notes`, before the courses, not on every sheet. A course note is Info unless something is left
  out of the print: white-out, alternative controls, variations and unknown elements.
- File names: `course_file_stem` precomposes the Latin-1 letters by hand (no Unicode crate, ADR 0006), replaces
  `/ \ : * ? " < > |` and control characters with `-`, trims dots and spaces, cuts the stem to 150 bytes, turns an
  empty stem into `course`, appends `-` to a Windows device name (the part before the first dot, without trailing
  spaces: `con.1` becomes `con-.1`), and makes the stem unique ignoring case with ` (2)`, ` (3)` and so on. Stems
  are made over all courses in file order, so `--course` keeps the names. The single file is the event title's
  stem plus " - alle Bahnen" (German sheets) or " - all courses" (English), unique against the course stems.
  Course names in messages stand in quotes („…“ in German, "…" in English).
- With no course to export (the reader left every course out), `export` writes nothing, not even a PDF without
  pages, and says so.
- Export with a course file writes one PDF per course; `--course` (repeatable) picks courses, `--single-file`
  writes all into one PDF. `--layout` exports layouts without courses and cannot be combined with the two. An
  unknown course is a usage error (exit 2) that lists the known ones. Without a course file, `export` and `check`
  work as in M2, byte for byte.
- The single file goes through `MultiPagePdf`: each page becomes PDF objects at once and is dropped, so one page
  is held at a time and only the PDF bytes grow. Each page embeds its own font subsets and images. `write_pdf`
  keeps its object order, so one-page PDFs stay as they were.
- `check` with a course file prints the course file notes, then every course on its layout, and exits with 1 when
  the file or a course has a note that counts.

## Consequences

- A sheet per course needs no layout per course, and a filtered export writes the same file names as a full one.
- The single file repeats fonts per page; a PDF of 30 courses is larger than it needs to be.
- Cost if wrong: the stem rules change file names users have printed from; the event-level notes move onto
  sheets.

Source: M3 plan Task 7 and the controller rulings for it (numbers, notes, CLI, single file, stems, frame,
length).
