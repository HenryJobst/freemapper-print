# Core extraction stage 2: freemapper-omap and freemapper-course Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The `.omap` model and reader, the course model with its readers and fitting, and the number placement live in freemapper-core as `freemapper-omap` and `freemapper-course`. freemapper-print uses them through git dependencies, with no change to any printed output. freemapper-courseplan builds its `.ppen` writer on `freemapper-course`.

**Architecture:** Three tasks. Tasks 1 and 2 happen in the freemapper-core repository (`../freemapper-core`, worktree there), where freemapper-print is the owner of both crates. Task 3 releases core v0.3.0 and switches freemapper-print to the tag. The modules `omap/`, `course/` and `placement/` already depend only on each other (ADR 0039, enforced by a test), so the move is a cut and paste with an `omap` → `course` dependency. To keep call sites unchanged, freemapper-print re-exports the crates under the old module names (`pub use freemapper_omap as omap;`).

**Tech Stack:** Rust 2024, quick-xml, toml/serde where the modules already use them, thiserror; `freemapper-omap` embeds the symbol sets from `assets/symbol-sets/`.

**Spec:** `docs/superpowers/specs/2026-11-08-freemapper-print-design.md` (§5.1 map input, §5.3 course input, §5.7 placement). Core: `../freemapper-core/CLAUDE.md` and its ADRs 0001–0011. Ours: ADR 0014 (renderer and reader copied from freemapper), 0032 (course coordinates), 0033 (Purple Pen reader), 0038 (number placement), 0039 (module boundaries), 0042 (suite crates as git dependencies).

## Global Constraints

