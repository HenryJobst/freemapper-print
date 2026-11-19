# 0070. The repository checks its own housekeeping

Date: 2026-11-19
Status: Accepted

## Context

CI ran the compiler and `cargo deny` and nothing else. The documents, the ADR table, the changelog
pair and the dependency list were kept in order by hand: a link that pointed at a renamed file, an
ADR without a row in `docs/adr/README.md`, a changelog that missed a version, a dependency that no
code used — each of those surfaced in review or, worse, never. freemapper, freemapper-core and
freemapper-courseplan have the same gap, so the gates are cut the same way in all four.

Two of the claims were untrue. `rust-version = "1.88"` never built: freemapper-core v0.29.0 has a
workspace member that overrides `default-features` of an inherited workspace dependency, and cargo
rejects that until 1.99. And nothing measured test coverage, so a change could take away proof
without anyone noticing.

## Decision

- `scripts/check.sh` is the single entry point for the gates. It has no dependencies beyond git and
  the tools named below, and CI calls the same commands as a person does.
- The gates are `spdx`, `typos`, `links`, `secrets`, `machete`, `actions`, `adr`, `changelog`,
  `doc`, `coverage` and `msrv`. The first eight need no build and run on every commit
  (`lefthook.yml`); `secrets` runs before a push because it walks the history; `doc`, `coverage`
  and `msrv` build the workspace and each have their own CI job.
- Every Rust file starts with `// SPDX-License-Identifier: AGPL-3.0-or-later`. The line is not
  repeated in documents and data: the licence is in `LICENSE` and the Cargo manifests, and a
  copyright line in every Markdown file would age without telling anyone anything.
- `typos` checks the English documents only — `*.en.md`, `AGENTS.md`, `CLAUDE.md`, `docs/adr/`,
  `docs/superpowers/` and anything named `en.md`. German documents, the examples whose English
  README quotes German sheet names, fixtures and vendored files stay out. The German words that
  stand as quotes in English text are listed in `_typos.toml`, each with the place that needed it;
  a real misspelling is fixed, not listed.
- `lychee` checks links and anchors offline, so the gate looks at this repository and not at the
  internet.
- `gitleaks` scans the whole history, and its findings are redacted: a result names the rule and the
  file, never the value. `.gitleaks.toml` allows a path only with a reason.
- `cargo-machete` found four dependencies that nothing used (`pdf-writer`, `quick-xml` and
  `subsetter` in the library, `schemars` in the CLI); three are gone. `schemars` stays because the
  `JsonSchema` derive expands to paths in it even though no code names it, so the crate carries a
  `[package.metadata.cargo-machete]` entry instead.
- `cargo doc` runs with `RUSTDOCFLAGS=-D warnings`. Doc comments that linked to private items now
  name them in backticks, and TOML snippets such as `[[masks]]` stand in code spans.
- Coverage has a floor of 95.0 % in `scripts/check.sh` and the last measurement in
  `docs/coverage.json`. A run below either fails; a run above the snapshot rewrites it, and the
  commit belongs with the change that raised it. The first measurement is 96.6 %.
- The MSRV is 1.99, not 1.88. The workspace itself would build with less, but cargo reads the
  manifests of the pinned freemapper-core tag, and that tag needs 1.99. The way back down is a
  freemapper-core release that repeats `default-features = false` in `[workspace.dependencies]`;
  until then the manifest states the truth, and the `msrv` job checks it with 1.99.0.
- CI gives the gates their own jobs (`hygiene`, `docs`, `msrv`, `coverage`) beside `check` and
  `deny`. The tools are pinned by version and checksum in the workflow, and caching keeps the
  hygiene job cheap.

## Consequences

- A pull request now fails on a broken link or anchor, a missing ADR row, a changelog that misses a
  version, a declared but unused dependency, an action that hangs on a tag, a secret in the
  history, a rustdoc warning and a coverage drop.
- The coverage job costs an instrumented build. A run that raises the snapshot fails the "snapshot
  must be committed" step until the file is committed — deliberate, so the ratchet is a reviewable
  edit.
- Raising coverage later means editing the floor in `scripts/check.sh` in the same commit as the
  better number.
- `typos` will meet a new German quote in English text and fail. The answer is an entry in
  `_typos.toml` with its reason, not switching the gate off.
- The MSRV can only fall when freemapper-core releases a tag whose members inherit
  `default-features` without overriding it.
