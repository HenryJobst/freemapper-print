# 0044. The base area of sprint maps

Date: 2026-11-09
Status: Accepted

## Context

freemapper ADR 0069 puts a base area under every sprint map: one area of symbol 990 in the colour "Lower brown 30%
for walkable area", appended at colour priority 32 behind all others. It is the map area plus a margin, alone in
the map part `base`, tagged `freemapper:source=base`. Schoolyard maps and freemapper v0.7.0 maps have none.

## Decision

- The base area is map content and always prints, clipped to the map frame and painted first by its priority. No
  switch: `load` refuses `base` in `hide_parts` (file:line:column, DE and EN); a layout built in code keeps it.
- Primitives of tagged objects carry `Primitive::base`, and `painted_bounds` skips them: the page centre, `auto`
  orientation, the overflow, the frame for generated north lines (both passes) and the footer's overlap note come
  from the map alone. Counted, the margin would report an overflow on every sprint sheet. Number placement skips
  the base area as it skips north lines; a number over it stands on ground, as on paper.
- The colour is an ordinary colour. Its name is not canonical and its CMYK is far from black, brown and blue, so
  rule R19 (ADR 0034) keeps the lower purple where it was, over the base area. Masks (M4) will cover it too. The
  tag alone marks the base area; the part name matters only for `hide_parts`.

## Consequences

- Maps without the tag print byte for byte as before; the PDFs of all four examples are unchanged. Sprint maps
  should be printed in colour: in greyscale the 30 % tint looks like paper and like paved areas.
- Cost if wrong: another object with the tag drops out of centring and overflow, though it still prints.

Source: user decision 2026-11-09; freemapper ADR 0069.
