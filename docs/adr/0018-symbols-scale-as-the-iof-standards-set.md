# 0018. Symbols scale as the IOF standards set

Date: 2026-11-08
Status: Accepted

## Context

ADR 0012 kept sprint symbols at the same size from 1:3000 to 1:5000 and gave forest maps 150 % at 1:10000,
1:7500 and 1:5000. The standards say otherwise. ISSprOM 2019-2 §3.1 sets 1:4000 as the base, recommends
1:3000 for the youngest age groups and requires: "When a map is enlarged, all lines, symbols and screens shall
be enlarged proportionally." §4.7 gives 133 % for 1:3000. ISOM 2017-2 §2.9.1 and §3.7 set 1:15000 as the base
and name 150 % for 1:10000 and 300 % for 1:5000, so 1:7500 is 200 %. Neither standard foresees a print below
the base scale.

## Decision

The symbol size table in `scale.rs` holds, per map kind and scale, the size relative to the base scale and
whether the standard foresees the scale:

| Kind | Scale | Size | Standard |
|---|---|---|---|
| Sprint | 1:3000 / 1:4000 / 1:5000 | 133 % / 100 % / 100 % | yes / yes / no |
| Forest | 1:5000 / 1:7500 / 1:10000 / 1:15000 / 1:20000 | 300 / 200 / 150 / 100 / 100 % | yes / yes / yes / yes / no |
| Schoolyard | 1:1000 | 100 % | yes |

The symbol factor is the size at the print scale divided by the size at the map scale. A reduction below the
base scale is allowed (user decision 2026-11-08): the symbols keep their base size, the report flags the scale
as non-standard, and the CLI says the print is for training only. Other scales are refused.

## Consequences

- A 1:3000 sprint print for the youngest classes has 0.36 mm small footpaths instead of 0.27 mm, and the map
  needs a third more paper.
- Cost if wrong: a wrong row prints wrong symbol sizes. Each row quotes the standard's section.

Source: ISSprOM 2019-2, ISOM 2017-2 Rev 6, user decision 2026-11-08 (final review of M1). Supersedes 0012.
