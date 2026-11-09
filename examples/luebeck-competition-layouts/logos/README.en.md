# Logos for the example

[Deutsch](README.md)

Both logos are made up and serve only as placeholders for this example. There is no club called "OL-Verein
Musterstadt" and no company called "Beispiel-Sponsor GmbH". Any likeness to real clubs, companies or brands is
unintended. Both files were made for freemapper-print on 9 November 2026 and are dedicated to the public domain
under [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/): anyone may use them without conditions.

## ol-verein-musterstadt.svg

Written by hand, 210 × 60 units. On the left a control flag (white and orange, split along the diagonal), on the
right "OL-VEREIN" and "MUSTERSTADT". The letters are lines (`polyline`, `line`, `path`, one `rect` for the O),
not `<text>`, because freemapper-print draws no text in logos. The file uses only elements from the list
freemapper-print accepts for SVG logos (ADR 0026) and prints without a note.

## beispiel-sponsor.png

640 × 200 pixels, RGBA with an alpha channel: a green badge with rounded, transparent corners, a yellow circle
with a "B", and "BEISPIEL-SPONSOR" and "GMBH" in white. A short Python script drew it with the standard library
alone (`zlib`, `struct`, `math`), without an image library. It builds the letters from strokes on a grid of
6 × 10 units, smooths the edges by each pixel's distance to the stroke centre line and writes the PNG chunks
`IHDR`, `IDAT` and `IEND` itself. The script is not in the repository.
