# luebeck-competition-layouts

[English](README.en.md)

Ein Stadtsprint in der Lübecker Altstadt mit zwei Layouts: A4 in 1:4000 für die älteren Klassen und A3 in
1:3000 für die jüngsten. Beide Blätter haben Passepartout, Titelblock, Vereins- und Sponsorenlogo, eine
Kontrollstrecke und unten links die Quellenangaben und die Namensnennung. Bahnen kommen erst mit M3 dazu.

![Links das A4-Blatt in 1:4000, rechts das A3-Blatt in 1:3000, im selben Maßstab abgebildet](preview.jpg)

| Datei | Inhalt |
|---|---|
| `freemapper-print.toml` | die Projektdatei |
| `luebeck.omap` | die Karte (OOM, ISSprOM 2019, 1:4000) |
| `freemapper.toml` | Begleitdatei von freemapper; `init` liest daraus nur die Kartenart |
| `logos/` | zwei erfundene Logos, siehe [logos/README.md](logos/README.md) |
| `out/a4-4000.pdf` | Blatt der älteren Klassen, A4 quer, 1:4000 |
| `out/a3-3000.pdf` | Blatt der jüngsten Klassen, A3 quer, 1:3000 |
| `preview.jpg` | beide Blätter nebeneinander |

## Was das Beispiel zeigt

**Gemeinsame Elemente und Layout-Elemente.** Titelblock, Logos und Kontrollstrecke stehen einmal unter
`[[shared]]` und erscheinen auf beiden Blättern. Jedes Layout hat dazu ein eigenes Textelement `classes` mit
seinen Klassen. Alle Elemente hängen an einer Ecke des Kartenausschnitts und haben auf A4 und A3 dieselbe Größe
in Millimetern, gleich in welchem Maßstab die Karte gedruckt wird.

**Platzhalter.** Der Titelblock schreibt `{event}`, `{date} · {organiser}` und
`1:{print_scale} · Äquidistanz {contour_interval}`. Auf dem A4-Blatt steht daraus „1:4000 · Äquidistanz 2 m“,
auf dem A3-Blatt „1:3000 · Äquidistanz 2 m“. Die Kontrollstrecke misst „200 m = 50 mm“ bei 1:4000 und
„100 m = 33,3 mm“ bei 1:3000.

**Symbole auf 133 %.** Das A3-Layout druckt in 1:3000, also wachsen Gelände und Symbole nach ISSprOM 2019-2
(Abschnitt 3.1) auf 133 %, wie im Beispiel [luebeck-scales](../luebeck-scales/README.md).

**Nordlinien.** Die Karte bringt ihre eigenen Nordlinien mit. Das Programm ergänzt deshalb keine.

**Fußzeile.** Die Karte nennt in ihren Notizen vier Zeilen Quellen. Mit der Namensnennung von freemapper-print
wären das fünf Zeilen, und auf dem A3-Blatt passt so ein Block nur in einen Rand von 14 mm. Der Kartenausschnitt
von 1:3000 braucht aber fast die ganze Seite. `print.copyright` fasst die Quellen deshalb in zwei Zeilen
zusammen, dann reicht ein Passepartout von 10 mm, und die Fußzeile steht unter dem Kartenausschnitt statt auf
einem weißen Kasten in der Karte.

## Was `check` meldet

Das A3-Blatt hat keine Hinweise. Das A4-Blatt hat einen, und der ist gewollt: Die Karte ist in 1:4000 14,0 mm
höher als der Kartenausschnitt (210 mm Papier minus zweimal 10 mm Passepartout). Das Programm schneidet oben und
unten je 7 mm ab und verkleinert nichts. Für einen echten Wettkampf legt man mit `map_frame.centre_map_mm` den
Ausschnitt auf das Wettkampfgebiet. `check` endet deshalb mit Exit-Code 1.

