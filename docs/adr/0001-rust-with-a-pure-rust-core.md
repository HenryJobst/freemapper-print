# 0001. Rust with a pure-Rust core

Date: 2026-11-08
Status: Accepted

## Context

freemapper-print must run on macOS, Linux and Windows for club course setters who have no GIS and no OCAD. It
builds on freemapper, which is written in Rust and already renders OOM symbols to PDF with pdf-writer.
freemapper's ADR 0001 made the same choice for the same users.

## Decision

freemapper-print is written in Rust (edition 2024) with a pure-Rust stack. All domain logic lives in the
library crate `crates/freemapper-print`. The binary crate `crates/freemapper-print-cli` holds the CLI, the MCP
server and the layout editor's web server. Releases ship one static binary per target, built by the release
workflow taken over from freemapper.

## Consequences

- Code from freemapper (renderer, OOM model, CLI and MCP patterns) can be reused without a language boundary.
- Readers for `.omap`, `.ppen`, IOF XML and later `.ocd` are written in Rust or taken from Rust crates.
- Cost if wrong: a format or PDF feature that no Rust crate covers has to be written by hand.

Source: brainstorming 2026-11-08; freemapper ADR 0001.
