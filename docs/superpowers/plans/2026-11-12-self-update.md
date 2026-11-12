# Self-update implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port freemapper's self-update: `freemapper-print update` installs the newest signed release in place, a daily check reports new versions (CLI, MCP, editor start), an opt-in mode installs on its own, `--rollback` undoes it.

**Architecture:** The pure parts come unchanged from core's `freemapper-update` (core ADR 0026; first in v0.17.0, in the pinned v0.21.1). Print adds freemapper's app part as `src/update/{mod,source,command,background}.rs`, two MCP tools with the first-result note, and freemapper's release steps with a fallback that skips signing when the secret is missing.

**Tech Stack:** Rust 1.99.1, `freemapper-update` (core v0.21.1), `ureq` =3.4.2, `dirs` 7.0.0, dev `minisign-verify` =0.3.0 and `sha2` 0.11.1; `flate2` moves to 1.1.11 through core.

**Spec:** freemapper's, binding: `../freemapper/docs/superpowers/specs/2026-11-11-self-update-design.md` (section numbers are its own), ADR 0094 with addendum, its plan `../freemapper/docs/superpowers/plans/2026-11-11-self-update.md` (Stage B is the model), core ADR 0026. Code to port, under `../freemapper/`: `crates/freemapper-cli/src/update/*.rs`, `src/mcp.rs` (`UpdateNote`, `call_tool`), `tests/{cli,mcp,update_manifest}.rs`, `tests/support/mod.rs`, `.github/workflows/{release,ci}.yml`, `.github/scripts/*.sh`. Port; do not redesign. Print's calls P1 to P10 go into ADR 0060.

## Global Constraints

