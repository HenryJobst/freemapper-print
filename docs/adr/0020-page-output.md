# 0020. Page output: margin, clipping, overflow and orientation

Date: 2026-11-08
Status: Accepted

## Context

A print must keep its scale (ADR 0018), and office printers rarely print to the paper edge. A map larger than
the paper is cut, and the user has to learn by how much. Later backends (SVG or tiles in the browser editors)
must draw the same page.

## Decision

- The page is exactly the paper, with 5 mm white margin on each side (`PAGE_MARGIN_MM`); the map is clipped to
  the rest and never scaled to fit. Without an explicit centre, the middle of what is painted lands in the
  middle of the page.
- The overflow is measured on the painted bounds (line widths, symbols, texts, clipped patterns), per axis.
- `Auto` orientation takes the orientation with the smaller overflowing area, then the one with the smaller
  overflow lengths, and portrait on a tie. The CLI suggests the other orientation only when the user chose one.
- `render_page` returns a `RenderedPage` (`render::page`) for every backend: primitives in paint order (highest
  priority number first), clip, page size, colours with CMYK, RGB and opacity. `write_pdf` adds `PdfMeta`.
- Objects of hidden or unknown symbols are skipped and counted. The information block of
  `--show-sheet-objects` scales with the map, so its scale text can be wrong at another print scale.

## Consequences

- The user sees in millimetres how much is cut and can choose A3, the other orientation or another scale.
- Cost if wrong: a map that just fits may be reported as overflowing by half a line width, because a stroke
  counts half its width beyond flat caps too.

Source: plan M1 Task 7, controller rulings 2026-11-08, final review of M1.
