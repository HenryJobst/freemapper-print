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

| Kind | Scale | Size | Standard foresees it | Section |
|---|---|---|---|---|
| Sprint | 1:3000 | 133 % | yes | ISSprOM 2019-2 §3.1 (youngest age groups), §4.7 (133 %) |
| Sprint | 1:4000 | 100 % | yes | ISSprOM 2019-2 §3.1 (base scale) |
| Sprint | 1:5000 | 100 % | no | ISSprOM 2019-2 §3.1 (below the base scale) |
| Forest | 1:5000 | 300 % | yes | ISOM 2017-2 §2.9.1/§3.7 (300 %) |
| Forest | 1:7500 | 200 % | yes | ISOM 2017-2 §2.9.1/§3.7 (proportional, 15000 / 7500) |
| Forest | 1:10000 | 150 % | yes | ISOM 2017-2 §2.9.1/§3.7 (150 %) |
| Forest | 1:15000 | 100 % | yes | ISOM 2017-2 §2.9.1/§3.7 (base scale) |
| Forest | 1:20000 | 100 % | no | ISOM 2017-2 §2.9.1/§3.7 (below the base scale) |
| Schoolyard | 1:1000 | 100 % | yes | IOF school symbol set 2019 (base scale) |

The symbol factor is the size at the print scale divided by the size at the map scale. A reduction below the
base scale is allowed (user decision 2026-11-08): the symbols keep their base size, the report flags the scale
as non-standard, and the CLI says the print is for training only. Other scales are refused.

## Consequences

- A 1:3000 sprint print for the youngest classes has 0.36 mm small footpaths instead of 0.27 mm. Each side of
  the map grows by a third, so it covers 78 % more paper: a map that fills A4 at 1:4000 needs A3 at 1:3000.
- Cost if wrong: a wrong row prints wrong symbol sizes. Each row quotes the standard's section.

Source: ISSprOM 2019-2, ISOM 2017-2 Rev 6, user decision 2026-11-08 (final review of M1). Supersedes 0012.

## Amendment 2026-11-09

The table moved from `scale.rs` into the embedded rule profile `iof-print` (rules `iofprint.scale.*`, ADR 0031).
The values are unchanged.
