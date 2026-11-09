# 0024. Program texts in Fluent files (suite rule)

Date: 2026-11-08
Status: Accepted, loader superseded by freemapper-text (see the end)

## Context

The suite programs (freemapper, freemapper-courseplan, freemapper-print) show German and English texts, and more
languages should be easy to add later. freemapper-courseplan settled the approach in its ADR 0022 and built the
module. freemapper-print had English-only messages in `main.rs`.

## Decision

- Program texts live in Project Fluent files: `crates/freemapper-print-cli/i18n/<language>/freemapper-print.ftl`,
  embedded at build time and read with `fluent-bundle` (concurrent bundle, isolation marks off).
- The loader is copied from freemapper-courseplan (`src/text.rs`, commit cf1dae0 on `m2a-editing`) and says so
  in its first doc lines. A shared crate may replace the copies later.
- English is the reference and the fallback. A test fails when a language has other message ids than English.
- The language comes from `--lang`, `FREEMAPPER_LANG`, `LC_ALL`, `LC_MESSAGES` and `LANG`, in that order. An
  unknown `--lang` is a usage error (exit code 2); an unknown environment value passes to the next source.
- `--lang` is a global option. The help texts are set on the derived clap command after the language is known.
- Tool, parameter, JSON and identifier names stay English.
- `fluent-langneg` is not a direct dependency: the program matches language codes itself and needs no
  negotiation (`fluent-bundle` still brings 0.13.1).
- German texts follow Duden on capitals after a colon: a capital when a complete sentence follows, lower case
  otherwise. This replaces the blanket lower case of an earlier review note.
- What the library reports to the user comes as values, not English sentences: sheet notes and their reasons are
  enums (`LogoSkipReason`, `LogoSimplification`, `MissingItem`), and the CLI gives each variant a message. Only
  details from the operating system, a decoder or the SVG check (an element name, a limit) stay as they come.
- A sheet note that does not count toward `check`'s exit code (`SheetNote::is_info`) starts with "Info:" /
  „Zur Information:“, every other with "Note:" / „Hinweis:“.

## Consequences

- Clap's own words (`Usage`, `Options`, `Print help`) and its parse errors stay English.
- Error messages from the library crate stay English `Display` texts. The CLI puts a translated lead-in in front
  ("could not print the map: ..."). Localising library errors comes later.
- Numbers in messages use the decimal separator of the language (`decimal-separator`).
- Each new message is written in every language, and reviews check the German.
- Cost if wrong: Fluent files are plain text and convert to another catalog format.

## Supersede note: the loader moves to freemapper-text (core switch, Task 1)

- The bullet "The loader is copied from freemapper-courseplan" no longer holds. The catalog, the language
  matching and the fallback come from `freemapper-text` (freemapper-core v0.2.0, core ADR 0010; ADR 0042). The
  copy is gone. `text.rs` keeps a static `Catalog` with our `.ftl` files and thin wrappers, so call sites did not
  change. `unic-langid` and `fluent-syntax` are no longer direct dependencies; `fluent-bundle` stays for
  `FluentArgs`.
- The test that every language has the ids of English is core's `problems()` check now. It also demands the
  same variables in every language.
- Ruling R3: English `note-missing-scale-or-contour` has a select on `$count` with two identical variants, because
  German uses `$count` and the check refuses a message whose variables differ. The output is the same as before.
- `every_id_used_in_code_exists` scans every `.rs` file under the CLI crate's `src/`, so a new source file with
  ids is covered without a change to the test.

Source: user decision 2026-11-08 (suite rule); freemapper-courseplan ADR 0022.
