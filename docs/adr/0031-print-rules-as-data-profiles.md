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

## Addendum: the loader moves to freemapper-rules (core switch, Task 2)

- The file format, the parser, the merge of base and overlays, the id, unit and range checks and the types
  `Rule`, `RuleProfile`, `SetRule`, `SwitchedOff`, `Source`, `SourceRef`, `LocalizedText`, `Severity` and
  `ProfileKind` now come from `freemapper-rules` (freemapper-core v0.2.0, core ADR 0011). `rules.rs` re-exports
  them. The copy of courseplan's loader is gone.
- `rules.rs` describes the print profiles in a `static SCHEMA`. It lists the eight keys with the values each
  reads (formerly `KNOWN_KEYS`) and the units. The only table is `scale_table` `rows`, whose check (formerly
  `TABLE_VALUES` and part of the resolution) refuses a row of the wrong shape and rows that do not ascend by
  scale. There are two filters, `map_kinds` (text) and `print_scales` (whole numbers from 1 to 2^32 - 1).
  `course_colour`, `placement_weights` and `number_gap` take neither filter. The proposal source is `fmp`.
- A value check in the schema keeps `description_box` `cell_mm` within 4 to 10 mm (`MIN_CELL_MM` and
  `MAX_CELL_MM` of `project.rs`). It also refuses a list under any value name but `rows`. freemapper-rules
  accepts a list of numbers under a unit name and checks it item by item. Every value print reads is one
  number, so a list there was an error before the switch and stays one ("is a list, but north_lines reads
  one number").
- These stay in freemapper-print: the embedded profiles and `base()`, the resolution of every value per map
  kind, `needed` and `optional`, the completeness check (`Incomplete`), the base-scale row rule, `number_gap`
  with `GapRange`, `KindValues` and the typed accessors. Our `RuleSet` wraps the merged set of
  freemapper-rules and adds the resolved values. `RuleError` keeps its variants and texts. A core error is
  mapped to the variant of the same name.
- The resolution asks `RuleSet::pick` with the map kind and its base scale. This gives the rule the old
  `applies(kind, base_scale)` chose. When `pick` finds several rules (`Ambiguous`), the set is `Incomplete`
  and the message names every rule and the map kind. Two rules read as before ("rules a and b both give
  north_lines spacing_metres for sprint maps"). Three or more are now all named ("rules a, b and c all
  give ...").
- Three refusals changed. Rows that do not ascend are now `Malformed` when the profile is parsed, where they
  were `Incomplete` when the set was resolved. A rule that cites `fmp` without a note is now refused by the
  loader. Before, only a test over the embedded profile checked it. Some `Malformed` texts read differently:
  unknown filter values, print scales, a filter on a kind-independent key, non-finite numbers, value names a
  key does not read, and an id that differs from the file name. Variants and rule ids are unchanged.
- A golden snapshot (`tests/fixtures/rules/resolved.txt`), made with the old loader, holds every resolved
  value of the base set and the test overlays. One test case per refusal the old tests had checks variant
  and rule id. Refusals the old tests never covered (`print_scales` not a list, `[4000.0]`, `rows = []`,
  `enabled = "yes"`) are not pinned by a case here; core tests them. The structural refusals (TOML syntax,
  `[profile]` fields, the type of `enabled`, the shape of a scale row) live in core now and are covered there.
  Both pass unchanged after the switch, and the example PDFs are byte-identical.

- Ruling R4: the `Incomplete` text for a missing base-scale row keeps the old wording and names no rule id, so
  nothing observable changes in this switch. Three tests for it assert the variant only.
- Accepted, no caller: `RuleSet` derefs to the core set, so core's `pick` is reachable from outside. Profile
  and overlay names that match several sets give `Ambiguous`, which `profile()` and `parse_profile()` map to
  `Incomplete`.

Source: spec §5.9; M3 plan Task 1; freemapper-courseplan ADR 0025; ISSprOM 2019-2 (January 2022), ISOM 2017-2
(errata to February 2022), IOF Printing and Colour Definitions Rev. 4, International Specification for Control
Descriptions 2024; Purple Pen source code.
