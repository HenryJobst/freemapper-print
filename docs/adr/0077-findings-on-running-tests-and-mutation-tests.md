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
- **Running the tests.** `cargo test` stays; nextest stays rejected, and its numbers are replaced by the re-measurement below. One run of
  `cargo test --workspace --all-features` took 13:04 min with 750 tests, the state after #11.
  CI runs two separate steps: `cargo test --locked --workspace` and
  `cargo test --locked -p freemapper-print-cli --features test-stub`; the second one needs the
  feature that the first does not turn on.
- **The mutation job stays on pull requests and on the diff.** The baseline costs about 100 s of
  tests plus 280 s of build, and 9 mutants took 20 min with two jobs (0072). That is too much
  for every push, so the job runs on pull requests only.
- **cargo-hack runs with `--locked`, without `--no-dev-deps`.** `--no-dev-deps` rewrites the
  manifests and with them `Cargo.lock` while it runs, and that does not mix with `--locked`.
  Decision: the flag is gone, `scripts/check.sh hack` and the `hack` job run
  `cargo hack check --locked --workspace --each-feature`, and every gate keeps `--locked`. Cost:
  the job also builds the dev dependencies, so it no longer proves that each feature checks
  without them; a feature that only builds thanks to a dev dependency would pass. Time: 44 s
  locally with a warm cache; the `hack` job took 43 s on the last run of `main` before the change.
- **The SBOM is reproducible only normalised and with `--target all`** (details in 0072); a diff
  of `docs/sbom.cdx.json` after a lock file change is expected, not a flaw.
- **Badges: order matters.** After a change to tests or ADRs run `scripts/check.sh tests --fix`
  first, then `bash scripts/badges.sh`; the badge reads the count that `tests --fix` writes.
- **Timings are not taken next to foreign runs.** A measurement of test or build time is made
  while no other mutant or build run is active on the machine; a parallel run inflates it.

## Measurement: cargo-nextest, again

ADR 0072 rejected nextest with 8:42 min for `cargo test` against 11:33 min for nextest. That
measurement was taken on a busy machine, so it was repeated.

**Local.** Apple M1, 8 cores, nextest 0.9.152, Rust 1.99.1, `github/main` at 5aa1fad. The
machine was still noisy (load 13 to 60); foreign processes were waited out beforehand. Both runners
ran 723 tests (plus 1 ignored).

| Run | `cargo test --locked --workspace` | nextest |
| --- | --- | --- |
| first | 103 s | 105 s |
| second | 124 s | 125 s |

That is within the noise, no gain. Only for `-p freemapper-print-cli --features test-stub` was nextest
faster (50 s and 57 s against 70 s), but cargo has just one valid run there. Cause: a few CPU-heavy
render and PDF tests. nextest starts up to 8 at once and overloads the cores
(`project_cli::export_twenty_layouts` took 55 to 69 s under nextest). In core and courseplan,
which have many light test binaries, nextest does win. No shared resource was found, and no test fails
under only one runner.

**CI probe.** Draft pull request #14, closed, not merged. Test steps in seconds (build plus run);
mean of the last three green runs on `main` / probe 1 (cold, because the cache key contains the
workflow hash) / probe 2:

| Job | Test | `test-stub` |
| --- | --- | --- |
| Ubuntu | 102 / 154 / 129 | 75 / 83 / 92 |
| macOS | 91 / 175 / 240 | 81 / 78 / 120 |
| Windows | 137 / 279 / 167 | 101 / 135 / 116 |

On macOS, probe 2 was cold, so it is not comparable. The spread on `main` is large (Ubuntu 82 to
142 s, Windows 97 to 175 s). Warm: Ubuntu test build 24 s plus run 105 s; Windows build 51 s plus run
115 s. `cargo install --locked` of nextest took 230 to 396 s cold and 0 to 1 s with a cache. Doctests
take 1 to 2 s (there are currently 0). The budget gate passed (154 s and 129 s).

**Decision.** nextest stays rejected. The earlier figure (11:33 against 8:42 min) came from a noisy
measurement and is replaced by this one in what it says, not in the decision. Revisit when the suite
gets many light test binaries.

## Measurement: `opt-level` for tests

Does an optimised dependency build speed up the tests? Measured locally on the same machine with
`cargo test --locked --workspace`:

| Variant | Cold build | Test run |
| --- | --- | --- |
| A: no profile | 4:16 | 4:03 |
| B: `[profile.dev.package."*"] opt-level = 3` | 13:05 (about 4 times the CPU time) | 0:48 (lib test 41.7 to 11.6 s) |
| C: B plus own crates at `opt-level = 1` | not measured | 0:53 |

C brings no extra gain over B (noise). D (own crates at 2) was not measured. Foreign processes ran
next to A; only 3 to 4 next to B and C. The `"*"` also covers the git dependency freemapper-core.
freemapper had the same experience: its own code at `opt-level = 1` made the build 2.5 to 3.5 times
slower and was removed later.

**Decision.** No profile is added. Open: B only pays off if CI caches the dependencies (not checked).
`cargo-mutants` builds anew on every run; the effect on the mutation gate was not measured.

## Consequences

- The numbers (13:04 min, 750 tests, the nextest and `opt-level` tables) are measurements at one
  state of the code, on a noisy machine; they
  are measurements, not budgets. The budget stays in `docs/tests.json` (0074).
- Reviewers look for a value-pinning test next to each changed calculation, not only for green
  mutants.
- Nothing changes in the gates; this ADR only records findings and the order of two commands.
