# 0050: Reader tests against the suite contract

Status: accepted

## Context

freemapper-print reads maps and `freemapper.toml` files that freemapper writes. What print may rely on was agreed
between the sessions and written into `CLAUDE.md`, and tested nowhere. freemapper v0.9.0 tagged the scale bar
`freemapper:source=scale-bar`; print knew only `sheet` and printed the bar as map content until v0.5.1.
freemapper-core v0.8.0 holds the contract as a crate, `freemapper-contract` (core ADR 0018), with two maps exactly
as freemapper wrote them and the values a reader must derive from each.

## Decision

- `freemapper-contract` is a dev-dependency with the feature `fixtures`, on the same core tag as the other core
  crates (ADR 0042).
- `crates/freemapper-print/src/contract.rs` holds the reader tests. Each writes every fixture into a temporary
  folder, reads it with print's own code and compares with `expected.toml`:
  - `contract_kind`: `read_sidecar` and `infer_kind` name the kind of the sidecar.
  - `contract_title`: `read_sidecar_title` returns the title.
  - `contract_classes`: `is_sheet_object` and `is_base_object` accept as many objects as the fixture counts.
  - `contract_scale`: the map scale is allowed for the kind.
  - `contract_bounds`: a page rendered without sheet objects ends north of the title block and the scale bar, and
    its painted box equals the box of the map content within 1 mm.
- The names start with `contract_`. The gate planned for core's CI runs them by that prefix against a new core
  before it is released.
- `is_sheet_object` and `is_base_object` are `pub(crate)` for these tests.
- A failing contract test is a finding about print or about the contract. It is settled with the freemapper
  session and never by loosening the test here.

## Consequences

- With `is_sheet_object` reduced to the old tag, `contract_classes` and `contract_bounds` fail. That was tried
  before the merge.
- The fixtures name templates that are not shipped. print never draws templates, so nothing is missing.
- The test build embeds about 540 kB of fixture text. Release binaries are unchanged.
- Moving the core tag now also moves the fixtures. A core release that changes the contract fails these tests
  until print follows.
