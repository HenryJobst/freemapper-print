# 0054. The MCP tool set

Date: 2026-11-11
Status: Accepted, implements [0004](0004-mcp-over-stdio-as-a-primary-interface.md), amended by [0060](0060-self-update-from-signed-releases.md)

## Context

ADR 0004 chose MCP over stdio with rmcp, tools that mirror the CLI plus tools that change the project file, compact
JSON and absolute paths. In M5 the command logic sat in `crates/freemapper-print-cli/src/main.rs` (3.8k lines),
mixed with the terminal output. A server needs the same work without the printing, and an agent drives it
differently from a person at a terminal: calls can arrive together, a call can be cancelled, and an agent cannot
run a CLI flag that a message tells it to use.

## Decision

### Logic and output apart: the `ops` module

- `crates/freemapper-print-cli/src/ops/` holds the work of `check`, `export`, `masks window`, `masks corridor`,
  `numbers accept` and `init`, plus `ops::project` (what `project_info` shows) and `ops::edit` (the new edits).
  Each op takes a plain argument struct (no clap types, no `Texts`) and returns a result struct that serialises
  with serde. `main.rs` keeps clap, `Texts` and every output line.
- Errors are `ops::OpError`, typed variants that carry what the message names, with the cause kept as `source`.
  The caller words them: `op_error(texts, OpError) -> anyhow::Error` builds the same chain the CLI printed before,
  and usage errors come back as `UsageError` (exit 2).
- Notes are `ops::Notice { course, value }` with `kind()` (stable snake_case, exhaustive matches with no wildcard)
  and `severity()` (`info`, `warning`, `error`). `Notice::describe(texts)` in `main.rs` gives
  `{kind, severity, course, message}` through the CLI's own `*_line` functions, so the CLI and the server print the
  same text by construction. A `Notice` serialised alone has no message, because the message needs `Texts`.
