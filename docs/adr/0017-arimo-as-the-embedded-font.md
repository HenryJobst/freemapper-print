# 0017. Arimo as the embedded font

Date: 2026-11-08
Status: Accepted

## Context

OOM symbol sets ask for Arial. The renderer used the PDF standard fonts Helvetica and Helvetica-Bold with
WinAnsi encoding, so Czech or Polish letters such as ř printed as "?", italic text came out upright, and each
PDF viewer chose its own Helvetica. Arial itself may not be shipped with the program.

## Decision

Arimo replaces Arial and every other font family of a map. Arimo has the same advance widths as Arial, and its
licence is the SIL Open Font License 1.1, which allows bundling it with GPL software. The four static files
(regular, bold, italic, bold italic) are built into the program. Each PDF page embeds a subset of the faces its
texts use, with a ToUnicode map so the text can be searched and copied. A character Arimo lacks prints as the
empty box glyph, and the page reports it.

## Consequences

- Text widths match OOM for maps set in Arial.
- The program grows by about 1.9 MB. A page with one short text stays near 6 kB.
- The licence file must ship with every binary release.
- Cost if wrong: maps that use condensed or decorative fonts look different, and text set in a narrower font
  runs longer. `fonts::substituted` lets the CLI name each replaced family.

Source: plan M1 Task 6, controller rulings 2026-11-08.
