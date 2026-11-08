# 0006. Dependencies at least two weeks old

Date: 2026-11-08
Status: Accepted

## Context

freemapper only uses crate versions that are at least two weeks old (its ADR 0012). freemapper-print shares
its tooling and many of its crates.

## Decision

Every dependency version must be at least two weeks old. Where a newer release is younger, the older version is
pinned exactly, in `Cargo.toml` or in `Cargo.lock` for transitive crates. The first `Cargo.lock` was taken from
freemapper, so shared crates start at versions already checked there.

## Consequences

- `cargo update` can pull in a version that is too young, so every lock file change is checked against this
  rule.
- Cost if wrong: fixes from the newest releases arrive up to two weeks later.

Source: freemapper ADR 0012.
