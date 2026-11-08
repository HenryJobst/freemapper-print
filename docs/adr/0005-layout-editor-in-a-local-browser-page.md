# 0005. Layout editor in a local browser page

Date: 2026-11-08
Status: Accepted

## Context

Placing a title, logos or a mask by coordinates alone is tedious, so the user wants a visual editor. A native
desktop GUI would need a GUI toolkit per platform. freemapper's area selector (its ADR 0009) already runs as a
local web page served by the binary.

## Decision

`freemapper-print edit` starts a local web server in the binary and opens the editor in the browser. The page
shows the event (sources, layouts, mask variants, shared elements, assignments) on the left, the sheet of one
chosen course in the centre and the selected element's properties on the right. Export lives in the menu. The
security rules follow freemapper: listen on 127.0.0.1 only, a random 128-bit token in every path, a Host check,
a CSP, `X-Frame-Options: DENY` and `no-store`.

## Consequences

- The editor reads and writes the same project file as the CLI and the MCP server (ADR 0008).
- The sheet preview must match the PDF, so the page shows the renderer's own output, not a second renderer.
- Cost if wrong: a browser page limits drawing tools compared with a native GUI.

Source: brainstorming 2026-11-08 (mockups editor.html and editor-v2.html); freemapper ADR 0009.
