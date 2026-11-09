# Logos für das Beispiel

[English](README.en.md)

Beide Logos sind erfunden und nur Platzhalter für dieses Beispiel. Den Verein „OL-Verein Musterstadt“ und die
Firma „Beispiel-Sponsor GmbH“ gibt es nicht. Ähnlichkeit mit echten Vereinen, Firmen oder Marken ist nicht
gewollt. Beide Dateien sind am 9. November 2026 für freemapper-print entstanden und stehen unter
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/deed.de): Jeder darf sie ohne Bedingungen nutzen.

## ol-verein-musterstadt.svg

Von Hand geschrieben, 210 × 60 Einheiten. Links ein Postenschirm (weiß und orange, diagonal geteilt), rechts
„OL-VEREIN“ und „MUSTERSTADT“. Die Buchstaben sind Linien (`polyline`, `line`, `path`, ein `rect` für das O),
kein `<text>`, weil freemapper-print in Logos keinen Text zeichnet. Die Datei nutzt nur Elemente aus der
Liste, die freemapper-print für SVG-Logos annimmt (ADR 0026), und wird ohne Hinweis gedruckt.

## beispiel-sponsor.png

640 × 200 Pixel, RGBA mit Alphakanal: ein grünes Schild mit runden, durchsichtigen Ecken, ein gelber Kreis mit
„B“ und darin in Weiß „BEISPIEL-SPONSOR“ und „GMBH“. Ein kurzes Python-Skript hat es gezeichnet, nur mit der
Standardbibliothek (`zlib`, `struct`, `math`), ohne Bildbibliothek. Es setzt die Buchstaben aus Strichen auf
einem Raster von 6 × 10 Einheiten zusammen, glättet die Kanten über den Abstand jedes Pixels zur Strichmitte
und schreibt die PNG-Blöcke `IHDR`, `IDAT` und `IEND` selbst. Das Skript liegt nicht im Repository.
