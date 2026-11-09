# luebeck-scales

[English](README.en.md)

Dieselbe Sprintkarte zweimal: in 1:4000 auf A4 für die älteren Klassen und in 1:3000 auf A3 für die jüngsten.

![1:4000 auf A4 links, 1:3000 auf A3 rechts, im selben Maßstab abgebildet](preview.jpg)

| Datei | Inhalt |
|---|---|
| `luebeck.omap` | die Karte (OOM, ISSprOM 2019, 1:4000) |
| `luebeck-4000-a4.pdf` | Druck in 1:4000 auf A4 quer |
| `luebeck-3000-a3.pdf` | Druck in 1:3000 auf A3 quer |
| `preview.jpg` | beide Seiten nebeneinander, mit grauem Seitenrahmen |

## Was das Beispiel zeigt

Die Seiten sind genau A4 und A3, und die Karte darauf hat genau den Maßstab, der in der Datei steht. Eine
Strecke von 100 m misst im PDF 25 mm bei 1:4000 und 33,3 mm bei 1:3000.

Die Sprintnorm (ISSprOM 2019-2, Abschnitt 3.1) zeichnet in 1:4000 und empfiehlt 1:3000 für die jüngsten
Altersklassen. Dabei wächst alles im selben Verhältnis, Gelände und Symbole, auf 133 % (ADR 0018). Ein
schmaler Fußweg (Symbol 506) ist bei 1:4000 0,27 mm breit und bei 1:3000 0,36 mm. Die Karte braucht deshalb
ein Drittel mehr Papier in jeder Richtung und passt nur noch auf A3.

Bei 1:4000 ist die Karte 4,0 mm zu hoch für den bedruckbaren Bereich von A4. Das Programm warnt und schreibt
das PDF trotzdem. Das ist hier gewollt. Bei 1:3000 auf A3 passt die ganze Karte.

Unten links steht auf einem weißen Kasten die Fußzeile: die Quellenangaben aus den Kartennotizen und die
Namensnennung „Erstellt mit freemapper-print“. Der Rand von 5 mm ist zu schmal für sie, deshalb deckt sie
eine Ecke der Karte ab, und `render` meldet das.

## Geprüft

- Strichbreiten aus den PDF-Inhalten gelesen: Symbol 506 hat 0,27 mm in `luebeck-4000-a4.pdf` und 0,36 mm in
  `luebeck-3000-a3.pdf`, alle anderen Linien wachsen ebenso auf 133 %.
- Seitengrößen: 297 × 210 mm und 420 × 297 mm (`/MediaBox` im PDF).
- Die Vorschau angesehen: beide Karten vollständig, Wasser blau, Seitenrahmen grau, Fußzeile unten links.

## Neu erzeugen

```sh
freemapper-print --lang de render luebeck.omap --out luebeck-4000-a4.pdf --paper a4
freemapper-print --lang de render luebeck.omap --out luebeck-3000-a3.pdf --paper a3 --scale 3000
```

Die Vorschau entsteht mit `pdftoppm` und ImageMagick in einem temporären Ordner. `FONT` ist unter macOS ein
Pfad zu einer Schriftdatei, unter Linux und Windows ein Schriftname aus `magick -list font`, etwa
`DejaVu-Sans`.

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

## Drucken

Drucken Sie mit 100 % („Tatsächliche Größe“), und schalten Sie „An Seite anpassen“ aus. Jede andere Größe
verfälscht den Maßstab.

## Herkunft

Karte aus dem Beispiel `schleswig-holstein-luebeck` von [freemapper](https://github.com/HenryJobst/freemapper),
Version 0.5.0 (Commit `e8b6db9`), unverändert. Kartendaten © OpenStreetMap-Mitwirkende (ODbL) und
© GeoBasis-DE/LVermGeo SH (CC BY 4.0, Quelle verändert); die Kartennotizen (`<notes>`) nennen die Quellen. Die
PDFs sind mit [freemapper-print](https://github.com/HenryJobst/freemapper-print) erzeugt.
