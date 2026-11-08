# luebeck-scales

[Deutsch](README.md)

The same sprint map printed twice: at 1:4000 on A4 for the older classes and at 1:3000 on A3 for the youngest.

![1:4000 on A4 left, 1:3000 on A3 right, shown at the same scale](preview.jpg)

| File | Content |
|---|---|
| `luebeck.omap` | the map (OOM, ISSprOM 2019, 1:4000) |
| `luebeck-4000-a4.pdf` | print at 1:4000 on A4 landscape |
| `luebeck-3000-a3.pdf` | print at 1:3000 on A3 landscape |
| `preview.jpg` | both pages side by side, with a grey page frame |

## What the example shows

The pages are exactly A4 and A3, and the map on them has exactly the scale named in the file. A distance of
100 m measures 25 mm in the PDF at 1:4000 and 33.3 mm at 1:3000.

The sprint standard (ISSprOM 2019-2, section 3.1) draws at 1:4000 and recommends 1:3000 for the youngest age
groups. Everything grows by the same ratio, terrain and symbols, to 133 % (ADR 0018). A small footpath
(symbol 506) is 0.27 mm wide at 1:4000 and 0.36 mm at 1:3000. So the map needs a third more paper in each
direction and only fits on A3.

At 1:4000 the map is 3.9 mm too tall for the printable area of A4. The program warns and still writes the PDF.
That is intended here. At 1:3000 on A3 the whole map fits.

## Checked

- Stroke widths read from the PDF content: symbol 506 is 0.27 mm in `luebeck-4000-a4.pdf` and 0.36 mm in
  `luebeck-3000-a3.pdf`, and every other line grows to 133 % as well.
- Page sizes: 297 × 210 mm and 420 × 297 mm (`/MediaBox` in the PDF).
- The preview viewed: both maps complete, water blue, page frames grey.

## Regenerate

```sh
freemapper-print render luebeck.omap --out luebeck-4000-a4.pdf --paper a4
freemapper-print render luebeck.omap --out luebeck-3000-a3.pdf --paper a3 --scale 3000
```

The preview comes from `pdftoppm` and ImageMagick, working in a temporary folder. On macOS `FONT` is the path
to a font file; on Linux and Windows it is a font name from `magick -list font`, such as `DejaVu-Sans`.

```sh
FONT=/System/Library/Fonts/Helvetica.ttc
tmp=$(mktemp -d)
pdftoppm -r 50 -png -singlefile luebeck-4000-a4.pdf "$tmp/p4"
pdftoppm -r 50 -png -singlefile luebeck-3000-a3.pdf "$tmp/p3"
magick "$tmp/p4.png" -bordercolor gray60 -border 1 -background white -gravity north -splice 0x40 \
  -font "$FONT" -pointsize 28 -annotate +0+4 "1:4000, A4" -bordercolor white -border 8 "$tmp/l4.png"
magick "$tmp/p3.png" -bordercolor gray60 -border 1 -background white -gravity north -splice 0x40 \
  -font "$FONT" -pointsize 28 -annotate +0+4 "1:3000, A3" -bordercolor white -border 8 "$tmp/l3.png"
magick "$tmp/l4.png" "$tmp/l3.png" -background white -gravity south +append -quality 80 \
  preview.jpg
rm -r "$tmp"
```

## Printing

Print at 100 % ("actual size") with "fit to page" switched off. Any other size breaks the scale.

## Credits

Map from [freemapper](https://github.com/HenryJobst/freemapper) (commit `a0cc8ef`, GPL-3.0-or-later), data
sources in the map notes (`<notes>`), rendered with
[freemapper-print](https://github.com/HenryJobst/freemapper-print).
