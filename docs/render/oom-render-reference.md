Copied from freemapper 743ce4b (ADR 0014).

# OOM rendering reference for freemapper PDF output

Scope: exactly what is needed to draw the symbols 101, 102, 201, 321, 401, 410.1, 418, 501,
501.1, 501.5, 501.10, 505.1, 505.2, 506, 513.2, 515, 518, 520, 520.1, 521, 521.1, 541–545, 601,
991–994 the way OpenOrienteering Mapper (OOM, master, GPL-3) renders them into a PDF.
Everything is derived from the OOM sources; each fact carries a source tag.

## 0. Conventions

Source tags (all paths below `src/` of github.com/OpenOrienteering/mapper, master):

| Tag | File |
|---|---|
| LS / LSH | core/symbols/line_symbol.cpp / .h |
| PS | core/symbols/point_symbol.cpp |
| AS | core/symbols/area_symbol.cpp |
| CS | core/symbols/combined_symbol.cpp |
| TS / TSH | core/symbols/text_symbol.cpp / .h |
| SY | core/symbols/symbol.cpp |
| RI | core/renderables/renderable_implementation.cpp |
| RN / RNH | core/renderables/renderable.cpp / .h |
| VP | core/virtual_path.cpp |
| PC / PCH | core/path_coord.cpp / .h |
| MC | core/map_coord.h |
| MCL | core/map_color.cpp |
| MAP | core/map.cpp |
| OBJ | core/objects/object.cpp |
| TO / TOH | core/objects/text_object.cpp / .h |
| XML | fileformats/xml_file_format.cpp |
| MP | core/map_printer.cpp |
| PDF | printsupport/qt-5.12.4/advanced_pdf.cpp, printsupport/patches/devicecmyk.diff |

- Units: all symbol lengths and object coordinates are integers in 1/1000 mm on paper
  ("native", µm). OOM converts with `0.001 * value` to mm everywhere [MC `MapCoord::x()`, LS].
- Map coordinate system: x to the right (east), **y down** (south) [MC, used throughout].
  PDF is y up: `x_pt = x_mm * 72/25.4`, `y_pt = (page_top_mm - y_mm) * 72/25.4`.
- `MapCoordF::angle() = atan2(y, x)` in y-down coordinates [MC `angle`].
- `perpRight(x, y) = (-y, x)` [MC `perpRight`]. For a direction east (1,0) this is (0,1) = south =
  the **right-hand side** when walking along the path. "Right" in OOM is always this side,
  "left" the opposite one.
- Rotating a local point (ox, oy) by r: `x = ox*cos r - oy*sin r + X`, `y = oy*cos r + ox*sin r + Y`
  [PS `createRenderablesScaled`]. In y-down coordinates positive r turns **clockwise** on paper.
- Missing XML attributes: OOM reads them with `toInt()`/`== "true"`, so a missing int is 0 and a
  missing bool is false (except where noted) [LS/PS/AS/TS `loadImpl`].

### 0.1 Features actually used by the listed symbols (ISSprOM 2019_4000.omap + additions)

Colour numbers are colour indices = priorities (see 1.1). Values at 1:4000 (school set: x1.25 via
enlarge.rs, except school additions 321/541–545 and sheet symbols 991–994, which are final sizes).

| Code | Type | Used features |
|---|---|---|
| 101 / 102 | line | colour 8, w 210 / 300, cap 1 round, join 2 round. Plain. |
| 201 | line | colour 2, w 500, cap 0, join 1 miter. **Mid symbol** (rotatable point; element line (0,100)->(0,750), w 180, colour 2, flat). segment_length 750, end_length 180, `show_at_least_one_symbol` absent = **false**. |
| 321 | point | ring: inner_radius 520, outer_width 130, outer colour 7, no inner colour. Element: point symbol (dot r 250, colour 7) at (0,0). Not rotatable. |
| 401 | area | inner colour 30, no patterns. |
| 410.1 | line | colour 3, w 400, flat, miter. |
| 418 | point | dot r 100 colour 21 + ring outer_width 300 colour 3. |
| 501 | combined | part 1 = symbol index 105 (501.5 area colour 14); part 2 private line colour 15, w 200 (school: 2 x 438 = 876, set by symbol_set.rs). |
| 501.1 | line | colour 2, w 100 (school: 438). |
| 501.5 | area | inner colour 14. |
| 501.10 | combined | part 1 = index 114 (501.11 area colour 13); part 2 private line colour 15, w 100 (school 876). |
| 505.1 / 505.2 | line | colour 14, w 350 / 500, not dashed. **Borders** (one `<border>` = both sides): colour 15, width 100, shift 50, **dashed** dash 2000 / break 250. |
| 506 | line | colour 2, w 270, **dashed** dash 1500, break 370, dashes_in_group 1, no half outer dashes. |
| 513.2 | line | colour 2, w 210. **Mid symbol**: rotatable point, element area colour 2 = half disc r 300 on +y made of 2 Béziers. segment 1900, end 850, show_at_least_one true. `mid_symbol_placement="2"` is ignored (line not dashed). |
| 515 | line | colour 2, w 400, segment_length 0 (no mid symbol anyway). |
| 518 | line | colour 2, w 400. **Mid symbol**: rotatable, two lines w 140: (450,0)->(825,650) and (-450,0)->(-75,650). segment 3750, end 2325, show_at_least_one **false**. |
| 520 | area | inner colour 22. |
| 520.1 | line | colour 15, w 100. |
| 521 | combined | parts by index: 143 (521.3 area colour 9), 144 (521.4 line colour 2, w 140). |
| 521.1 | point | rotatable. Elements: area colour 9 square ±180 (4 coords, no close flag); line colour 2 w 140 closed square (last coord flag 18). |
| 541 | line | colour 2, w 120. **Start + end symbol**: ring inner_radius 120, outer_width 180, colour 2, not rotatable. |
| 542 | point | not rotatable. Elements: closed square ±750 line w 220; diagonals w 120. |
| 543 | line | colour 2, w 220. **Start + end symbol**: rotatable, element line (0,-400)->(0,400) w 180. |
| 544 | point | ring inner_radius 370, outer_width 180, colour 2. |
| 545 | point | ring 520/180 colour 2 + two lines ±212 w 120. Not rotatable. |
| 601 | line | colour 2, w 140. |
| 991 / 992 / 993 | text | Arial 5000 bold / 2500 / 1800, colour 2, line_spacing 1, paragraph 0, char spacing 0, kerning true, no framing, **no `rotatable` → not rotatable**. |
| 994 | line | colour 2, w 250, cap 0 flat, join 1 miter. |

Not used by any listed symbol: line patterns/point patterns, pointed caps, start/end offsets,
dash symbols, dash points, mid symbols on dashed lines, text framing. Real maps use them; §2.2,
§2.4, §2.5, §2.7, §4 and §6 cover them (freemapper-print M1).

---

## 1. Colours and draw order

### 1.1 Colour references
- A symbol's `color="N"` is an index into the map's colour list: `map.getColor(N)`; `-1` (or any
  other negative except special values) yields nullptr = "no colour", the part is not drawn
  [LS/PS/AS/TS `loadImpl`; MAP `Map::getColor`].
- The colour at index i has priority i: colours are stored in file order and
  `colors[i]->setPriority(i)` [MAP line ~155; XML `importColors`]. In the ISSprOM file the
  `priority` attribute equals the position.

### 1.2 Draw order (the single most important rule)
- Every renderable carries `color_priority = color->getPriority()` [RNH `Renderable::Renderable`].
- All renderables of all objects are collected in `std::map<int priority, std::map<const Object*, …>>`
  [RNH `MapRenderables`, `ObjectRenderablesMap`].
