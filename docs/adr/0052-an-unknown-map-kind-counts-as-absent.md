# 0052: An unknown map kind in freemapper.toml counts as absent

Status: accepted, amends ADR 0050

## Context

`read_sidecar` parsed `map_kind` straight into `MapKind`. A kind print does not know, such as the `gym` planned
for freemapper, made the whole `freemapper.toml` invalid: the user got the note for a broken file, and `init`
lost the `title` of the same file and fell back to the file name. freemapper-core v0.10.0 adds the fixture
`future-kind` for this case and `legacy-0.9.0-scale-bar` for a map from freemapper 0.9.0 (core ADR 0018,
addendum).

## Decision

- `map_kind` is read as text. A name that is none of `sprint`, `schoolyard` and `forest` gives no kind and no
  error (the user's decision of 2026-11-10; the alternative was an error asking for a newer print). The kind
  then comes from the map file as before, or from `--kind`.
- `title` is read whatever `map_kind` says.
- Names are matched as written: `Sprint` is unknown.
- A `map_kind` that is no text, and a file that is no TOML, stay errors with the note.
- The contract tests run over all four fixtures. For `future-kind` they expect no kind from the sidecar, a kind
  from the map, and the title. For the legacy map they expect no title and the old scale bar among the sheet
  objects.

## Consequences

- A map with a kind print cannot print (a gym map) is treated as whatever its scale and symbol set suggest, or
  stops at "kind unknown" with the hint to pass `--kind`. No message names the unknown value. If that proves
  confusing once gym maps exist, a note can name it.
- An unknown key or an unknown `freemapper:sheet` role never mattered to print; the fixture now pins that.