- Rust 2024, `unsafe_code = "forbid"`, English source, glossary terms. Paths starting with `src/`, `tests/` or `i18n/`, and `main.rs`, `mcp.rs`, are in `crates/freemapper-print-cli/`.
- Before each commit: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --locked`; from Task 3 also clippy and `cargo test --locked` for `-p freemapper-print-cli --features test-stub`.
- Dependencies two weeks old (ADR 0006), at freemapper's locked versions; toolchain pinned (ADR 0007); core crates on one tag, v0.21.1, no core change, no `[patch]` on `main` (ADR 0042).
- Nothing unsigned or unverified is installed or run; a bad signature is an error, never "no update".
- Names: app `freemapper-print`, `freemapper-print-update.json` + `.minisig`, `freemapper-print-<tag>-<platform>.gz`, `freemapper-print.previous[.exe]`, `.stale-<unix-seconds>[.exe]`. `--version` prints exactly `freemapper-print x.y.z` (the `ProcessRunner` contract).
- Environment: `FREEMAPPER_PRINT_UPDATE=off|notify|install`, `FREEMAPPER_PRINT_CONFIG_DIR`, `FREEMAPPER_PRINT_CACHE_DIR`, `FREEMAPPER_DIST=github-release` (`option_env!`), `FREEMAPPER_PRINT_UPDATE_TEST_BASE` (debug + `test-stub` only). Settings `<config_dir>/freemapper-print/settings.toml` `[update] mode` (default `notify`); record `<cache_dir>/freemapper-print/update-check.json`.
- HTTP: own `ureq` agent, redirects by hand (at most 3, each through `check_url`), `User-Agent: freemapper-print/<version> (+https://github.com/HenryJobst/freemapper-print)`, never the crates.io-only `freemapper-print-dev` agent. No e-mail, name, id or project data in any request, file or test; no telemetry.
- Key: freemapper's (P1), `assets/update/minisign.pub` a byte copy. Only the user handles secrets; agents never make, read, set or store one. Tests use core's test pair in `tests/keys/`, which no release build accepts.
- Texts DE+EN in `i18n/{de,en}/freemapper-print.ftl`; `same_ids_in_every_language` passes. Whoever writes user-facing text first reads `/Users/henryprivat/.claude/skills/unslop/SKILL.md` and `/Users/henryprivat/.claude/skills/humanizer/SKILL.md` and applies both. Docs bilingual; nothing private in the repository.
- Conventional Commits with a scope, English, ending with a blank line and the session's `Co-Authored-By:` and `Claude-Session:` lines.

## Print-side design calls (ADR 0060)

- **P1 Key.** freemapper's key (the user's decision of 2026-11-12), id 9353842ABFFAC9FB; no new pair. Why: one secret to guard; the manifest's `app` field keeps one program's manifest from installing as the other. Cost: a compromised key signs updates for both; the key list allows rotation (a release signed with the old key adds the new one, in both programs).
- **P2 Release gating.** freemapper's workflow (signing in `environment: release` with `secrets.MINISIGN_SECRET_KEY`/`MINISIGN_PASSWORD`), plus a fallback job `gate` that reports only whether the secret is set. Without it the release is today's, with a notice, unless an earlier release has a manifest; then `gate` fails. Why: the builds must know before compiling whether the binary checks, since a checking binary whose release has no manifest tells everyone "no manifest"; once a manifest exists, a release without one breaks `latest/download`. The secrets exist, so the first release after the merge signs. Cost: protection rules make `gate` wait for approval too.
- **P3 When.** After `init`, `check`, `export`, `render`; at the start of `mcp` and `edit`; not after `numbers`, `masks`, `update`. No `--offline` (mode `off` does that). Why: mask and number edits come in series, and `check` follows them. Cost: a mask-only user sees the notice a day later.
- **P4 Editor.** One stderr line at the start of `edit`, no page banner. Why: no endpoint, JS or CSP change; `edit` runs from a terminal that already shows its address; the suite's `#banner` can take it later. Cost: a browser-only user misses it until another command runs.
- **P5 Install mode** installs only after exit 0 (a `check` with notes exits 1), never in `mcp` or `edit`. Cost: the install waits for the next clean run.
- **P6 Record** in the cache folder, override `FREEMAPPER_PRINT_CACHE_DIR` (keeps tests off `HOME`, on Windows too).
- **P7 MCP style.** Both tools take `lang` and return compact JSON with a `message`, like print's others; the note uses the `agent-<id>` twins (`update-notice` for CLI and editor, `agent-update-notice` for the server). Cost: two JSON shapes to keep.
- **P8 Non-release builds** say they were built from source or by a release without signed updates, and name the releases page plus `git pull` and `cargo build --release --locked`: a release built without the secret (P2) takes this path too.
- **P9 HTTP** through `ureq` =3.4.2 with default features (rustls, ring, webpki-roots: about 30 crates new to print), as freemapper; a hyper client on print's axum/tokio stack would bring the same TLS crates plus own code. Cost: 1 to 2 MB; `ring` needs a C compiler, which the runners have.
- **P10** Folders from `dirs` 7.0.0; the clock from `SystemTime`, no `chrono`.

## Review Focus

1. **Program folder not writable** (`/usr/local/bin`, `C:\Program Files`): message naming the folder, nothing changed, exit 1. Test `update_in_a_read_only_folder_names_it` (Task 3, Unix).
2. **Windows: the running program replaces itself** (server `install_update`; an old server holding `.previous.exe`): renames only, the old previous becomes `stale-<time>`. Core tests the swap on three systems; Task 5's `install_update_with_confirm_installs_and_asks_for_a_restart` (Unix) and `update-smoke` (Windows too) run the path.
3. **Offline, slow or silent host**: output and exit code unchanged, at most 3 s more once a day, attempt recorded failed, next one in 24 h. Tests `slow_source_is_dropped_after_3_s_and_recorded_failed`, `export_against_a_silent_stub_ends_within_the_budget` (Task 4).
4. **Garbage record, clock gone back**: counts as due and is replaced, no panic, no message. Test `a_garbage_or_future_record_is_due_and_replaced` (Task 4).
5. **No asset for this platform**: `update` names platform and releases page, exit 1; the daily check fails silently. Tests `update_without_an_asset_for_this_platform_names_it` (3), `no_asset_is_recorded_failed_and_not_shown` (4).

---

## Stage H: the human step

