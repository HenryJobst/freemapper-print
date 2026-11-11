# M6 Browser Editor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `freemapper-print edit <project>` opens a local browser page in which a person places sheet elements, edits layouts, draws masks and moves control numbers on the real sheet preview, and exports, all writing the same `freemapper-print.toml` as CLI and MCP.

**Architecture:** A third adapter beside CLI and MCP in `crates/freemapper-print-cli/src/editor/`, calling the shared `ops::*` functions for every change. The server frame is `freemapper-web` from freemapper-core (token, Host/Origin guard, CSP, shutdown). The page is vanilla JS without a build step, embedded with rust-embed, on the shared suite assets of `freemapper-web` (core ADR 0030, `/{token}/suite/suite.css`, `api.js`, `texts.js`, `status.js`, `tree.js`), so all three suite pages look and behave alike. print builds on them from the start; until the core tag exists it follows courseplan's files, which differ only in the header name and the accent. The sheet preview is the renderer's own SVG (core `write_svg`), clicks are resolved with the core hit test and `BuildReport.source_objects`.

**Tech Stack:** Rust 2024, `freemapper-web` (axum 0.8, tokio), rust-embed, vanilla JS/CSS, no CDN, no JS dependency beyond what the page needs (none planned).

**Spec:** `docs/superpowers/specs/2026-11-08-freemapper-print-design.md` §5.6 (editor), §5.7 (numbers), §8; ADR 0005, 0008. Suite facts come from the sessions freemapper, FableWorker and freemapper-courseplan on 2026-11-11. The decision is core ADR 0030 (agreed by all four sessions on 2026-11-11, user's order: one look, one technique).

## Global Constraints

- Source code, plans, ADRs English; READMEs, manual, changelog bilingual (ADR 0003). User-facing text passes `unslop` and `humanizer`.
- Dependencies at least two weeks old (ADR 0006); the toolchain is pinned. Core crates only through `lib.rs` re-exports (boundary test); `freemapper-web` is new there and needs the re-export and the boundary test entry.
- Server rules unchanged from `freemapper-web`: 127.0.0.1 only, random 32-hex token as the first path segment, Host allowlist, Origin check, `strict_post: true`, CSP with `'self'` only (no CDN, no inline script).
- Page texts (ADR 0030): Fluent messages `editor-*` in the print-cli catalog (`i18n/{de,en}`), delivered by `GET api/texts` as `{ lang, texts }`; `texts.js` applies `data-text` / `data-label` / `data-title` / `data-placeholder`, sets `document.documentElement.lang` and offers `t(id, args)` and `tn(id, n, args)`; simple `{ $name }` placeholders, plural by `<id>-one` / `<id>-other`; no language switch in the page. Language: `--lang`, then `FREEMAPPER_LANG`, then `Accept-Language`, then English. A test checks both catalogs have the same `editor-*` keys and that every `data-*` key exists. Values from the server come formatted by it.
- Server rules beyond the `freemapper-web` base: every POST needs `Origin`, a non-simple content type and, where it changes the file, the revision header; the browser never names a file; bodies at most 1 MiB; no inline script or handler, nothing from the network; own assets plus suite assets under 300 KiB (core test helper checks it).
- Look: the suite tokens of `suite.css`, names stable (`--bg --fg --line --bar --accent --sel --map-bg --pool --warn-bg --warn-fg --warn-line --sev-error --sev-warning --sev-hint`), light and dark, accent blue `#0b6bcb` / `#6cb0ff`. print-specific tokens stay in `app.css` (the course overprint colour of the sheet is drawn by the renderer, not a token). Layout: body grid with menu on top, three panes, status bar; tree 16rem left, sheet preview in the middle with a tool bar, properties and notes about 20rem right. Text buttons, no icons.
- Accessibility (suite CSS and `tree.js` provide the base): focus ring, reduced motion, `role="tree"` with arrow keys, Home, End, Enter and one tab stop; sheet by keyboard (arrows pan, `+`/`-` zoom, `0` fits the page); `aria-live` for messages; colour never the only sign; tab order tree, sheet, properties, menu; a real `<dialog>` for confirmations.
- Revision: FNV-1a-64 of the file bytes, 16 lower-case hex digits, header `X-Freemapper-Revision`, from core `revision(bytes)`; 200 `{revision, result}`, 400 form, 404 unknown operation, 409 `{error, revision}` when the file changed, 422 `{error}` for a refused change (file unchanged); a read-only page answers 423 itself. Save is atomic (tmp, rename) and refused when the file changed since it was read. The page polls `api/revision` every 2 s while visible and on focus. Editors write each change at once and keep an undo stack of whole-file snapshots (ADR 0030).
- Operations: `POST api/op/{name}` with a fixed list `EDITOR_OPS` that maps to the `ops::*` functions (the MCP tools' functions); an unknown name is 404.
- Edits go through `ops::*`; no second TOML writer. Comments in the project file stay (ADR 0008). A link as project file is not written (existing rule).
- Per task: `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test` green; branch/worktree `.worktrees/m6-editor`; changelog entry DE and EN per merged feature; ADR in the same branch.

## Review Focus

- Two writers: the editor and an MCP session or text editor change `freemapper-print.toml` at once. Expect: no silent overwrite; the person is told and can reload.
- Undo after an external change or after the file vanished. Expect: undo refuses with one sentence, never writes stale text.
- Closing the window or `Fertig` with unsaved work in a drawing mode (corridor half drawn). Expect: a question, nothing lost silently, Ctrl+C in the terminal still ends the server cleanly.
- A project without courses, without `.ppen`, or a layout with a free page (`page_mm`). Expect: the page still opens, preview works at the free size, course-only panels are hidden with a hint.
- Large maps: a preview request while the previous one still renders. Expect: the older request is dropped, the page shows the newest result and never an old sheet after a newer edit.
- Keyboard-only person: everything that can be set by a form is reachable without the mouse; drawing on the map is mouse-only in v1 and says so.

---

### Task 1: Adopt core ADR 0030 in an ADR of print (0059)

**Files:**
- Create: `docs/adr/0059-editor-follows-the-suite-pages.md`; modify `docs/adr/README.md`, `docs/glossary.md`

**Interfaces:**
- Produces: the print-side decisions: (a) the editor follows core ADR 0030 (suite assets, tokens, Fluent texts via `api/texts`, revision header, immediate writes with a whole-file undo stack, no dirty state, security and size rules); (b) the `ops` functions are the editor's operations (`EDITOR_OPS`); (c) `Fertig` and window close ask only while a drawing mode is open; (d) print adds no token of its own beyond app-specific ones in `app.css`.

- [ ] **Step 1:** Write the ADR in English, short, pointing at core ADR 0030 and its reasons instead of repeating them; alternatives not taken: own tokens or a Save button (rejected by ADR 0030 for editors); "Cost if wrong".
- [ ] **Step 2:** Glossary rows: *editor*, *revision*, *undo snapshot*.
- [ ] **Step 3:** Commit `docs(adr): the editor follows the suite page rules`.

### Task 2: Server skeleton and the `edit` command

**Files:**
- Create: `crates/freemapper-print-cli/src/editor/mod.rs`, `editor/server.rs`, `editor/web/index.html` (placeholder)
- Modify: `Cargo.toml` (workspace deps `freemapper-web` at the core tag that carries the suite assets, `rust-embed`, `axum` same versions as core; the tag number comes from the freemapper session), `crates/freemapper-print/src/lib.rs` (re-export `web`), boundary test, `main.rs` (new `Edit` variant only; logic stays in `editor/`)

**Interfaces:**
- Produces: `pub fn run(args: &EditArgs, texts: &Texts) -> Result<(), OpError>`; `EditArgs { project: PathBuf, port: u16 (default 0), no_browser: bool }`; routes `GET /{token}/` (index), `GET /{token}/assets/*`, `POST /{token}/api/done`.

- [ ] **Step 1:** Tests in `editor/server.rs`: wrong token → 404; wrong `Host` → rejected; POST without JSON content type → 403; `/api/done` ends `run`; the address printed on stdout contains the token; `--no-browser` does not open a browser.
- [ ] **Step 2:** Implement with `freemapper_web::serve`, `ServeOptions { port, csp: "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; form-action 'none'", strict_post: true, .. }`, `open_browser` unless `--no-browser`, project argument resolved with `ops::project_file`. Ctrl+C and `/api/done` shut down.
- [ ] **Step 3:** Help text and Fluent keys DE/EN (`edit-about`, `edit-no-browser`, `edit-listening`), through unslop/humanizer. Commit `feat(editor): serve the editor shell`.

### Task 3: Page shell on the suite assets

**Files:**
- Create: `editor/web/{index.html,app.css,app.js}`; Fluent keys `editor-*` in `i18n/{de,en}`; test in `editor/page_tests.rs`

**Interfaces:**
- Consumes: `/{token}/suite/{suite.css,api.js,texts.js,status.js,tree.js}`, core `revision(bytes)`, the core test helper for page assets.
- Produces: `GET api/texts`; page regions tree (`tree.js`), sheet, properties, `#notices` (`aria-live`), top menu `Datei`, `Bearbeiten` (Undo/Redo), `Ansicht`, `Export`, status bar (`status.js`).

- [ ] **Step 1:** Tests: both catalogs have the same `editor-*` keys; every `data-*` key exists; the core test helper passes on the page's assets (no inline script/handler, nothing from the network, size limit); POST without Origin or revision header is refused; language order `--lang`, `FREEMAPPER_LANG`, `Accept-Language`, `en`.
- [ ] **Step 2:** Implement the shell with the suite modules and only app-specific CSS in `app.css`. Pin core v0.21.0; no local copy of suite files is needed.
- [ ] **Step 3:** Commit `feat(editor): page shell on the suite assets`.

### Task 4: Read API and the sheet preview

**Files:**
- Create: `editor/api.rs`, `editor/preview.rs`

**Interfaces:**
- Consumes: `ops::project::info`, `ops::check::check`, `ops::load_project`, core `write_svg` / `SvgView`, `BuildReport.source_objects`, hit test.
- Produces: `GET /api/project` → the shape of `ops::project::info` (`shared_elements`, `courses_file`, `map`, no absolute path) plus `lang` and `revision`; `GET /api/preview?course=&layout=&rev=` → `{svg, page_mm, notes, elements: [{id, kind, rect_mm}]}`; `POST /api/hit {course, layout, x_mm, y_mm}` → `{kind: element|number|map_object|mask, id}`.

- [ ] **Step 1:** Tests against a fixture project (example `luebeck-competition-layouts`): `/api/project` equals `ops::project::info` shape; preview SVG is the exact output of the renderer for the same inputs (compare with `write_svg` directly); a free page layout previews at `page_mm`; a project without courses previews the layout; a newer `rev` request supersedes an older one (older returns 409 `superseded`).
- [ ] **Step 2:** Implement; rendering runs on a blocking task so the server stays responsive; revision is core `revision(bytes)`.
- [ ] **Step 3:** Page: tree lists sources, layouts, masks, shared elements, assignments (as in mockup editor-v2); course selector; preview in an `<svg>` container scaled to fit, 1 mm = 1 user unit; notes below the sheet. Commit `feat(editor): project tree and sheet preview`.

### Task 5: Write API, revision guard and undo

**Files:**
- Create: `editor/state.rs`; modify `editor/api.rs`

**Interfaces:**
- Produces: `POST /api/layout`, `/api/element`, `/api/assign` taking the `ops` argument structs plus `base_revision`; `POST /api/undo`, `/api/redo`; every response `{revision, can_undo, can_redo}`. `State { undo: Vec<String>, redo: Vec<String> }` with a cap of 100 snapshots.

- [ ] **Step 1:** Tests: edit then undo restores the exact bytes (including comments); redo re-applies; `base_revision` mismatch → 409 and the file is untouched; the file changed on disk between edit and undo → undo refuses with the notice `edit-changed-outside`; new edit clears redo; the 101st snapshot drops the oldest.
- [ ] **Step 2:** Implement: read file text and hash, call `ops::set_layout` etc., push the previous text on success only. Writes stay atomic (existing writer).
- [ ] **Step 3:** Page: Undo/Redo menu items and Ctrl+Z / Ctrl+Shift+Z, conflict notice with a `Neu laden` button, revision polling. Commit `feat(editor): edits with revision guard and undo`.

### Task 6: Layout and element editing on the sheet

**Files:**
- Modify: `editor/web/app.js`, `app.css`, `api.rs`

- [ ] **Step 1:** Tests (Rust, API level): moving an element by dragging maps to `set_element` with new `offset_mm` in the anchor's coordinates for each of the four corners and both `to` values (`page`, `map_frame`); a layout form with `page_mm` sets and clears the key.
- [ ] **Step 2:** Page: click selects via `/api/hit`; drag moves with 0.5 mm snap and arrow keys nudge by 0.5 mm (1 mm with Shift); right panel forms for the selected layout (paper or free page, orientation, print scale, margin, border, north lines) and for elements (kind-specific keys as in `set_element`), placeholders list from the manual. Only the final position is sent on pointer-up. Commit `feat(editor): select and move elements, layout form`.

### Task 7: Masks

**Files:**
- Modify: `editor/api.rs`, `app.js`

- [ ] **Step 1:** Tests: window mask for given controls and radius equals `ops::masks::window`; corridor from a polyline equals `ops::masks::corridor`; assignment per course; a mask id that exists is refused with the existing sentence.
- [ ] **Step 2:** Page: window mode (click controls, radius field in mm on paper), corridor mode (click points, double-click ends, Escape cancels, width field), assign to a course; mouse-only drawing is stated in the panel hint; the corridor needs a line the person draws (no automatic corridor, as the rules say). Leaving the page with a half-drawn corridor asks first. Commit `feat(editor): draw window and corridor masks`.

### Task 8: Control numbers

**Files:**
- Modify: `editor/api.rs`, `app.js`

- [ ] **Step 1:** Tests: dragging a number to a point stores direction (0 = east, counter-clockwise) and gap in mm on paper through `ops::numbers::accept`, for `layout_wide` and `sheet`; the proposal of `check` is accepted with one call; an existing correction is refused with the existing sentence.
- [ ] **Step 2:** Page: conflicts from `check` are listed in the panel with `Vorschlag annehmen`; dragging a number asks `für das Layout` or `nur dieses Blatt`. Commit `feat(editor): move and accept control numbers`.

### Task 9: Export, check, closing

**Files:**
- Modify: `editor/api.rs`, `app.js`

- [ ] **Step 1:** Tests: `POST /api/export {out_dir, courses?, layouts?, single_file}` equals `ops::export`, refuses a missing folder with the existing sentence, can be cancelled and stops after the sheet in hand; `/api/check` returns the notes with severity and `needs_attention`.
- [ ] **Step 2:** Page: Export menu with a folder text field (a browser cannot pick a folder), remembered in `localStorage` with try/catch only as a convenience; result list with the reminder to print at actual size; `Fertig` ends the server after the question in Review Focus; the terminal prints where the project file is. Commit `feat(editor): export, check and closing`.

### Task 10: Docs, examples, release prep

- [ ] **Step 1:** Manual chapter DE/EN (editor: start, regions, undo, keyboard and mouse), README.md/README.en.md status line (M6 now present, no longer "noch nicht da"), `AGENTS.md` (the editor is for people; agents use CLI/MCP; `edit` row in the command table), glossary, changelog DE/EN, ADR index. A screenshot per language is a user step, not committed from this machine.
- [ ] **Step 2:** Final whole-branch review on the most capable model; `Ruling:` lines of the ledger into ADR 0059 before the worktree goes; merge `--no-ff`, push both remotes, watch CI, release as a minor version when the Linux runners are back, `cargo clean`, scratch cleanup, update the memory file.

## Open questions (not blocking Task 1 to 4)

1. Core v0.21.0 carries the suite assets (`/{token}/suite/` is reserved; helpers `revision`, `REVISION_HEADER`, `stale`, `refused`, `texts_language`; feature `page-rules` with `check_page_rules(files, budget)`). `api/texts` must also deliver `suite-banner-dismiss`; the title comes via `<title data-text>`; contracts (class names, `body.frame`, `#status-message`, `#banner`) are in the addendum of core ADR 0030. `tree.js` keeps `collapsed` per module, so one tree per page.
2. The area page's conversion (who, when) is the user's decision; it does not block print.
3. Dark mode, accent and text mechanism are settled by core ADR 0030.