- `MapRenderables::draw` iterates priorities with `rbegin()…rend()`, i.e. from the **highest
  priority number down to 0**. Priority 0 is painted last and therefore **on top** [RN
  `MapRenderables::draw`]. PDF export uses this same `Map::draw` path [MP `drawPage` -> `map.draw`].
- Object order, part order, symbol order and the order of parts in a combined symbol have
  **no influence** across colours. A single object's renderables are split up by colour and
  interleave with those of all other objects.
- Same colour, different objects: order is `std::map<const Object*>` = pointer order, i.e.
  arbitrary [RNH `ObjectRenderablesMap`]. Inside one object and colour, `PainterConfig` order is
  clip path, then pen before brush, then pen width [RNH `operator<(PainterConfig)`]. For opaque
  colours none of this is visible, so freemapper may draw same-colour items in any order.
- Do **not** merge fills of different objects into one even-odd PDF path: overlapping same-colour
  areas would punch holes. Emit one fill per area renderable (merging strokes of equal
  width/cap/join is harmless).

Priorities of the colours used (ISSprOM 2019), top to bottom:

| Prio | Name | C | M | Y | K |
|---|---|---|---|---|---|
| 2 | Black 100% | 0 | 0 | 0 | 1 |
| 3 | Green 100% | 0.76 | 0 | 0.91 | 0 |
| 7 | Blue 100% | 1 | 0 | 0 | 0 |
| 8 | Brown 100% | 0 | 0.56 | 1 | 0.18 |
| 9 | Black 60% | 0 | 0 | 0 | 0.6 |
| 13 | Lower brown 50% | 0 | 0.28 | 0.5 | 0.09 |
| 14 | Lower brown 30% | 0 | 0.168 | 0.3 | 0.054 |
| 15 | Black below lower light brown | 0 | 0 | 0 | 1 |
| 21 | White over green | 0 | 0 | 0 | 0 |
| 22 | Green 50%/Yellow 100% | 0.38 | 0.27 | 0.886 | 0 |
| 30 | Yellow 100% | 0 | 0.27 | 0.79 | 0 |

Consequences: 501.5/501.11 fills (14/13) cover the inner half of their own edge line (15);
505.x brown fill (14) covers the black dashed borders (15) of a crossing path; 520.1 (15) lies
under every black-100% symbol; 418's white dot (21) shows over green areas of priority > 21.

### 1.3 CMYK for DeviceCMYK
- OOM's "Device CMYK" PDF mode uses `AdvancedPdfPrinter` [MP `makePrinter`], whose engine writes
  `QColor::cyanF/magentaF/yellowF/blackF` with `scn`/`SCN` in a `/DeviceCMYK` colour space
  [PDF devicecmyk.diff].
- The QColor is built from `MapColor::cmyk` (`q_color = cmyk` unless CMYK is derived from RGB)
  [MCL `updateCalculatedColors`]. So use the colour element's `c m y k` attributes (0..1) directly
  [XML `importColors`].
- For `<cmyk method="spotcolor">` OOM recomputes CMYK from the spot components:
  `x = x + factor * spot.x * (1 - x)` per channel, starting from white [MCL `cmykFromSpotColors`].
  The stored attributes already equal that result (checked: 14 = 0.3 x Brown, 22 = 0.5 Green over
  Yellow), so reading the attributes is enough.
- Normal PDF drawing is opaque painter's algorithm. Knockout/overprint flags only affect
  separations and the raster overprinting simulation [RN `drawColorSeparation`,
  `drawOverprintingSimulation`], never the normal PDF. "White over …" colours paint opaque white.

### 1.4 Opacity
- `<color opacity="…">`; if < 1 the QColor gets `setAlphaF(opacity)` [RN `MapRenderables::draw`].
  All ISSprOM colours have opacity 1. If ever < 1: PDF constant alpha (ExtGState `/ca`, `/CA`).
- Symbols with `is_hidden` are skipped, helper symbols are skipped unless enabled [RN `draw`].

---

## 2. Line symbols

### 2.1 Basic line [LS `createSinglePathRenderables`, RI `LineRenderable`]
- Drawn if `color && line_width > 0`. Stroke width = `0.001 * line_width` mm, pen only.
- `cap_style`: 0 Flat, 1 Round, 2 Square, 3 Pointed (rendered with flat cap + extra tapered area,
  see 2.7) [LSH `CapStyle`; RI `LineRenderable`]. PDF `J`: flat 0, round 1, square 2 [PDF `setPen`].
- `join_style`: **0 Bevel, 1 Miter, 2 Round** [LSH `JoinStyle`]. PDF `j`: miter 0, round 1, bevel 2.
- Miter limit: `LineSymbol::miterLimit() = 1` (Qt units) [LSH]; for PDF engines OOM converts it with
  `sqrt(1 + 4 * limit²)` [RI `fixPenForPdf`] and writes `max(1, limit) M` [PDF `setPen`].
  **Use PDF miter limit √5 = 2.2361** for every miter-joined line. Text framing uses Qt limit 0.5
  -> PDF √2.
- Each path part (see 7) is drawn separately with `closed = part.isClosed()` [LS
  `createRenderables`]. Closed parts: `closeSubpath()` -> join at the closing vertex, no caps
  [RI `LineRenderable`]. Qt's PDF writer emits `h` whenever a subpath ends on its start point
  [PDF `generatePath`].
- `minimum_length` is **not used for rendering**; it only drives the "line too short" check
  [OBJ `PathObject::isLineTooShort`].
- Simple case (no dashes, no start/end offset): the stroke is the unmodified path; borders and mid
  symbols are computed from the same path [LS `createSinglePathRenderables`].

### 2.2 Dashed main line (506) [LS `processDashedLine`, `createDashGroups`]
Only if `dashed="true"` and `dash_length > 0`. A part is split into sections at coordinates with
the DashPoint flag (32); freemapper paths normally have one section = the whole part.

`dashed="true"` with `dash_length <= 0`: `processDashedLine` is not called, so there is no
processed path: no main line, no mid symbols and no borders; dash, start and end symbols and
pointed caps remain [LS `createSinglePathRenderables`]. A border with `dashed="true"` and
`dash_length == 0` is invisible [LS `LineSymbolBorder::isVisible`]; with a dash length but no
break length it is solid.

Sections (taken from OOM **v0.9.6**) [LS `processDashedLine`; VP `PathCoordVector::findNextDashPoint`]:
the sections of [S, E] (E, S after the offsets of 2.7) end at every dash point strictly between
S and E, the last at E. Each section runs the algorithm below with `is_part_start` (first section)
and `is_part_end` (last section). At a dash point between two sections:
- `k == 1`: the section before ends with a half dash and the section after starts with one
  (`half_first`/`half_last`). The section before **does not draw its last dash**: it returns that
  dash's start (`line_start`), and the next section's first dash runs from there, so the two
  halves form one dash through the dash point.
- `k > 1`: no halves; the last dash before the dash point is likewise carried into the next
  section's first dash, which then spans the dash point.
- A section too short for dashes that is not the last one draws nothing and hands its whole
  length to the next section (`return line_start`).

Per section [S, E] (arc lengths in mm), with d = dash_length, b = break_length,
g = in_group_break_length, k = dashes_in_group (all x0.001):