### Task H1: The `release` environment (the user, not an agent)

`MINISIGN_SECRET_KEY` and `MINISIGN_PASSWORD` (freemapper's) exist at repository level, with no `release` environment. Task 6 signs either way, but any workflow on any branch can read repository secrets. So:

1. GitHub, Settings > Environments: create `release` with freemapper's rules (tags `v*`, a required reviewer); a dry run then needs its branch allowed.
2. Add both secrets there under the same names, then delete the repository-level ones.
3. Tell the session it is done. No agent reads or sets a secret value.

Tasks 1 to 7 do not wait; do H1 before Task 8.

---

## Stage A: freemapper-print

Branch `self-update` in `.worktrees/self-update`.

### Task 1: Dependencies, app identity, keys, ADR, glossary

**Files:**
- Modify: `Cargo.toml` and the CLI crate's `Cargo.toml` (the deps of Tech Stack, commented; `[features] test-stub = []` as freemapper's), `Cargo.lock`, `main.rs` (`mod update;`), `docs/glossary.md`, `docs/adr/README.md`
- Create: `src/update/{mod.rs,source.rs}` (`source.rs` only `HttpUpdateSource::new()` until Task 2), `assets/update/minisign.pub`, `tests/keys/` (core's test pair, key id 09F80E494B3AE92B, README as freemapper's), `docs/adr/0060-self-update-from-signed-releases.md`

**Interfaces:**
- Produces (`update/mod.rs`): `pub fn print_app() -> freemapper_update::App` (name, repo, keys from `KEY_FILE` plus the test key in test builds, `current` from `CARGO_PKG_VERSION`); `pub fn release_build() -> bool`; `pub fn update_source() -> HttpUpdateSource`; `pub fn settings_dir() -> Option<PathBuf>` (override if non-empty, else `dirs::config_dir()/freemapper-print`); `pub fn record_path() -> Option<PathBuf>` (override if non-empty, else `dirs::cache_dir()/freemapper-print`, plus `update-check.json`); `pub fn mode_from_env() -> Option<String>`. Private as freemapper's: `is_release`, `key_file_allowed` (also false for a release build without a key line), `key_lines`, `accepted_keys`, the three `const` assertions.

- [ ] **Step 1: Pin and lock.** `cargo update -p` only the new crates; each new lock entry has freemapper's version and is two weeks old. Expected: `cargo tree -p freemapper-print-cli -i ureq` shows only the CLI crate.
- [ ] **Step 2: Copy the key file**; `cmp` with freemapper's. Expected: identical, key line `RWT7yfq/KoRTk8uMufGlMksUd2aWa9IYvxnXeFKFT27pGSu9CG2WAynA`.
- [ ] **Step 3: Write the failing tests** (ports unless "new"): `app_keys_are_the_key_lines_of_the_key_file` (`name == "freemapper-print"`), `release_build_accepts_only_the_release_key` (`RELEASE_KEY_LINE` the line above), `a_release_build_refuses_the_test_key`, new `a_release_build_needs_a_key_line` (`!key_file_allowed(true, "# no key\n")`, `key_file_allowed(false, "# no key\n")`), `every_key_line_is_a_minisign_public_key`, `folders_are_the_override_or_the_system_folder` (override wins; empty override gives `/conf/freemapper-print`; `(None, None)` gives `None`; same for the record path). New in `tests/render_cli.rs`: `version_prints_the_contract_line` (stdout of `--version` and `--lang de --version` is exactly `"freemapper-print {CARGO_PKG_VERSION}\n"`).
- [ ] **Step 4: Run** `cargo test -p freemapper-print-cli update::`. Expected: FAIL (the version test may pass; it pins the contract).
- [ ] **Step 5: Implement** by porting freemapper's `update/mod.rs`.
- [ ] **Step 6: Run.** Expected: PASS; `FREEMAPPER_DIST=github-release cargo build -p freemapper-print-cli` compiles, with `--features test-stub` it fails on its assertion.
- [ ] **Step 7: ADR 0060** "Self-update from signed releases" (Proposed; context ADR 0094, core ADR 0026, the user's request of 2026-11-12; decision the port and P1 to P10; amends ADR 0054). Glossary: freemapper's five rows (Selbstaktualisierung, Aktualisierung (des Programms), Aktualisierungsmanifest, Aktualisierungsmodus, Tägliche Prüfung), adapted to print. unslop and humanizer.
- [ ] **Step 8: Commit** `feat(update): app identity, key file and folders of the self-update`, then `docs(adr): self-update from signed releases`.

### Task 2: The HTTP update source

**Files:** Modify `src/update/source.rs`.

**Interfaces:**
- Produces: `pub struct HttpUpdateSource` (`UpdateSource + Send + Sync`), `new()`, `pub fn was_not_found(&self, url: &str) -> bool`, `#[cfg(any(test, all(debug_assertions, feature = "test-stub")))] pub fn stub(base: &str) -> Self` (maps the two GitHub hosts to `base` after `check_url`); `const USER_AGENT = concat!("freemapper-print/", env!("CARGO_PKG_VERSION"), " (+", env!("CARGO_PKG_REPOSITORY"), ")")`; freemapper's limits and agent settings.

- [ ] **Step 1: Write the failing tests:** all ten of freemapper's `source.rs` tests, same names; new `requests_carry_only_the_fixed_user_agent` (`User-Agent == USER_AGENT`; no `Cookie`, `Authorization`, `From`).
- [ ] **Step 2: Run** `cargo test -p freemapper-print-cli update::source`. Expected: FAIL.
- [ ] **Step 3: Implement** by porting freemapper's `source.rs`.
- [ ] **Step 4: Run.** Expected: PASS.
- [ ] **Step 5: Commit** `feat(update): http source with checked redirects`.

### Task 3: `freemapper-print update`

**Files:** Create `src/update/command.rs`, `tests/update_cli.rs`, `tests/support/mod.rs` (port of freemapper's stub helpers; `MANIFEST_PATH = "/HenryJobst/freemapper-print/releases/latest/download/freemapper-print-update.json"`), `tests/fixtures/update/`; modify `main.rs` (`Commands::Update(UpdateArgs)`, `localised_command`, dispatch), catalogs, `.github/workflows/ci.yml`.

**Interfaces:**
- Produces: freemapper's `Request`; `pub fn run(texts: &Texts, request: Request) -> anyhow::Result<u8>` (0, 10 for `--check` with an update, errors exit 1); `pub(super) fn describe(texts: &Texts, error: &UpdateError) -> String`; for Task 5 `pub fn check_for_mcp(texts: &Texts) -> Result<CheckReport, String>` with `CheckReport { current: Version, available: Option<AvailableReport>, message: String }`, `AvailableReport { version: Version, released: Option<String>, notes: String, size_bytes: Option<u64>, manual: bool }` (`manual` for `TooOldToSelfUpdate`, then no date or size), and `pub fn install_for_mcp(texts: &Texts) -> Result<InstallReport, String>` with `InstallReport { installed: Option<Version>, notes: Option<String>, message: String }`, all fields `pub`.
- CLI as freemapper's: `update [--check] [--yes] [--version <X.Y.Z>] [--rollback] [--mode notify|install|off]`; help ids `update-about`, `help-update-{check,yes,version,rollback,mode}`.
- Messages: freemapper's `update-*` ids from `update-source-build` to `update-no-config-dir`, with print's name and variables. `update-source-build` per P8 with `https://github.com/HenryJobst/freemapper-print/releases/latest`; `update-no-asset` names the releases page; `update-needs-yes` gets an `agent-` twin naming `install_update` and `confirm`.

- [ ] **Step 1: Fixtures**, signed with the committed test key (`minisign -S -s tests/keys/test.key -m <f> -x <f>.minisig -t "freemapper-print test fixture"`): `manifest-99.json` (app `freemapper-print`, 99.0.0, `min_self_update_from` 0.1.0, five platforms, URLs under `…/freemapper-print/releases/download/v99.0.0/`), `manifest-99-manual.json` (min 98.0.0), `manifest-99-noasset.json` (`"assets": {}`), `manifest-99.json.wrong-key.minisig` (throwaway key, deleted after, never committed), `README.md` (source, 2026-11-12, commands, GPL-3.0-or-later).
- [ ] **Step 2: Write the failing tests:** all of freemapper's `update_*` tests in `tests/cli.rs` (outside `mod update_check`) and the unit tests of its `command.rs`, same names and assertions with print's names (`FREEMAPPER_LANG=en`, `CI` and `FREEMAPPER_PRINT_UPDATE` removed, both overrides in a temp dir; the ones against the stub behind `test-stub`; the run-time signing ones Unix-only and skipped without `minisign` unless `FREEMAPPER_PRINT_TEST_REQUIRE_MINISIGN`). Changed: `update_in_source_build_says_how_to_update` expects `releases/latest` and `cargo build --release --locked` (P8). New: `update_without_an_asset_for_this_platform_names_it` (exit 1; stderr names `current_platform()` and the releases page).
- [ ] **Step 3: Run** `cargo test -p freemapper-print-cli --features test-stub --test update_cli`. Expected: FAIL.
- [ ] **Step 4: Implement** by porting freemapper's `command.rs` with `texts.get(id, Some(&args!(…)))` and `u8` codes; EN and DE texts, unslop and humanizer.
- [ ] **Step 5: CI:** freemapper's minisign install steps (apt, brew), `FREEMAPPER_PRINT_TEST_REQUIRE_MINISIGN: "1"`, the two `test-stub` runs.
- [ ] **Step 6: Run** all of Global Constraints. Expected: PASS.
- [ ] **Step 7: Commit** `feat(update): the update command`.

### Task 4: Daily check, install mode, stale files, editor notice

**Files:** Create `src/update/background.rs`; modify `main.rs`, catalogs, `tests/update_cli.rs`.

**Interfaces:**
- Produces freemapper's `background.rs` API with these changes: `CheckContext { record: Option<PathBuf>, ci, mode, release, now: i64, source }` (no `offline`, `of_process()` instead of `of_command`); `BackgroundCheck::finish(self, texts: &Texts, command_ok: bool) -> Option<Report>`; `check_for_server(ctx: &CheckContext, texts: Texts, tell: impl FnOnce(String) + Send + 'static)` words its note with `update-notice`, `update-note-manual`, `update-note-bad-signature`, so `Texts::for_agent` gives Task 5's twins. `Report`, `start_background_check`, `recorded_update`, `remove_stale_files` as freemapper's.
- `main.rs`: `Commands::update_check(&self) -> bool` (P3); `remove_stale_files()` at every start except `mcp`; `command_ok = matches!(result, Ok(0))` (P5); before `editor::run`, `check_for_server(&CheckContext::of_process(), texts.clone(), |l| eprintln!("{l}"))` (P4).
- Messages: `update-notice` (`freemapper-print { $new } is available (you have { $old }). Update with: freemapper-print update`), `update-note-manual`, `update-note-bad-signature`, `update-applies-next-start`, `update-install-failed`.

- [ ] **Step 1: Write the failing unit tests:** freemapper's `background.rs` tests (`FakeSource` over `manifest-99`; `no_check_with_offline` dropped, "cache folder" read as "record path"); new: `slow_source_is_dropped_after_3_s_and_recorded_failed` (source sleeps 5 s, `finish` within 3.5 s), `no_asset_is_recorded_failed_and_not_shown`, `a_garbage_or_future_record_is_due_and_replaced` (`not json` and `at = now + 86_400`: both check; the file then parses with `at == now`), `install_mode_skips_after_failed_command`, `server_check_never_installs_in_install_mode`, `the_note_is_worded_for_the_reader` (`Texts::new` names `freemapper-print update`, `Texts::for_agent` names `install_update`).
- [ ] **Step 2: Write the failing CLI tests** behind `test-stub` (port of freemapper's `mod update_check`): `notice_line_on_stderr_after_command_output` (`check` on Viktoriapark: stdout and exit code as without it, notice the last stderr line), `no_second_check_within_24_h`, `numbers_and_masks_do_not_check`, `export_against_a_silent_stub_ends_within_the_budget` (own time plus at most 3.5 s, exit 0, record `failed`), `install_mode_installs_only_after_exit_0` (Unix, signed at run time: `export` installs, `check` with notes does not), `edit_prints_the_notice_on_stderr_at_start` (`edit --no-browser --port 0`, `available` record, ended through `done` as in the editor tests).
- [ ] **Step 3: Run** `cargo test -p freemapper-print-cli --features test-stub update`. Expected: FAIL.
- [ ] **Step 4: Implement** by porting freemapper's `background.rs` and wiring; texts, unslop, humanizer.
- [ ] **Step 5: Run** all of Global Constraints. Expected: PASS.
- [ ] **Step 6: Commit** `feat(update): daily check after the main commands`.

### Task 5: MCP tools and the update note

**Files:** Modify `src/mcp.rs`, `tests/mcp_cli.rs`, catalogs.

**Interfaces:**
- Produces: tools `check_update { lang? }` → `{"current", "available": null | {"version", "released", "notes", "size_bytes", "manual"}, "message"}` and `install_update { confirm: bool = false, lang? }` → `{"from", "installed": null | "x.y.z", "notes", "message"}` (after an install `message` ends with `update-mcp-restart-now`), inputs `deny_unknown_fields`. Source build: both give `{"message"}` from `agent-update-source-build`; `confirm` not true: tool error `update-mcp-needs-confirm`. `UpdateNote`, the `update_note` field and the `call_tool` override as freemapper's; `PrintServer::new` starts `check_for_server` with `Texts::for_agent`. `INSTRUCTIONS` gets one sentence: the two tools, `confirm: true` only after the person agreed, pass the note on.
- Messages: `agent-update-notice` (`Note for the person: freemapper-print { $new } is available (this is { $old }). Ask them before calling install_update.`), `agent-update-note-manual`, `agent-update-note-bad-signature`, `agent-update-source-build`, `update-mcp-needs-confirm`, `update-mcp-restart-now`.

- [ ] **Step 1: Write the failing tests:** freemapper's two `UpdateNote` unit tests in `mcp.rs`; in `tests/mcp_cli.rs` its `mcp_update_tools_in_a_source_build_say_how_to_update` and, behind `test-stub`, its `mod update` of `tests/mcp.rs`, reading JSON results. New: `tools_list_has_the_two_update_tools` (`confirm` is boolean), `first_tool_result_carries_the_note_once` (`project_info` twice on Viktoriapark: only the first has a second block starting `Note for the person: freemapper-print 99.0.0`), `mcp_never_installs_in_install_mode` (`FREEMAPPER_PRINT_UPDATE=install`: no asset requested), `stdout_stays_json_rpc_with_an_update_known`.
- [ ] **Step 2: Run** `cargo test -p freemapper-print-cli --features test-stub mcp`. Expected: FAIL.
- [ ] **Step 3: Implement**; descriptions inline in English as print's other tools; unslop and humanizer.
- [ ] **Step 4: Run** all of Global Constraints. Expected: PASS.
- [ ] **Step 5: Commit** `feat(update): mcp tools and the update note`.

### Task 6: Release workflow

**Files:** Create `.github/scripts/{update-manifest,release-tag}.sh` (ports with print's names, default repo and `"app"`), `tests/update_manifest.rs` (`#![cfg(unix)]`); modify `.github/workflows/release.yml`.

**Interfaces:**
- The scripts keep freemapper's arguments, variables and `min_self_update_from` rule (pin, else previous manifest, else own version: ADR 0094 addendum). Add no pin now.
- `release.yml` = freemapper's, every job and step, plus P2's `gate` job (`environment: release`, `contents: read`): with the secret in an environment variable, `updater=true` if it is non-empty; else `::error::` if `gh release` shows a published release with `freemapper-print-update.json`, otherwise `updater=false` and `::notice::The release environment has no MINISIGN_SECRET_KEY; this release ships without the updater (ADR 0060).` The value is never printed. `build` needs `gate`; `FREEMAPPER_DIST: ${{ needs.gate.outputs.updater == 'true' && 'github-release' || '' }}`; the `.gz` and `.exe.sha256`, the update steps, `update-smoke` and the release-text sentence on them run only with `updater == 'true'`.

- [ ] **Step 1: Write the failing tests:** all eight of freemapper's `tests/update_manifest.rs`, same names, parsing with `parse_manifest(…, "freemapper-print")`; minisign rule as in Task 3.
- [ ] **Step 2: Run** `cargo test -p freemapper-print-cli --test update_manifest`. Expected: FAIL.
- [ ] **Step 3: Write both scripts**, `chmod +x`. Run. Expected: PASS.
- [ ] **Step 4: Rewrite `release.yml`**; `actionlint` if installed. With `updater=false` it must give today's archives, `.sha256` files and release text.
- [ ] **Step 5: Commit** `ci(release): signed update manifest and bare executables`.

### Task 7: Documentation and merge

**Files:**
- `docs/manual/{en,de}.md`: chapter 2 "Installation" (README pointer, then "Updates": command and switches, exit 10, the check per P3, what is sent (manifest and signature requests to GitHub with the fixed user agent, nothing else), turning it off, folders and overrides, `.previous`, read-only folders, source builds); chapter 12's "makes no network requests" becomes "the editor sends nothing; the program contacts GitHub only for the update check"; chapter list; chapter 3's tool table.
- `README.md`, `README.en.md`: a paragraph under Installation; status, tool list.
- `AGENTS.md`: "The program makes no network requests." becomes: it contacts GitHub only for the update check, at most once a day, sends no data about the person or the project, and stops with `update --mode off` or `FREEMAPPER_PRINT_UPDATE=off`. The Install section says to prefer `freemapper-print update` (`--yes` once the person agreed) over unpacking a new archive. Rows `update`, `check_update`, `install_update`; exit 10; pass the note on and ask; troubleshooting lines.
- `CHANGELOG.md`, `CHANGELOG.en.md` (Unveröffentlicht/Unreleased); ADR 0060 Accepted, index row.

- [ ] **Step 1: Write the texts** after reading both SKILL.md files; run every quoted command; check both READMEs for currency.
- [ ] **Step 2: Commit** `docs: self-update in manual, READMEs and AGENTS.md`, `docs(changelog): self-update`.
- [ ] **Step 3: Whole-branch review**, fix all findings incl. Minor; merge `--no-ff`; fmt, clippy, tests on `main` with and without `test-stub`; push to `origin` and `github`; watch CI; remove worktree and branch; `cargo clean`.

## Stage B: the first updater release

### Task 8: Signed dry run, then the release

- [ ] **Step 1:** After the merge and H1, `dry_run: true`. Expected: `updater=true`; five archives, five `.gz`, manifest and `.minisig`; `minisign -V -p assets/update/minisign.pub -m freemapper-print-update.json` passes. Run URL into ADR 0060, commit `docs(adr): signed dry run`.
- [ ] **Step 2:** Release under the release policy: `min_self_update_from` is its own version, `update-smoke` skipped. At the next release, check `update-smoke` on three systems.

## Self-review

- Coverage: 3.1 (Tasks 1, 6, 8), 3.2 (core; 2, 3), 3.3 (4), 3.4 (1, 3, 4), 3.5 (core; 3), 3.6 (1), 3.7 (4, 5), 3.8 (1, 2), 3.9 (core; 2), 3.10 (2 to 6), 5 (H1, 8), 6 (7), addendum (6). P3 replaces D11, P4 D8, P1/P2 D16. Names stay the same from task to task; no bodies.

## Size and order

H1 (the user, 10 minutes, before Task 8); Tasks 1 to 7 in order (about 600 lines of Rust, 150 of scripts and workflow, docs); Task 8 after the merge. Recommended: subagent-driven, because the tasks share interfaces and a broken install path or workflow reaches every user at once; reviewers matter most after Tasks 3, 5 and 6.
