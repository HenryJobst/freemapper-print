# freemapper-print: working rules

Sibling project of freemapper (`../freemapper`, https://github.com/HenryJobst/freemapper).
Remotes: `origin` = https://gitlab.jobst-server.de/henry/freemapper-print.git (private), `github` =
https://github.com/HenryJobst/freemapper-print.git (public). Push at own discretion, never force-push; nothing
secret or private goes into the repository. Read
`docs/superpowers/specs/2026-11-08-freemapper-print-design.md`, `docs/glossary.md` and `docs/adr/README.md`
before changing code.

- Rust, edition 2024, pure-Rust core; domain logic in `crates/freemapper-print`, CLI/MCP/editor in
  `crates/freemapper-print-cli` (ADR 0001).
- Source code (identifiers, comments) is English. Specs, plans and ADRs are English. READMEs, manual, examples
  README and changelog are bilingual, German and English (ADR 0003).
- Glossary first: a new domain term gets its row in `docs/glossary.md` before or with its code. Terms that
  freemapper's glossary defines keep freemapper's names.
- User-facing text goes through the `unslop` and `humanizer` skills before it is committed: READMEs, manual,
  examples README, changelog, release notes, ADRs, and program messages longer than a short phrase (warnings,
  errors, CLI help, MCP tool descriptions). A subagent that writes such text reads and applies both skills'
  SKILL.md files.
- Every significant decision gets an ADR in `docs/adr/` in the same branch. Every merged feature or fix gets an
  entry under "Unveröffentlicht"/"Unreleased" in `CHANGELOG.md` and `CHANGELOG.en.md`.
- Dependencies must be at least two weeks old (ADR 0006). The toolchain is pinned (ADR 0007).
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test` must pass; hooks in
  `.claude/` and `lefthook.yml` run them.
- Superpowers process: brainstorming, then writing-plans, then subagent-driven development; TDD for readers,
  writers and geometry.