```
half_first = is_part_start ? (half_outer_dashes || closed) : (flag(S) has DashPoint && k == 1)
half_last  = is_part_end   ? (half_outer_dashes || closed) : (k == 1)
H   = half_first + half_last                     // 0, 1 or 2
G   = k*d + (k-1)*g ;  T = G + b
L   = E - S
nf  = H + (L + b - H*d/2) / T
lo  = floor(nf)
min_opt_dashes = 2*k - H/2                       // C++ integer division
sw  = 0.2 * T / k
if L <= 0 or (lo <= 1 and L < 2*T - min_opt_dashes*sw):
    too short: draw the section as a SOLID line (if it is the last section),
               otherwise hand the length to the next section
else:
    hi  = ceil(nf)
    dlo = (L + b - lo*T) / lo ;  dhi = (hi*T - (L + b)) / hi
    n   = (dlo > dhi) ? hi : lo                  // n >= 2
    D   = max(0, (L - (n-1)*b - n*(k-1)*g) * 2 / (2*n*k - H))
    pos = S
    for grp in 1..n:
        for dash in 1..k:
            first = grp==1 && dash==1 ;  last = grp==n && dash==k
            len = D/2 if exactly one of (first && half_first), (last && half_last) else D
            emit stroke sub-path from pos to pos+len   // follows the original geometry
            pos += len ; if dash < k: pos += g
        if grp < n: pos += b
```
- The break length is fixed; the **dash length is stretched/shrunk** so an open line starts and
  ends with a full dash (unless `half_outer_dashes`). Dashes run continuously over corners: a dash
  that spans a vertex is a polyline through that vertex, stroked with the symbol's join.
  OOM does not snap dashes to corners (only explicit dash points split the pattern).
- Closed parts: H = 2, the first and last dash are halves; `LineRenderable` appends the first
  sub-path to the last one (`connectPath`) so the two halves form one dash across the closing
  point, joined, without caps [RI `LineRenderable` `first_subpath`].
- Example 506 at 1:4000 (d 1.5, b 0.37): open line shorter than 2*1.87 - 2*0.374 = 2.992 mm is
  solid. Count: n = floor or ceil of (L + 0.37)/1.87, whichever deviation is smaller.
- Caps of each dash: the symbol's cap (506: flat).

### 2.3 Borders (505.1, 505.2) [LS `createBorderLines`, `shiftCoordinates`, `LineSymbolBorder`]
- `<borders>` sets `have_border_lines`. The first `<border>` is the **left** border; a second one
  (only with `borders_different`) the right border; with only one, `right_border = border`
  [LS `loadImpl`].
- Visible if `width > 0 && color && !(dashed && dash_length == 0)` [LS `isVisible`].
- Centre-line offset of a border = `line_width/2 + shift` (mm x0.001), to the left (negative
  perpRight) or right: `shiftCoordinates(path, ±0.0005*line_width, ±0.001*shift, …)`, total shift =
  main_shift + border_shift [LS `shiftCoordinatesLeft/Right`]. So `shift` is measured from the
  **edge** of the main line; 505.1: 175 + 50 = 225 µm, border spans 175..275 µm from the centre.
- Border stroke: width = border.width, colour = border.color, **cap always flat** (temporary
  `LineSymbol` default), join = Round if the main join is Round, otherwise **Miter**
  [LS `createBorderLines`, `LineSymbolBorder::setupSymbol`].
- Dashed border (`dashed && dash_length > 0 && break_length > 0`): the dash pattern is computed
  with the algorithm of 2.2 on the **main centre line** (k = 1, no half outer dashes, closed ->
  halves at the closing point), then the dashed coordinate list (including the undrawn gap
  stretches) is offset. Left and right borders reuse the identical pattern unless they differ
  [LS `createBorderLines`]. Effect: dashes sit exactly opposite each other; on curves the outer
  border dash is longer than the inner one.
- If the main line is dashed, borders are built from the processed (dashed) path; not the case for
  505.x (main line solid).
- Too short for dashes (505 at 1:4000: open L < 2*2.25 - 2*0.45 = 3.6 mm): solid border.

Offset algorithm per vertex (shift = signed total shift, t_in/t_out unit tangents from
`calculateIncomingTangent/OutgoingTangent`, which wrap around for closed parts) [LS `shiftCoordinates`]:
```
open start:  P = c + shift*perpRight(t_out)          open end: P = c + shift*perpRight(t_in)
corner:
  mid = normalize(t_in + t_out)
  outer = (t_out.x*t_in.y - t_in.x*t_out.y) * shift > 0
  if outer and join == Miter:
     if dot(mid, t_in) <= cos(atan(4/2)) = 0.4472    // deflection >= 126.87 deg
        mid1 = normalize(t_in + mid); off = 2*|main_shift| + tan(acos(dot(mid1,t_in)))*|border_shift|
        emit c + shift*perpRight(t_in) + off*t_in ;  P = c + shift*perpRight(t_out) - off*t_out
     else
        off = |shift / tan(acos(dot(perpRight(mid), t_in)))|  // = |shift|*tan(deflection/2)
        P = c + shift*perpRight(t_out) - off*t_out          // exact miter point
  if outer and join in {Bevel, Round}: two points with off = tan(phi1)*|border_shift| (+ middle point for Round)
  inner side (no Bézier on both sides):
     phi = acos(dot(perpRight(mid), t_in)); off = -|shift/tan(phi)|   // intersection of offset lines
     if tan(phi) >= 1 or |off| < min(len_in, len_out): P = c + shift*perpRight(t_out) - off*t_out
     elif len_in < len_out: P = c + shift*perpRight(t_out) + len_in*t_out
     else:                  P = c + shift*perpRight(t_in) - len_out*t_in
  inner side between two Béziers: P = c + shift*inset*perpRight(mid),
     inset = sin(phi) > 0.5 ? 1/sin(phi) : 2
```
`len_in/len_out` = distance to the neighbouring vertex. Bézier segments are offset with
`QBezier::shifted(…, |shift|, 0.03)` between the corner points (approximate offset curve); an
equivalent is to offset a flattened polyline (tolerance ≤ 0.005 mm). The resulting border
polyline is then stroked with miter joins (PDF limit √5).

### 2.4 Mid symbols on solid lines (201, 513.2, 518) [LS `createMidSymbolRenderables`]
The sections [A, B] below end at the dash points as in 2.2 (v0.9.6: the loop uses
`findNextDashPoint`); with start/end offsets (2.7) the line is S..E instead of the whole part. With
e = 0 every section end gets a symbol, so every dash point does.
Created only if `mid_symbol` is non-empty and `segment_length > 0` and the line is not dashed
[LS `createSinglePathRenderables`]. `mid_symbol_placement` is used only on dashed lines.
`minimum_mid_symbol_count` and `minimum_mid_symbol_count_when_closed` are loaded, saved and
scaled but **never used for rendering** (grep: only `loadImpl`, `saveImpl`, `scale`, `equals`).

With s = segment_length, e = end_length, m = mid_symbols_per_spot, q = mid_symbol_distance (mm),
msl = (m-1)*q:
```
if e == 0 and not closed: place(S)                         // symbol at the very start
for each section [A, B] (between dash points; normally the whole part):
  L   = B - A
  seg = max(0, L - 2e) - msl
  raw = max(e == 0 ? 1 : 0, seg / (s + msl)); lo = floor(raw); hi = ceil(raw)
  if e > 0:
     if L <= msl:
        if show_at_least_one: place(A); place(B)
     else:
        dlo = |L - lo*s - (lo+1)*msl - 2e| ; dhi = |L - hi*s - (hi+1)*msl - 2e|
        n   = (dlo >= dhi) ? hi : lo                       // tie -> hi
        dev = (dlo >= dhi) ? -dhi : dlo
        ideal = n*s + 2e
        e2 = e + dev*e/ideal ;  s2 = s + dev*s/ideal
        if s2 >= 0 and (show_at_least_one or hi > 0 or L > 2e - (s+msl)/2):
           for i in 0..=n: for j in 0..m-1: place(A + e2 + i*(s2+msl) + j*q)
  else:                                                    // e == 0
     if L > msl:
        dlo = |L - lo*s - (lo+1)*msl| / lo ; dhi = |L - hi*s - (hi+1)*msl| / hi
        n = (dlo > dhi) ? hi : lo ;  a = (L - (n+1)*msl)/n + msl
        if a >= msl: for i in 0..=n, j in 0..m-1: p = A + i*a + j*q;
                     skip (i==0 && j==0); stop at (i==n && j==m-1); place(p)
     place(B)
```
- For m = 1 (all listed symbols) this is: n+1 symbols, the first and last at e2 from the ends,
  equally spaced s2 in between, where `e2 = e*L/ideal`, `s2 = s*L/ideal`, `ideal = n*s + 2e`, and n
  is the integer that makes `n*s + 2e` closest to L (ties to the larger n).
