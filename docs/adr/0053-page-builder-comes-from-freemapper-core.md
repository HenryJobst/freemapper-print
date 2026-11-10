# 0053: The page builder comes from freemapper-core

Status: accepted, amends ADR 0051

## Context

ADR 0051 moved the renderer to freemapper-core but kept the page builder here: the code that turns a map, the
print scale and the paper into a `RenderedPage`. The browser editor of freemapper-courseplan needs the same page
as the PDF, so the builder has to live next to the renderer.

## Decision

- The builder is `freemapper_render::build` since freemapper-core v0.13.0 (core ADR 0023): `build_page`,
  `build_page_within`, `PageSpec`, `BuildReport`, `Paper`, `Orientation`, `frame_box` and the tag helpers.
- The scale policy stays here. `scale::symbol_factor` and `scale::geometry_factor` decide how much symbols and
  geometry grow; the builder takes both as inputs in `PageSpec` and does not know map kinds. `render_page`
  computes them first, so a print scale outside the table still fails before any symbol is touched.
- `print.rs` wraps `BuildReport` into `PrintReport` (adding `print_scale` and `non_standard_scale`) and maps
  `BuildError::Symbols` to `PrintError::Symbols`, so the error text and variant do not change. It re-exports the
  moved public items, so other modules keep their paths.
- Tests that checked the builder alone moved to core; the ones that check the scale policy end to end stay.

## Consequences

No printed output changes: the example PDFs are byte-identical to the committed ones. The builder does not
validate its factors; this crate never passes zero or negative ones.
