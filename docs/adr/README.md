# Architecture Decision Records

An ADR records one decision that shapes freemapper-print: what was decided, why, and what it costs if it turns
out wrong. Each ADR ends with a `Source:` line that names where the decision was made. Several ADRs take over a
freemapper decision; their source line names the freemapper ADR
(<https://github.com/HenryJobst/freemapper/tree/main/docs/adr>).

## Numbering

ADRs are numbered from 0001 in the order the decisions were made. A number is never reused. The file name is
`NNNN-short-kebab-title.md`. A decision that replaces an older one gets a new ADR, and the old one changes its
status to `Superseded by NNNN`. Its text stays as it was.

## Template

```markdown
# NNNN. Title

Date: YYYY-MM-DD
Status: Accepted

## Context

What forced a decision: the facts, limits and data that were known.

## Decision

What was decided, in the active voice.

## Consequences

What follows from it, good and bad, and what it costs if the decision is wrong.

Source: spec §…, plan …, commit …
```

Keep an ADR between 10 and 30 lines. Write it in English, like all technical docs, and use the terms from
`docs/glossary.md`.

## Index

| Number | Title | Status | Date |
|---|---|---|---|
| [0001](0001-rust-with-a-pure-rust-core.md) | Rust with a pure-Rust core | Accepted | 2026-11-08 |
| [0002](0002-gpl-3.0-or-later.md) | GPL-3.0-or-later | Accepted | 2026-11-08 |
| [0003](0003-glossary-first-english-code-bilingual-docs.md) | Glossary first, English code, bilingual user docs | Accepted | 2026-11-08 |
| [0004](0004-mcp-over-stdio-as-a-primary-interface.md) | MCP over stdio as a primary interface | Accepted | 2026-11-08 |
| [0005](0005-layout-editor-in-a-local-browser-page.md) | Layout editor in a local browser page | Accepted | 2026-11-08 |
| [0006](0006-dependencies-at-least-two-weeks-old.md) | Dependencies at least two weeks old | Accepted | 2026-11-08 |
| [0007](0007-rust-toolchain-pinned.md) | The Rust toolchain is pinned in rust-toolchain.toml | Accepted | 2026-11-08 |
| [0008](0008-one-project-file-for-editor-cli-and-mcp.md) | One project file for editor, CLI and MCP | Accepted | 2026-11-08 |
| [0009](0009-own-renderer-conformant-not-pixel-identical.md) | Own renderer, conformant to the standard but not pixel-identical | Accepted | 2026-11-08 |
| [0010](0010-courses-from-iof-xml-and-purple-pen-masks-owned-here.md) | Courses from IOF XML 3.0 and Purple Pen, masks owned by freemapper-print | Accepted | 2026-11-08 |
| [0011](0011-club-training-first.md) | Club training first | Accepted | 2026-11-08 |
| [0012](0012-symbols-keep-their-printed-size.md) | Symbols keep the printed size the standard sets | Accepted | 2026-11-08 |
| [0013](0013-example-projects-built-on-freemapper-examples.md) | Example projects built on freemapper's examples | Accepted | 2026-11-08 |
| [0014](0014-renderer-copied-from-freemapper.md) | The renderer is copied from freemapper and developed here | Accepted | 2026-11-08 |
| [0015](0015-course-content-in-course-setting-presentation-here.md) | Course content stays in course setting, presentation belongs here | Accepted | 2026-11-08 |
