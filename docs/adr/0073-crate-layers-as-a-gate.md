# 0073. The crate layers are a gate

Date: 2026-11-20
Status: Accepted

## Context

The workspace has two crates. `freemapper-print` is the library: it reads maps, applies the print
rules and writes the PDFs. `freemapper-print-cli` is the driver: arguments, the MCP and web servers,
the self-update and the user's settings folders. The line between them held by habit only. Nothing
stopped a convenient `anyhow`, `clap` or `ureq` from slipping into the library, which would drag a
command line, an async runtime or a network client into every program that builds on it.

A test crate, as in the sibling projects with many crates, would be too much for two. The check
needs no build: `cargo metadata --no-deps` lists every dependency.

## Decision

- **`scripts/check.sh architecture`** reads the workspace with `cargo metadata` and fails when
  - a workspace member is in no layer (a new crate is a decision, placed in `LAYERS` in the gate),
  - the library depends on the driver,
  - the library depends on a crate only the driver may use: `anyhow`, `axum`, `clap`, `dirs`,
    `fluent-bundle`, `freemapper-text`, `freemapper-update`, `rmcp`, `rust-embed`, `schemars`,
    `tokio`, `ureq`.
  Development dependencies are not counted: tests may use what they need.
- **Measured:** the gate needs under a second and passes on today's tree. Adding `anyhow` to the
  library fails it with a line that names the crate and the dependency; the probe was reverted.
- **It is a cheap gate:** it runs in the `hygiene` job and before a commit, and the badge for the
  number of checks counts it.
- **Rejected: a rule on source text** (for example no `eprintln!` in the library). The library
  prints one warning with `eprintln!` today (`print.rs`); a ban would need an exception list that
  outlives its reason. The dependency rule carries the layer line; the print stays a candidate.

## Consequences

- A new dependency of the library that belongs to the driver is a failing gate, not a review remark.
- Moving a crate between layers, or adding one, is an edit of `LAYERS` and `DRIVER_ONLY` in
  `scripts/check.sh` with a reason in an ADR.
