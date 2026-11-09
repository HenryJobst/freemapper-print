# 0034. Overprint layers and blending

Date: 2026-11-09
Status: Accepted

## Context

IOF "Printing and Colour Definitions" Rev. 4 splits the course purple in two (spec §5.9). Lower purple
(start, controls, legs, forbidden lines) lies below black, brown and blue 100 %. Upper purple (numbers, map
exchange, forbidden areas, marked routes) lies above the map. An office printer has no spot colour and no
overprint flag, so the PDF has to show the result. A map file numbers its colours by priority and paints the
highest number first; it does not say which colour is black, brown or blue.

## Decision

- `overprint::overprint` turns one fitted course into shapes in page mm, sorted into `Overprint::lower` and
  `Overprint::upper`. Lower: start, control and finish circles, legs, out-of-bounds boundaries, forbidden
  routes and their crosses, crossing points, first aid, water and plain lines. Upper: control numbers, map
  exchanges, out-of-bounds and dangerous-area hatches, marked routes (legs and specials) and texts. White-out
  is left for masks (M4).
- Sizes are the rule profile's course symbol sizes times the symbol factor (ADR 0018, ADR 0031). Leg gaps and
  number offsets are map mm and scale with the geometry. The hatches (0.25 mm lines, 0.6 mm apart) and the
  shapes of crossing point, first aid and water come from OOM's ISOM 2017 and ISSprOM 2019 sets until the
  profile carries them.
- `full_strength_priority` finds the full-strength colours by CMYK: black is 0/0/0/1; brown has m ≥ 0.5,
  y ≥ 0.75, c ≤ 0.3 and k ≤ 0.2; blue has c ≥ 0.9, m ≤ 0.1, y ≤ 0.1 and k = 0, each with a tolerance of 0.001.
  The highest priority number among them is the colour painted first. The PDF writer paints the lower layer
  just before the first primitive of that priority or a lower one, and the upper layer after the last map
  primitive. Without a full-strength colour, the lower layer goes after the map, just before the upper one.
- Both layers are painted in the course colour inside the map frame's clip, each in a `q … Q` block that sets
  one ExtGState with `/BM /Multiply`. Multiply only darkens, so map detail under the purple still shows and
  black under the upper purple stays black, as on a press that overprints the purple. Opaque purple would hide
  the map under every number and route.
- `RenderedPage::overprint` is empty by default. An empty overprint writes no ExtGState and no extra block, so
  pages without a course stay byte for byte as before (`existing_outputs_unchanged`).
- Masks (M4) will clip the map to their holes. They will not paint white over the map. White painted after
  the map would also cover the lower purple, which lies among the map primitives; a clip leaves both layers
  intact and keeps the overprint where the course setter put it.

## Consequences

- Course sheets (Task 7) build the overprint per course and call `Overprint::on_map` with the rule set's purple
  and the page's colours.
- Taking the highest matching priority puts the lower purple below every full-strength colour, but also below
  every other colour painted after it. OOM's ISSprOM set has "Black for open land" at priority 29 and an
  orange at 20 that passes the brown test, so on the Lübeck map the lower purple lies below the buildings and
  the road fills (`luebeck_full_strength_priority`).
- Cost if wrong: one function chooses the place of the lower layer, and the writer only reads `below_priority`.
