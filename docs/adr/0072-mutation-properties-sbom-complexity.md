# 0072. Mutation tests on the diff, property tests, SBOM and a complexity limit

Date: 2026-11-20
Status: Accepted

## Context

ADR 0070 added the housekeeping gates. The next candidates measure what the tests prove and what
ships: mutation testing, property tests, a faster runner, a feature matrix, a complexity limit and
an SBOM. Each was measured here first (macOS, Apple silicon, warm `target`) and is adopted or
dropped by that number.

## Decision

- **Mutation tests on the diff: adopted.** `scripts/check.sh mutants` runs `cargo-mutants
  --in-diff` over the Rust lines the branch changed against `github/main`, in its own CI job on
  pull requests. Measured: all 9 mutants of `parse_points` caught, baseline 100 s of tests plus a
  280 s build, 20 min for the 9 with two jobs. A whole-workspace run is not a gate; the diff is.
  `cargo-mutants` copies the tree without `.git`: a test that reads the repository's history, or
  a path above the workspace, fails the baseline there (exit code 4) and is a bug in the test.
  The tests here read only fixtures under the tree and pass. A mutant that survives is fixed
  with a test, not with an exclusion; `mutants.out/missed.txt` names it. A mutant that ends in
  the timeout (exit code 3, for example an endless loop) counts as caught, as in the other three
  repositories; exit codes 2 (missed) and 4 (baseline failed) fail the gate.
- **Property tests: adopted at one place.** `proptest` checks `parse_points`, the `--points`
  parser: text written from any finite points reads back unchanged, and arbitrary text is read
  or refused with `BadPoints`, never a panic or a non-finite value. The parser is the one place
  here where free text becomes numbers. The project file is TOML and checked by serde and the
  mask checker; a property test there would test the `toml` crate.
- **cargo-nextest: rejected.** 731 tests: `cargo test` 8:42 min, `cargo nextest run` 11:33 min
  (10 tests flagged slow). The suite is dominated by a few editor tests and the build, not by
  the harness; nextest runs more processes and is slower. The test budget argues against it. (Re-measured later on 723 tests: no gain, the earlier figure came from a noisy run; the decision stands, ADR 0077.)
- **cargo-hack: adopted as a compile check.** `check --each-feature` over the
  three features (`test-hooks`, `page-rules`, `test-stub`) catches a feature that only builds
  when another is on. It builds, it does not test, and it costs about 10 minutes cold in its own
  job. `--feature-powerset` would test 2^3 combinations of features that exist only for tests;
  it is not worth the time. (`--no-dev-deps` was dropped later so the job can run with `--locked`, ADR 0077.)
- **Complexity: adopted as a clippy threshold.** `clippy::cognitive_complexity` with the
  baseline `cognitive-complexity-threshold = 35` in `clippy.toml`; today's worst function scores
  35 (five functions are above clippy's default of 25). `scripts/check.sh complexity` fails on
  anything above. Lowering the number is welcome, raising it needs an ADR. `rust-code-analysis`
  was not tried: clippy is already in the toolchain and needs no second parser.
- **SBOM: adopted.** `scripts/check.sh sbom` renders a CycloneDX 1.5 list of the CLI with
  `cargo-cyclonedx` 0.5.10 and compares it with `docs/sbom.cdx.json`; `sbom --fix` rewrites it.
  The tool writes a random serial number, a time stamp, the build host and absolute paths; they
  are removed, so the same dependencies give the same bytes. `--target all` lists the dependencies of every platform: without it
  macOS and Linux disagree (`core-foundation-sys`, `linux-raw-sys`). The gate needs only `cargo metadata`
  and runs with the cheap ones.

## Consequences

- Three new CI jobs (`mutants`, `hack`, `complexity`), one new cheap gate (`sbom`), and the
  badge for the number of checks counts them.
- A change of `Cargo.lock` changes `docs/sbom.cdx.json` in the same commit.
- Tool versions are pinned in `ci.yml`: cargo-mutants 27.1.0, cargo-hack 0.6.46, cargo-cyclonedx
  0.5.10, installed with `cargo install --locked --version`.
