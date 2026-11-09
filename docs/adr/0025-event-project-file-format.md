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
- `init` takes the map kind from `--kind`, else from `map_kind` in `freemapper.toml` next to the map. It does not
  infer the kind from the symbol set as `render` does (ADR 0021): the kind is stored in the project, and a wrong
  guess would stay. Without either source `init` stops and asks for `--kind`.
- `init` writes the map path relative to the project file (canonical paths on both sides), the event title from
  `--title` or the map's file name, the sheet language from the program language, and one A4 layout `a4` with the
  map scale written out. Its shared elements are a title block with `{event}` and
  `1:{print_scale} · {contour_interval}` and a scale check bar, both in corners of the map frame. Copyright and
  credit lines stay with the default footer: it moves inside the frame on a white box when the passe-partout is too
  narrow, while an element hung on the page cannot and would cover the map.
- The proposed contour interval (sprint 2.5 m, forest 5 m, schoolyard none) gets a comment in the file, in the
  program language, telling the user to confirm it; `init` prints the same hint. `save_new_with_comments` adds such
  comments to the serializer's output. An empty `[print]` table is left out.
- On a map that does not fit the A4 frame at its scale (Lübeck, 10 mm too tall), `check` reports the overflow for
  the generated project. That is the intended signal; `init` does not switch to A3 on its own.

## Consequences

- Older programs refuse newer formats instead of misreading them.
- A new optional key is compatible with `format = 1`. A change that old programs would misread bumps `format`.

Source: M2 plan, task 3.
