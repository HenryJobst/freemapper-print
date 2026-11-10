# M5: MCP server Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task.

**Goal:** `freemapper-print mcp` serves the CLI's work over MCP (stdio), so an AI agent can read a project, check it, change masks, number placements and layouts, and export PDFs.

**Architecture:** The command logic sits in `crates/freemapper-print-cli/src/main.rs` (3.8k lines), mixed with output formatting. Task 1 splits it: an `ops` module returns structured results (serde-serialisable), `main.rs` formats them for the terminal as before, the MCP server returns them as compact JSON. The server follows freemapper's `crates/freemapper-cli/src/mcp.rs` (rmcp `=3.6.0`, features `server`, `transport-io`, `macros`; pinned because newer releases are younger than two weeks, ADR 0006). The MCP server calls no model (ADR 0016).

**Spec:** `docs/superpowers/specs/2026-11-08-freemapper-print-design.md` §5.6 and ADR 0004 (tools mirror the CLI, plus list/change tools; compact JSON; relative paths refused).

## Global Constraints

- CLI output stays byte-identical: every existing CLI test (`project_cli`, golden texts DE/EN) passes unchanged after Task 1.
- Source code English; tool names, descriptions and the JSON keys English; program texts in results follow `FREEMAPPER_LANG` like the CLI (`lang` argument optional).
- Absolute paths only; a relative path is refused with a message that says so (ADR 0004). No path outside the project folder is written except the export folder the caller names.
- Writes go through `freemapper-fs` like the CLI (no write through links, ADR 0041); the project file keeps comments it did not create.
- Tool descriptions longer than a short phrase go through `unslop` and `humanizer`.
- rmcp pinned `=3.6.0`; tests run with `--locked` as in freemapper.

## Review Focus

- A relative path, a path through a link, a missing project, a project file that is no TOML: clear error text, no panic, nothing written.
- Two writes in a row (masks window, then numbers accept) on the same project: the second sees the first, comments survive.
- `export` of a project with a 1000 x 200 mm sheet (hostile case of ADR 0040): returns an error or the budget note, never hangs the server.
- Output size: `check` on a large project returns bounded JSON (notes capped with a count of the rest).
- stdio hygiene: nothing but JSON-RPC on stdout; logs and progress go to stderr.

## Tools (names fixed here)

Read: `project_info` (layouts, elements, courses, mask variants, assignments, sources), `check_project` (the CLI's `check` as structured notes with severity, kind, course, message), `list_courses`.
Write: `init_project`, `masks_window`, `masks_corridor`, `numbers_accept`, `set_layout`, `set_element`, `assign_mask`, each returning what changed.
Run: `export_project` (courses, layouts, single_file, out_dir; returns the files written with size and the notes).

## Tasks

### Task 1: `ops` module (split logic from formatting)
- Create `crates/freemapper-print-cli/src/ops/` (one file per command: `check.rs`, `export.rs`, `masks.rs`, `numbers.rs`, `init.rs`, `project.rs`) with functions taking plain argument structs and returning serde-serialisable result structs; no printing, no `Texts`. The note enums (`SheetNote`, `MaskNote`, `CourseNote`, `FitNote`, `NumberConflict`, `Stale`) are serialised as `{kind, severity, course, message}` through one `describe(texts)` function that reuses today's `*_line` functions.
- `main.rs` keeps clap, `Texts` and output; each command calls its op and formats.
- Test: whole existing CLI test suite green and unchanged; plus one unit test per op on a fixture project (Viktoriapark) asserting the structured result.
- Interfaces produced: `ops::check(&Loaded, &CheckArgs) -> CheckResult`, `ops::export(...) -> ExportResult`, and so on, as listed under Tools.

### Task 2: server skeleton and read tools
- `freemapper-print mcp` subcommand; `mcp.rs` with `PrintServer` (rmcp `#[tool_router]`), stdio transport, shutdown on EOF, `--lang`. Tools `project_info`, `check_project`, `list_courses`. Path guard `absolute_path(&str) -> Result<PathBuf>` refusing relative paths and non-UTF-8.
- Tests (in-process client over a duplex pipe as freemapper's mcp tests do): tool list, `project_info` on Viktoriapark, relative path refused, stdout carries only JSON-RPC.

### Task 3: write tools
- `init_project`, `masks_window`, `masks_corridor`, `numbers_accept`, `set_layout`, `set_element`, `assign_mask`. A shared `with_project_lock` serialises writes in one server.
- Tests: window mask round trip (writes `[[masks]]` and assignment, comments preserved), corridor with bad width refused, `numbers_accept` for the Lübeck example, writes through a link refused.

### Task 4: `export_project`
- Calls `ops::export`; returns files, sizes, notes; `out_dir` must exist and be absolute; disk-full and same-file-as-map errors as structured errors.
- Tests: exports Viktoriapark PDFs byte-identical to the CLI's (cmp with committed PDFs), `single_file`, missing out_dir refused, hostile big sheet returns an error within the work budget.

### Task 5: docs, ADR, example transcript
- ADR 0054 (MCP tool set and path rule), manual DE/EN chapter "Mit einem KI-Agenten" (replace the stub), `docs/mcp.md` with a Claude Code `.mcp.json` snippet, README sections, glossary rows, CHANGELOG DE/EN.
- Example: `examples/mcp-session/` transcript of an agent preparing a window-OL (tool calls and results) built from a real run.

### Task 6: release
- Final whole-branch review on the most capable model, merge, release v0.6.0 (the unknown-`map_kind` fix, core pins, renderer move and MCP in one), CI and release workflow green.

## Order

1 → 2 → 3 → 4 → 5 → 6. Task 1 is the risky one (3.8k lines); it runs first so a failure shows early.