- Short lines (n = 0): one symbol at L/2, drawn only if `show_at_least_one_symbol` or
  `L > 2e - s/2`. 201: always (2e - s/2 < 0). 518 at 1:4000: only if L > 2.775 mm, else no tick.
  513.2: always.
- Closed parts: no wrap-around. Positions still start at the first coordinate: symbols at e2 after
  and before the closing point (gap 2*e2 across it) [same function; `closed` only suppresses the
  start symbol for e == 0].
- `place(p)`: position = point at arc length p (7.3). Orientation = `tangentVector().angle()` if
  the mid symbol is rotatable, else 0 [LS]. `tangentVector()` = `normalize(next - pos) -
  normalize(prev - pos)`, i.e. the sum of the unit incoming and outgoing directions: on a segment
  the segment direction, **at a vertex the bisector**, at an open end the single available
  direction, at the closing point of a closed part the bisector across the closing point
  [PC `SplitPathCoord::tangentVector`]. Points closer than 0.025 mm are skipped when searching
  next/prev (`tangentEpsilonSquared = 0.000625`) [PCH].
- The orientation is passed **unchanged** as rotation r to the point symbol (no sign flip):
  local +x runs along the path direction, local **+y points to the right** of the path direction
  [PS `createRenderablesScaled`; MC]. Hence 201 ticks, 513.2 half discs and 518 slanted ticks all
  lie on the right-hand side; reversing the path flips them.

### 2.5 Mid symbols on dashed lines [LS `createDashGroups`, `processContinuousLine`]
Rules taken from OOM **v0.9.6**.

`mid_symbol_placement`: 0 CenterOfDash, 1 CenterOfDashGroup, 2 CenterOfGap, 99 NoMidSymbols
[LSH `MidSymbolPlacement`]. Only evaluated for `dashed="true"`. Mid symbols exist on a dashed line
if the mid symbol is not empty and `mid_symbols_per_spot > 0` and the placement is not 99;
`segment_length` plays no part [LS `createDashGroups`]. m = `mid_symbols_per_spot`,
q = `mid_symbol_distance` (mm). Per section (2.2), with D the stretched dash length:

```
set_mid = placement != None and (L >= d - sw or show_at_least_one)
CenterOfDash, before the dashes:
  if line_start < S or not is_part_start:           // the dash through a dash point
     set_mid = false
     pos = S - (m-1)q/2; for s in 0, 2, 4, … < m: if pos >= 0: place(pos); pos += q
  if half_first:
     pos = S + (m%2 + 1) q/2; for s in 1, 3, … < m while pos <= part length: place(pos); pos += q
too short: continuous(line_start, E, set_mid) if is_part_end
per group:
  CenterOfDashGroup:
     pos = (last group and half_last) ? E
         : (first group and half_first) ? cur : cur + (k·D + (k-1)g)/2
     pos -= q(m-1)/2
     for i in 0..m while pos <= E: if pos > S: place(pos); pos += q
  per dash: continuous(dash_start, dash_end, placement == CenterOfDash and the dash is not a half)
  after each group but the last, CenterOfGap:
     pos = cur + (b - q(m-1))/2
     for i in 0..m: skip pos < S; stop pos > E; place(pos); pos += q
after the groups, if half_last and CenterOfDash:
  pos = E - (m-1)q/2; for s in 0, 2, … < m: place(pos); pos += q
continuous(a, b, set_mid) [processContinuousLine]:
  stroke [a, b]; if set_mid and (m-1)q <= b - a:
     pos = (a + b - (m-1)q)/2; place(pos); m-1 times: pos += q; place(pos)
```
- So with m = 1: CenterOfDash puts one symbol in the middle of every full dash, one at the
  closing point of a closed line (the two half dashes there form one dash, which gets one
  symbol), and one at each dash point where two halves meet. CenterOfDashGroup with k = 1 gives
  the same positions; CenterOfGap one per main break.
- A line too short for dashes is drawn solid with one spot in its middle if `set_mid`, whatever
  the placement.
- Orientation as in 2.4 (tangent at the position, local +y right).
- The `s += 2` loops advance `pos` by q per iteration, not 2q (ported as is).
- freemapper-print bounds the steps (sections, dashes, spots and skipped positions, plus the
  tangent searches) by its per-object budget and by 100 000; past 100 000 steps the stretch is
  drawn solid without mid symbols.

### 2.6 Start and end symbols (541, 543) [LS `createStartEndSymbolRenderables`]
- Start symbol at the **first coordinate of the first part**, end symbol at the **last coordinate
  of the last part** (also for closed parts, then both at the closing point). Not affected by
  start/end offsets.
- Orientation (only if that point symbol is rotatable): start = angle of
  `calculateOutgoingTangent(first)` = `coords[k] - coords[first]` (first vertex farther than
  0.025 mm); end = angle of `calculateIncomingTangent(last)` = `coords[last] - coords[k]`. Both
  point in the direction of travel. Same rotation rule as mid symbols (local +y = right).
- 541: non-rotatable ring (2.4 does not apply) at both ends. 543: bar along local y -> a 0.8 mm bar
  perpendicular to the line at each end.

### 2.7 Start/end offsets, pointed caps, dash symbols
Rules taken from OOM **v0.9.6**.

Attributes [LS `loadImpl`]: `start_offset`, `end_offset` (µm, since file version 8). If neither is
present and `cap_style == 3` (PointedCap), both are `pointed_cap_length` (versions ≤ 7). The pointed
cap has no length of its own: it is the start offset at the start and the end offset at the end.
`<dash_symbol>` (a point symbol, dropped if empty: no elements, no visible dot or ring
[PS `isEmpty`]), `suppress_dash_symbol_at_ends` (default false), `scale_dash_symbol` (default
**true**: anything but `"false"`).

Offsets [LS `createSinglePathRenderables`]:
- Used only for **open** parts with `start_offset > 0 || end_offset > 0` (negative values count
  as 0).
- `so = 0.001·start_offset`, `eo = 0.001·end_offset`, L = part length. If `so + eo > L`: both are
  scaled by `L/(so + eo)` and **no main line is drawn**; the function then returns after the caps,
  so there are no borders and no mid symbols either. Also no main line, borders or mid symbols
  when the line has no colour or width and offsets apply (`create_line` false → return).
- S = so, E = max(S, L - eo). The main line, its dashes (2.2), mid symbols (2.4, 2.5) and borders
  (2.3) run on [S, E] only. Start and end symbols (2.6) and dash symbols stay at the part's ends.
- With cap 3: `createPointedLineCap` on [0, S] (start) and [E, L] (end) when that offset is > 0.

Pointed cap [LS `createPointedLineCap`; RI `LineRenderable`]:
- Stroke cap of cap style 3 is **flat**.
- A filled area in the line colour (own `AreaRenderable`; fill rule even-odd as all areas).
  For every point of the copied stretch [a, b] (start split, vertices in between, end split):
  `dist` = distance along the line from the tip (`len - a` at the start, `b - len` at the end),
  `factor = clamp(dist / cap_length, 0, 1)`, `cap_length = 0.001·start_offset` or `end_offset`
  (**unscaled**, so a scaled-down cap never reaches full width),
  `(dir, scaling) = calculateTangentScaling(i)` on the stretch (open, no wrap),
  `radius = clamp(half_width · factor · scaling, 0, 2·half_width)`,
  right point = p + radius·normalize(perpRight(dir)), left point = p - radius·….
