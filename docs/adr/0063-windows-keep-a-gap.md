# 0063. Windows keep a gap

Date: 2026-11-12
Status: Accepted, amends [0047](0047-generated-masks.md) and, for generated windows, [0045](0045-mask-variants.md)

## Context

`masks window` put a circle of the same radius around each control, and two controls close together gave one merged
window (ADR 0045: holes may overlap; ADR 0047: holes may overlap and unite). The runner then sees two controls in
one patch of map. The user decided on 2026-11-12: "beim Fenster-OL sollen die Fenster sich nicht überlappen und einen
Mindestabstand haben" (in window orienteering the windows must not overlap and must keep a minimum distance).

## Decision

- Windows from `masks window` never overlap. At least the window gap `gap_mm` stays between the edges of two
  windows, in print mm like the radius: default 3 mm (12 m of ground at 1:4000), range 1 to 100 with the error of a
  radius. CLI `--gap`, MCP `masks_window` `gap`, a field in the editor's window form.
- Two windows with centres `d` print mm apart (map mm over map mm per print mm) keep `d >= r_i + r_j + gap`. A pair
  that breaks it shrinks to `r_i + r_j = d - gap`, in proportion to the radii asked for. A radius from
  `--radius-for` is a request and shrinks the same way; the start and the finish count like any window.
- Pairs go in order of least room, `(d - gap) / (requested_i + requested_j)`; equal room keeps the course order. A
  radius never grows: when one window of a pair is already smaller from a tighter pair, the other takes the rest of
  the room. One round over the pairs is enough, since a pair that keeps the gap keeps it while radii shrink; the code
  stops after 8 rounds and checks again. Reduced radii are rounded down to 0.001 mm, so a gap is at most 0.001 mm
  wider than asked.
- No window shrinks below `MIN_WINDOW_RADIUS_MM`, 5 mm, so that it still holds the control circle. A window asked
  smaller keeps its radius and does not shrink. A pair that does not fit at its floors fails the build
  (`BuildError::WindowsTooClose`, exit 2, nothing written). The error names the pair with the least room by
  `StopName`, in course order, with its distance in print mm and, if it is at least 1 mm, the largest gap (to 0.1 mm)
  at which every pair fits. It says what helps: leave one out (`--controls`, `--no-start`, `--no-finish`) or a
  smaller `--gap`.
- `Window` has `requested_mm` next to `radius_mm`. `WindowResult` has `gap_mm` and `reduced` (stop, `requested_mm`,
  `radius_mm`). `own_radius` keeps its meaning: the windows asked for with a radius other than the default, with the
  radius asked for. Whether such a window shrank shows in `reduced`. The CLI, the MCP messages and the comment above
  the table name the gap and print one line per reduced window. A window that shrank gets that line and not the line
  about its own radius.
- Hand-written `[[masks]]` stay as ADR 0045 says: their holes may overlap and unite. Masks written before this
  decision are not changed.

## Consequences

- With the defaults the Viktoriapark training gets 10 of its 13 windows smaller; 45 and 58 lie 16.75 mm apart and
  get 6.88 mm each. The committed examples luebeck-training-window and mcp-session keep the masks they were made
  with, in which 50 and the finish (Lübeck) overlap. Their READMEs say what the command prints today.
- The fixture `luebeck-numbers.xml` has 31 and 32 5.8 mm apart. CLI tests that are not about the gap give 31 alone a
  window.
- Cost if wrong: windows near close controls smaller than the course setter wants. `--gap 1` gives back most of the
  room, and a hand-written mask gives back the overlap.

Source: the user's requirement of 2026-11-12; branch `window-gap`.
