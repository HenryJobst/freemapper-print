# Example projects as test fixtures

These folders were the example projects in `examples/` until 2026-11-12. They stay here as fixtures for the tests;
the new examples in `examples/` build on maps by Remo Madella. They are test data, not documentation: their
READMEs and previews were removed, and nothing here is kept up to date for readers. The committed PDFs are the
reference outputs that the tests compare byte for byte.

## Sources and licences

All maps come from freemapper 0.8.0 (https://github.com/HenryJobst/freemapper, tag `v0.8.0`, commit `a2b41b4`,
2026-11-09), copied unchanged from its examples. The templates the maps name (candidates, orthophoto, hillshade)
are not copied. freemapper itself is GPL-3.0-or-later.

| Folder | Map from the freemapper example | Licence of the map data |
|---|---|---|
| `luebeck-scales`, `luebeck-competition-layouts`, `luebeck-competition-courses`, `luebeck-training-window`, `luebeck-training-corridor` | `schleswig-holstein-luebeck` | © OpenStreetMap contributors (ODbL); © GeoBasis-DE/LVermGeo SH (CC BY 4.0, https://creativecommons.org/licenses/by/4.0/, source modified) |
| `berlin-viktoriapark-training` | `berlin-viktoriapark` | © OpenStreetMap contributors (ODbL); © GeoBasis-DE/LGB 2026 (dl-de/by-2-0); Geoportal Berlin datasets (dl-de/zero-2-0, partly modified) |

The `ATTRIBUTION.txt` next to a map (and its `freemapper.toml`) is freemapper's own and lists the sources in full.
`luebeck-scales` and `luebeck-competition-layouts` have no `ATTRIBUTION.txt`; the map's `<notes>` name the sources
there, and the table above applies. The course files (`*.ppen`, `courses.xml`) were written for freemapper-print and
are CC0 1.0, as are the made-up logos with the script that draws one of them
(`luebeck-competition-layouts/logos/`, see the README there). The project files belong to freemapper-print. The PDFs were made with
[freemapper-print](https://github.com/HenryJobst/freemapper-print).

## What the tests use

| Folder | Used by |
|---|---|
| `luebeck-scales` | `render` outputs compared with the two committed PDFs (`crates/freemapper-print/tests/overprint_outputs.rs`); the one example without a project file (`src/project.rs`) |
| `luebeck-competition-layouts` | the committed layout PDFs in `out/` (`overprint_outputs.rs`), project round trips and defaults (`src/project.rs`, `tests/project_courses.rs`), the CLI and editor tests, the logos (copied by `luebeck_copy` in `src/ops/mod.rs`, used in `src/editor/api.rs`) |
| `luebeck-competition-courses` | course sheets in `out/` and `iof/out/` compared byte for byte (`overprint_outputs.rs`), the Purple Pen and IOF XML readers (`tests/project_courses.rs`), the MCP server (`tests/mcp_cli.rs`, `src/mcp.rs`) |
| `berlin-viktoriapark-training` | the course sheet `out/Training.pdf` (`overprint_outputs.rs`), the CLI, update and MCP tests (`tests/project_cli.rs`, `tests/update_cli.rs`, `tests/mcp_cli.rs`, `src/ops/mod.rs`) |
| `luebeck-training-window` | window masks on a course sheet (`src/sheet/course_sheet.rs`), the editor preview and server tests (`src/editor/preview.rs`, `src/editor/server.rs`) |
| `luebeck-training-corridor` | kept as a project with a corridor mask; no test reads it yet |
| `mcp-session` | `session.jsonl`, the recording of one MCP session on the Viktoriapark training (version 0.5.1 of the server); no test reads it. The scripts that recorded it were removed |

Tests copy these folders to a temporary directory when they write; they never change the files here. A test that
fails here after a change in the program shows a change in the output. Rebuild the PDFs from the project files
only after you checked that the change is intended.
