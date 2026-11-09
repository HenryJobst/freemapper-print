# 0038. Number positions and the placement module

Date: 2026-11-09
Status: Accepted

## Context

Spec §5.7 gives control numbers four levels of position: automatic placement, the course file, a correction for
the layout and a correction for the sheet. `check` scores every number and proposes a free position, and
freemapper-print never moves a number on its own. The scoring and the search are shared with
freemapper-courseplan, which places numbers by the same rules: its ADR 0030 (the gap), ADR 0031 (behind the
circle, off map symbols) and ADR 0032 (the gap range). Purple Pen stores number positions as offsets from the
control, so they need turning into a direction and a gap.

## Decision

- **Levels.** The more specific level wins. (1) Automatic placement, only for a numbered stop the course file
  gives no position for. (2) The course file: `Stop::number` if a reader sets it, else the offset of the `.ppen`
  `number-location` or IOF XML `MapTextPosition`, both per course. (3) A `[[numbers]]` correction with `layout`
  equal to the sheet's layout. (4) A correction with `course` equal to the sheet's course. A correction without
  `gap_mm` keeps the gap of the level below; over an automatic number that is the smallest gap of the range.
- **Gap.** `gap_mm` is the shortest distance between the outer edge of the circle line (radius plus half the
  line width) and the number box, with the box centred on the ray in the number's direction (ruling R4,
  courseplan ADR 0030). The forward form has a corner case and an edge case; the inverse is the distance from
  the circle centre to the box minus the outer radius. Gaps are print mm at the base scale and grow with the
  symbol factor, like the circle, so a number keeps its look at every print scale. The box is the number's width
  in Arimo by its digit height.
- **Offsets from the course file** (ruling R12). Purple Pen adds `number-location` to the control's position in
  map coordinates and, with its default item scaling, draws course symbols at their nominal print size at the
  course's print scale (`CourseFormatter.CreateControlNumber` and `CourseView.CourseObjRatio`, commit
  `201417f9`). So the offset is scaled by map scale ÷ print scale (the course's `print-scale`, else the map
  scale), and the box and circle take the nominal sizes of the profile. Direction and gap are then exact, not
  rounded, so the number lands where Purple Pen prints it when both use the same scale. Purple Pen's other item
  scalings (relative to the map, relative to 1:15000) are not read. A number that touches its circle in the file
  has a negative gap; it is drawn so, and `base_seen` takes any finite gap for that reason.
- **Placement rules** (ruling R14, the user's rules of 2026-11-09, courseplan ADR 0031): the number covers no
  point or line symbol; it goes "behind", in line with the incoming leg (for the first control, from the start);
  if the box behind, at the smallest gap, comes within half a line width of the outgoing leg, the outer bisector
  is preferred; a blocked place gives way to the free place that deviates least. Score courses and stops
  without legs keep their current direction as preferred, and automatic placement starts at 45°.
- **Gap range** (rulings R29 and R30, courseplan ADR 0032). The rule `iofprint.number.gap` (key `number_gap`)
  allows 0.5 to 5 mm. With several rules the largest minimum and the smallest maximum count; without one, or with
  it switched off, the search uses 0.5 mm alone. A rule with neither value is skipped. A missing minimum is 0.5
  mm, a missing maximum stays open. Both limits are held to 0.01 to 10 mm, the minimum rounded up and the
  maximum down to 0.01 mm, and a maximum below the minimum is raised to it. Values that are not finite or not
  above 0 still stop the profile from loading. The search tries gaps from the minimum in 0.5 mm steps, each
  rounded to 0.01 mm, the last exactly at the maximum. `check` and placement use the same normalised range.
- **Ranking** (rulings R20 and R29, courseplan ADR 0031 and 0032). 24 directions,
  `normalize(round((preferred + 15·k)·10)/10)`, the rounded value evaluated and stored, at every gap of the
  range. Candidates rank lexicographically by overprint conflicts, covered map symbols, angle to the preferred
  direction, gap, angle to the outer bisector and direction. Courseplan gives areas no cost; ADR 0031 expects
  freemapper-print to differ only by a small area cost. So the soft cost sits after the angles and the gap and
  before the direction: it only breaks ties, and the user's rules decide everything else.
- **Obstacles.** Overprint, ranked first: the course's stop symbols as circles out to the outer edge of their
  line (a triangle as the circle through its corners, a crossing point by half its length), the legs as drawn
  between the symbols (leg gaps ignored), the other numbers, the sheet elements with the default footer, and
  leaving the map frame. Map symbols, ranked second: primitives of point and line symbols as drawn, rings as
  polygons and strokes as segments with half their width. Courseplan tests a point within 0.5 mm of its position
  and a line within 0.15 mm of its centre; freemapper-print has the drawn shapes and uses them. Each map object
  counts once. North lines (601 and 601.x, not 6011) never count. Areas and map texts are soft: their boxes,
  weighted per mm² by colour class (`placement_weights`, classes by name and CMYK as for the lower purple). Not
  obstacles yet: special objects and masks (M4).
- **Symbol class.** `Primitive` carries `class: SymbolClass` (point, line, area, text), a north-line flag, the
  index of its map object and its symbol id, set where the renderer draws. A combined symbol counts by its path,
  as courseplan's map reader does: closed is an area (a building with its outline), open a line. The PDF writer
  ignores these fields; the M1 and M2 outputs stay byte-identical (`cmp` on the examples).
- **Check.** Every number on a course sheet is scored and gets a proposal: the best placement among the same
  obstacles. Proposals are made in stop order, and a proposal for a number in conflict becomes an obstacle for
  the stops after it in place of that number's box, so two numbers are not sent to the same spot (ruling R32). A
  warning `NumberConflict` names what the number covers and the proposal when it has an overprint conflict or
  covers any point or line symbol of the map; rule 1 is hard (ruling R31). Map symbols are named by their code
  and counted per symbol. Without a free candidate the note says so and proposes the least covered one. A
  warning `NumberGapOutOfRange` names a gap from the file or a correction outside the rule's range; crossing
  points print no number and are not checked. The sheet always draws the number where its level puts
  it. `Stale::CorrectionOverridesChangedBase` is reported when a correction's `base_seen` matches
  (`base_changed`, 0.5° and 0.05 mm) none of the file's current placements for its control in the courses it
  applies to. A layout correction may stand over several courses with different file positions, and
  `base_seen` holds the one it was made against.
- **Project file.** `gap_mm` of a correction lies in 0 < gap ≤ 10 mm, as courseplan accepts.
- **Module boundary.** `src/placement/mod.rs` uses nothing of the crate; a test reads the file and refuses
  `crate::`, `super::` and the crate name. It knows rectangles, obstacle shapes with a tier and a label, the
  number box, the score, the gap range, the targets with their legs and the search. `src/numbers.rs` resolves the
  levels, converts file offsets, turns the course, the sheet elements and the map into obstacles, and reports.
  The module moves into a crate of its own later, and courseplan takes it over then.

## Consequences

Numbers from Purple Pen print where Purple Pen puts them, numbers without a position are placed by the shared
rules, and every conflict reaches `check` with a proposal to copy into `[[numbers]]`. Task 11 adds a command to
accept proposals. A Purple Pen file with another item scaling places numbers a little off, and leg gaps and
special objects do not free or block space for numbers yet.

Source: M3 plan Task 10, controller rulings R4, R12, R14, R20 and R29 to R32, freemapper-courseplan ADR 0030,
0031 and 0032.
