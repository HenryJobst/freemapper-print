# 0055. The course overprint comes from freemapper-core

Date: 2026-11-11
Status: Accepted, amends [0051](0051-render-comes-from-freemapper-core.md)

## Context
The browser editor of freemapper-courseplan needs the same course symbols as the sheet (parity test, print
preview). The code that draws them, `overprint`, takes core types only, apart from one struct of sizes.

## Decision
- The module `overprint` moves to `freemapper-render` (core ADR 0024, v0.15.0). print re-exports it as
  `overprint`, so the paths in print stay as they were.
- `CourseSymbolSizes` moves with it. The rule set (`rules.rs`) still fills it from a profile and re-exports it.
- The scale policy stays here: print computes the sizes, the course factor and the page transform, and passes
  them in.
- The tests that need print's rule sets or page builder stay in `overprint_tests.rs`. Output is unchanged; the
  example PDFs stay byte-identical.
