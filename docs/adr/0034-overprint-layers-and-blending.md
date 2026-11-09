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
  `Overprint::upper`.
  - Lower: start, control and finish circles, legs, out-of-bounds boundaries (707/708, the "forbidden
    lines" of Rev. 4), crossing points, first aid (712), refreshment points (713) and plain lines. ISOM
    2017-2 section 3.7 gives 712 and 713 as lower purple; ISSprOM 2019-2 has neither symbol.
  - Upper: control numbers, map exchanges, out-of-bounds and dangerous-area hatches, forbidden routes (711,
    upper purple in ISOM 2017-2 section 3.7, one point or several), marked routes (legs and specials) and
    texts.
  - White-out is no overprint shape. It becomes a mask (ADR 0045).
- Sizes are the rule profile's course symbol sizes times the course factor (ruling R35): the size factor of the
  print scale's row in the symbol size table (ADR 0018, ADR 0031), `RuleSet::course_factor`. The profile gives
  course symbols at the base scale, and ISOM 2017-2 section 3.7 and ISSprOM 2019-2 section 4.7 enlarge them with
  the print scale, as the map's own symbols. The symbol factor (print scale's size over map scale's) is only
  right for the map's symbols, which the `.omap` holds already enlarged for its own scale: with it, a forest map
  drawn at 1:10000 printed 5.0 mm circles at 1:10000 instead of 7.5 mm. Leg gaps and number offsets are map mm
  and scale with the geometry. A leg starts and ends where its path first leaves a
  circle, so a bend inside a circle is not drawn.
- Text specials scale with the geometry factor, not the symbol factor. Purple Pen treats them as map
  objects: the text box is placed and sized on the map, and the text has to stay in its box when the map
  is enlarged.
- The profile gives per map kind:
  - The out-of-bounds hatch. ISSprOM 2019-2 and ISOM 2017-2 both draw symbol 709 as a cross hatch at 45°
    and 135°, 1.2 mm apart, with 0.2 mm lines, and Purple Pen (`OOBCourseObj` at 201417f) agrees. ISOM
    2000 had vertical stripes; a profile can still say so with one angle. A dangerous area is always a
    cross hatch with the same spacing and width, as Purple Pen draws it.
  - First aid (ISOM 2017-2 712: bars 4.0 mm long, 1.33 mm wide) and refreshment point (713: 3.5 mm outside
    measure, 0.4 mm line). Sprint maps take these paper sizes too, noted as an assumption.
  - The shapes of crossing point and cup come from OOM's ISSprOM 2019 and forest sets. A forbidden route (711)
    is a row of crosses about 5 mm apart, the middle of ISOM's 4.0 to 6.0 mm, with no line between them, as
    ISOM 2017-2 draws it and OOM's ISOM 2017-2 set has it. Each cross has two arms 3.0 mm long and 0.35 mm wide,
    so it spans 2.1 mm, as OOM draws the 3.0 of the standard. One point is Purple Pen's single cross.
- A colour name is canonical when it is "black", "brown" or "blue", alone or with " 100%" or " 100 %",
  ignoring case and surrounding spaces. A colour matches by CMYK when it lies within 0.02 of black
  0/0/0/100, blue 100/0/0/0, or brown 25/75/100/0 (Rev. 4) or 0/56/100/18 (Rev. 3).
- `below_priority` is the highest priority number among the canonical colours the page paints: the lower
  layer goes right under the deepest of them, and the PDF writer paints it before the first primitive with
  that priority or a lower one. Only when no painted colour has a canonical name does the CMYK list decide
  the same way. Only colours the page's primitives use count, because OOM's symbol sets carry colours such as
  Blue 100 % or the course purple that a map may never paint. With no match `below_priority` is `None` and
  the layer goes after the map, just before the upper one. The upper layer always goes after the last map
  primitive.
- Duplicates such as "Black for open land" have no canonical name, so they stay under the purple, as do all
  fills painted before the canonical colour. On OOM's ISSprOM order the purple lies right under Brown 100 %
  (8): over every fill, under Blue, Black and Green 100 %. Green 100 % (3, impassable vegetation) over the
  purple is accepted; it is a small area colour, and Rev. 4 places only black, brown and blue over the purple.
- Both layers are painted in the course colour inside the map frame's clip, each in a `q … Q` block that sets
  one ExtGState with `/BM /Multiply`. Multiply only darkens, so map detail under the purple still shows and
  black under the upper purple stays black, as on a press that overprints the purple. Opaque purple would hide
  the map under every number and route.
- `RenderedPage::overprint` is empty by default. An empty overprint writes no ExtGState and no extra block, so
  pages without a course stay byte for byte as before (`existing_outputs_unchanged`).
- Masks (ADR 0045) clip the map to their windows and paint no white. White painted after the map would also
  cover the lower purple, which lies among the map primitives. Each mask of a sheet is a clip block of its own
  inside the frame's clip, and every map primitive lies inside these blocks, the base area included. The
  overprint and the sheet layer lie outside them.
- With a mask the lower layer goes after the whole map, after the mask blocks are closed. At its usual place it
  would sit inside them and lose its circles and legs outside the windows. Under black it looks the same with
  Multiply: black over the purple and the purple multiplied with black are both black. A full-strength colour
  other than black that is painted after the usual place (blue or brown 100 %) is darkened by the purple instead
  of covering it. The Lübeck map paints no such colour (see Consequences). Without a mask the writer keeps the
  usual place, byte for byte.

## Consequences

- Course sheets (Task 7) build the overprint per course and call `Overprint::on_map` with the rule set's purple
  and the page's colours and primitives.
- The Lübeck page paints only one canonical colour, Black 100 % (2); Blue 100 % (7) and Brown 100 % (8)
  paint nothing there. The lower purple lies under it and over the buildings (Black 50 %, 10), the road fills
  (Lower brown 30 %, 14), Black below lower light brown (15) and Blue 100 % for area features (16).
- On the forest sample the painted canonical colours are Black (1) and Brown (7). The lower purple lies
  under Brown and everything painted after it, Brown 50 % (2) included, and over the greens and yellows.
- Cost if wrong: one function chooses the place of the lower layer, and the writer only reads `below_priority`.

Amended in the final fix wave of M3: the course factor (ruling R35) and the crosses of a forbidden route.
Amended in M4 Task 2: masks clip the map, and with a mask the lower layer goes after the map (rulings R3, R5).