- Overlap: if `b - a > 0.2 mm`, one extra point 0.05 mm beyond the wide end (along the tangent
  there: incoming tangent at the last point for a start cap, outgoing at the first point for an end
  cap, set to length 0.05) and 0.05 mm further **in** on each side.
- Polygon: start cap = right side from the tip to the wide end (plus overlap), then the whole left
  side back to the tip (the tip appears first and last). End cap = right side from the overlap and
  wide end to the tip, then the left side back **without** its tip.
- Béziers inside the cap get control points offset by `tan_angle = half_width/cap_length`; on a
  polyline flattened to 0.005 mm this is the same shape.

Dash symbols [LS `createDashSymbolRenderables`; VP `calculateTangentScaling`]:
- Drawn first for every part, from the original path, before offsets and dashes.
- At every coordinate with the DashPoint flag, index range: all; with
  `suppress_dash_symbol_at_ends` not at the first and last coordinate; else on a closed part not at
  the first coordinate (the closing point counts once).
- `calculateTangentScaling(i)`: incoming = `coords[i] - coords[k]` for the nearest earlier k farther
  than 0.025 mm (closed parts wrap from the end), outgoing likewise forward. No outgoing: direction
  = incoming, scaling 1. No incoming: outgoing, 1. Both: unit vectors; if outgoing = -incoming the
  direction is `perpRight(outgoing)` and scaling ∞, else direction = normalize(in + out) and
  scaling = 1/dot(direction, in) = 1/cos(half the turn).
- Rotation = angle of the direction if the dash symbol is rotatable, else 0 (local +y right).
- Scale = `min(scaling, 2·miterLimit()) = min(scaling, 2)` if `scale_dash_symbol`, else 1; it
  multiplies **element coordinates only**, not the dot radius or ring [PS
  `createRenderablesScaled`, `coord_scale`].
- freemapper-print takes the tangents from the flattened polyline; at a dash point on a Bézier
  the direction can differ slightly from OOM's control-point tangent.
- The searches for the nearest point farther than 0.025 mm (`calculateIncoming/OutgoingTangent`,
  `SplitPathCoord::tangentVector`) are linear scans in OOM, quadratic over many dash points or cap
  vertices in a cluster. freemapper-print gives the same answers but skips each run of identical
  points in one step, and counts every distance check against the object's budget (64 checks
  per unit), so a cluster of distinct points within 0.025 mm stops at the budget.

Borders of a non-trivial line [LS `createSinglePathRenderables`, `createBorderLines`]: when offsets
or dashes apply, the borders are built from the processed path (the stretches actually stroked).
A solid border on a dashed line is therefore dashed like it; a dashed border runs its own pattern
on that path, which freemapper-print computes on [S, E] and cuts to the main dashes.

---

## 3. Point symbols [PS `createRenderables`, `createRenderablesScaled`; RI Dot/CircleRenderable]

- Dot: if `inner_color && inner_radius > 0`: filled circle, radius `0.001*inner_radius`, colour
  `inner_color` [RI `DotRenderable`].
- Ring: if `outer_color && outer_width > 0`: **stroked** circle of radius
  `0.001*inner_radius + 0.0005*outer_width`, pen width `0.001*outer_width`, colour `outer_color`
  -> covers radii `inner_radius .. inner_radius + outer_width` [RI `CircleRenderable`]. The ring is
  drawn even when there is no inner colour (321, 541, 544, 545); the centre then stays open.
- Dot and ring are independent renderables with their own colour priority (418: white dot prio 21,
  green ring prio 3).
- Elements: each `<element>` holds a nested `<symbol>` (point, line or area) and an `<object>`
  (`type` 0 point, 1 path) with coordinates relative to the point centre in µm, **y down**
  [PS `loadImpl`]. Every element coordinate is transformed by
  `x = ox*cos r - oy*sin r + X`, `y = oy*cos r + ox*sin r + Y` (also Bézier control points), then
  the element symbol renders the transformed coordinates exactly like a map object (lines with their
  own width/cap/join/close flags, areas filled) [PS `createRenderablesScaled`]. Element colours are
  sorted into the global priority order like everything else.
