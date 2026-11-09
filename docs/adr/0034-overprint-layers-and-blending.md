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
  number offsets are map mm and scale with the geometry. The out-of-bounds hatch comes from the profile per
  map kind: ISSprOM 2019-2 and ISOM 2017-2 both draw symbol 709 as a cross hatch at 45° and 135°, 1.2 mm
  apart, with 0.2 mm lines, and Purple Pen (`OOBCourseObj` at 201417f) agrees. ISOM 2000 had vertical
  stripes; a profile can still say so with one angle. A dangerous area is always a cross hatch with the same
  spacing and width, as Purple Pen draws it. The shapes of crossing point, first aid and water come from
  OOM's ISOM 2017 and ISSprOM 2019 sets until the profile carries them.
- A map colour is full strength when its name is "black", "brown" or "blue", alone or with " 100%" or
  " 100 %" (ignoring case and surrounding spaces), or else when its CMYK lies within 0.02 of black 0/0/0/100,
  blue 100/0/0/0, or brown 25/75/100/0 (Rev. 4) or 0/56/100/18 (Rev. 3).
- The lower layer goes right above the topmost colour that is not full strength:
  `below_priority = (lowest priority number among those colours) - 1`, and the PDF writer paints the layer
  before the first primitive with that priority or a lower one. Only colours the page's primitives use
  count, because OOM's symbol sets carry the course purple itself as colour 0 and would put the layer over
  everything. With no such colour, or no colours at all, `below_priority` is `None` and the layer goes after
  the map, just before the upper one. The upper layer always goes after the last map primitive.
- Full-strength colours painted before that place stay under the lower purple: on the Lübeck page Blue 100 %
  for area features (16) and Black below lower light brown (15), and Black for open land (29) on maps that use
  it. That is fine, because the fills painted over them cover them anyway. The lower purple lies over the buildings (Black
  50 %, 10) and the road fills (Lower brown 30 %, 14), and Black 100 % (2) lies over the purple.
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
  and the page's colours and primitives.
- On the forest sample the topmost painted colour that is not full strength is Brown 50 % (2), so the lower
  purple lies under Black (1) and over everything else, Brown 100 % and Blue included.
- Cost if wrong: one function chooses the place of the lower layer, and the writer only reads `below_priority`.
