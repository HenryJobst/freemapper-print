# 0049: The description symbols come from freemapper-core

Status: accepted, amends ADR 0036

## Context

ADR 0036 embedded Purple Pen's `symbols.xml` and its reader in freemapper-print. freemapper-courseplan needs the
same table, with the mapping between Purple Pen ids and IOF 2024 numbers. freemapper-core v0.6.0 holds it as
`freemapper_course::description_symbols` (core ADR 0017): the same file, byte for byte (SHA-256 `84b51153…`),
with the reader, the geometry and the mapping.

## Decision

- freemapper-print uses the module through `crate::description_symbols`, re-exported in `lib.rs` like the other
  core items (ADR 0043).
- `describe::symbols` keeps the old names as re-exports and one function that takes the sheet language.
- The embedded file and its README leave this repository. Source, commit and the BSD notice are in core's
  `crates/freemapper-course/data/README.md`; `THIRD_PARTY.md` here keeps the notice for the release archives.

## Consequences

No printed output changes: the example PDFs are byte-identical. The reader's tests live in core now.
