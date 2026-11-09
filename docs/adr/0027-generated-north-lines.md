# 0027. Generated north lines

Date: 2026-11-09
Status: Accepted

## Context

ADR 0023 made north lines map content and left generating them for M2. A map without any gets them from the
layout when `north_lines = "auto"`. Open were the spacing, the symbol, what to do without a 601 symbol, and
when a map counts as having north lines.

## Decision

- **Detection.** The program looks for objects of symbol 601 or 601.x in the whole map, before the layout hides
  parts. A layout that hides the part holding the mapper's north lines gets no replacements; hiding means the
  layout does not want them. Hidden symbols count too, because the decision is about the file, not the drawing.
- **Spacing.** Ground distance per map kind, `north_line_spacing_m` in `scale.rs`: sprint 120 m (30 mm at 1:4000),
  forest 300 m (20 mm at 1:15000), schoolyard 37.5 m (37.5 mm at 1:1000). The schoolyard value is not verified
  against the IOF school set. On the map the spacing is `spacing_m * 1000 / map scale` mm, so a print at another
  scale keeps the ground distance (40 mm at 1:3000 for sprint).
- **Position.** Lines are vertical in map coordinates, because OOM draws the map oriented to magnetic north, and lie
  at map x = k times the spacing from x = 0. They span the frame of the page and nothing more. The program draws
  the map once to learn the frame (page size, orientation, which map point is at the page centre), then draws it
  again with the lines, orientation and centre fixed. So the lines change neither the page nor the overflow report.
  They stop short of the frame edge by half the painted width of the chosen symbol (at the symbol factor) plus
  0.01 mm for that reason, so the gap is under 0.1 mm for a thin line. Positions that would saturate the `i32`
  µm range of print coordinates are dropped, so a hostile `centre_map_mm` cannot stack lines on one x. At most 10 000 lines are made; a frame holds a few dozen.
- **Symbol.** The map's 601, else 601.1, else the lowest 601.x by number. A candidate counts only if it is a
  line symbol (type 2) or a combined symbol (type 16) and is not hidden in the file. In the Lübeck and forest
  sample sets 601.1 is an area symbol (the "North lines pattern") that draws nothing along a two-point path, and
  Lübeck's 601.4 and 601.9 are hidden, so the renderer skips them. The lines are objects of that symbol in an
  extra part "north lines (generated)", without the sheet tag, so they are drawn and sort by the colour priority of
  the symbol like the mapper's lines. They run unbroken. Breaking them at small features is the mapper's job in OOM;
  the note says so.
- **Without any usable 601 symbol.** Plain black lines of 0.14 mm go on the sheet layer, within the frame, and a separate
  note says the map has no north-line symbol. The sheet layer lies over the map, not under it. The width is
  multiplied by the symbol factor of ADR 0018, as every map line is, so an enlarged print keeps the proportion.
- Maps that already have north lines print byte-identical to before.

## Consequences

- Two drawings of the map for a layout that adds lines; maps with lines or `never` draw once.
- Cost if wrong: schoolyard maps get lines 37.5 mm apart until the value is checked. A map that uses 601 for
  something else gets no generated lines (ADR 0023). A hidden-symbol 601 object also suppresses generation.
