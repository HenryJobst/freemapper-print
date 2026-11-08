# 0009. Own renderer, conformant to the standard but not pixel-identical

Date: 2026-11-08
Status: Accepted

## Context

OOM has no command-line or library mode for PDF export (open issues #772 and #2345). OCAD does not run on the
user's Mac. freemapper already renders OOM symbols to PDF for its own map model. The user accepts small
differences from OOM and OCAD as long as the print is correct and readable.

## Decision

freemapper-print renders maps itself. The renderer follows ISSprOM and ISOM dimensions and OOM's drawing rules
as described in freemapper's `docs/render/oom-render-reference.md`, and it may port OOM code (ADR 0002). Tests
compare the output with OOM's PDF export by raster difference within a tolerance, not pixel by pixel.

## Consequences

- No dependency on OOM being installed or remote-controllable.
- The renderer has to grow area patterns, embedded fonts, images and a full `.omap` object reader.
- Cost if wrong: a symbol that renders differently enough to mislead a runner. The OOM comparison tests are
  the guard.

Source: brainstorming 2026-11-08 (renderer research, user: small differences are tolerable if the print is
correct).
