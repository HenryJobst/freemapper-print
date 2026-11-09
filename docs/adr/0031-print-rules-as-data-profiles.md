# 0031. Print rules as data profiles

Date: 2026-11-09
Status: Accepted

## Context

Spec §5.9 says print rules become data profiles, an IOF base with national or regional overlays, in the format
freemapper-courseplan defines. Its ADR 0025 (v0.2.0) has that format now. Until M2 the values sat in code:
the symbol size table (ADR 0018), the north-line spacing (ADR 0027) and the proposed contour intervals in
`scale.rs`. M3 adds the course overprint, which needs more of them: course symbol sizes, the purple, the cell
size of control descriptions and weights for placing control numbers.

## Decision

- Print rules live in a TOML profile in courseplan's format 1: `[profile]` with `id`, `kind`, `base`, `name`
  and `valid_from`, `[sources.<doc>]`, and `[[rules]]` with `id`, `key`, `severity`, `values`,
  `source = { doc, section }`, an optional `note` and `enabled`. Rule ids are stable and dotted,
  `<profile>.<family>.<name>`. A number names its unit (`metres`, `mm`, `degrees`, `percent`, `factor`); the
  only table is `rows` of `scale_table`, documented in `TABLE_VALUES`. Unknown fields, keys and value names are
  errors. The loader and its validation are copied from courseplan (commit 86f9a42, v0.2.0) with a provenance
  comment in `rules.rs`; freemapper-print does not depend on courseplan.
- Changes against courseplan: print rules have no `disciplines` filter. Each rule key names the values it reads,
  so a misspelt value name is an error even when it carries a unit. Every value holds at the base scale of its
  map kind, so a `print_scales` filter is read at the base scale. Loading a set resolves every value for every
  map kind at once. A value no rule gives, or two rules give, makes the set unusable, so the program never runs
  into a gap later. `course_colour` and `placement_weights` hold for every map kind and take no filter.
- Profiles are embedded with `include_str!`. M3 embeds one base profile, `iof-print`. Profiles from user files
  come later. The overlay mechanism works now: an overlay replaces a base rule with the same id or switches it off
  with `enabled = false`, and the set records which profile switched it off. Tests use test-only overlays.
- The project file names its profile in `[print] rule_profile` (default `iof-print`) and its overlays in
  `rule_overlays` (default empty). Both are left out of a written file while they hold the default, so existing
  files write as before. An id that is not embedded, a base given as an overlay, or an overlay for another base
  is an error with file, line and column.
- `load` loads the named rule set once and keeps it in `LoadedProject::rules`. These values follow the
  project's profile and overlays: course symbol sizes, the description cell, placement weights and the purple
  (read by the M3 course tasks), and the north-line spacing on sheets. Base-only for now: the scale table, the
  north-line spacing and the contour intervals behind `scale.rs`, which keeps its public functions and reads
  `rules::base()`. So `render` without a project, the allowed scales, the symbol factor and the contour
  interval `init` proposes come from `iof-print`. Outputs stay byte-identical to M2.
- Every `scale_table` needs a row for the base scale of its kind at `size_factor = 1.0` and standard, because
  every other value holds there. A set without one cannot be used.
- Values in `iof-print` and their sources:
  - Symbol size table and north-line spacing as ADR 0018 and ADR 0027 give them. The schoolyard spacing stays
    a working value with a note.
  - Contour interval proposals: sprint 2 m (ISSprOM 2019-2 §3.2), forest 5 m (ISOM 2017-2 §2.10).
  - Course symbols, one rule per symbol and standard, from ISSprOM 2019-2 §4.7 (at 1:4000) and ISOM 2017-2
    §3.7 (at 1:15000). Purple Pen's `NormalCourseAppearance` (commit 201417f) gives the same sizes:

    | Value | Sprint | Forest |
    |---|---|---|
    | control circle (703), centre to centre | 6.0 mm | 5.0 mm |
    | line width (705) | 0.35 mm | 0.35 mm |
    | start triangle side (701) | 7.0 mm | 6.0 mm |
    | finish circles (706), centre to centre | 5.0 and 7.0 mm | 4.0 and 6.0 mm |
    | control number height (704) | 4.0 mm | 4.0 mm |
    | marked route (707), dash and gap | 2.0 and 0.5 mm | 2.0 and 0.5 mm |
    | out-of-bounds boundary (708) | 1.0 mm | 0.7 mm |
    | crossing point (710), length | 3.0 mm | 3.0 mm |

    Schoolyard maps take the sprint values at 1:1000. That is an assumption, so the rule cites
    `doc = "fmp"` and must carry a note that says so.
  - Control description cell: 6 mm, the middle of the 5 to 7 mm the International Specification for Control
    Descriptions 2024 allows.
  - Purple: CMYK 35/85/0/0, IOF Printing and Colour Definitions Rev. 4, section 6 (spec §5.9).
  - Placement weights: black 10, brown 4, blue 4, other colours 1. This is our own call, cited as
    `doc = "fmp", section = "ADR 0031"`. A control number over black detail (paths, buildings, walls) hides
    the most, brown and blue less, and green or yellow areas the least.
  - `severity` follows the courseplan format. It says how a deviation would be reported; M3 does not read it yet.

## Consequences

- A national or club profile is a TOML file and a new build, not a code change. User profile files follow when
  someone needs one; then `rule_profile` may also name a path.
- An overlay that changes the scale table has no effect on the allowed scales or the symbol factor until
  `scale.rs` takes a rule set; it must still keep the base-scale row.
- Cost if wrong: renaming keys or value names in the embedded file. A shared `freemapper-rules` crate with
  courseplan may come later (courseplan ADR 0025).

Source: spec §5.9; M3 plan Task 1; freemapper-courseplan ADR 0025; ISSprOM 2019-2 (January 2022), ISOM 2017-2
(errata to February 2022), IOF Printing and Colour Definitions Rev. 4, International Specification for Control
Descriptions 2024; Purple Pen source code.
