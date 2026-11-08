# 0021. Map kind inference, older files and CLI exit codes

Date: 2026-11-08
Status: Accepted

## Context

The symbol factor depends on the map kind (ADR 0018), which most users cannot name. Club maps are often in
old OOM formats. Scripts need to tell a wrong command line from a map that cannot be printed.

## Decision

- `freemapper.toml` next to the map decides the kind. A file that cannot be read or parsed counts as missing,
  and the CLI says so in a note.
- Otherwise the symbol set name and the map scale decide: a sprint marker (`ISSprOM`, `sprint`) at 1:3000 to
  1:5000 is a sprint map, at 1:1000 a schoolyard map; `ISOM` at 1:5000 to 1:15000 is a forest map. Without a
  marker only 1:7500 to 1:15000 count as forest; 1:5000 is both a sprint and a forest scale and stays unknown.
  An unknown kind asks for `--kind`.
- Every OOM XML version is read; defaults follow `<map version>`, so a text symbol without `rotatable` is
  rotatable before version 9. Rotatable texts turn about their anchor.
- The CLI exits with 0 on success, 1 when the map cannot be read or printed, and 2 for a wrong command line
  (clap's convention).

## Consequences

- Most maps print without `--kind`, and old maps print as OOM shows them.
- Cost if wrong: an unnamed 1:5000 forest map needs `--kind`; a script that expects 1 for every error has to
  accept 2.

Source: plan M1 Tasks 3, 5 and 8, controller rulings 2026-11-08.
