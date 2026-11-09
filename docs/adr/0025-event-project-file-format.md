# 0025. Event project file format

Date: 2026-11-08
Status: Accepted

## Context

ADR 0008 says one project file serves the editor, the CLI and the MCP server. M2 needs its schema: event, map
source, shared sheet elements and layouts.

## Decision

- The file is TOML, named `freemapper-print.toml`, and starts with `format = 1`. Any other value is an error
  that names the supported version.
- Paths inside the file (map, logos) are relative to the file's directory, so a project folder can move.
- A project has any number of layouts, at least one. There is no upper limit.
- All tables use `deny_unknown_fields`. A misspelled key is an error with file, line, column and key name
  instead of a silently ignored setting.
- Sheet elements are one flat table with a `kind` and optional fields per kind. Required fields per kind are
  checked after parsing (a logo needs `file` and `width_mm`).
- Ids of layouts are unique. Ids of shared elements and of a layout's own elements are unique together.
  `hide_shared` names existing shared ids.
- Every sheet carries the credit line naming freemapper-print and its project URL. There is no switch to turn it
  off; the `credit_line` element kind only places it, and a default footer position applies when no element does.
- `init` writes the file with `save_new`, which refuses to overwrite.
- The `toml` crate rewrites the file without comments. The editor (later) must keep the user's comments and
  layout, so it switches to `toml_edit` for saving. Until then only `init` writes the file.

## Addendum: `init`, `check` and `export` (M2 Task 10)

- Layout ids name the PDF files (`<id>.pdf`), so the loader accepts only ids that are plain file names on Linux,
  macOS and Windows: no `/ \ : * ? " < > |` or control characters, no leading or trailing dot or space, no Windows
  device name, at most 200 bytes. Two ids that differ only in upper and lower case are an error, because their
  PDFs would be one file on macOS and Windows.
- `footer` is the id the default footer has in sheet notes, so no element may take it.
- `init` finds the map kind as `render` does (ADR 0021): `--kind`, else `map_kind` in `freemapper.toml` next to
  the map, else the symbol set name and scale. A `freemapper.toml` that cannot be read or parsed gets a note that
  says so. Only when the whole chain finds nothing does `init` stop and ask for `--kind`.
- `init` refuses a `--title` with control or invisible formatting characters (usage error). As a second guard,
  `save_new_with_comments` reads the commented text back and writes the file without comments if it does not give
  the same project, because a multi-line string can fool the line-based comment placement.
- `init` writes the map path relative to the project file (canonical paths on both sides, `/` between the parts
  on every system), the event title from `--title` or the map's file name, the sheet language from the program
  language, and one layout with the map scale written out. Its shared elements are a title block with `{event}`
  and `1:{print_scale} · {contour_interval}` and a scale check bar, both in corners of the map frame. Copyright and
  credit lines stay with the default footer: it moves inside the frame on a white box when the passe-partout is too
  narrow, while an element hung on the page cannot and would cover the map. A comment in the file says how to move
  them with elements.
- `fit_starter` picks paper and passe-partout through the same drawing as `check`, with auto orientation: A4 with
  8 mm, A4 with a wider passe-partout (see the addendum below; tried only while the footer is the one problem),
  A3 with 8 mm, A3 the same way. It keeps the first candidate whose only notes are added north lines and, for
  schoolyard maps, the missing contour interval. So the starter passes `check` whenever an A4 or A3 starter can
  (Lübeck: A3 with 14 mm). Otherwise the file keeps A4 with 8 mm, and `init` prints the notes `check` will report.
- The proposed contour interval (sprint 2 m, forest 5 m, schoolyard none) gets a comment in the file, in the
  program language, telling the user to confirm it; `init` prints the same hint. An empty `[print]` table is left
  out.
- The case check on layout ids compares lower-case forms without Unicode normalisation (no extra dependency). Ids
  with combining accents (U+0300 to U+036F) are refused instead, so `é` written as e plus accent cannot slip past
  it. Other canonically equivalent spellings are not caught; such ids are unlikely in a hand-written file.
- The test hooks (a fake free-space value from `FREEMAPPER_PRINT_TEST_FREE_SPACE`, the live-page counter of the
  export loop) exist only with the library feature `test-hooks`, which only the CLI's dev-dependencies enable.
  Release binaries do not contain them.

## Addendum: final review of M2 (2026-11-09)

- Keys that do not belong to an element's kind are errors, never ignored. Numbers are checked after parsing: sizes
  and widths larger than 0, and upper bounds that keep absurd values out of the PDF: `size_pt` at most 200 (a
  `credit_line` at least 5, so it stays readable), `width_mm` and each `offset_mm` within ±2000, `border_mm` at
  most 20, `margin_mm` below half the shorter paper side, `contour_interval_m` at most 100, `centre_map_mm`
  within ±2 000 000. A missing logo file is a sheet note, not a load error, so the rest of the sheet prints.
- An error about a value names the line and column of its field, or of the table above it when the field is
  missing. Errors without a field (no layout) name the file alone.
- `init` proposes 2 m for sprint maps (ISSprOM 2019 and freemapper both use 2 m), 5 m for forest maps, none for
  schoolyard maps. It prints where the map kind came from, which equals `render`'s chain.
- `fit_starter` widens the passe-partout in 2 mm steps up to 24 mm. It tries 24 mm right after 8 mm; when the
  footer is still in the way there, no width helps and the next paper follows. At most ten sheets per paper.
- `map_frame.crop = true` marks an intended crop (ADR 0028).

## Addendum: courses, assignments and number corrections (M3 Task 5)

- `[sources]` takes `courses`, the path of the course file relative to the project file. The reader follows the
  extension, in any case: `.ppen` is Purple Pen, `.xml` is IOF XML 3.0. Another extension is an error that names
  the file. The file is read again at every `check` and `export`, after its size passed the 50 MiB cap.
- `course_y_axis` (`"up"` or `"down"`) and `course_georef_check` (default true) in `[sources]` become the fit's
  `y_axis` and `check_georef` (ADR 0032). Both are left out of a written file while they have their default, as are
  `courses`, `assignments` and `numbers`, so projects without courses write as before.
- `[[assignments]]` has `course` and `layout`. The layout must exist. One course has at most one entry; a second
  is an error with file and line. A course without an entry prints on the first layout, which `check` mentions as
  information that does not count for its exit code.
- `[[numbers]]` is a number correction with `control`, `direction` and an optional `gap_mm`, and exactly one of
  `layout` (all courses on that layout) or `course`. `direction` must be finite and is read into [0, 360);
  `gap_mm` lies between 0 and 20. Without `gap_mm`, the gap of the level below stays. `base_seen` holds the
  course file's placement when the correction was made (`direction`, `gap_mm`); `numbers accept` writes it.
  Loading only checks the entries. Applying them is the job of the overprint (Task 10).
- Stale data stays in the file. An assignment whose course the course file lacks, and a correction whose control
  is not in the file (or not in the course it names), are reported as `Stale` and counted as warnings. The other
  courses load. A correction whose `base_seen` differs from the course file's placement by more than 0.5 degrees
  or 0.05 mm is `CorrectionOverridesChangedBase`; `base_changed` makes the comparison, and the overprint calls
  it once it has converted the file's raw number offsets to a placement.

## Consequences

- Older programs refuse newer formats instead of misreading them.
- A new optional key is compatible with `format = 1`. A change that old programs would misread bumps `format`.

Source: M2 plan, task 3.
