# Switch to freemapper-core (stage 1: text, rules, fs) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** freemapper-print uses the shared crates `freemapper-text`, `freemapper-rules` and `freemapper-fs` from freemapper-core v0.2.0 instead of its own copies, with no change to any printed output.

**Architecture:** The apps depend on core through git dependencies pinned to the tag `v0.2.0` (core ADR 0006). Our own code shrinks to what is specific to print: the schema and the embedded profile, typed rule access, completeness and per-kind resolution, the `Texts` wrapper over a `Catalog`, and the decisions about which writes go where. `georef` (owned by freemapper) and the extraction of `omap`, `render` and `course` (stage 2, owned by us) are not part of this plan.

**Tech Stack:** Rust 2024, freemapper-text 0.2.0 (`Catalog`, Fluent), freemapper-rules 0.2.0 (`Schema`, `load_rule_set`, `RuleSet::pick`), freemapper-fs 0.2.0 (`write_inside`, `write_cache_file`, `create_new_file`, …).

**Spec:** `docs/superpowers/specs/2026-11-08-freemapper-print-design.md` (§5.9: rule profiles, program texts). Core: `../freemapper-core` ADRs 0006, 0009 (writes never follow links), 0010 (catalogs supplied by the app), 0011 (rule profiles validated against an app schema). Ours: ADR 0024 (program texts), 0031 (print rules as data), ADR 0014 (renderer copied from freemapper).

## Global Constraints

