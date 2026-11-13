# 0064. Examples on two released maps

Date: 2026-11-12
Status: Accepted, amends [0062](0062-manual-figures-from-two-released-maps.md)

## Context

The examples in `examples/` stood on Lübeck and Viktoriapark maps made by freemapper from open data. They are small
and plain, and the suite now has two real sprint maps: Remo Madella released "Gleisdreieck" and "Rummelsburger
Bucht" (MOO Challenge Berlin 2025) on 2026-11-12 for use inside the freemapper suite, and confirmed by e-mail on
the same day that the maps may serve as example maps in the public project (ADR 0062). The user decided on
2026-11-12 to replace the examples by Remo's maps. freemapper-courseplan 0.14.1 can make the courses for them.

## Decision

- `examples/` holds seven projects: `gleisdreieck-scales`, `gleisdreieck-training`, `gleisdreieck-training-window`,
  `gleisdreieck-training-corridor`, `rummelsburg-competition-layouts`, `rummelsburg-competition-courses` (with
  `iof/`) and `rummelsburg-mcp-session`. Each has READMEs in German and English, an `ATTRIBUTION.txt` with the
  author and the permission, a preview and the PDFs in `out/`.
- The courses were made with freemapper-courseplan 0.14.1. They, the events, dates and clubs are made up.
- Each example carries its own copy of its map, so the folder runs on its own. Git stores identical files once.
  The permission covers the suite, not an open licence, and `ATTRIBUTION.txt` says so.
- The old examples moved to `tests/fixtures/examples/`, where the tests already read their PDFs byte for byte, as
  freemapper-courseplan did with its own. They are test data now; their READMEs and previews were removed.
- `crates/freemapper-print/tests/examples.rs` loads every project in `examples/`, checks that its map, course
  file and logos exist and that every `ATTRIBUTION.txt` names "Remo Madella".

## Consequences

- The examples weigh about 35 MB. A PDF of the Gleisdreieck map is 2.1 to 3.2 MB, of the Rummelsburg map 1.1 MB.
  Before freemapper-core v0.22.0 every PDF carried the whole map even when `crop = true` cut the frame: the
  Gleisdreieck PDFs were 3.6 to 3.7 MB and the examples about 40 MB. Since v0.22.0 the PDF drops what the frame
  cannot show (core ADR 0031), and the sheets look the same at 150 dpi. What remains is the visible map, about
  570 000 curve points per sheet. Writing the curves as Bezier segments would shrink the files further and is not
  done. Release archives do not carry `examples/`.
- The `.omap` files are in the repository now. ADR 0062 kept them out because no figure needed them; the examples do.
- The sheets show the program of 0.8.0 with the window gap of ADR 0063. A change in the output means new PDFs and,
  where a README quotes them, new numbers.