- Print report notes are typed (`ops::note::PrintNote`). `export_error` and `output_error` keep the cause chain as
  text (the "a: b: c" of anyhow's `{:#}`), so a failed sheet can be a `Notice`; nothing downcasts those causes.
- `export` and `init` report progress through a callback, so the CLI prints each page as before even when a later
  step fails. Text that goes into the project file (mask comments, the comments of `init`) comes from a function
  the caller passes, so it stays in the user's language.
- The CLI output stays byte-identical. A harness of 136 scenarios (68 per language: stderr, stdout, exit code and
  the SHA-1 of every file written) compared each step against the binary before the split, and again in Task 5.

### The server

- `freemapper-print mcp` follows freemapper's server: a `#[tool_router]` struct, a `#[tool_handler]`
  `ServerHandler`, `serve(rmcp::transport::stdio())` and `waiting()`. It ends with exit code 0 when stdin closes.
  Nothing but JSON-RPC goes to stdout. The server calls no model (ADR 0016).
- The server's language is the CLI's (`--lang`, else the environment). Each call may choose another with `lang`;
  an unknown value is a tool error worded in the server's language.
- Ops run in `tokio::task::spawn_blocking`, because they read files and draw sheets. A panic in an op becomes the
  tool error "internal error: …", and the server keeps serving. A test makes a read and a write op panic through a
  seam that exists in test builds only (`panic_for_test`, a project path named `panic-for-test`) and checks that
  the next read and write succeed.
- The server info carries the website https://github.com/HenryJobst/freemapper-print, and the instructions name
  the tools and end with that link.
- Tests talk to the server in-process over a `tokio::io::duplex` pipe with hand-written JSON-RPC lines, so rmcp's
  `client` feature and its dependencies stay out. Integration tests spawn the binary for stdout hygiene, EOF and
  disk-full.

### The tools

| Tool | Like | Required | Optional |
|---|---|---|---|
| `project_info` | | `project` | |
| `check_project` | `check` | `project` | |
| `list_courses` | | `project` | |
| `init_project` | `init` | `folder`, `map` | `title`, `kind`, `courses` |
| `masks_window` | `masks window` | `project`, `course` | `controls`, `radius`, `radius_for`, `no_start`, `no_finish`, `id`, `assign` |
| `masks_corridor` | `masks corridor` | `project`, `points` | `width`, `id`, `assign`, `course` |
| `numbers_accept` | `numbers accept` | `project`, `course`, `control`, `scope` | `direction`, `gap` |
| `set_layout` | | `project`, `id` | the keys of a `[[layouts]]` table |
| `set_element` | | `project`, `id`, `kind`, `anchor` | `layout`, `lines`, `size_pt`, `box_white`, `file`, `width_mm`, `cell_mm` |
| `assign_mask` | | `project`, `course` | `mask`, `layout` |
| `export_project` | `export` | `project`, `out_dir` | `courses`, `layouts`, `single_file` |

Every tool takes `lang`. `project` is the folder or the `freemapper-print.toml` in it. There is no `render` tool.

### Paths

`absolute_path(texts, &str)` refuses an empty path, a path with NUL and a relative path. The relative-path message
says why: the server does not know the agent's working folder. `~` is not expanded and counts as relative. JSON
strings are always UTF-8, so NUL is the only unusable case left. The guard takes `&Texts`, because its messages
are Fluent texts. A logo `file` in `set_element` must be absolute too; the project file stores it relative to
itself, both sides canonicalised where possible, as `init` stores the map.

### Errors and results

- Every failure is a tool result with `isError: true` and one text block, never a JSON-RPC error: the agent sees
  it and can correct the call (MCP spec), and all failures look alike, as in freemapper. The text is `{:#}` of
  `op_error` through `printable`, without the "Error:" lead of the CLI. Only rmcp's own errors for a missing field,
  an unknown field or a wrong type or enum value are rmcp's English text and ignore `lang`. Every input struct has
  `#[serde(deny_unknown_fields)]`, so a typo such as `print_scal` is an error that names the field; before, it
  was ignored and `set_layout` answered `unchanged`.
- Results are `serde_json::to_string`, compact. `check_project` has its own output structs that put
  `describe(texts)` into every note. It returns at most 200 notes over the whole result, in the order course file,
  courses, layouts; `more` counts the rest and is left out at 0. It lists at most 200 sheets, course sheets first,
  and `more_sheets` counts the rest; a sheet left out takes no notes from the budget. One drawn sheet without notes
  is about 190 bytes, so the hostile project of ADR 0040 would return about 190 KB of sheets without the cap.
  `needs_attention` covers every sheet, also those left out, and `more_sheets_needing_attention` counts the sheets
  left out whose outcome needs attention, so an agent knows whether the cut part hides a problem. A failed sheet has `outcome: "failed"` and its `sheet_failed` note (severity
  error). `needs_attention` at the top matches the CLI's `check` exit code 1.
- `list_courses` reuses `ops::project::info` and returns `{courses_file, courses, more_courses?}`, so an agent can
  tell "no course file" from an empty one. `project_info` and `list_courses` returned 56 KB each on the hostile
  project (1000 courses). Both now list at most 200 courses, and `project_info` also at most 200 mask variants and
  200 assignments (`init` writes one assignment per course); `more_courses`, `more_masks` and `more_assignments`
  count the rest. `init_project` names at most 200 courses over all its layouts (`more_courses`). One constant,
  `MAX_ENTRIES` = 200, caps every list, the export's files included.
- The four tools that wrap a command (`init_project`, `masks_window`, `masks_corridor`, `numbers_accept`) return
  the op result plus `messages`, the lines the CLI prints after the command; init's progress lines are among them
  except the fitting line, which is for a terminal. The CLI's printing moved into functions that return lines, so
  both use the same text. `init_project` adds `check` (a described sheet) when the layout does not fit. The edit
  tools return structured results with `change` (`added`, `changed`, `unchanged`) and no messages, since the CLI
  has no wording for them.
- `export_project` returns `{out_dir, files, more_files?, notes, more?, reminder?, message?}`. A file per course or
  layout carries course, layout, paper and the print facts the CLI's "Wrote …" line gives; the single file carries
  `pages`. At most 200 files and 200 notes come back; `more_files` and `more` count the rest. A hostile export
  without the file cap returned 1000 entries, 403 KB of JSON. `reminder` is left out when nothing was written, and
  `message` then carries `note-no-courses-to-export`.
- `export_courses` and `export_layouts` stop at the first sheet that fails and keep the files before it (a full
  disk at the third of ten courses leaves two PDFs). The tool's progress callback collects the PDFs written, and an
  error after some of them gets `mcp-error-export-written` appended: their number and the first 200 paths. So the
  rule "a failed tool has written nothing" holds for the project file only, and the manual says so.

### Messages worded for an agent

Some CLI messages name flags or commands: "run the command again with --assign", "--course and --single-file need
a course file", "run numbers accept with --course … and --sheet or --layout-wide", "Run check to see the result".
Each gets a twin `agent-<id>` in both catalogs that names the tool and its input instead. `Texts::for_agent(lang)`
looks up `agent-<id>` first; the server builds its texts with it, and the CLI keeps `Texts::new`, so its output
does not change. A twin uses only variables of its original, which a test checks for both languages, and every
English twin differs from its original. Text written into the project file (the mask comments) has no twin.

### Writes

- One write lock per server: an `Arc<tokio::sync::Mutex<()>>`, taken with `lock_owned` before the project is read.
  The guard moves into the blocking task, so a call the client gives up on keeps the lock until its write is done.
  rmcp's `notifications/cancelled` only cancels the request's token and does not abort the handler. Every tool that
  takes the lock therefore takes the `RequestContext` and checks its token in the blocking task right after the
  lock is acquired: a call cancelled while it waited in the queue does not run and writes nothing
  (`mcp-error-cancelled`). Before, it wrote once the lock was free, and since rmcp drops the answer to a cancelled
  request, the agent never learned of it. rmcp also cancels every request's token when the connection closes, so
  a client that sends writes and closes stdin at once (`… | freemapper-print mcp`) may find a write skipped,
  depending on timing; its answer was dropped either way. An agent reads `project_info` after such a session to see
  what the file holds.
  Every write tool and `export_project` (it reads the project) take it; the read tools do not. The lock is per
  server, so writes to different projects queue too, which is simple and safe.
- The project already had no `toml_edit`: `masks` and `numbers` append text. The new edits follow that and use an
  own line editor in `project/edit.rs` instead of adding `toml_edit`. A new dependency would have to pass ADR 0006
  and brings its own formatting choices, while a small editor that changes only the lines it must keeps every
  comment and blank line it does not touch, which is what the project file needs. A line lexer tracks strings (basic,
  literal and both multi-line kinds) and bracket depth, so it finds headers and keys at top level only. A byte order
  mark before the first line is skipped. Values are written with `toml::ser::ValueSerializer` (inline tables,
  struct field order). It writes a string with a line break as a multi-line string; `one_line_strings` rewrites
  those as basic strings with `\n` escapes. Without that, a CRLF file turned the `\n` inside the string into
  `\r\n`, the text read back as another value, and the change was refused with a misleading `NotEditable`.
- Editing rules: a key whose value does not change keeps its line byte for byte, comment included. A changed key is
  replaced at its first line; dotted keys and sub-tables of that key go. A new key follows the table's last key. A
  new table follows the last table of its kind (a layout after the last layout and its elements, a first shared
  element before the first layout, a first assignment at the end). Comment and blank lines directly above a header
  belong to the table below them. A sub-table that becomes an inline key goes with the comment and blank lines
  above its header; its comment lines, those above the header and those inside, move above the new key line when
  the key stays, so no comment is left above a table it does not describe. When the key goes away there is no line
  to move them to, and they go with the sub-table. On a CRLF file inserted text gets CRLF. Tests pin CRLF,
  a byte order mark, multi-line strings holding header text, anchor sub-tables, a file without a last line break
  and a `[layouts.map_frame]` placed after the layout's `[[layouts.elements]]`.
- Preflight validation, before anything is written: the changed project goes through `validate` (the load checks),
  so a bad value gives the field error. Then the new text goes through `load_text` and must equal the expected
  project, else `ProjectError::NotEditable { file, target }` (tables not written as `[[…]]` of their own, such
  as inline arrays). `target` is an `EditTarget` (a layout, the elements of a layout, the shared elements, the
  assignment of a course), so the CLI words it in the user's language (`error-not-editable-*`): the path and
  "was not changed" once each, with the verb agreeing with "elements". Then `rewrite` replaces the file, loads it and puts the old content back on a mismatch, as for
  masks and numbers. `load` became `load` plus a private `load_text` for this.
- `set_layout` merges: keys left out keep their value (or the default for a new layout), `null` clears
  `print_scale` and `centre_map_mm`, and the layout's elements are never touched. Full replacement was rejected
  because `project_info` does not show margin, border and crop, so an agent would reset them without knowing.
- `set_element` replaces the whole element by id, because which fields apply depends on `kind`. Without `layout`
  the element is shared.
- `assign_mask` sets the whole assignment of a course. `mask` left out or `null` removes the mask; `layout` left out
  keeps the course's layout (its assignment's, else the first). It needs a course file
  (`OpError::AssignNeedsCourseFile`) and a known course, and the mask must be a `[[masks]]` id or a white-out id of
  the course file (`OpError::UnknownMask`). Unlike `masks window --assign` it replaces a mask the course has.
