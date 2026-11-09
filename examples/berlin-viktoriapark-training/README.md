# berlin-viktoriapark-training

[English](README.en.md)

Ein Vereinstraining im Viktoriapark in Berlin-Kreuzberg: eine Bahn von 1,7 km mit 10 Posten, aus einer
Purple-Pen-Datei, gedruckt auf A4 in 1:4000 mit Postenbeschreibung.

![Das Blatt der Trainingsbahn, A4 quer in 1:4000, mit Beschreibungskasten oben rechts](preview.jpg)

| Datei | Inhalt |
|---|---|
| `freemapper-print.toml` | die Projektdatei |
| `training.ppen` | die Bahn als Purple-Pen-Datei, für dieses Beispiel von Hand geschrieben |
| `berlin-viktoriapark.omap` | die Karte (OOM, ISSprOM 2019, 1:4000) |
| `freemapper.toml` | Begleitdatei von freemapper; `init` liest daraus nur die Kartenart |
| `ATTRIBUTION.txt` | die Quellen der Karte, von freemapper |
| `out/Training.pdf` | das Blatt der Bahn, A4 quer, 1:4000 |
| `preview.jpg` | dasselbe Blatt als Bild |

## Die Bahn

Start auf der Straße am Nordrand des Parks, Ziel ebenfalls dort, 200 m weiter östlich. Dazwischen führt die Bahn
gegen den Uhrzeigersinn durch den Park und um ihn herum: fünf Posten an Wegegabelungen im Park, fünf an
Gebäudeecken. Die Zielzeile der Beschreibung hat das Symbol 14.3: Vom letzten Posten orientiert man sich ohne
Markierung ins Ziel.

Jeder Posten liegt auf einem Objekt der Karte, einem Eckpunkt eines Gebäudes (Symbol 521) oder einem Knoten
zweier Wege (505.1). Ein kleines Skript hat diese Punkte aus `berlin-viktoriapark.omap` gelesen, in Millimeter
umgerechnet und mit der y-Achse nach oben geschrieben, wie Purple Pen es tut (ADR 0032). Die Beschreibungen
folgen der IOF-Norm von 2018: Spalte D „Gebäude“ oder „Weg“, Spalte G die Außenecke mit Himmelsrichtung,
Spalte F „Gabelung“. Die Datei gibt keine Lage für Postennummern an, alle Nummern setzt das Programm selbst.

## Was `check` meldet

Das Blatt ist in Ordnung, `check` endet mit Exit-Code 0. Die Zeilen „Zur Information“ zählen nicht.

```
Zur Information: Die geografischen Koordinaten wurden nicht geprüft, denn kein Posten hat sowohl eine Kartenposition als auch geografische Koordinaten.
Bahn „Training“ auf Layout a4-4000: A4 Querformat, Druckmaßstab 1:4000, Kartenmaßstab 1:4000, Geometriefaktor 1,000, Symbolfaktor 1,000.
Zur Information: Die Karte ist 37,4 mm zu breit und 18,3 mm zu hoch für den Kartenausschnitt und wird dort mit Absicht beschnitten (map_frame.crop = true).
1 Bahn geprüft, keine Hinweise.
```

## So ist es entstanden

Nur mit freemapper-print selbst, im Ordner dieses Beispiels:

```sh
freemapper-print --lang de init . --map berlin-viktoriapark.omap --title "Training Viktoriapark" --courses training.ppen
```

```
Kartenart: Sprintkarten, wie in der freemapper.toml neben der Karte angegeben.
Bahndatei …/training.ppen gelesen: 1 Bahn, an die Karte angepasst.
Suche ein A4- oder A3-Blatt, das die Karte in 1:4000 zeigt …
./freemapper-print.toml geschrieben mit einem Layout und einer Bahnzuordnung, für Sprintkarten.
Layout a4 bei 1:4000: A4, Passepartout 8 mm, für „Training“.
Warnung: Kein A4- oder A3-Blatt zeigt die Karte bei 1:4000 ohne Hinweise oder Fehler, deshalb bleibt Layout a4 bei A4. check meldet das.
Die Druckmaßstäbe stammen aus der Bahndatei, ein Layout je Maßstab. Eine Bahn ohne Druckmaßstab bekommt den Kartenmaßstab 1:4000.
Jedes Layout hat einen Beschreibungskasten in der oberen rechten Ecke des Kartenausschnitts, mit der Zellengröße des Regelprofils.
Ein Bahnblatt schneidet die Karte mit Absicht auf die Bahn zu, deshalb haben die Layouts map_frame.crop = true.
Das Projekt schlägt eine Äquidistanz von 2 m vor, üblich für Sprintkarten. Sehen Sie auf der Karte nach und bestätigen Sie den Wert oder ändern Sie map.contour_interval_m.
Als Nächstes freemapper-print check ./freemapper-print.toml aufrufen. Es meldet die Hinweise oben, bis das Layout geändert ist.
```

