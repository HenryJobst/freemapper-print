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
- `init` writes the file with `save_new`, which refuses to overwrite.
- The `toml` crate rewrites the file without comments. The editor (later) must keep the user's comments and
  layout, so it switches to `toml_edit` for saving. Until then only `init` writes the file.

## Consequences

- Older programs refuse newer formats instead of misreading them.
- A new optional key is compatible with `format = 1`. A change that old programs would misread bumps `format`.

Source: M2 plan, task 3.
