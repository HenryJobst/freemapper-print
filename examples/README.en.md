# Example projects

[Deutsch](README.md)

This folder holds seven finished events on two maps by Remo Madella, drawn for the MOO Challenge Berlin 2025:
"Gleisdreieck" and "Rummelsburger Bucht". Each example has the project file, its inputs, the generated PDFs and a
preview image. The README of each example explains what the sheet shows and how it was made.

| Example | Shows | Map |
|---|---|---|
| [`gleisdreieck-scales`](gleisdreieck-scales/) | the same map section at 1:4000 on A4 and 1:3000 on A3: exact scale, symbols grow to 133 % as the standard sets | Gleisdreieck |
| [`rummelsburg-competition-layouts`](rummelsburg-competition-layouts/) | one competition with two layouts, A4 1:4000 for the older classes, A3 1:3000 for the youngest, with passe-partout, title block, two logos, scale check bar and footer, built with `init`, `check` and `export` alone | Rummelsburger Bucht |
| [`rummelsburg-competition-courses`](rummelsburg-competition-courses/) | the same competition with three courses (Kurz, Mittel, Lang) from a Purple Pen file, control descriptions, control numbers (one corrected with `numbers accept`), an out-of-bounds area, a forbidden crossing and a marked route to the finish, plus the same courses as IOF XML 3.0 without descriptions | Rummelsburger Bucht |
| [`gleisdreieck-training`](gleisdreieck-training/) | a club training: two courses on A4 at 1:4000, one sheet each with title block, club logo, scale check bar and control descriptions | Gleisdreieck |
| [`gleisdreieck-training-window`](gleisdreieck-training-window/) | window orienteering: map only in round windows around start, controls and finish, built with `masks window`, one window larger than the others | Gleisdreieck |
| [`gleisdreieck-training-corridor`](gleisdreieck-training-corridor/) | corridor orienteering: map only in a corridor 12 mm wide, the line chosen by hand and built with `masks corridor` | Gleisdreieck |
| [`rummelsburg-mcp-session`](rummelsburg-mcp-session/) | a recorded session with the MCP server: an AI agent turns a training course into window orienteering with 12 mm windows, checks the sheet and writes the PDF, in six tool calls | Rummelsburger Bucht |

## Maps and courses

Remo Madella released both maps for the freemapper suite on 12 November 2026 and agreed to their use as example
maps in this public project. That is a permission for the suite, not an open licence. The `ATTRIBUTION.txt` in each
example has the wording, in German and English. Whoever wants to use the maps otherwise asks Remo Madella.

The courses are made up, and so are the events, dates and clubs. freemapper-courseplan 0.14.1 made them, and they
come as a Purple Pen file or as IOF XML. The club logos were drawn for the examples and are CC0.

Each example carries its own copy of the map so that the folder runs on its own. Git stores identical files once.
The examples weigh about 35 MB together. A PDF of the Gleisdreieck map is 2.1 to 3.2 MB, one of the Rummelsburg map
1.1 MB. Since freemapper-core v0.22.0 a PDF leaves out what the frame cannot show, so a sheet with `crop = true`
holds only the visible part of the map.

## The old examples

The earlier examples on Lübeck and Berlin maps from freemapper 0.8.0 moved to
[`tests/fixtures/examples/`](../tests/fixtures/examples/README.md). They are test data, and the tests compare
their PDFs byte for byte.

Tour sheets for control placers, team orienteering, a schoolyard map and a forest map follow later.