- Core is used as `freemapper-… = { git = "https://github.com/HenryJobst/freemapper-core", tag = "v0.2.0" }`. During development a `[patch."https://github.com/HenryJobst/freemapper-core"]` entry with a path to `../freemapper-core/crates/…` is allowed; it never reaches `main` (core ADR 0006).
- The suite's own crates are exempt from the two-week rule (core ADR 0004). Everything else stays at least two weeks old (our ADR 0006). The toolchain pin stays 1.99.1.
- A change to a core crate is agreed with its owner over SendMessage first (`freemapper-text`, `freemapper-rules`: freemapper-courseplan; `freemapper-fs`: freemapper). We do not edit `../freemapper-core`.
- No change to any printed output: every example PDF (`examples/luebeck-scales`, `luebeck-competition-layouts`, `luebeck-competition-courses` with `iof/`, `berlin-viktoriapark-training`) stays byte-identical, checked with `cmp` by the commands in the example READMEs.
- Program texts keep their DE/EN wording; no message id changes unless a task says so.
- `unsafe_code = "forbid"`; `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass after every task, on Linux, macOS and Windows (CI).
- Every user-visible change gets an entry in both changelogs; every significant decision gets an ADR. User text goes through the unslop and humanizer skills. German uses „…“ and umlauts; „Bundesland“ for German states.

## Review Focus

1. **Language choice must not change.** `--lang`, `FREEMAPPER_LANG`, `LC_ALL`, `LC_MESSAGES` and `LANG` give the same language as before, including values such as `de_DE.UTF-8`, `C`, `POSIX` and an invalid `--lang xx`. Pinned in Task 1: `language_detection_matches_before`.
2. **An embedded profile must load exactly as before** and every accessor return the same numbers; a profile or overlay that the old loader refused must still be refused with a message that names the rule. Pinned in Task 2: `resolved_values_unchanged` and `refused_profiles_still_refused`.
3. **A link where we write.** A project folder, out folder or project file that is a symlink must not make us write outside it. Pinned in Task 3: `export_does_not_follow_a_link_at_the_target` and `numbers_accept_replaces_a_link_instead_of_writing_through_it`.
4. **A full disk or an unwritable folder** still leaves no partial PDF and gives the same translated message. Pinned in Task 3: `disk_full_leaves_no_part_file` (the existing test, adapted).
5. **Windows.** `freemapper-fs` has Windows-specific behaviour (`same-file`); our CI must stay green on Windows, and tests that depend on Unix symlinks are `cfg(unix)`. Pinned in every task through CI.

---

### Task 1: Dependencies and program texts

**Files:**
- Modify: `Cargo.toml` (workspace dependencies), `crates/freemapper-print-cli/Cargo.toml`, `crates/freemapper-print-cli/src/text.rs`, `crates/freemapper-print-cli/src/main.rs` (only where call sites change), `Cargo.lock`
- Test: `crates/freemapper-print-cli/src/text.rs` (tests), existing CLI tests

**Interfaces:**
- Consumes: `freemapper_text::{Catalog, Lang, FluentValue}`; `Catalog::new(&'static [(&str, &str)])` (English first), `.available()`, `.parse_lang_arg(&str) -> Result<Lang, Vec<Lang>>`, `.detect_lang(Option<&str>, &dyn Fn(&str) -> Option<String>) -> Lang`, `.text(&Lang, id, &[(&str, FluentValue)]) -> String`, `.problems(&[(&str, FluentValue)]) -> Vec<String>`, `freemapper_text::ids_in_source`.
- Produces: `pub static TEXTS: Catalog` in `text.rs`; the existing `Texts` (`new(&Lang)`, `get`, `lang`, `plain`, `decimal`) and `detect_lang`/`available` keep their signatures, now thin wrappers. `UnknownLang` and the local `Localize` trait stay in the app.

- [ ] **Step 1: Failing tests.** In `text.rs`: `language_detection_matches_before` (a table of `(arg, env)` → language, copied from the current tests and extended with `de_DE.UTF-8`, `C`, `POSIX`, `en_US`, an invalid `xx`, and `--lang` beating the environment); `catalog_has_no_problems` (`TEXTS.problems(&sample_args())` is empty, `sample_args` supplying a value for every variable of every message, as the current test does); `every_id_used_in_code_exists` (uses `ids_in_source` over the CLI sources with the id prefixes we use).
- [ ] **Step 2: Add the dependency** `freemapper-text = { git = "https://github.com/HenryJobst/freemapper-core", tag = "v0.2.0" }` to `[workspace.dependencies]` and the CLI crate; remove `fluent-bundle`, `unic-langid` and any crate only the old `text.rs` used.
- [ ] **Step 3: Replace the body of `text.rs`** by `static TEXTS: Catalog = Catalog::new(&[("en", include_str!(…)), ("de", include_str!(…))])` and the wrappers above. Delete the copied bundle, detection and fallback code. Keep the isolation-mark handling in `Texts::get` as it is if the core `Catalog::text` already does the same (check with the test); otherwise keep our wrapper's behaviour and say so in the report.
- [ ] **Step 4: Run** `cargo test --workspace`; the CLI tests in both languages must pass unchanged. Re-run the CLI texts test with `FREEMAPPER_LANG=de` and `=en`.
- [ ] **Step 5: Commit** `refactor(cli): program texts from freemapper-text`.

### Task 2: Rule profiles on freemapper-rules

**Files:**
- Modify: `crates/freemapper-print/Cargo.toml`, `crates/freemapper-print/src/rules.rs`, `crates/freemapper-print/src/project.rs` (only the rule-set loading call), `docs/adr/0031-…md` (addendum)
- Test: `crates/freemapper-print/src/rules.rs` (tests)

**Interfaces:**
- Consumes: `freemapper_rules::{Schema, KeySchema, TableSchema, FilterSchema, FilterKind, FilterArg, ValueCheck, TableCheck, RuleSet, Rule, RuleProfile, RulesError, load_rule_set, profile, parse_profile, value_number, toml}` as in core ADR 0011; `Schema::problems()`; `RuleSet::pick(key, value, &[(field, FilterArg)])` returning `Ambiguous` with all matching ids.
- Produces: unchanged public surface of our `rules` module (`RuleSet` accessors `course_symbol`, `north_line_spacing_m`, `proposed_contour_interval_m`, `description_cell_mm`, `purple_cmyk`, `placement_weights`, `number_gap`, `scale_table`, …, `base()`, `load_rule_set(base, overlays)`, `RuleError` incl. `Incomplete`). Internally our `RuleSet` wraps the core one and adds the per-kind resolved values.

- [ ] **Step 1: Failing tests (before any change, on the old loader).** `resolved_values_unchanged`: a golden test that serialises every accessor of `base()` for all three map kinds, with the overlay test profiles, into a stable text and compares it with a checked-in snapshot file `tests/fixtures/rules/resolved.txt` (generate it from the current code first and commit it). `refused_profiles_still_refused`: one case per refusal the current tests have (unknown value name, non-finite, ≤ 0 length, percent > 100, cell size out of 4..=10, scale table without the base row, two rules for one slot, overlay naming an unknown rule, a rule with `map_kinds` on a kind-independent key, a proposal rule without a note); each asserts the error names the rule id.
- [ ] **Step 2: Schema.** Write `static SCHEMA: Schema`: the keys with their value names (our `KNOWN_KEYS`), `KIND_INDEPENDENT_KEYS` as `without_filters`, units, the `scale_table` `TableSchema` with `check` = our `scale_rows` shape and ascending order, filters `map_kinds` (Text) and `print_scales` (Int), `proposal_doc = "fmp"`, and a `value_check` for `description_box.cell_mm` (4..=10) that also refuses a list under a value name our code reads as one number. Add `schema_has_no_problems` (`SCHEMA.problems()` empty).
- [ ] **Step 3: Loader.** Replace the parser, merge and unit/range checks by core calls. Keep in the app: per-kind resolution via `pick` (map the `Ambiguous` error to a message that adds the map kind), `needed`/`optional`, completeness (`Incomplete`), the base-scale-row rule, `number_gap` with `GapRange`, `KindValues` and the typed accessors. Delete the code the core crate now provides.
- [ ] **Step 4: Run** the golden and refusal tests unchanged, then `cargo test --workspace`, then the four example commands and `cmp` against the committed PDFs.
- [ ] **Step 5: ADR 0031 addendum** (which parts moved to core, what stayed, the refused list under a scalar). Commit `refactor(rules): rule profiles on freemapper-rules`.

### Task 3: Writes through freemapper-fs

**Files:**
- Modify: `crates/freemapper-print/Cargo.toml`, `crates/freemapper-print/src/output.rs`, `crates/freemapper-print/src/project.rs` (`save_new`, `save_new_with_comments`, `append_correction`, `replace_file`), `crates/freemapper-print/src/export.rs`, `crates/freemapper-print-cli/src/main.rs` (`init`, `render`, `export`, `numbers accept`), `docs/adr/` (new ADR)
- Test: `crates/freemapper-print/src/output.rs`, `crates/freemapper-print/tests/project_courses.rs`, `crates/freemapper-print-cli/tests/project_cli.rs`

**Interfaces:**
- Consumes: `freemapper_fs::{write_inside(root, path, bytes), write_cache_file(path, bytes), create_new_file(path), check_folder_inside(root, folder), check_inside(root, path), remove_inside, is_plain_name, stays_inside}`; error type `(PathBuf, io::Error)`; the messages `OUTSIDE_PROJECT` and `NOT_COPIED`.
- Produces: `output::write_pdf_file` keeps its signature and its checks (free space with the 50 MB reserve, `DiskFull`); only the writing of the partial file and the rename go through core. Per-call rule: files inside the project folder (the project file, `out/` below it) use `write_inside(project_dir, …)`; a PDF written to an explicit `--out-dir` outside the project folder and the `render` command's `--out` use `write_cache_file` semantics (replace, never write through a link at the target). New error text for a link that leads out of the project folder, in DE and EN, naming the file.

- [ ] **Step 1: Failing tests.** `export_does_not_follow_a_link_at_the_target` (a symlink `out/A.pdf` → a file outside the project; after `export` the outside file is unchanged and `out/A.pdf` is a regular file; `cfg(unix)`), `numbers_accept_replaces_a_link_instead_of_writing_through_it` (the project file is a symlink to a file elsewhere: `numbers accept` writes a regular file at the project path and leaves the target untouched; this replaces the current "canonicalise and write to the target" behaviour from M3 Task 11, record the decision), `init_refuses_a_linked_out_folder` (`out` is a link leading out: `init`/`export` stop with the new message before writing), `disk_full_leaves_no_part_file` (existing test, adapted to the new partial-file name `<file>.<pid>.<counter>.part`), `part_file_name_is_not_predictable` (two exports in a row leave no `.part` and never reuse the old name).
- [ ] **Step 2: Implement.** Route the writes as described. Keep our free-space check before the write. Update the cleanup code that removed `<name>.part` to use core's `remove_stale_parts_inside`/`remove_stale_cache_parts` where it applies. Use `check_folder_inside` once at the start of `init`, `check` (read-only: no) and `export` for the out folder.
- [ ] **Step 3: Verify** every example command and `cmp`; run the project-file round-trip tests (comments kept byte for byte, CRLF kept, permissions kept: if core replaces the file via a new partial file, copy the permissions as `replace_file` does today and keep the test).
- [ ] **Step 4: ADR "Writes go through freemapper-fs"** citing core ADR 0009: which of our writes are project files and which are not, the change for symlinked project files, the disk-space check that stays ours. Commit `refactor(output): writes through freemapper-fs`.

### Task 4: Documentation, CI and dependency rule

**Files:**
- Modify: `README.md`, `README.en.md` (suite section: from "will move" to "uses text, rules and fs", developer section with the `[patch]` workflow), `docs/manual/{de,en}.md` (only if a message or behaviour is described: symlinked project files, write errors), `CHANGELOG.md`, `CHANGELOG.en.md`, `docs/glossary.md`, `docs/adr/` (new ADR "Suite crates as git dependencies"), `docs/adr/0006-…md` (exemption line), `CLAUDE.md` (one line: core crates, owners, `[patch]` rule), `.github/workflows/*.yml` (only if the git dependency needs it)

- [ ] **Step 1: ADR** "Suite crates as git dependencies": the tag pin, the local `[patch]` rule (never on `main`), the exemption of the suite's own crates from ADR 0006 (cite core ADR 0004), the ownership table, how a change to a core crate is agreed.
- [ ] **Step 2: READMEs and manual** as above, both languages. The README must say that text, rules and fs come from freemapper-core and that `omap`, `render`, `course` follow; the `[patch]` command example is the one in the core README.
- [ ] **Step 3: Changelogs.** "Geändert/Changed": program texts, rule profiles and writes now come from freemapper-core. "Behoben/Fixed" or "Geändert": a project file or an out folder that is a link no longer makes the program write outside the project folder (describe what the user sees).
- [ ] **Step 4: CI.** Push the branch and confirm the three CI jobs fetch the git dependency (public GitHub repo over HTTPS) and pass. Fix the workflow only if it does not.
- [ ] **Step 5: Commit** `docs: freemapper-core as a dependency`.
