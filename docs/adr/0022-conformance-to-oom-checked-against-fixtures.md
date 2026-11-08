# 0022. Conformance to OOM, checked against fixtures and OOM exports

Date: 2026-11-08
Status: Accepted

## Context

ADR 0009 binds the renderer to OOM's drawing rules, not to pixels. The implementation plan sketched some types
with booleans where OOM has more modes, and the comparison with OOM needs PDFs that only a person with OOM can
export.

## Decision

- Where OOM has more modes than two, the renderer models OOM's modes (pattern clipping, mid symbol placement,
  text framing) instead of a boolean. Counts that follow from an OOM rule, such as the points of a pattern, are
  derived from that rule and recorded in `docs/render/oom-render-reference.md`.
- The test maps are freemapper's Lübeck map (now commit `a0cc8ef`, with water) and OOM 0.9.6's forest sample.
  Their sources are listed in `tests/fixtures/maps/README.md`, and freemapper's renderer tests come along when
  they apply.
- The OOM reference exports use, per map, the first paper and orientation on which the whole map fits at map
  scale, so the comparison sees the full map. Until someone exports them, the comparison test skips with a
  message and does not block a merge. A home test print checks the scale on paper.

## Consequences

- Differences from OOM show up as a raster difference with a known tolerance instead of a review argument.
- Cost if wrong: M1 merges without the comparison having run once; the README asks for the exports.

Source: plan M1 Tasks 2, 4 and 9, controller rulings 2026-11-08.
