# Renderer to core with an SVG backend: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task.

**Goal:** Move `render` out of freemapper-print into freemapper-core as `freemapper-render`, add an SVG backend next to the PDF backend, and give browser editors the page-to-map mapping and a hit test. Display and PDF draw the same `RenderedPage`.

**Architecture:** `render/` depends only on `freemapper-omap` (checked: its `crate::omap` uses are `MapCoord`, `MapObject`, `SymbolSet`, `PatternOrigin`, `closed_ring`, `OmapError`). `RenderedPage` (page millimetres, y down) is already the single input of the PDF writer. The page builder (`print::render_page`, symbol factor, geometry factor, centring, overflow, North lines, base area) moves too, because the editor needs the same page. print keeps options, layouts, sheet elements, the course overprint and masks, and re-exports the crate as `render` (ADR 0043 pattern).

**Spec:** `docs/superpowers/specs/2026-11-08-freemapper-print-design.md`; the agreement with freemapper-courseplan (one renderer, no third copy).

## Global Constraints

- Core crates share one tag (ADR 0042); the release is a minor bump (v0.9.0), print pins it afterwards.
- No output change: the 11 example PDFs stay byte-identical across the move (pdf content stream compared).
- Source is English; glossary row first for new terms ("page mapping", "hit"); ADR in core and in print.
- Boundary test in print: core paths only via `lib.rs` re-exports.
- Owner after the move: freemapper-print (render), courseplan consumes.

## Review Focus

- Huge map or tiny viewport: SVG size stays bounded (viewport crops primitives; budget as the PDF page budget).
- Text: SVG must not depend on installed fonts (glyph outlines as paths, same font data as the PDF).
- Overprint Multiply and map clips (masks) render the same in SVG as in PDF.
- Click on a hole, on an object under a mask, on the sheet layer: defined results, no panic.
- Zoom and pan: the same viewport always yields the same SVG text (deterministic, golden-testable).

## Tasks

### Task 1: Crate `freemapper-render` in core (move, no behaviour change)
- Create `crates/freemapper-render` in `../freemapper-core` with the contents of `crates/freemapper-print/src/render/` plus the page builder from `print.rs` (`render_page`, `PrintOptions`-independent parts split into a `PageSpec { print_scale, kind, show_sheet_objects }`).
- Dependencies move with the code (`pdf-writer`, `miniz_oxide`, `image`, fonts data). Check ADR 0006 (two weeks old) stays satisfied.
- Test: the existing render and print tests move unchanged; core CI green.
- Interfaces produced: `freemapper_render::{page::RenderedPage, pdf::write_pdf, build::render_page(map, &PageSpec) -> (RenderedPage, PageReport)}`.

### Task 2: print switches to the crate
- Pin v0.9.0 (tag after Task 1 merges), `pub use freemapper_render as render;` in `lib.rs`, boundary test list extended, delete `src/render/`.
- Test: `cargo test --workspace` green; all 11 example PDFs identical (content stream) to v0.5.1 output.

### Task 3: Page mapping and hit test (in `freemapper-render`)
- `PageMapping { scale, geometry_factor, centre_mm, page_mm }` with `to_page(map_mm: Pt) -> Pt`, `to_map(page_mm: Pt) -> Pt` and `world`-style helpers; carried in `RenderedPage`.
- `RenderedPage::hits_at(&self, p: Pt, tolerance_mm: f64) -> Vec<Hit>` with `Hit { object: u32, symbol: u32, priority: i32 }`, topmost first; map primitives only; respects `clip` and `map_clips`.
- Tests: round trip of the mapping at several scales; hit on an area, a line within tolerance, a hole; miss outside clip; hit under a mask hidden.

### Task 4: SVG backend `svg::write_svg`
- `write_svg(page: &RenderedPage, view: &SvgView) -> Result<String, SvgError>` with `SvgView { rect: Option<PaperBox>, zoom: f64 }` (None = whole page); layers in the same paint order as the PDF writer: map primitives (with lower overprint inserted at `below_priority`), upper overprint, sheet layer; masks as `clip-path` with `fill-rule`; overprint via `mix-blend-mode: multiply`; CMYK to sRGB with one documented conversion.
- Text as glyph-outline paths from the PDF's font data; images as data URIs.
- Primitives outside `view.rect` are dropped; budget error like the PDF's.
- Tests: golden SVGs for a small map (area, line, dashed line, pattern, text, mask, overprint); a parity test that walks the same `RenderedPage` through both backends and checks each primitive count and bounding box.

### Task 5: Release and hand-over
- Core v0.9.0 (changelogs DE/EN, ADR 0019 in core, ADR in print), print pins it, README/manual note for developers, message courseplan with the API.

## Order

Task 1 and 2 first (pure move, safe), then 3 and 4 (parallel in principle, serial in practice), then 5. Before M5.