- A project file that is a link is refused by the write tools ("is a link") and nothing changes. The CLI replaces
  such a link with a regular file (ADR 0041, ruling R1), which an agent would not notice. A project folder that is a
  link is followed (macOS temp folders sit under the `/var` link). A link made after the check is still replaced by
  the library and never written through. `export_project` does not refuse a linked project file, since it does not
  write it.
- The CLI's masks and numbers paths run no `check` after writing; their writer reloads and validates, and the
  tools reuse it.

### Argument rules clap enforced

They are the tools' now: `assign` in `masks_corridor` needs `course` (`mcp-error-corridor-assign`); `direction` and
`gap` go together (`mcp-error-numbers-position`); `layouts` does not go with `courses` or `single_file`
(`mcp-error-export-layouts`); `out_dir` must be absolute and an existing folder, checked before the lock and before
the project is read. `scope` is an enum in the schema. Unknown courses and layouts, and `courses` or `single_file`
without a course file, are refused by the op with the CLI's texts, which list the known names.

The CLI's `export` has no "same file as the map" rule, and the tool needs none: export names are `<layout id>.pdf`,
`<course stem>.pdf` or `<event> - alle Bahnen.pdf`, never `.omap` or `.xmap`. Disk-full and "a link leads out of the
project" are the writer's `OutputError`, worded as in the CLI.

