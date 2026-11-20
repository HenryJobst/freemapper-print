# 0077. Findings on running tests and mutation tests

Date: 2026-11-20
Status: Accepted, supplements [0072](0072-mutation-properties-sbom-complexity.md) and [0074](0074-test-count-and-budget.md)

## Context

ADR 0072 adopted mutation tests on the diff, a property test, a feature check, a complexity limit and
an SBOM, and ADR 0074 made the test count and budget data. Since then the gates ran on real pull
requests, the test-gap pull request (#11) added tests, and the mutation gate was aligned with the
other three repositories (#8). This ADR records what running them taught, and the rulings that
follow. It repeats nothing 0072 and 0074 already say: the diff-only scope, the exit codes, the
`.git`-less copy, the nextest numbers, the proptest place, the threshold of 35 and the SBOM
normalisation stay there.

## Decision

- **A survivor is closed by a test that fixes the value.** The lesson from core: every changed
  line of arithmetic needs a test that pins the result, not one that only shows that no error
  occurs. A mutant that replaces `+` by `*` survives a test that checks "does not fail". The
  fix is the test, never an exclusion (as in 0072).
- **Running the tests.** `cargo test` stays; nextest is not revisited (0072). One run of
  `cargo test --workspace --all-features` took 13:04 min with 750 tests, the state after #11.
  CI runs two separate steps: `cargo test --locked --workspace` and
  `cargo test --locked -p freemapper-print-cli --features test-stub`; the second one needs the
  feature that the first does not turn on.
- **The mutation job stays on pull requests and on the diff.** The baseline costs about 100 s of
  tests plus 280 s of build, and 9 mutants took 20 min with two jobs (0072). That is too much
  for every push, so the job runs on pull requests only.
- **cargo-hack and `--locked` do not mix.** `--no-dev-deps` rewrites the manifests and with them
  `Cargo.lock` while it runs, so `scripts/check.sh hack` runs without `--locked`. Everything else
  keeps `--locked`.
- **The SBOM is reproducible only normalised and with `--target all`** (details in 0072); a diff
  of `docs/sbom.cdx.json` after a lock file change is expected, not a flaw.
- **Badges: order matters.** After a change to tests or ADRs run `scripts/check.sh tests --fix`
  first, then `bash scripts/badges.sh`; the badge reads the count that `tests --fix` writes.
- **Timings are not taken next to foreign runs.** A measurement of test or build time is made
  while no other mutant or build run is active on the machine; a parallel run inflates it.

## Consequences

- The numbers (13:04 min, 750 tests) are the state at #11; they
  are measurements, not budgets. The budget stays in `docs/tests.json` (0074).
- Reviewers look for a value-pinning test next to each changed calculation, not only for green
  mutants.
- Nothing changes in the gates; this ADR only records findings and the order of two commands.
