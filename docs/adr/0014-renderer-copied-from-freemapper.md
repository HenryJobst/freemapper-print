# 0014. The renderer is copied from freemapper and developed here

Date: 2026-11-08
Status: Accepted, amended by 0043

## Context

freemapper's renderer (`crates/freemapper/src/render/`) draws its own map model and lacks area patterns,
embedded fonts and images. freemapper-print needs all of them. A crate shared by both repositories would make
every renderer change a commit in two repositories plus a version bump.

## Decision

The renderer is copied into `crates/freemapper-print` and developed further here. The copy records the
freemapper commit it came from (`743ce4b`). freemapper reports every later fix to its renderer. freemapper switches to this renderer later, in a separate step.

## Consequences

- M1 does not wait for changes in freemapper.
- Until freemapper switches, the code exists twice. A fix to freemapper's renderer must be carried across by
  hand.
- Cost if wrong: the two copies drift apart and the later merge takes longer.

Amended by ADR 0043: the `.omap` model and reader, copied together with the renderer, now come from
`freemapper-omap` (freemapper-core v0.3.0) and are gone from this repository. The renderer is still copied and
developed here.

Source: user decision 2026-11-08 after the design review.
