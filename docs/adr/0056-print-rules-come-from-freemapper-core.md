# 0056. The print rules come from freemapper-core

Date: 2026-11-11
Status: Accepted, amends [0031](0031-print-rules-as-data-profiles.md) and [0018](0018-symbols-scale-as-the-iof-standards-set.md)

## Context
freemapper-courseplan draws the course symbols in its editor with the same sizes as the sheet and had to copy
the values from `rules::base().course_symbol(kind)`, which lived here. Two copies of a standard's numbers
drift apart.

## Decision
- The rule set, the profile `iof-print` with its README and the scale tables move to the core crate
  `freemapper-printrules` (core ADR 0025, v0.16.0). Behaviour does not change; the golden snapshot of the
  resolved values moved with them.
- print re-exports the crate as `printrules`, the module `rules` from it, and `scale` re-exports its `scale`.
  The paths `crate::rules` and `crate::scale` stay as they were.
- What needs a map file stays here: `read_sidecar`, `read_sidecar_title` and `infer_kind` in `scale.rs`.
- The range of a description cell (`MIN_CELL_MM`, `MAX_CELL_MM`) is defined in the crate, and `project` re-exports
  it.
- Tests of print that need test-only profiles use the feature `test-support`.