Die zweite Zeile nennt den vollen Pfad der Bahndatei, hier gekürzt. Der erste `check` hat drei Hinweise:

```
Hinweis: Die Bahn passt mit der y-Achse nach oben und nach unten auf die Karte, und nichts in der Datei entscheidet. Das Programm hat „oben“ genommen. Prüfen Sie ein Blatt und tragen Sie dann in der Projektdatei unter [sources] course_y_axis = "up" oder "down" ein.
Hinweis: Layout a4 zeigt 5 von 9 Quellenangaben. Eine weitere Zeile zählt die übrigen.
Hinweis: Das Passepartout ist zu schmal für die Fußzeile mit Quellenangaben und Namensnennung. Sie steht deshalb auf einem weißen Kasten im Kartenausschnitt und deckt einen Teil der Karte ab. Ein breiteres margin_mm oder ein Element copyright_line oder credit_line verschiebt sie.
```

Die Kartennotizen haben neun Zeilen, zu viele für die Fußzeile. Danach ist die Projektdatei
von Hand bearbeitet:

- `[event]`: Datum und Ausrichter ergänzt.
- `[sources]`: `course_y_axis = "up"` eingetragen. Das Blatt zeigt die Posten an ihren Gebäudeecken und
  Wegegabelungen, also stimmt „oben“.
- `[print]`: die Quellen in drei Zeilen. Die sechs Datensätze des Geoportals Berlin stehen in einer Zeile,
  weil alle unter derselben Lizenz dl-de/zero-2-0 stehen.
- Im Titelblock eine Zeile mit `{date} · {length_km} km · {controls} Posten` ergänzt und „Äquidistanz“ vor
  `{contour_interval}` geschrieben.
- Das Layout `a4` heißt jetzt `a4-4000`: A4 quer statt `auto`, 12 mm Passepartout statt 8 mm. So passen die
  vier Zeilen der Fußzeile unter den Kartenausschnitt.

Dann prüfen und das PDF schreiben:

```sh
freemapper-print --lang de check .
freemapper-print --lang de export .
```

`export` schreibt `out/Training.pdf` und endet mit Exit-Code 0.

## Offen

- Öffnen Sie `training.ppen` einmal in Purple Pen (Beta für macOS) und prüfen Sie die Datei: Lädt Purple Pen die
  Karte `berlin-viktoriapark.omap`? Stehen die Posten an ihren Gebäudeecken und Wegegabelungen, und stimmen die
  Beschreibungen? Der Wert `Spr2019` für den Kartenstandard ist nicht gegen Purple Pen geprüft.

## Geprüft

- `check` und `export` wie oben, mit den Meldungen von oben.
- Zweimal exportiert: Das PDF ist Byte für Byte gleich.
- Die Seite bei 110 dpi angesehen: Jeder Posten steht an seiner Gebäudeecke oder Weggabelung, der
  Beschreibungskasten deckt keinen Posten ab, die Fußzeile steht unter dem Kartenausschnitt.

## Neu erzeugen

Das PDF mit `export` wie oben, die Vorschau wie bei
[luebeck-competition-courses](../luebeck-competition-courses/README.md#neu-erzeugen), mit `out/Training.pdf` als
Eingabe und der Beschriftung „Training, 1:4000, A4“.

## Drucken

Drucken Sie mit 100 % („Tatsächliche Größe“), und schalten Sie „An Seite anpassen“ aus. Mit der
Kontrollstrecke messen Sie am Ausdruck nach, ob der Drucker skaliert hat.

## Herkunft und Lizenz

Karte, `freemapper.toml` und `ATTRIBUTION.txt` aus dem Beispiel `berlin-viktoriapark` von
[freemapper](https://github.com/HenryJobst/freemapper), Version 0.6.0 (Commit `a2f8b80`), unverändert. Die
Vorlagen der Karte (Orthofoto, K5 und andere unter `templates/`) sind nicht kopiert; freemapper-print druckt
keine Vorlagen. Für die Karte gilt, was `ATTRIBUTION.txt` nennt: © OpenStreetMap-Mitwirkende (ODbL),
© GeoBasis-DE/LGB 2026 (dl-de/by-2-0) und Daten des Geoportals Berlin (dl-de/zero-2-0, zum Teil geändert). Die
Bahn in `training.ppen` ist für dieses Beispiel geschrieben und steht unter CC0. Das PDF ist mit
[freemapper-print](https://github.com/HenryJobst/freemapper-print) erzeugt.
