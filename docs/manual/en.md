# freemapper-print user manual

[Deutsch](de.md) · [README](../../README.en.md)

This manual is being written with the first usable version; so far only chapter 9 is done. In the end it should
lead step by step from the finished OOM map and the Purple Pen file to the printed sheets, without needing the
command line.

Planned chapters:

1. What freemapper-print does
2. Installation
3. With an AI assistant
4. Without an assistant: editor and command line
5. Setting up an event: map and courses
6. Layouts: paper size, scale, map frame
7. Title, logos and other sheet elements
8. Masks for window and corridor orienteering
9. Printing at the right scale at home and in a copy shop
10. Messages and what to do

## 9. Printing at the right scale at home and in a copy shop

An orienteering map must have the scale printed on it. The PDF from freemapper-print has it exactly: 1 mm on
the paper is the ground distance the print scale sets. Whether the printout keeps it is decided in the print
dialog.

### Creating the PDF

```sh
freemapper-print render luebeck.omap --out luebeck-4000-a4.pdf --paper a4
```

Without `--scale` the program prints at the scale of the map. `--scale 5000` prints at 1:5000. Choose A4 or A3
(`--paper`) and, if needed, the orientation (`--orientation`). With `auto` the program takes the orientation
that cuts off less of the map. First the program writes a line with the paper, the print scale and the factors,
then come notes and a reminder to print at actual size.

### Printing at actual size

Open the PDF and choose 100 % or "Actual size" in the print dialog. Switch off "Fit to page" and "Shrink to
printable area". Any other setting changes the scale, and the map is no longer right. In a copy shop, say the
same: no scaling, 100 %, paper as in the PDF.

### A4 or A3

The program leaves 5 mm free on every side, because many printers do not print to the edge of the paper. If the
map is larger than the page, the program cuts it off and tells you, for example:

```
Warning: the map does not fit on the page. It is 3.9 mm too tall for the printable area. Try A3 paper, or ...
```

The number says by how many millimetres the map extends beyond the printable area. The map is not shrunk,
because that would falsify the scale. You have three options: larger paper (A3), the other orientation, or a
print scale with a larger number, such as 1:5000 instead of 1:4000. The message names suitable suggestions. The
PDF is written even with an overflow, so you can see what is missing.

### Why symbols keep their size

When you go from 1:4000 to 1:5000, the terrain shrinks to 80 %. The symbols do not shrink with it. The
specification for the map kind sets how wide a path and how large a building symbol are on the paper, whatever
the scale. The map stays readable, and more terrain fits on the page. In the example, a footpath is 0.27 mm wide
in both PDFs.

For this the program needs the map kind (sprint, schoolyard, forest). If it can tell it neither from
`freemapper.toml` nor from the map file, give it with `--kind`.

### Checking the scale with a ruler

The example [examples/luebeck-scales](../../examples/luebeck-scales/README.en.md) holds the same map at
1:4000 and 1:5000. Print both at actual size. Then measure between two points whose distance you know, such as
two building corners or the ends of a straight stretch of street. For 100 m your ruler should show:

| Print scale | 100 m on the map |
|---|---|
| 1:4000 | 25 mm |
| 1:5000 | 20 mm |

If the value is off, the printer has scaled. Check the print dialog. The program draws no scale check bar of its
own yet; it follows with M2 and the layouts.

### Messages from the program

- "not drawn because the symbol is unknown or hidden": objects whose symbol the map file hides or does not
  know. They are missing in the PDF.
- "simplified or cut short": an object needs more drawing commands than the program allows per object. It
  appears simplified or cut off.
- "the font Arimo replaces ...": the PDF contains only the Arimo font. The program replaces the other fonts of
  the map with it.
- "missing glyphs": a character of the map is not in Arimo and is not printed.
- The north lines and the information block of the map are printed only with `--show-sheet-objects`. At another
  print scale they scale with the map, so a scale printed in them can be wrong.

The other chapters follow with later versions.
