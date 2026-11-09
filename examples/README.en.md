# Example projects

[Deutsch](README.md)

This folder holds finished events built on maps from
[freemapper's examples](https://github.com/HenryJobst/freemapper/tree/main/examples). Each example has the
project file, its inputs, the generated PDFs and a preview image. Every kind of event gets one example as soon
as the program can print it:

| Example | Shows | From |
|---|---|---|
| [`luebeck-scales`](luebeck-scales/) | the same sprint map at 1:4000 on A4 and 1:3000 on A3: exact scale, symbols grow to 133 % as the standard sets | M1 |
| [`luebeck-competition-layouts`](luebeck-competition-layouts/) | one competition with two layouts, A4 1:4000 for the older classes, A3 1:3000 for the youngest, with passe-partout, title block, two logos, scale check bar and footer, built with `init`, `check` and `export` alone | M2 |
| `schoolyard` | a schoolyard map at 1:1000 on A4; waits for a schoolyard map among freemapper's examples (version 0.8.0 has none) | planned |
| [`luebeck-competition-courses`](luebeck-competition-courses/) | the same competition with three courses from Purple Pen, control descriptions, control numbers (one corrected with `numbers accept`), an out-of-bounds area, a forbidden crossing and a marked route to the finish, plus the same courses as IOF XML 3.0 without descriptions | M3 |
| [`berlin-viktoriapark-training`](berlin-viktoriapark-training/) | a club training in Berlin's Viktoriapark: one course from Purple Pen on A4 at 1:4000 with control descriptions, on the map from freemapper version 0.8.0 | M3 |
| `luebeck-training-window` | window orienteering with a mask variant | M4 |
| `luebeck-training-corridor` | corridor orienteering with a mask variant | M4 |

All maps come from freemapper 0.8.0 (commit `a2b41b4`). As sprint maps they lie on the light brown base area
(ADR 0044), and the parks are yellow.

Tour sheets for control placers, team orienteering and a forest map follow later.
