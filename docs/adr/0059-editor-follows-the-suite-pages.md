# 0059. The editor follows the suite page rules

Date: 2026-11-11
Status: Accepted, implements [0005](0005-layout-editor-in-a-local-browser-page.md)

## Context

The suite agreed on one look and one frame for its local pages (freemapper-core ADR 0030, tag v0.21.0). The
courseplan editor is the reference. print builds its editor on the shared parts from the start.

## Decision

- Core ADR 0030 applies as written: suite assets at `/{token}/suite/`, the suite tokens, `editor-*` Fluent
  messages through `GET api/texts`, and `X-Freemapper-Revision` on every write (409 when the file changed, 422
  for a refused change). print adds no token of its own; print-specific styles stay in `app.css`.
- Writes are immediate. There is no Save button and no dirty state; an undo stack of whole-file snapshots
  refuses with one sentence when the file changed outside the editor.
- `POST api/op/{name}` maps a fixed list, `EDITOR_OPS`, to the `ops::*` functions that CLI and MCP call. An
  unknown name is 404. The editor has no second writer for the project file (ADR 0008).
- `move_element` is the one op of the table that is not an `ops::*` function: it reads the element, shifts its
  anchor offset and calls `ops::edit::set_element`, so a drag stays one change in the file. `GET api/spec`
  (layouts and shared elements for the forms) can be read just before a write by another process; the page then
  works on an older state, and its next write answers 409, which is harmless.
- The revision check and the op are two steps, not one lock against other programs. A writer that changes the
  file between the editor's read and the op's own read is not noticed; its change stays in the file, and a
  snapshot taken after it holds it. When the file differs from what the editor left at the next edit, the older
  snapshots are dropped, so undo never writes over an outside change. Undo and redo read the file once more just
  before they replace it. The window is milliseconds on one person's machine and is accepted.
- `replace_text` (undo, redo) writes like the ops' writer: a project file that is a link becomes a regular file,
  and the file the link pointed to stays as it was (core ADR 0009).
- Moving a shared element from the sheet of one layout moves it in every layout. The page asks once per
  session and element before it does.
- `Fertig` and closing the window ask only while a drawing mode is open. Ctrl+C ends the server at any time.
- `freemapper-web` comes through the re-export `web` (ADR 0043). `axum` and `rust-embed` are pinned to core's
  versions; the lock file also holds their helper crates at versions at least two weeks old (ADR 0006).

## Consequences

- The editor looks and behaves like the courseplan editor; a fix in the frame arrives with the next core tag.
- Each editor operation is also a CLI command or an MCP tool, so the three adapters cannot drift apart.
- Cost if wrong: a drawing mode that needs its own keys or dialogs is added in `app.css` and `app.js`, and the
  core ADR is amended. The `ops` boundary stays, so the cost is page code, not data.