### Cancellation between sheets

- The library's export loops (`export_layouts`, `export_courses`, `export_courses_single_file`) are generic over a
  small public trait `freemapper_print::export::Proceed`, implemented for `()` (go on) and `ControlFlow<()>`, so
  every caller with a `()` closure compiles unchanged. Stopped per sheet: `Ok(())`, the files so far stay. Stopped
  single file: `Ok(None)`, nothing is written.
- `ops::export`'s progress callback returns `ControlFlow<()>`. A stop ends with `OpError::ExportStopped { written }`
  (the PDFs written before; empty for the single file), worded by `error-export-stopped`. The CLI always answers
  `Continue`, so its output does not change.
- `export_project` takes rmcp's `RequestContext<RoleServer>` and clones its `ct` into the blocking task; the
  callback answers `Break` once the token is cancelled, and the token is also checked before the export starts (for
  example after waiting for the lock). No `tokio-util` dependency was added. In rmcp 3.6.0 each request's token is a
  child of the serve loop's token. rmcp cancels it on `notifications/cancelled` for that request, and cancels the
  loop's token at shutdown after draining in-flight responses: 5 s on EOF, 2 s on cancel, fixed in 3.6.0. So EOF
  costs up to 5 s of drain plus one sheet. rmcp drops the answer to a cancelled request, so an agent seldom sees
  `error-export-stopped`.
