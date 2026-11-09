# 0042. Suite crates as git dependencies

Date: 2026-11-09
Status: Accepted

## Context
Shared suite code lives in freemapper-core, not on crates.io. Core ADR 0006 says how an app takes it in.

## Decision
- Core crates are git dependencies pinned to a tag (`{ git = "https://github.com/HenryJobst/freemapper-core",
  tag = "v0.2.0" }`) in `[workspace.dependencies]`. One tag for the workspace.
- Against an unreleased core change, `Cargo.toml` carries a `[patch."https://github.com/HenryJobst/freemapper-core"]`
  section with paths into `../freemapper-core`. It never reaches `main`; it goes out before the merge, after core
  has released the change and we pin the new tag.
- The suite's own crates are exempt from ADR 0006 (core ADR 0004): a core release may be pinned the day it
  appears. Third-party crates keep the two-week rule.
- Owners (core ADR 0005): `text` and `rules` freemapper-courseplan; `georef` and `fs` freemapper; `omap`,
  `render` and `course` freemapper-print, after the extraction.
- A change to a crate we do not own is agreed with its owner over SendMessage first.
- In use: `freemapper-text` (ADR 0024), `freemapper-rules` (ADR 0031), `freemapper-fs` (ADR 0041). `omap`,
  `render` and `course` follow and move to core with us as owner.

## Consequences
- CI fetches core over HTTPS from the public repository; no credentials, no workflow change.
- A fault in a fresh core release reaches us at once; our tests and the byte-identical PDFs are the guard.
- Cost if wrong: a pinned tag lags behind a fix on core's `main`; we patch locally and wait for the tag.

Source: core ADRs 0004, 0005, 0006; core-switch plan Task 4.
