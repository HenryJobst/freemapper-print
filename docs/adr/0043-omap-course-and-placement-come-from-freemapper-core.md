# 0043. Omap, course and placement come from freemapper-core

Date: 2026-11-09
Status: Accepted

## Context
The `.omap` model and reader (copied from freemapper, ADR 0014), the course model with its readers and fit
(ADR 0032, 0033) and the number placement (ADR 0038) moved to freemapper-core v0.3.0 as `freemapper-omap` and
`freemapper-course` (core ADR 0012, 0013). freemapper and freemapper-courseplan read the same files.

## Decision
- Both crates are git dependencies at tag `v0.3.0` (ADR 0042), and text, rules and fs move to the same tag.
- `lib.rs` re-exports them under the old module names: `pub use freemapper_omap as omap;`,
  `pub use freemapper_course as course;` and `pub use freemapper_course::placement;` (ruling R1). Call sites
  did not change. No other file names the crates directly; a test in `lib.rs` checks that.
- `src/omap/`, `src/course/`, `src/placement/`, `assets/symbol-sets/`, the reader and fit tests and the
  fixtures only they read left with the code. A smoke test loads the Lübeck example's courses through
  `load_courses` (ruling R4).
- `numbers::file_placement` calls the crate's `placement_from_offset`, the same formula.
- For the model and the reader this replaces the "copied from freemapper" part of ADR 0014; the renderer stays
  here. ADR 0039's tests for `omap/` and `course/` give way to the crate boundary, which the compiler checks.
  The test for `render/` stays and now forbids the course crate too.

## Consequences
- All 11 example PDFs are byte-identical, and `check` takes as long as before.
- A fix to the reader, the fit or the placement is a core change and needs a core tag. Print owns both crates
  (core ADR 0005); courseplan and freemapper pin the tag they want.
- Cost if wrong: print waits for a core release before it can ship such a fix, or patches locally (ADR 0042).

Source: plan "core extraction stage 2" Task 3; rulings R1 and R4; core ADR 0012 and 0013.
