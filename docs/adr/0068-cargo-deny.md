# 0068. cargo-deny checks the dependencies

Date: 2026-11-15
Status: Accepted

## Context
ADR 0067 put freemapper-print under AGPL-3.0-or-later. The licences of the dependencies change with every
`cargo update`, and nothing checked them. ADR 0006 limits the age of a dependency but says nothing about its licence,
known vulnerabilities or source.

## Decision
- `deny.toml` at the repository root configures `cargo deny check`. It allows the licences the dependencies use now
  (permissive ones, MPL-2.0 and AGPL-3.0-or-later), denies registries and git sources other than crates.io and
  freemapper-core, denies yanked crates, and reports crates present in two versions as a warning only.
- Wildcard dependencies are allowed: the workspace members and the core crates are path or git dependencies without a
  version, and the check flags those.
- Three advisories are ignored, each with its reason in the file. RUSTSEC-2026-0457 and RUSTSEC-2026-0462 concern the
  OAuth and Streamable HTTP client of rmcp, which print does not build (it uses the `server` and `transport-io`
  features: a server on stdin and stdout). RUSTSEC-2026-0192 says `ttf-parser` is unmaintained and offers no upgrade;
  replacing it is a decision for freemapper-core.
- CI runs the check as its own job, with the action pinned to a commit.

## Consequences
- A new dependency with a licence not on the list fails CI until someone decides and adds it here. A new advisory fails
  CI as well, also on a day when nothing in the repository changed; the fix is an upgrade or a reasoned ignore.
- The rmcp ignores end when rmcp 3.9.0 or later is old enough to take (ADR 0006); the file says so.

## Amendment 2026-11-15
freemapper-core v0.29.0 reads fonts with skrifa instead of ttf-parser (ADR 0037 there), and print's own
`ttf-parser` dependency had no use left in the code and is gone. The ignore for RUSTSEC-2026-0192 is removed; two
ignores remain, both for rmcp.
