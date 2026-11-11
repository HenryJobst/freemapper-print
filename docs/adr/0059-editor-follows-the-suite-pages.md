# 0059. The editor follows the suite page rules

Date: 2026-11-11
Status: Accepted, implements [0005](0005-layout-editor-in-a-local-browser-page.md)

## Context

ADR 0005 chose a local browser page for the layout editor. Since then the suite has agreed on one look and one
frame for its local pages (freemapper-core ADR 0030, tag v0.21.0): shared assets served by `freemapper-web`,
named tokens, Fluent texts through `api/texts`, a revision header for writes, and fixed security and size rules.
The courseplan editor is the reference. print builds its editor on these parts from the start.

## Decision

- **Core ADR 0030 applies as written.** The editor serves the suite assets at `/{token}/suite/`, takes the
  tokens of `suite.css`, delivers its texts as `editor-*` Fluent messages through `GET api/texts`, and guards each
  write with `X-Freemapper-Revision` (409 when the file changed, 422 for a refused change). The reasons are in
  that ADR and are not repeated here.
- **Writes are immediate.** Each change is written at once. There is no Save button and no dirty state. The
  editor keeps an undo stack of whole-file snapshots (glossary: undo snapshot). Undo refuses with one sentence
  when the file changed outside the editor.
- **The operations are the `ops` functions.** `POST api/op/{name}` maps a fixed list, `EDITOR_OPS`, to the
  `ops::*` functions that the CLI and the MCP tools call. An unknown name is 404. The editor has no second
  project file writer (ADR 0008).
- **`Fertig` and closing the window ask only while a drawing mode is open**, for example a corridor that is half
  drawn. Everything else is already written. Ctrl+C in the terminal ends the server at any time.
- **print adds no token of its own.** The suite tokens are used as they are. Anything specific to print stays in
  the editor's `app.css`. The course overprint colour of the sheet is drawn by the renderer, not by a token.
- **Server rules** are those of `freemapper-web`: 127.0.0.1 only, random token as first path segment, Host and
  Origin checks, `strict_post`, a CSP with `'self'` only, bodies up to 1 MiB.
- `freemapper-web` is a core crate and comes through the re-export `web` in `lib.rs` (ADR 0043). `axum` and
  `rust-embed` are used at the versions core pins.

## Alternatives not taken

- Tokens of our own, or a different look: core ADR 0030 rejects this for all three pages.
- A Save button with a dirty state: core ADR 0030 keeps it for freemapper's area page only, because saving there
  can drop fetched data. A print project file change is small and has an undo.

## Consequences

- The editor looks and behaves like the courseplan editor, and a fix in the frame arrives with the next core tag.
- Every operation of the editor is also a CLI command or an MCP tool, so the three adapters cannot drift apart.
- A token rename or a change of the 409/422 meaning is a core change that print takes with the tag.
- Cost if wrong: if the suite rules prove too narrow for a sheet editor (for example a drawing mode that needs its
  own keys or dialogs), the editor adds app-specific parts in `app.css` and `app.js` and the core ADR is amended.
  The `ops` boundary stays, so the cost is page code, not data.

Source: freemapper-core ADR 0030; plan `docs/superpowers/plans/2026-11-11-m6-browser-editor.md`.
