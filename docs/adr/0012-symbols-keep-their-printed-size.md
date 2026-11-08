# 0012. Symbols keep the printed size the standard sets

Date: 2026-11-08
Status: Accepted

## Context

Layouts for different age classes print the same map at different scales. Scaling the whole map like a photo
makes symbols too small or too large. The user named wrong scale and illegible symbols as the two failures
that would make the program useless.

## Decision

- The map geometry scales by map scale divided by print scale. One millimetre at print scale is one PDF
  millimetre, and the page size equals the paper size.
- Symbol dimensions get a symbol factor that depends on the map kind and the print scale, from a table kept as
  data (design §5.2). Sprint maps keep their sizes from 1:3000 to 1:5000; ISOM maps printed larger than
  1:15000 get 150 %.
- The map kind is a project setting that `init` proposes and the user confirms. A print scale outside the
  table is refused.
- Every sheet carries a scale check bar by default.

## Consequences

- One map file serves every layout of an event.
- Cost if wrong: a wrong table row prints wrong symbol sizes. Each row is checked against the standard's text.

Source: brainstorming 2026-11-08 (user: anti-goals "Maßstab im Druck stimmt nicht", "Symbole unleserlich").