```
Layout a4-4000: A4 Querformat, Druckmaßstab 1:4000, Kartenmaßstab 1:4000, Geometriefaktor 1,000, Symbolfaktor 1,000.
Warnung: Die Karte passt nicht auf die Seite. Sie ist 14,0 mm zu hoch für den bedruckbaren Bereich. Versuchen Sie Papier im Format A3, die andere Ausrichtung oder einen Druckmaßstab mit größerer Zahl, zum Beispiel 1:5000 (nicht normgerecht, nur fürs Training).
Layout a3-3000: A3 Querformat, Druckmaßstab 1:3000, Kartenmaßstab 1:4000, Geometriefaktor 1,333, Symbolfaktor 1,333.
Layouts mit Hinweisen: 1 von 2.
```

## So ist es entstanden

Nur mit freemapper-print selbst, im Ordner dieses Beispiels:

```sh
freemapper-print --lang de init . --map luebeck.omap --title "Stadtsprint Lübeck"
```

```
./freemapper-print.toml geschrieben, mit einem Layout a3: A3, Passepartout 14 mm, Druckmaßstab 1:4000, für Sprintkarten.
Das Projekt schlägt eine Äquidistanz von 2,5 m vor, üblich für Sprintkarten. Sehen Sie auf der Karte nach und bestätigen Sie den Wert oder ändern Sie map.contour_interval_m.
Als Nächstes freemapper-print check ./freemapper-print.toml aufrufen.
```

Danach ist die Projektdatei von Hand bearbeitet:

- `[event]`: Datum und Ausrichter ergänzt.
- `map.contour_interval_m` von 2,5 auf 2 gesetzt. So steht es im Informationsblock der Karte („Maßstab 1:4000 ·
  Äquidistanz 2 m“).
- `[print]` mit den zwei Quellenzeilen ergänzt.
- Im Titelblock die Zeile mit Datum und Ausrichter ergänzt und „Äquidistanz“ vor `{contour_interval}`
  geschrieben.
- Die beiden Logos als gemeinsame Elemente ergänzt.
- Das Layout `a3` von `init` (1:4000, 14 mm) durch die Layouts `a4-4000` und `a3-3000` ersetzt, beide quer und
  mit 10 mm Passepartout, jedes mit seinem Text `classes`.

Dann prüfen und die PDFs schreiben. `export` legt den Ordner `out` neben der Projektdatei an.

```sh
freemapper-print --lang de check .
freemapper-print --lang de export .
```

`export` gibt dieselben Zeilen aus wie `check`, nennt in der ersten Zeile jedes Layouts den Pfad des PDFs und
erinnert zum Schluss ans Drucken in Originalgröße. Es endet mit Exit-Code 0, auch mit dem Hinweis zum A4-Blatt.

Die Vorschau entsteht wie bei [luebeck-scales](../luebeck-scales/README.md#neu-erzeugen), nur mit
`out/a4-4000.pdf` und `out/a3-3000.pdf` als Eingabe.

## Geprüft

- `check` und `export` wie oben, mit den Meldungen von oben.
- Zweimal exportiert: Die PDFs sind Byte für Byte gleich.
- Die Seiten bei 200 dpi angesehen: Titelblock und Klassen oben links, die Logos oben rechts, die
  Kontrollstrecke unten rechts, die drei Zeilen der Fußzeile unter dem Kartenausschnitt, die Nordlinien der Karte
  über die ganze Höhe.

## Drucken

Drucken Sie mit 100 % („Tatsächliche Größe“), und schalten Sie „An Seite anpassen“ aus. Mit der
Kontrollstrecke messen Sie am Ausdruck nach, ob der Drucker skaliert hat.

## Herkunft

Karte und `freemapper.toml` aus dem Beispiel `schleswig-holstein-luebeck` von
[freemapper](https://github.com/HenryJobst/freemapper), Version 0.5.0 (Commit `e8b6db9`), unverändert.
Kartendaten © OpenStreetMap-Mitwirkende (ODbL) und © GeoBasis-DE/LVermGeo SH (CC BY 4.0, Quelle verändert). Die
Logos sind für dieses Beispiel gemacht (CC0). Die PDFs sind mit
[freemapper-print](https://github.com/HenryJobst/freemapper-print) erzeugt.
