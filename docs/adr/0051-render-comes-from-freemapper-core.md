# 0051: The renderer comes from freemapper-core

Status: accepted, amends ADR 0014 and ADR 0043

## Context

ADR 0014 copied the renderer from freemapper and developed it here, with the aim of moving it to the shared
library later. freemapper-courseplan builds a browser editor that has to show the map exactly as the PDF shows
it, and the agreement of the suite is one renderer, not a third copy. The renderer depends only on
freemapper-omap, and `RenderedPage` (page millimetres, y down) is the single input of the PDF writer.

## Decision

- The renderer lives in freemapper-core as `freemapper-render` since v0.9.0 (core ADR 0019): the page model, the
  drawing of OOM symbols, the PDF writer, the fonts with their metrics. The page builder `print::render_page`,
  the options and the report stay here for now; a later step splits them out.
- freemapper-print re-exports the crate as `render` in `lib.rs`, like `omap` and `course` (ADR 0043). The
  boundary test for the module `render` is gone, because Cargo keeps a crate from reaching into this one. The
  test that code here names core crates only through `lib.rs` covers `freemapper_render` too.
- The Arimo font files left this repository; only the licence text stays, because the release archives ship it.
- Dev-dependency: `freemapper-render` with the feature `test-support`, which opens a few test helpers.
- Owner of the crate stays freemapper-print.

## Consequences

No printed output changes: the eight example PDFs are byte-identical to the committed ones. A SVG backend and
the mapping between page and map follow in the crate, for the browser editors.