- Not interruptible: a sheet being drawn (bounded by the number search budget of ADR 0038 and the render page
  budget), loading project, map and course file, and every tool other than `export_project` once it has started.
  A tool that writes and is cancelled while it waits for the lock does not start. The single file is
  built in memory and written at the end, so a cancel leaves no file. Per-sheet files go through
  `write_project_pdf` (temporary `.part` and rename), so no half-written PDF remains.
- The hostile case of the plan ("a 1000 x 200 mm sheet") is ADR 0040's course file of 1000 courses with 200 controls
  each; paper is only A4 or A3. Measured on the development Mac with that file (14 MB IOF XML on the Lübeck map, one
  A3 layout):

| What | Build | Time |
|---|---|---|
| CLI `export` of all 1000 courses (1000 PDFs, 367 MB) | release | 144.3 s |
| `export_project` of all 1000 courses, until the answer | release | 118.5 s |
| one hostile sheet | release / debug | about 0.14 s / 0.5 to 1.5 s |
| cancel until the lock is free | release | 0.11 s (one more PDF) |
| cancel until the next call answers (in-process test) | debug | 1.6 s |
| stdin closed until the server exits | release | 5.09 s (46 more PDFs during the drain) |
| stdin closed until the server exits (integration test) | debug | 5.7 s |
| CLI `check` of the hostile project (not interruptible) | release | 110 s |

### Dependencies

`rmcp = "=3.6.0"` (features `server`, `transport-io`, `macros`), `tokio 1.54.1` (`macros`, `rt-multi-thread`,
`io-std`, `io-util`, `sync`), `schemars 1.2.3`, and in the CLI `serde` and `serde_json 1.0.154`. rmcp is pinned
because newer releases were younger than two weeks (ADR 0006). The resolver picked `rmcp-macros 3.9.0`; it is held
at 3.6.0 with `cargo update -p rmcp-macros --precise 3.6.0`, in Cargo.lock only, as freemapper does, so builds and
tests run with `--locked`. Every new Cargo.lock entry has the version freemapper's lock has.

## Consequences

- An agent can prepare a whole event through the tools, and the CLI prints exactly what it printed before M5.
- A long export can be stopped between sheets. `check_project` cannot: on the hostile file it runs 110 s, and EOF
  during it waits for the end. The same `ControlFlow` approach could go into `ops::check` if that matters.
- Closing stdin during an export lets it draw for up to 5 s more, because rmcp's drain is fixed.
- Comments inside a rewritten key are lost: a comment at the end of a changed key line, a comment inside a value
  over several lines that changes (between the entries of `lines = [ … ]`), and comments at the end of the lines
  of a sub-table that becomes one inline key. The comment lines of such a sub-table move above the new key, or go
  with it when the key goes away.
  Comments above a changed key and everywhere else stay. A changed key line also loses its indentation.
- `rewrite` replaces, loads and restores. If the process dies between replace and restore, a bad file can remain.
  The edit tools avoid this with the in-memory checks first; masks and numbers keep the old order because a CLI
  test expects a link to become a regular file even when the reload fails.
- rmcp's own deserialisation errors and library validation texts inside a localised lead-in (ADR 0024) stay
  English.
- The plan's `docs/mcp.md` (Task 5) was dropped on purpose. The Claude Code `.mcp.json` snippet is in chapter 3
  of the manual and in both READMEs; a fourth copy would only drift from them.
- Cost if wrong: an editor of our own to maintain, and a switch to `toml_edit` later would touch `project/edit.rs`
  only.

Source: M5 plan (docs/superpowers/plans/2026-11-11-m5-mcp-server.md), Tasks 1 to 5; freemapper ADR 0005; rmcp 3.6.0
`service.rs`.

Amended by ADR 0060: the tools `check_update` and `install_update` make 13.