- Rules of the core repository apply to Tasks 1 and 2: `../freemapper-core/CLAUDE.md` (one crate per topic, `unsafe_code = "forbid"`, clippy `-D warnings`, CI on three OSes, English code and ADRs, bilingual README and changelog by crate, glossary first, ADRs that move here keep their content and name the app's ADR in their source line, Conventional Commits with the crate as scope, branch and worktree under `.worktrees/` there, merge with `--no-ff`, never force-push, `cargo clean` before removing a worktree).
- Licence GPL-3.0-or-later for every crate; toolchain 1.99.1 (the same pin as the apps); third-party dependencies at least two weeks old.
- No network access in core crates (core ADR 0007). No fluent dependency in these crates; `Display` of errors is English, the app translates.
- No change to any printed output of freemapper-print: all 11 example PDFs stay byte-identical (`cmp`, commands in the example READMEs); every test of the moved modules moves with it and passes unchanged.
- The public API of the moved modules keeps its names, so freemapper-courseplan and freemapper can read it. A rename needs a ledger ruling and a changelog line.
- `freemapper-course` depends on `freemapper-omap` and on nothing from freemapper-print. It has no dependency on `freemapper-rules` or on the renderer. `placement` keeps no `crate::` imports outside its own tree.
- Requirements from freemapper-courseplan (message 2026-11-09), applied in Task 2 where cheap and recorded where not: `fit` and the `MapDocument` dependency are optional for users who only need the model and the readers (a cargo feature `fit`, on by default, or a separate module behind the omap dependency); `Control` and `Course` carry an optional stable id from the `.ppen` (`id: Option<u32>`), filled by the reader; error `Display` stays English; the offset ↔ `{direction, gap_mm}` conversion that lives in `numbers.rs` moves into the crate with its reference point and unit documented.
- Docs and ADRs for user-visible text follow the unslop and humanizer skills; German uses „…“ and umlauts; „Bundesland“ for German states.

## Review Focus

1. **Byte-identical output** after the switch (Task 3), checked on all examples. Pinned by `cmp` in Task 3.
2. **A crate that does not build alone.** Each new crate builds and tests with `cargo test -p <crate>` inside core, without freemapper-print. Pinned in Tasks 1 and 2 by CI on three OSes.
3. **Embedded symbol sets** reach the crate: `include_str!` paths must be crate-relative, and the crate must package the assets (a `cargo package --list` check), or a git dependency from another repository fails to find them. Pinned in Task 1: `symbol_sets_are_found_from_a_git_dependency`, checked by building a scratch app against the crate by path outside the repository.
4. **Windows** CI stays green in core and in print (path separators in tests, `include_str!` paths).
5. **Hostile input limits** (ADR 0019, 0033) travel with the readers: the limit constants and their tests move unchanged.

---

### Task 1: freemapper-omap

**Where:** `../freemapper-core`, worktree `.worktrees/omap` on branch `omap`.

**Files:**
- Create: `crates/freemapper-omap/{Cargo.toml,src/…}`, `crates/freemapper-omap/assets/symbol-sets/*` (copied from freemapper-print `assets/symbol-sets/`), `docs/adr/00NN-the-omap-crate.md` (next free number)
- Modify: workspace `Cargo.toml`, `README.md`, `README.en.md`, `CHANGELOG.md`, `CHANGELOG.en.md`, `docs/glossary.md`

**Interfaces:**
- Produces: everything in freemapper-print `src/omap/` as `freemapper_omap`:
  - `coord::MapCoord`
  - `model::{MapDocument, MapGeoref, MapObject, MapObjectKind, MapPart, TemplateRef, …}`
  - `read::{read_omap, …}` and `OmapError`
  - `symbol_set::{SymbolSet, …}`
  - `enlarge::…`
  - the reader's limit constants

  Public paths stay as they are today (`freemapper_omap::coord::MapCoord`, `freemapper_omap::MapDocument`, …).
- Consumes: quick-xml, thiserror (versions as in freemapper-print's lock file, at least two weeks old).

- [ ] **Step 1: Copy `src/omap/` into the crate** with its tests and the three symbol-set assets. Fix `include_str!` paths to be crate-relative (`../assets/symbol-sets/…`). Record the source commit of freemapper-print in the crate README and the ADR.
- [ ] **Step 2: Failing test** `symbol_sets_are_found_from_a_git_dependency`: a scratch crate outside the repository (created by the test run or by a documented command in the report) depends on `freemapper-omap` by path and calls `SymbolSet::default_for(…)` or the equivalent public entry; it must compile and load. Also run `cargo package -p freemapper-omap --list` and check that the assets are listed.
- [ ] **Step 3: Make it build and pass:** `cargo test -p freemapper-omap`, fmt, clippy `-D warnings`. All moved tests pass unchanged.
- [ ] **Step 4: Docs.** README rows (German and English) update "geplant" to "enthalten" for `freemapper-omap`; changelogs "Hinzugefügt/Added: freemapper-omap"; glossary rows for the map terms the crate introduces (check freemapper's glossary for names to keep); an ADR "The omap crate": what moved, from where (print ADR 0014), the owner, what stays in the app, and the embedded assets.
- [ ] **Step 5: Commit** `feat(omap): the .omap model, reader and symbol sets`.

### Task 2: freemapper-course

**Where:** `../freemapper-core`, worktree `.worktrees/course` on branch `course` (after Task 1 is merged into core `main`).

**Files:**
- Create: `crates/freemapper-course/{Cargo.toml,src/…}` from freemapper-print `src/course/` (model, `iof_xml`, `ppen`, `fit`, `tm`, `xml`, their tests and fixtures from `tests/fixtures/courses/`) and `src/placement/`; an ADR; a crate README section.
- Modify: workspace `Cargo.toml`, `README*.md`, `CHANGELOG*.md`, `docs/glossary.md`

**Interfaces:**
- Produces, as `freemapper_course`:
  - `model::{CourseEvent<P>, RawEvent, FittedEvent, Control<P>, Course<P>, Stop, Leg<P>, Special<P>, Description, CourseNote, NumberPlacement, Offset, …}` with the limit constants;
  - `read_ppen`, `read_iof_xml`;
  - `fit::{fit, Fit, FitOptions, FitError, FitNote, YAxis}` and `tm`;
  - `placement::{number_box, score, best, GapRange, Obstacle, …}` (ADR 0038);
  - NEW: `Control::id` and `Course::id` (`Option<u32>`), filled by the `.ppen` reader from Purple Pen's ids and left `None` by the IOF reader;
  - NEW: the offset ↔ placement conversion from freemapper-print `numbers.rs` (`placement_from_offset`, `offset_from_placement`), with the reference point and units documented.
- Consumes: `freemapper_omap::{MapDocument, MapGeoref, coord::MapCoord}`.

- [ ] **Step 1: Copy** the modules, tests and fixtures. Keep the fixture READMEs (source, commit, licence). `fit` stays in the crate; make it a cargo feature `fit` (default on) so that a user without maps can drop the omap dependency from the model and readers: check whether that is feasible without code duplication; if `model.rs` needs `MapCoord`, keep `omap` required and say so in the ADR instead.
- [ ] **Step 2: Failing tests** `reader_fills_stable_ids` (a `.ppen` fixture: every control and course has an id equal to the file's), `offset_placement_round_trip` (a table of offsets, circle and box sizes: offset → placement → offset within 1e-6), `boundary_test_course_needs_only_omap` (the module-boundary test from print moves here: the crate's source mentions no other suite crate than `freemapper_omap`).
- [ ] **Step 3: Implement** the id fields and the conversion; all moved tests pass unchanged: `cargo test -p freemapper-course`, fmt, clippy.
- [ ] **Step 4: Docs.** README rows, changelogs, glossary rows, ADR "The course crate" with print's ADRs 0032, 0033, 0038 and 0039 as sources and the courseplan requirements it applies or defers.
- [ ] **Step 5: Commit** `feat(course): course model, readers, fitting and number placement`.

### Task 3: Release core v0.3.0 and switch freemapper-print

**Where:** first `../freemapper-core` (release), then `.worktrees/core-extract` in freemapper-print.

**Files:**
- freemapper-core: version, changelog dates, tag `v0.3.0`.
- freemapper-print: `Cargo.toml` (workspace deps: `freemapper-omap`, `freemapper-course` by tag v0.3.0; remove deps only the moved code used), delete `src/omap/`, `src/course/`, `src/placement/` and `assets/symbol-sets/` (if nothing else uses them) and `tests/fixtures/courses/`'s moved fixtures, `src/lib.rs` (re-exports), `src/numbers.rs` (use the crate's conversion), tests that reach into the moved modules by path, the boundary tests (now crate boundaries), docs.

- [ ] **Step 1: Release core.** After Tasks 1 and 2 are merged: set `[workspace.package] version = "0.3.0"`, `cargo update -w`, date both changelogs, commit, annotated tag `v0.3.0`, push `main` and the tag to both remotes, wait for the three-OS CI. Tell freemapper-courseplan and freemapper which version to pin. (Pushing core's `main` and tag is the owner's routine under core's CLAUDE.md for the crates we own.)
- [ ] **Step 2: Switch print** in a worktree. During development a local `[patch."https://github.com/HenryJobst/freemapper-core"]` is allowed; remove it before the merge. Re-export: `pub use freemapper_omap as omap;`, `pub use freemapper_course as course;`, `pub use freemapper_course::placement;`. Fix compile errors without changing behaviour.
- [ ] **Step 3: Verify.** `cargo fmt`, clippy `-D warnings`, `cargo test --workspace`; all 11 example PDFs byte-identical (`cmp`); startup time of `check` unchanged; `cargo tree` shows no duplicate versions of shared dependencies.
- [ ] **Step 4: Docs.** ADR "Omap, course and placement come from freemapper-core" (supersedes the "copied from freemapper" part of ADR 0014 for the reader and the model; ADR 0039's boundary tests are replaced by crate boundaries); README suite and developer sections (the moved crates are no longer "to follow"); changelog "Geändert/Changed" (internal; no user-visible change); CLAUDE.md line (owners table: omap and course now in core).
- [ ] **Step 5: Commit** `refactor: omap, course and placement from freemapper-core`. Merge with `--no-ff` after review, push, watch CI on three OSes.
- [ ] **Step 6: Tell the other sessions** (freemapper-courseplan: `freemapper-course` is available at v0.3.0, with the ids and the conversion; freemapper: `freemapper-omap` is available).
