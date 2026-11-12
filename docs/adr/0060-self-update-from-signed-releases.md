# 0060. Self-update from signed releases

Date: 2026-11-12
Status: Proposed, amends [0054](0054-mcp-tool-set.md)

## Context

A new release reaches nobody until they download it again. The checksums next to the archives prove integrity, not
origin, so the update needs a signature. freemapper
solved this on 2026-11-11 (its ADR 0094; the pure parts are in freemapper-core ADR 0026, crate `freemapper-update`,
pinned here since v0.21.1). The user asked for the same in print on 2026-11-12.

## Decision

Print takes freemapper's design unchanged: `freemapper-print update`, `--rollback`, a daily check, an opt-in install
mode, nothing unsigned installed. What is ported is described there. Print's own calls:

- **P1 Key.** freemapper's key (user's decision), id 9353842ABFFAC9FB, no new pair. One secret to guard; the
  manifest's `app` field keeps one program's manifest from installing as the other. A leak signs for both.
  Tests sign with core's test pair (`tests/keys/`), which a release build refuses at compile time.
- **P2 Release gating.** freemapper's workflow plus a job `gate` that checks only whether the signing secret is set.
  Without it the release is unsigned and says so, unless an earlier release has a manifest; then `gate` fails.
  `gate` runs in the `release` environment, so it waits for that environment's required reviewer, if it has one.
- **P3 When.** After `init`, `check`, `export`, `render`; at the start of `mcp` and `edit`; not after `numbers`,
  `masks`, `update`. No `--offline`; mode `off` does that.
- **P4 Editor.** One stderr line at the start of `edit`, no page banner (no new endpoint, script or CSP change).
- **P5 Install mode** installs only after exit 0, never in `mcp` or `edit`.
- **P6 Folders.** Record `<cache_dir>/freemapper-print/update-check.json` (`FREEMAPPER_PRINT_CACHE_DIR`), settings
  `<config_dir>/freemapper-print/settings.toml` (`FREEMAPPER_PRINT_CONFIG_DIR`), mode `FREEMAPPER_PRINT_UPDATE`.
- **P7 MCP.** Tools `check_update` and `install_update` (needs `confirm: true`) take `lang` and return compact JSON
  with a `message`; the first result may carry a note worded by the `agent-<id>` twins. Amends ADR 0054: 13 tools.
- **P8 Other builds** (source, or a release built without the secret) say so and name the releases page.
- **P9 HTTP.** `ureq` =3.4.2 with rustls, as freemapper: about 30 new crates, 1 to 2 MB, `ring` needs a C compiler.
  Redirects by hand (3 at most, each through core's host rule); User-Agent `freemapper-print/<version>`; no data
  about the person or project, no telemetry.
- **P10** Folders from `dirs` 7.0.0, clock from `SystemTime`, no `chrono`.

The new crates are at the versions of freemapper's Cargo.lock and at least two weeks old (ADR 0006): `freemapper-update`
0.21.1 from core, `ureq` 3.4.2, `dirs` 7.0.0, `minisign-verify` 0.3.0, `sha2` 0.11.1. The exception is `flate2`
1.1.11 (published 2026-11-06): core pins `=1.1.11` in `freemapper-update`, so 1.1.10 cannot resolve. ADR 0042
exempts core's own crates, not `flate2`; this exception ends with the next core tag that moves `flate2`.

## Consequences

- People update with one command or none; the path stays, so the MCP registration keeps working.
- The program now contacts GitHub, once a day at most; `AGENTS.md` and the READMEs say so.
- print and freemapper share one key and one update crate. A broken update reaches `install` users at once;
  rollback limits it. Cost if wrong: a leaked key needs a release of each program.

Source: user request of 2026-11-12; freemapper ADR 0094 and its spec; core ADR 0026; plan `docs/superpowers/plans/2026-11-12-self-update.md`.
