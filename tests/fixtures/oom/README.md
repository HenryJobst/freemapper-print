# Vergleich mit dem PDF-Export von OOM

Der ignorierte Test `matches_oom_within_tolerance` (`crates/freemapper-print/tests/oom_comparison.rs`) legt
unser PDF neben das PDF, das OpenOrienteering Mapper 0.9.6 für dieselbe Karte erzeugt. Beide werden mit
`pdftoppm` bei 150 dpi gerastert und auf den nicht weißen Inhalt zugeschnitten. Die Größen dürfen um 2 px
abweichen, und weniger als 3 % der Pixel dürfen sich um mehr als 96 Graustufen unterscheiden (ADR 0009: gleiche
Norm, nicht pixelgleich).

Die OOM-Dateien kann nur jemand mit OOM erzeugen. Fehlen sie, meldet der Test das und geht grün durch.

## Diese zwei Dateien fehlen noch

Ablage in diesem Ordner (`tests/fixtures/oom/`):

| Datei | Karte | Papier |
|---|---|---|
| `luebeck-4000-a3-landscape.pdf` | `tests/fixtures/maps/luebeck.omap` | A3 quer |
| `forest-sample-10000-a4-landscape.pdf` | `tests/fixtures/maps/forest-sample.omap` | A4 quer |

Das Papier ist so gewählt, dass die ganze Karte im Maßstab der Karte ohne Überlauf draufpasst. Dann vergleicht
der Zuschnitt auf den Inhalt die komplette Karte. Der Test sucht es selbst: erst A4, dann A3. Innerhalb eines
Papiers kommt zuerst die Ausrichtung, die zur Form der Karte passt (quer für eine Karte, die breiter als hoch
ist), dann die andere. Die erste Kombination ohne Überlauf gewinnt. Lübeck passt nicht auf A4, auf A3 quer
schon. Die Waldbeispielkarte passt auf A4 quer. Ändert sich eine Karte, ändert sich vielleicht auch der
erwartete Dateiname. Die Zeile "SKIP ... is missing" im Testlauf nennt dann den richtigen.

## Export in OOM 0.9.6

Die Bezeichnungen unten stammen aus dem Quelltext von OOM 0.9.6 (englisch, in Klammern die deutsche
Übersetzung). Für jede der beiden Karten:

1. Karte in OOM öffnen.
2. Im Menü `File > Export as... > PDF` wählen (deutsch `Datei > Exportieren als... > PDF`). Es öffnet sich der
   Druckdialog, und als Ziel steht dort "Save to PDF" ("Als PDF speichern").
3. "Page format:" ("Seitenformat:") auf das Papier aus der Tabelle oben stellen. "Page orientation:" auf
   "Landscape" ("Querformat") stellen, in beiden Fällen.
4. "Map area:" ("Kartenfläche:") auf "Single page" ("Einzelne Seite") stellen und "Center print area"
   ("Druckbereich zentrieren") ankreuzen.
5. "Print in different scale:" ("In anderem Maßstab drucken:") nicht ankreuzen. Dann gilt der Maßstab der Karte
   selbst.
6. "Mode:" ("Modus:") auf "Vector graphics" ("Vektorgrafik") lassen. Das ist die Voreinstellung.
7. "Show templates" ("Vorlagen ausgeben"), "Show grid" ("Gitter anzeigen") und "Simulate overprinting"
   ("Überdruck-Simulation") nicht ankreuzen.
8. "Export..." ("Exportieren...") wählen, als Zielordner `tests/fixtures/oom/` dieses Repositorys angeben und
   den Dateinamen aus der Tabelle oben eintragen.

Kontrolle vor dem Export: Die Karte liegt vollständig auf der einen Seite. Passt sie nicht, war das Papier oder
die Ausrichtung falsch.

## Test starten

```sh
cargo test -p freemapper-print --test oom_comparison -- --ignored --nocapture
```

`pdftoppm` kommt mit `brew install poppler`. Fehlt es, meldet der Test das und geht durch.

Schlägt der Vergleich fehl, liegen in `target/oom-comparison/` unser PDF und ein Differenzbild
(`<name>-diff.pgm`, weiß heißt gleich, dunkler heißt weiter auseinander). Der Test gibt den Pfad und den
Prozentwert aus. Das Bild lässt sich mit der Vorschau von macOS öffnen.

## Erwartete Abweichungen

ADR 0009 nimmt kleine Unterschiede in Kauf. Diese Stellen im Differenzbild sind deshalb zu erwarten und kein
Grund zur Sorge, solange die Karte lesbar und maßstabsgerecht bleibt:

- Schrift. Wir betten Arimo ein, OOM nimmt die Schrift des Systems. Buchstaben sitzen um Bruchteile eines
  Millimeters anders, Zeilenumbrüche können sich verschieben.
