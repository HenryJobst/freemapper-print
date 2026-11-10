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
| [0012](0012-symbols-keep-their-printed-size.md) | Symbols keep the printed size the standard sets | Superseded by [0018](0018-symbols-scale-as-the-iof-standards-set.md) | 2026-11-08 |
| [0013](0013-example-projects-built-on-freemapper-examples.md) | Example projects built on freemapper's examples | Accepted | 2026-11-08 |
| [0014](0014-renderer-copied-from-freemapper.md) | The renderer is copied from freemapper and developed here | Accepted, amended by [0043](0043-omap-course-and-placement-come-from-freemapper-core.md) and [0051](0051-render-comes-from-freemapper-core.md) | 2026-11-08 |
| [0015](0015-course-content-in-course-setting-presentation-here.md) | Course setting decides what is the same on every sheet, freemapper-print the rest | Accepted | 2026-11-08 |
| [0016](0016-ai-features-optional.md) | AI features inside the program are optional | Accepted | 2026-11-08 |
| [0017](0017-arimo-as-the-embedded-font.md) | Arimo as the embedded font | Accepted | 2026-11-08 |
| [0018](0018-symbols-scale-as-the-iof-standards-set.md) | Symbols scale as the IOF standards set | Accepted | 2026-11-08 |
| [0019](0019-limits-for-broken-and-hostile-files.md) | Limits for broken and hostile files | Accepted | 2026-11-08 |
| [0020](0020-page-output.md) | Page output: margin, clipping, overflow and orientation | Accepted | 2026-11-08 |
| [0021](0021-map-kind-older-files-and-exit-codes.md) | Map kind inference, older files and CLI exit codes | Accepted | 2026-11-08 |
| [0022](0022-conformance-to-oom-checked-against-fixtures.md) | Conformance to OOM, checked against fixtures and OOM exports | Accepted | 2026-11-08 |
| [0023](0023-north-lines-belong-to-the-map.md) | North lines belong to the map | Accepted | 2026-11-08 |
| [0024](0024-program-texts-in-fluent-files.md) | Program texts in Fluent files (suite rule) | Accepted | 2026-11-08 |
| [0025](0025-event-project-file-format.md) | Event project file format | Accepted | 2026-11-08 |
| [0026](0026-logos-png-jpeg-and-svg-as-paths.md) | Logos: PNG and JPEG as images, SVG as paths from an allow-list | Accepted | 2026-11-08 |
| [0027](0027-generated-north-lines.md) | Generated north lines | Accepted | 2026-11-09 |
| [0028](0028-layout-geometry.md) | Layout geometry: map frame, passe-partout and anchors | Accepted | 2026-11-09 |
| [0029](0029-footer-placement.md) | Footer placement: below the frame when the whole block fits | Accepted | 2026-11-09 |
| [0030](0030-safe-pdf-writing.md) | Safe PDF writing: free-space check, `.part` file, 50 MB reserve | Accepted | 2026-11-09 |
| [0031](0031-print-rules-as-data-profiles.md) | Print rules as data profiles | Accepted | 2026-11-09 |
| [0032](0032-course-coordinates-and-the-map.md) | Course coordinates and the map | Accepted | 2026-11-09 |
| [0033](0033-reading-purple-pen-files.md) | Reading Purple Pen files | Accepted | 2026-11-09 |
| [0034](0034-overprint-layers-and-blending.md) | Overprint layers and blending | Accepted | 2026-11-09 |
| [0035](0035-course-sheets-and-export-per-course.md) | Course sheets and export per course | Accepted | 2026-11-09 |
| [0036](0036-control-description-symbols-from-purple-pen.md) | Control description symbols from Purple Pen | Accepted | 2026-11-09 |
| [0037](0037-control-description-box.md) | Control description box | Accepted | 2026-11-09 |
| [0038](0038-number-positions-and-the-placement-module.md) | Number positions and the placement module | Accepted | 2026-11-09 |
| [0039](0039-module-boundaries-for-freemapper-core.md) | Module boundaries for freemapper-core | Accepted, amended by [0043](0043-omap-course-and-placement-come-from-freemapper-core.md) | 2026-11-09 |
| [0040](0040-m3-closing-rulings.md) | M3 closing rulings: German state names and the slow probe | Accepted | 2026-11-09 |
| [0041](0041-writes-go-through-freemapper-fs.md) | Writes go through freemapper-fs | Accepted | 2026-11-09 |
| [0042](0042-suite-crates-as-git-dependencies.md) | Suite crates as git dependencies | Accepted | 2026-11-09 |
| [0043](0043-omap-course-and-placement-come-from-freemapper-core.md) | Omap, course and placement come from freemapper-core | Accepted | 2026-11-09 |
| [0044](0044-the-base-area-of-sprint-maps.md) | The base area of sprint maps | Accepted | 2026-11-09 |
| [0045](0045-mask-variants.md) | Mask variants | Accepted | 2026-11-10 |
| [0046](0046-checking-masks.md) | Checking masks | Accepted | 2026-11-10 |
| [0047](0047-generated-masks.md) | Generated masks | Accepted | 2026-11-10 |
| [0048](0048-white-outs-by-purple-pen-id.md) | White-outs keyed by Purple Pen id | Accepted | 2026-11-10 |
| [0049](0049-description-symbols-from-core.md) | Description symbols from core | Accepted | 2026-11-10 |
| [0050](0050-reader-tests-against-the-suite-contract.md) | Reader tests against the suite contract | Accepted | 2026-11-10 |
| [0051](0051-render-comes-from-freemapper-core.md) | The renderer comes from freemapper-core | Accepted, amends [0014](0014-renderer-copied-from-freemapper.md) | 2026-11-10 |
| [0052](0052-an-unknown-map-kind-counts-as-absent.md) | An unknown map kind in freemapper.toml counts as absent | Accepted, amends [0050](0050-reader-tests-against-the-suite-contract.md) | 2026-11-10 |
| [0053](0053-page-builder-comes-from-freemapper-core.md) | The page builder comes from freemapper-core | Accepted, amends [0051](0051-render-comes-from-freemapper-core.md) | 2026-11-11 |
