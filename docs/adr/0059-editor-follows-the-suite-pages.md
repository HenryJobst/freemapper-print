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
- `Fertig` and closing the window ask only while a drawing mode is open. Ctrl+C ends the server at any time.
- `freemapper-web` comes through the re-export `web` (ADR 0043). `axum` and `rust-embed` are pinned to core's
  versions; the lock file also holds their helper crates at versions at least two weeks old (ADR 0006).

## Consequences

- The editor looks and behaves like the courseplan editor; a fix in the frame arrives with the next core tag.
- Each editor operation is also a CLI command or an MCP tool, so the three adapters cannot drift apart.
- Cost if wrong: a drawing mode that needs its own keys or dialogs is added in `app.css` and `app.js`, and the
  core ADR is amended. The `ops` boundary stays, so the cost is page code, not data.