- Linienenden, Ecken und Strichelungen. Strichelmuster und Eckenverbindungen setzen wir nach den Regeln aus
  freemapper um. Einzelne Striche können um einen Pixel versetzt beginnen.
- Flächenmuster (Schraffuren, Punktraster). Phase und Ursprung des Musters können gegenüber OOM verschoben
  sein. Die Dichte muss stimmen.
- Kantenglättung. Das Rastern mit `pdftoppm` glättet Kanten, daher entstehen schmale Differenzsäume entlang
  aller Linien.
- Ränder. OOM kann einen Rahmen oder Rand setzen, den wir nicht zeichnen. Der Zuschnitt auf den Inhalt
  gleicht das aus, solange der Rand nicht dunkel ist.

Nicht erwartet, also ein Fehler im Renderer: ein ganzes Symbol fehlt oder hat die falsche Farbe, ein Objekt
liegt in der falschen Reihenfolge (zum Beispiel Wasser über Gebäuden), die Größe weicht um mehr als 2 px ab.

## Ergebnis des ersten echten Vergleichs

Nach dem ersten Lauf mit echten OOM-Dateien hier eintragen.

- Datum:
- OOM-Version:
- `luebeck-4000-a3-landscape`: Größe (unser PDF / OOM) in px, Anteil abweichender Pixel in %:
- `forest-sample-10000-a4-landscape`: Größe (unser PDF / OOM) in px, Anteil abweichender Pixel in %:
- Gefundene Abweichungen im Differenzbild und was daraus wurde (Fehler behoben oder als erwartet akzeptiert):

## Probedruck

Der Test prüft Bilder, nicht Papier. Ob der Maßstab auf dem Drucker wirklich stimmt, zeigt nur ein Ausdruck.
Eine Maßstabsleiste gibt es erst mit M2, deshalb misst du eine bekannte Strecke nach. Der Test
`probedruck_distances_match_readme` rechnet die Zahlen unten aus der Karte nach, sie veralten also nicht still.

Gemessen wird zwischen den Westspitzen zweier großer Gebäude (Symbol 521) am linken und am rechten Kartenrand.
Die Westspitze ist der Eckpunkt, der am weitesten links liegt; bei beiden Gebäuden ist das genau ein Punkt.

| Gebäude | Lage | Westspitze (Karten-Koordinate) |
|---|---|---|
| P | am linken Rand, etwa 11 × 14 mm groß | -89,453 / 0,633 mm |
| Q | am rechten Rand, etwa 9 × 17 mm groß | 109,319 / 5,093 mm |

Soll-Abstand P-Q: 198,8 mm auf dem Papier, bei 1 : 4000 sind das 795,3 m in der Natur. Du brauchst ein Lineal
von mindestens 20 cm.

### Variante A4 (Standard)

1. PDF erzeugen:

   ```sh
   freemapper-print render tests/fixtures/maps/luebeck.omap --out luebeck-a4.pdf --paper a4
   ```

   Das Programm warnt, dass die Karte 4,0 mm zu hoch für den bedruckbaren Bereich ist, und schreibt das PDF
   trotzdem. Das ist erwartet: Die Karte ist etwas höher als ein A4-Blatt, oben und unten fehlen je 2 mm. Die
   beiden Messpunkte liegen innerhalb des bedruckten Bereichs.
2. In 100 % ausdrucken, "An Seite anpassen" aus (im Druckdialog von macOS "Tatsächliche Größe"). Quer, A4.
3. Die Westspitzen liegen hier, von der linken und der oberen Papierkante gemessen:

   | Punkt | von links | von oben |
   |---|---|---|
   | P | 59,5 mm | 105,8 mm |
   | Q | 258,3 mm | 110,2 mm |

### Variante A3 (falls ein A3-Drucker da ist)

1. PDF mit `--paper a3 --orientation portrait` erzeugen, dann in 100 % auf A3 hoch drucken.
2. Lage der Westspitzen:

   | Punkt | von links | von oben |
   |---|---|---|
   | P | 59,5 mm | 210,8 mm |
   | Q | 258,3 mm | 215,2 mm |

### Ergebnis eintragen

Die Spitzen lassen sich auf etwa 0,5 mm genau anlegen, und der Papiervorschub eines Druckers weicht oft um
einige Zehntelprozent ab. Bis etwa 1,5 mm Abweichung vom Soll (198,8 mm) ist deshalb in Ordnung. Ein Ausdruck
mit "An Seite anpassen" ist um einige Prozent kleiner, bei 199 mm also um mehrere Millimeter.

- Datum:
- Drucker und Papier:
- Variante (A4 oder A3) und gemessener Abstand:
- Bemerkung:
