# 0060. Self-update from signed releases

Date: 2026-11-12
Status: Proposed, amends [0054](0054-mcp-tool-set.md)

## Context

freemapper-print ships as an archive that a person unpacks once and registers with an AI assistant by its absolute
path. A new release reaches nobody until they download it again. freemapper solved this on 2026-11-11 (its ADR 0094),
with the pure parts in freemapper-core (core ADR 0026, in the crate `freemapper-update`, pinned here since
v0.21.1). The user asked for the same in print on 2026-11-12. The release checksums prove integrity, not origin, so
the update needs a signature.

## Decision

Print ports freemapper's self-update as it is and does not redesign it. `freemapper-print update` replaces the
running executable in place with the newest release, keeps the old one as `freemapper-print.previous` for
`--rollback`, and installs nothing that is unsigned or unverified. A bad signature is an error, never "no update".
A daily check reports new versions, an opt-in mode installs them, and two MCP tools let an agent do both after the
person agreed. The app part lives in `crates/freemapper-print-cli/src/update/`; the manifest is
`freemapper-print-update.json` with a `.minisig`, the files are `freemapper-print-<tag>-<platform>.gz`. The text of
freemapper's ADR 0094 and its spec apply; the calls below are print's own.

- **P1 Key.** Print uses freemapper's key (the user's decision of 2026-11-12), key id 9353842ABFFAC9FB. There is no
  new pair. One secret is easier to guard, and the manifest's `app` field keeps one program's manifest from
  installing as the other. If the key leaks, it signs updates for both programs. The key list allows a rotation: a
  release signed with the old key adds the new one, in both programs. `assets/update/minisign.pub` is a byte copy of
  freemapper's. Agents never make, read, set or store a secret key. The tests sign with core's test pair in
  `tests/keys/` (key id 09F80E494B3AE92B), which no release build accepts: the build refuses to compile if the key
  file holds it or holds no key line.
- **P2 Release gating.** The release workflow is freemapper's, plus a job `gate` that reports only whether the
  signing secret is set. The builds must know before they compile whether the binary checks for updates, because a
  checking binary whose release has no manifest tells everyone "no manifest". Without the secret, the release is
  today's, with a notice, unless an earlier release has a manifest. Then `gate` fails, since a release without a
  manifest breaks `latest/download`. The secrets exist, so the first release after the merge signs. Protection rules
  on the environment make `gate` wait for approval too.
- **P3 When.** The check runs after `init`, `check`, `export` and `render`, and at the start of `mcp` and `edit`. It
  does not run after `numbers`, `masks` or `update`, because mask and number edits come in series and `check`
  follows them. There is no `--offline`; mode `off` does that. A user who only edits masks sees the notice a day
  later.
- **P4 Editor.** `edit` prints one line to stderr at its start and adds no banner to the page. That needs no new
  endpoint, script or CSP change, and `edit` runs from a terminal that already shows its address. The suite's
  `#banner` can take the line later. A person who works only in the browser misses it until another command runs.
- **P5 Install mode** installs only after exit 0, so a `check` with notes (exit 1) installs nothing, and never in
  `mcp` or `edit`. The install waits for the next clean run.
- **P6 Record.** The record of the last check sits in the cache folder, `<cache_dir>/freemapper-print/update-check.json`,
  and `FREEMAPPER_PRINT_CACHE_DIR` overrides the folder. That keeps tests off `HOME`, on Windows too. Settings are
  `<config_dir>/freemapper-print/settings.toml`, overridden by `FREEMAPPER_PRINT_CONFIG_DIR`; the mode comes from
  `[update] mode` (default `notify`) or `FREEMAPPER_PRINT_UPDATE`.
- **P7 MCP style.** The tools `check_update` and `install_update` take `lang` and return compact JSON with a
  `message`, like print's other tools. `install_update` needs `confirm: true`. The first result of a session may carry
  a note about a new version, worded with the `agent-<id>` twins that ADR 0054 describes (`update-notice` for the
  CLI and the editor, `agent-update-notice` for the server). The cost is two JSON shapes to keep in step. This adds
  two rows to the tool table of ADR 0054, which now lists thirteen tools.
- **P8 Builds not made by the release workflow** say that they were built from source or by a release without signed
  updates, and name the releases page plus `git pull` and `cargo build --release --locked`. A release built without
  the secret (P2) takes this path too.
- **P9 HTTP.** `ureq` =3.4.2 with default features (rustls, ring, webpki-roots), as freemapper uses it. That is about
  30 crates new to print. A hyper client on print's axum and tokio stack would bring the same TLS crates plus code of
  our own. Cost: 1 to 2 MB of binary, and `ring` needs a C compiler, which the runners have. The agent follows
  redirects by hand, at most 3, each through the host rule of `freemapper-update`, and sends
  `User-Agent: freemapper-print/<version> (+https://github.com/HenryJobst/freemapper-print)`. It carries no e-mail
  address, name, id or project data. There is no telemetry.
- **P10 Folders and clock.** Folders come from `dirs` 7.0.0. The clock is `SystemTime`; print adds no `chrono`.

New dependencies, all at freemapper's locked versions and older than two weeks (ADR 0006): `freemapper-update`
(core v0.21.1), `ureq` =3.4.2, `dirs` 7.0.0, and for tests `minisign-verify` =0.3.0 and `sha2` 0.11.1. `flate2`
moves to 1.1.11 through core.

## Consequences

- People update with one command, or with none, and `--rollback` undoes it. The program's path stays the same, so
  the MCP registration keeps working.
- The program now contacts GitHub, once a day at most, where it made no network request before. `AGENTS.md` and the
  READMEs say so.
- print and freemapper share one signing key (P1) and one update crate, so a fix in core's swap reaches both.
- An update that breaks reaches everyone in `install` mode at once. Rollback and the post-release smoke test limit
  that.
- Cost if wrong: a compromised key signs for both programs, and the rotation then needs a release of each.

Source: user request of 2026-11-12; freemapper ADR 0094 with its addendum and spec
`docs/superpowers/specs/2026-11-11-self-update-design.md`; freemapper-core ADR 0026; plan
`docs/superpowers/plans/2026-11-12-self-update.md`.