- Element point objects (321's inner dot) use their own object's rotation, which is 0; a parent's
  rotation only moves them, it is not passed on [PS `createRenderablesScaled` comment].
- Rotation of a map point object: `r = rotatable ? -object.rotation : 0` [PS `createRenderables`].
  `rotation` is in **radians**, read from the `<object rotation="…">` attribute **only if the
  symbol is rotatable** (otherwise ignored on load) [OBJ `Object::load`]. With y down and
  r = -rotation, a positive object rotation turns the symbol **counter-clockwise** on paper.
  Rotatable among the listed: 521.1 (and the line sub-symbols' mid/start/end symbols).
- `<pattern rotation="…"><coord x y/></pattern>` inside a path `<object>` is the PathObject's
  fill-pattern rotation (stored as the object's rotation) and pattern origin [OBJ `Object::load`,
  `PathObject::getPatternRotation/Origin`]. It only affects area symbols with **rotatable fill
  patterns** [AS `FillPattern::createRenderables`]; see §4.2.
- `elements="N"` is only a reservation hint; the actual `<element>` children count [PS `loadImpl`].

---

## 4. Area symbols [AS `createRenderablesNormal`; RI `AreaRenderable`]

- Fill colour `inner_color`; one `AreaRenderable` per object containing **all parts** as
  subpaths, each subpath closed with `closeSubpath()` even without a close flag [RI
  `AreaRenderable::addSubpath`]. Requires the first part to have > 2 path coordinates; area objects
  need >= 3 coordinates [AS `createRenderables`, RI].
- Fill rule: QPainterPath default **OddEvenFill** (not changed for areas) -> PDF `f*` [RI; PDF
  `generatePath`]. Holes therefore work regardless of ring orientation. (Text uses WindingFill.)
- `min_area` is not used for rendering, only for the "area too small" check [OBJ
  `PathObject::isAreaTooSmall`].
- `rotatable` on the area symbol: whether the object rotation is loaded; patterns have their own
  `rotatable` flag [AS `loadImpl`, `FillPattern::load`].
- The area renderable is created even without a fill colour (`inner_color="-1"`): its path is
  the clip path of the patterns [AS `createRenderablesNormal`]. Patterns are drawn in file order
  after the fill, each with its own colour priority.

### 4.1 Pattern attributes [AS `FillPattern::load`, `FillPattern::FillPattern`]

Rules in §4.1–4.5 were taken from OOM **v0.9.6** (`src/core/symbols/area_symbol.cpp`,
`point_symbol.cpp`, `objects/object.cpp`).

- `<pattern>` children of `<area_symbol>`, any number (`patterns="N"` is a hint).
- `type`: 1 = line pattern, 2 = point pattern.
- `angle` (radians, read as `float`), `rotatable="true"` (flag `Rotatable`), `no_clipping` (int,
  masked with 0x03: 0 clip, 1 no clipping if completely inside, 2 if centre inside, 3 if
  partially inside), `line_spacing`, `line_offset`, `offset_along_line` (all µm).
- Line pattern: `color` (index, −1 none), `line_width` (µm).
- Point pattern: `point_distance` (µm) and a nested `<symbol>` (a point symbol). Without it the
  pattern draws nothing [AS `FillPattern::createRenderables`: `if (point && point_distance > 0)`].
- Missing attributes read as 0 (the constructor defaults 5 mm / 0.2 mm are overwritten by
  `load`).
- `scale(factor)` scales `line_spacing`, `line_width`, `line_offset`, `offset_along_line`,
  `point_distance` and the nested point symbol [AS `FillPattern::scale`].

### 4.2 Rotation and origin [AS `FillPattern::createRenderables(outline, delta_rotation, pattern_origin, output)`, `createRenderablesNormal`; OBJ `Object::load`]

- `line_spacing <= 0`: no pattern.
- `delta_rotation` = the object's pattern rotation (`<pattern rotation>` of the object, default
  0); set to 0 when the pattern is not `rotatable`.
- `rotation = fmod(angle + delta_rotation, π)`, plus π if negative: in [0, π).
- Pattern origin = the object's `<pattern><coord/>` (default (0, 0), the map origin). It counts
  only for a `rotatable` pattern: with the normal n = (sin r, cos r) and tangent
  t = (cos r, −sin r) (paper, y down; OOM: `(0,−1)` resp. `(1,0)` rotated by r, then y negated),
  `delta_line_offset = n · origin`, `delta_along_line_offset = t · origin`, both 0 when not
  rotatable [AS `createRenderables<T>`].

### 4.3 Pattern lines [AS `createRenderables<T>`]

- Lines are `n · p = offset + k · line_spacing` for integer k, with
  `offset = line_offset + delta_line_offset`: at angle r **counter-clockwise on paper** from
  east, through the origin when `line_offset = 0`.
- Canvas = extent of the area outline (flattened path) grown by the extent of what one line or
  point paints: `canvas.adjust(-e.right, -e.bottom, -e.left, -e.top)` with e = half the line
  width on each side for lines, the extent of the point symbol object at (0, 0) with rotation
  `delta_rotation` for point patterns (`PointObject::setRotation`, so −delta if the point symbol is
  rotatable, else 0) [AS `FillPattern::createRenderables`].
- Lines run across the canvas from edge to edge; the first is the smallest k with the line at or
  past the canvas corner of minimal n · p. Cases (first → second point):
  - |r − π/2| < 0.0001, vertical: x = offset + ⌈(left − offset)/s⌉·s, step s, while x < right;
    from (x, top) to (x, bottom); `delta_along_line_offset` negated.
  - |r| < 0.0001, horizontal: y from ⌈(top − offset)/s⌉, while y < bottom; from (left, y) to
    (right, y).
  - 0 < r < π/2: from the top edge (or the right edge once past it) to the left edge (or the
    bottom edge), first line through the upper left corner region; stops when the start point
    passes below the bottom; `delta_along_line_offset` negated.
  - π/2 < r < π: from the bottom edge (or the right edge) to the left edge (or the top edge),
    first line at the lower left; stops when the start point passes above the top.
  The negations make `delta_along_line_offset = dir · origin` for the line direction dir in
  every case, so point positions satisfy `dir · (p − origin) ≡ offset_along_line` (mod
  `point_distance`).
- OOM computes the first line's start and end with two separate `ceil` calls,
  `⌈(left − offset_x)/dist_x⌉` and `⌈(top − offset_y)/dist_y⌉` (`bottom` for r > π/2), which are
  equal in exact arithmetic. When the canvas corner lies on a pattern line, rounding can make
  them k and k + 1 and tilt every line by one spacing. freemapper-print takes k from the start
  formula for both ends, and computes line k and point j from their index instead of adding
  `dist_x`, `dist_y` or `point_distance` up step by step.
- Line pattern: each line is a `LineRenderable` of two points in `color`, width `line_width`,
  flat caps [AS `createLine<LinePattern>`].
- Example (pinned in `draw.rs` tests, code in `pattern.rs`): 10 × 10 mm square, horizontal lines, spacing 1 mm, width
  0.1 mm: canvas y ∈ [−0.05, 10.05], lines y = 0, 1, …, 10 = **11** lines.

### 4.4 Point positions on a line [AS `createPointPatternLine`]

- `dir` = unit vector first → second, `length` = |second − first|,
  `offset = dir · first − offset_along_line − delta_along_line_offset`,
  `start = ⌈offset / point_distance⌉ · point_distance − offset`; points at
  `first + dir · cur` for `cur = start, start + point_distance, …` while `cur < length`.
- Each point symbol is drawn with rotation **−delta_rotation** (the object's pattern rotation if
  the pattern is rotatable, not the pattern angle), via `createRenderablesScaled`, which applies
  the rotation whether or not the point symbol is rotatable [AS `createLine<PointPattern>`
  passes `delta_rotation`; PS `createRenderablesScaled`].
- Example (pinned in `draw.rs` tests): 10 × 10 mm square, horizontal point pattern, spacing =
  distance = 2 mm, a dot of radius 0.1 mm: canvas [−0.1, 10.1]², lines y = 0, 2, …, 10 (10 <
  10.1) and on each points x = 0, 2, …, 10 (cur = 0.1 … 10.1 < 10.2) = **36** placements.
- A point symbol that draws nothing has an empty extent (`PointObject::getExtent` of no
  renderables). OOM still walks its placements, but every placement draws nothing, so the pattern
  is invisible [AS `FillPattern::createRenderables`, `createPointPatternLine`]. freemapper-print
  skips the placements; the output is the same. Every point symbol that draws anything has a
  non-zero extent (dots and rings have a radius, strokes a width), so no visible case differs.

### 4.5 Clipping [AS `FillPattern::createRenderables`, `createPointPatternLine`; RN `PainterConfig::activate`; PS `createRenderablesIf*Inside`]

- Default (`no_clipping` 0): the outline's painter path (all parts, OddEvenFill) is the clip
  path of every pattern renderable; OOM intersects it with any clip already active (the print
  area) [RN `PainterConfig::activate`]. PDF: `q`, the rings, `W* n`, the shapes, `Q`.
- `no_clipping` 1–3 on a point pattern: no clip path; each part of the point symbol is tested
  against the outline (`QPainterPath::contains`, even-odd) and drawn unclipped or left out
  [PS `createRenderablesIfCompletelyInside`, `…IfCenterInside`, `…IfPartiallyInside`,
  `createPrimitivesIfCompletelyInside`, `createPrimitivesIfPartiallyInside`]:
  - 1 `NoClippingIfCompletelyInside` (`Clipping::NoClippingIfCompletelyInside`): dot if all four
    points (x ± r_in, y) and (x, y ± r_in) are inside; ring likewise with r = r_in + w/2; a point
    element only gets its own dot and ring, tested the same way at its transformed coordinate
    (no nested elements); any other element is drawn if all its transformed coordinates are
    inside.
  - 2 `NoClippingIfCenterInside` (`Clipping::NoClippingIfCentreInside`): dot and ring if the
    point's centre is inside; each element (point elements included, drawn in full) if the mean of
    its transformed coordinates is inside.
  - 3 `NoClippingIfPartiallyInside` (`Clipping::NoClippingIfPartlyInside`): as 1 with "any of
    the four points" and "any coordinate".
- `no_clipping` on a line pattern: no clip path, lines span the whole canvas
  (`createLine<LinePattern>` ignores the flag).

### 4.6 freemapper-print limits (not in OOM)

- Patterns with a non-finite `angle`, `line_spacing`, `line_offset`, `line_width`,
  `offset_along_line` or `point_distance`, or with `line_spacing` (or, for point patterns,
  `point_distance`) below 50 µm, are dropped when the symbols are read
  (`symbols::MIN_PATTERN_STEP`). OOM only skips values ≤ 0. A pattern whose combined rotation is
  not finite (broken object rotation) draws nothing.
- One budget of 250 000 units per map object (`draw::MAX_PRIMITIVES_PER_OBJECT`), shared by every
  nested symbol, pattern line, point placement and point symbol: each primitive costs one unit,
  each step that draws nothing (a line without points, a placement left out, a symbol call
  without output) costs one too. Copying an element's coordinates for a placement and testing a
  point against the outline in the `no_clipping` modes cost one unit per 64 points
  (`draw::POINTS_PER_UNIT`, rounded down). The extent probe of a point pattern draws into the
  same budget. When the budget is spent drawing stops and `primitives_counted` reports the object
  as cut.
- Dashes (§2.2) and mid symbols (§2.4) of a part are computed only up to the units left. A part
  that needs more than 100 000 of them (`geometry::MAX_REPEATS`) is drawn solid or without mid
  symbols. Both cases also report the object as cut. A main line or border without colour or
  width is skipped before its dashes are computed (it draws nothing in OOM either).
- All pattern shapes of one area hold the same outline (`Arc`); the PDF writer opens one clip
  block per run of shapes with the same outline.

---

## 5. Combined symbols (501, 501.10, 521) [CS]

- `<part symbol="N"/>` references the **symbol at index N of the map's symbol list**
  (`map->getSymbol(index)` in `loadingFinishedEvent`), not the `id` attribute. In ISSprOM ids equal
  indices (verified 0..184), and freemapper appends additions with consecutive ids, so both agree.
- `<part private="true"><symbol …/></part>`: an inline symbol owned by the combination; scaled with
  it [CS `loadImpl`, `scale`].
- Rendering: every part's `createRenderables` gets **the same path parts** of the object, in part
  order [CS `createRenderables`]. The area part fills all rings (even-odd); the line part strokes
  each ring as its own closed line (outer boundary and holes alike), centred on the outline.
- The part order is irrelevant for the result: visibility is decided by colour priority (1.2).
  501/501.10: area 14/13 above line 15 -> only the outer half of the edge line is visible
  (ISSprOM: 100 µm of 200; school: 438 of 876). 521: line 2 above fill 9 -> the full 0.14 mm
  outline is visible, half of it inside the building.

---

## 6. Text symbols (991–993) [TS, TSH, TO, RI `TextRenderable`]

- `<font family size bold italic underline>`, `<text color line_spacing paragraph_spacing
  character_spacing kerning>`, optional `<framing>`, `<line_below>`, `<tabs>` [TS `loadImpl`].
- **Font size = em size** in µm. OOM sets `QFont::setPixelSize(256)` (`internal_point_size`) and
  scales the glyph outlines by `font_size_mm / 256` [TS `updateQFont`; TSH `internal_point_size`;
  RI `TextRenderable` `scale_factor`]. Qt's pixel size is the em height, so in PDF use
  `Tf = font_size_mm * 72/25.4` pt. Cap height is font dependent (Arial ≈ 0.716 em).
- bold/italic select the font style; hinting off, kerning per `kerning`, extra letter spacing
  `character_spacing * advance(" ")` after every glyph (`QFont::AbsoluteSpacing`) [TS
  `updateQFont`].
- Rotation: `rotatable` attribute; if absent, rotatable only for file version < 9 [TS `loadImpl`].
  991–993 have no attribute in a v9 file -> **not rotatable, object rotation ignored** [OBJ
  `Object::load`]. If rotatable: rotate by -rotation (CCW on paper for positive values) about the
  anchor [RI `TextRenderable`, TO `calcTextToMapTransform`]. `TextRenderable` builds
  `translate(anchor) · scale · rotate(-degrees(rotation))` (Qt, y down), so a glyph origin at text
  coordinates (x, y) lands at `anchor + (x cos r + y sin r, -x sin r + y cos r)`. In PDF (y up)
  the text matrix is `[cos r, sin r, -sin r, cos r, x, y]`. freemapper-print takes the version from
  `<map version="…">` (9 if absent): without the attribute a text symbol is rotatable in files
  before version 9 and not rotatable from version 9 on.
- Object: `<object type="4" h_align v_align>` (missing -> 0), anchor = first coordinate; a second
  coordinate or `<size>` makes it a box text (word wrap) [OBJ `Object::load`].
  `h_align`: 0 Left, 1 HCenter, 2 Right; `v_align`: 0 Baseline, 1 Top, 2 VCenter, 3 Bottom [TOH].

Layout of a single-anchor text (`box_width = box_height = 0`), in text units (em = font size)
[TO `prepareLineInfos`]. A = font ascent, LS = `line_spacing * QFontMetricsF::lineSpacing()`
(= line_spacing x (ascent + descent + leading)), PS = paragraph_spacing (mm) converted to text
units. Each `\n` ends a line and (without wrapping) a paragraph; `\t` starts a tab part.
```
y0 = (v_align == Baseline) ? 0 : A           // first baseline, y down
line k baseline: y_k = y0 + k*(LS + PS)
w_k = advance width of line k (horizontalAdvance incl. kerning and letter spacing)
x_k = 0 (Left) | -w_k/2 (HCenter) | -w_k (Right)
if v_align in {VCenter, Bottom}:
   height = A + (n-1)*LS + (P-1)*PS          // n lines, P paragraphs (P = n here)
   delta  = (VCenter) ? -height/2 : -height
   y_k += delta
map position of glyph origin = anchor + R(-rotation) * (x_k, y_k)   [mm, y down]
```
- So: Top -> first baseline A below the anchor; VCenter (single line) -> baseline A/2 below the
  anchor (descent ignored); **Bottom (single line) -> baseline on the anchor** (last baseline on
  the anchor for several lines); Baseline -> first baseline on the anchor.
- Metrics come from Qt's font engine (hhea ascender/descender/lineGap on macOS/FreeType). Arial and
  its metric twin Liberation Sans: ascent 1854/2048 = 0.905, descent 434/2048 = 0.212,
  lineGap 67/2048 = 0.033 em -> lineSpacing ≈ 1.149 em. Take them from the embedded font's hhea
  table to match.
- Colour: `<text color>` fill (BrushOnly) with **non-zero winding** (`path.setFillRule(WindingFill)`)
  [RI `TextRenderable`]. OOM writes glyph outlines as filled paths (`ForceOutline`), not PDF text;
  real PDF text with the same font, size and positions is visually equivalent.
- Framing (rules taken from OOM **v0.9.6**) [TS `loadImpl`, `createRenderables`; RI
  `TextFramingRenderable`]: `<framing color mode line_half_width shadow_x_offset shadow_y_offset>`;
  `mode` 0 NoFraming, 1 LineFraming, 2 ShadowFraming [TSH `FramingMode`]; a missing attribute reads
  as 0. Drawn only with a framing colour (`color` -1 means none).
  - Line framing (if `line_half_width > 0`): the glyph outlines **stroked only** (PenOnly), width
    `2·0.001·line_half_width` mm, miter join with Qt limit 0.5 -> PDF √2, colour = framing
    colour.
  - Shadow framing: the same text filled in the framing colour at
    `anchor + 0.001·(shadow_x_offset, shadow_y_offset)` (offset in paper coordinates, not turned
    with the text; the text then turns about the shifted anchor).
  - Each is its own renderable with the framing colour's priority, so the colour priority decides
    whether it lies below the text. OOM inserts it after the text; freemapper-print emits it first,
    which only matters when both colours are equal and then looks the same.
  - Even without a text colour the framing is drawn.
  - freemapper-print (no glyph outlines yet) writes line framing as PDF text with render mode 1
    (stroke) in its own `q … Q`.
- `<line_below>` (a line under each paragraph) is not supported yet.

---

## 7. Path coordinates

### 7.1 Flags [MC `MapCoord::Flag`]
Compact XML: `x y [flags];` per coordinate (µm), flags bitwise:

| Bit | Value | Meaning |
|---|---|---|
| CurveStart | 1 | this point starts a cubic Bézier: the next two coords are control points, the third is the end point (which may itself carry CurveStart) |
| ClosePoint | 2 | on the last coordinate of a part: the part is closed (its coordinate must equal the first) |
| GapPoint | 4 | legacy / internal (end of a dash in processed paths) |
| HolePoint | 16 | last coordinate of a part; the next coordinate starts a new part (hole for areas) |
| DashPoint | 32 | splits dash and mid-symbol patterns |

`18 = HolePoint | ClosePoint` ends every closed ring (freemapper `closed_ring`). A HolePoint on the
last coordinate of the object is harmless.

### 7.2 Parts [OBJ `PathObject::recalculateParts`, `PathPart::calculatePathParts`; VP `PathCoordVector::update`]
- Parts end at coordinates with HolePoint (control points of a curve are skipped while
  searching); the remainder after the last hole point is the final part.
- `part.isClosed()` = ClosePoint on the part's last coordinate [VP `VirtualPath::isClosed`].
- Area: all parts in one even-odd fill (4). Line: each part separately; closed parts get joins at
  the closing point and no caps; start symbol only on the first part, end symbol only on the last.
- An area ring written without ClosePoint is still filled closed, but a line or combined line
  part drawn on it is **open** (caps, no join at the start). Always write flag 18.

### 7.3 Arc length and positions on curves [VP `curveToPathCoord`; PC `SplitPathCoord::at`]
- Béziers are flattened recursively (de Casteljau halving) until the chord is ≤ 1 mm
  (`bezier_segment_maxlen_squared = 1.0`) and control-polygon length minus chord ≤ 0.005 mm
  (`bezier_error`). Cumulative length `clen` is the sum of flattened chords.
- A position at length p is found on the flattened polyline; inside a curve OOM interpolates the
  Bézier parameter linearly between the flattened samples and evaluates the exact Bézier at that
  parameter (`splitBezierCurve`). Interpolating on a polyline flattened to ≤ 0.005 mm matches to
  within that tolerance.
- Tangents: see 2.4 (`tangentVector`) and 2.3/2.6 (`calculateIncoming/OutgoingTangent`, vertex
  differences, skipping points closer than 0.025 mm, wrapping for closed parts) [VP].

---

## 8. Behaviour a naive renderer gets wrong

1. **Draw order is per colour priority over the whole map** (highest number first, 0 on top), not
   per object, symbol or combined part (1.2). This is what makes 501 edges half-covered and 505
   junctions clean.
2. **501 / 501.10 edge lines**: priority 15 under fill 14/13; only the outer half shows. The school
   set doubles the private line width (876) to show 438 (symbol_set.rs).
3. **Combined symbols** pass the same parts to every sub-symbol; the line part strokes every ring
   including holes, centred on the outline (5).
4. **Dashes are stretched** to fit whole dashes at both ends; short lines become solid; patterns
   continue around corners; closed lines put half dashes at the closing point that join (2.2).
5. **Dashed borders** take their pattern from the centre line and are identical on both sides;
   border caps are flat; border offset is `line_width/2 + shift` (2.3).
6. **Mid symbols**: count chosen by least deviation, end and segment lengths both scaled by
   L/ideal; `show_at_least_one_symbol` missing means false (201, 518); 518 on short lines can have
   no tick; closed rings do not wrap (2.4).
7. **Orientation**: mid/start/end symbols use +angle with local +y on the right side; at vertices
   mid symbols use the corner bisector (2.4, 2.6). Map point objects use -rotation (3).
8. **Rotation attributes are ignored** for non-rotatable point symbols and for text symbols without
   `rotatable="true"` in v9 files (3, 6).
9. **Rings** are stroked circles at `inner_radius + outer_width/2` and exist without a dot (3).
10. **Unused for rendering**: `minimum_length`, `min_area`, `minimum_mid_symbol_count(_when_closed)`,
    `mid_symbol_placement` on solid lines, `<pattern>` in element objects (2.1, 2.4, 3, 4).
11. **Miter limit**: PDF `M` = √5 ≈ 2.236, not the PDF default 10 and not 1 (2.1).
12. **Fill rules**: areas even-odd, text non-zero (4, 6). Do not merge different objects' fills.
13. **Text font size is the em size**; VCenter centres the ascent only; Bottom on one line puts the
    baseline on the anchor (6).
14. **Combined part references are symbol indices** (5); keep ids = indices when adding symbols.
15. **enlarge.rs**: `segment_length`, `end_length`, `mid_symbol_distance`, dash/break lengths,
    start/end offsets and the old `pointed_cap_length`, text framing widths and offsets,
    border width/shift/dash/break, point radii/widths, element coordinates and pattern lengths are
    lengths and scale linearly; `min_area` quadratically; counts (`mid_symbols_per_spot`,
    `dashes_in_group`, `elements`) stay; OOM's `LineSymbol::scale` also multiplies the two
    `minimum_mid_symbol_count*` fields, which is harmless because they are unused. Pattern origin
    coordinates and `pattern rotation` are not lengths to scale (and 0 here). Qt 5 `qRound` rounds
    negative halves up (-937.5 -> -937) while Rust `round()` gives -938: a 1 µm difference,
    visually irrelevant.
16. **Areas need >= 3 coordinates; lines >= 2** per part; zero-length parts produce nothing
    [AS `createRenderables`; LS `createSinglePathRenderables`].
17. **Dash points split patterns**: dashes and mid symbols restart at every dash point, and the
    dashes on either side join into one dash through it (2.2, 2.4, 2.5). A renderer that ignores
    dash points draws wrong tick spacing on fences and fence-like lines.
18. **Pointed caps are offsets**: the cap length is `start_offset` / `end_offset`, the stroke cap is
    flat, and a cap of a line shorter than both offsets never reaches full width (2.7).
19. **Offsets that exceed the line** suppress the main line, its borders and its mid symbols, not
    only the stroke (2.7).
20. **Dash symbols scale only their elements**, by 1/cos(half the turn), at most 2 (2.7).
21. **Text framing has its own colour priority**; shadow offsets are not rotated (6).

---

## Rendering recipe

1. Load colours in file order; priority = index; CMYK = `c m y k` attributes; opacity 1.
2. Resolve symbols; combined `part symbol=N` -> symbol index N; private parts inline.
3. For every object, generate primitives tagged with a colour priority:
   - stroke(path, width, cap, join, closed) | fill(paths, rule) | dot | ring.
4. Path objects: split into parts at HolePoint; closed = ClosePoint on the part's last coord;
   flatten Béziers (≤ 1 mm chords, ≤ 0.005 mm error) for lengths and positions, keep the
   original Béziers for the strokes/fills.
5. Area symbol: one even-odd fill of all parts in `inner_color`, then each fill pattern (§4.2–4.5)
   clipped to the same parts.
6. Line symbol, per part:
   1. start symbol at the first coord of the first part, end symbol at the last coord of the last
      part (oriented by outgoing/incoming tangent if rotatable);
   2. solid: stroke the part (cap/join from the symbol, PDF miter limit √5, `h` if closed);
      dashed: run the dash algorithm (2.2) and stroke each dash sub-path;
   3. mid symbols (solid lines with `segment_length > 0`): algorithm 2.4, orientation = angle of
      the bisector tangent, local +y to the right;
   4. borders: offset centre line by ±(`line_width/2 + shift`) with the miter rules of 2.3; if
      dashed, dash the centre line first (k = 1) and offset the dashes; flat caps, miter join.
7. Point symbol at (X, Y): r = rotatable ? -rotation : 0 for map objects, r = tangent angle for
   mid/start/end symbols; dot (fill r_in), ring (stroke radius r_in + w/2, width w), then each
   element transformed by (x cos r - y sin r + X, y cos r + x sin r + Y) and rendered by its own
   symbol (recursively 5–7).
8. Combined symbol: run every part (5–7) on the same parts.
9. Text: em size = font_size mm; layout per section 6 (alignment, ascent, line spacing);
   ignore rotation unless the text symbol is rotatable; fill colour, non-zero winding.
10. Sort all primitives by colour priority descending (31 … 0) and paint in that order, opaque,
    DeviceCMYK; within one priority any order; never merge fills of different objects.
11. Convert mm (y down) to PDF points (y up): `pt = mm * 72/25.4`, flip y; line widths likewise.
12. Check against OOM: export the same .omap from OOM with "Device CMYK" vector PDF and compare
    501 edges, 505 junctions/dashes, 506 dash counts, 201/518 tick counts and sides, 541/543 ends,
    text baselines.
