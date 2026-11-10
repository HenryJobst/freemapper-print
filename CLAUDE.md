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
- Core crates (`freemapper-text`, `-rules`, `-fs`, `-omap`, `-course`) are git dependencies pinned to one tag; their own
  crates are exempt from ADR 0006 (ADR 0042). `lib.rs` re-exports omap and course as `omap`, `course` and `placement`
  (ADR 0043). Owners: text and rules freemapper-courseplan, georef and fs freemapper, omap and course freemapper-print
  (they live in core now; changes go there), render freemapper-print (still here, moves later). Ask the owner over
  SendMessage before changing a core crate. A local `[patch]` to `../freemapper-core` never reaches `main`.
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test` must pass; hooks in
  `.claude/` and `lefthook.yml` run them.
- Superpowers process: brainstorming, then writing-plans, then subagent-driven development; TDD for readers,
  writers and geometry.
- Work autonomously once a plan is approved: decide, record decisions, report them afterwards; stop only for
  destructive or truly blocking issues. Fix Minor review findings too, at the latest in the final fix wave.
- Before deleting an SDD workspace, every `Ruling:` line from its ledger goes into an ADR (or an existing ADR's
  text).
- Branch and worktree per plan under `.worktrees/<branch>`. Before a merge: fmt, clippy and tests green on the
  branch; merge with `--no-ff` into main; run them again on main; push to both remotes; watch CI on GitHub;
  then remove the worktree and branch and run `cargo clean` (target/ grows to tens of GB).
  Delete temporary builds in the session scratchpad too (reviewers' probe builds reach gigabytes).
- Before every push, check that README.md and README.en.md are current (status, version, what works,
  installation, usage) and fix them in the same push.
- Commits: Conventional Commits with a scope (`feat(render): …`, `fix(omap): …`, `docs(adr): …`), English
  subject, small logical units.
- Release when the user asks: bump `[workspace.package] version`, `cargo update -w`, turn "Unveröffentlicht"/
  "Unreleased" into `## [x.y.z] - YYYY-MM-DD` in both changelogs, annotated tag `vX.Y.Z`, push the tag;
  `release.yml` builds the archives and takes the notes from CHANGELOG.en.md.
- Every generated output (PDF metadata, visible credit line, notes) names freemapper-print and its public
  GitHub link.
- Test fixtures name source, date or commit, and licence in a README next to them.
- Exchange with freemapper and freemapper-courseplan goes through their stable contracts only: the tag
  `freemapper:source=sheet` and the map part "peripheral information", and from `freemapper.toml` only
  `map_kind`, `scale_denominator`, `crs`, `provider`, `title` and `[map_area]`. The tests `contract_*` in
  `crates/freemapper-print/src/contract.rs` check what print reads against the fixtures of `freemapper-contract`
  (ADR 0050); a contract change starts in freemapper-core.
