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
| `luebeck-4000-a3-portrait.pdf` | `tests/fixtures/maps/luebeck.omap` | A3 hoch |
| `forest-sample-10000-a4-portrait.pdf` | `tests/fixtures/maps/forest-sample.omap` | A4 hoch |

Das Papier ist so gewählt, dass die ganze Karte im Maßstab der Karte ohne Überlauf draufpasst. Dann vergleicht
der Zuschnitt auf den Inhalt die komplette Karte. Der Test wählt es selbst: erst A4, dann A3, Ausrichtung
automatisch, die erste Größe ohne Überlauf gewinnt. Ändert sich eine Karte, ändert sich vielleicht auch der
erwartete Dateiname. Die Zeile "SKIP ... is missing" im Testlauf nennt dann den richtigen.

## Export in OOM 0.9.6

Für jede der beiden Karten:

1. Karte in OOM öffnen (`File > Open...`).
2. `File > Export as...` und dort `PDF` wählen. Je nach Anordnung der Menüs steckt dieselbe Funktion unter
   `File > Print...` mit der Option "Export to PDF". Beide führen in den Dialog für Druck und Export.
3. Seitenformat: im Reiter für das Seitenformat das Papier aus der Tabelle oben einstellen und die
   Ausrichtung auf "Portrait" (hoch) setzen. Seitenränder und Überlappung so lassen, wie sie sind. Der
   Zuschnitt im Test schneidet Ränder ohnehin ab.
4. Maßstab: "Scale" auf den Kartenmaßstab setzen, also 1 : 4000 für Lübeck und 1 : 10000 für die
   Waldbeispielkarte. Nicht auf "Fit to page" gehen.
5. Druckbereich: den Druckbereich auf die Karte zentrieren (Schaltfläche "Center print area on map", falls
   vorhanden; sonst den Bereich von Hand so setzen, dass die ganze Karte darin liegt). Kontrolle in der
   Vorschau: Die Karte liegt vollständig auf der Seite, und es gibt nur eine Seite.
6. Modus: "Vector graphics" (nicht "Raster graphics", nicht "Separations").
7. Optionen ausschalten: "Show templates" (Vorlagen) aus, "Show grid" (Gitter) aus, "Simulate overprinting"
   (Überdruck simulieren) aus.
8. "Export" wählen, als Zielordner `tests/fixtures/oom/` dieses Repositorys angeben und den Dateinamen aus der
   Tabelle oben eintragen.

Wenn eine Bezeichnung in deiner OOM-Version anders lautet, gilt der Sinn: Papier, Maßstab gleich Kartenmaßstab,
ganze Karte auf einer Seite, Vektor, keine Vorlagen, kein Gitter, kein Überdruck.

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
- `luebeck-4000-a3-portrait`: Größe (unser PDF / OOM) in px, Anteil abweichender Pixel in %:
- `forest-sample-10000-a4-portrait`: Größe (unser PDF / OOM) in px, Anteil abweichender Pixel in %:
- Gefundene Abweichungen im Differenzbild und was daraus wurde (Fehler behoben oder als erwartet akzeptiert):

## Probedruck

Der Test prüft Bilder, nicht Papier. Ob der Maßstab auf dem Drucker wirklich stimmt, zeigt nur ein Ausdruck.
Eine Maßstabsleiste gibt es erst mit M2, deshalb wird eine bekannte Strecke nachgemessen.

1. PDF erzeugen:

   ```sh
   freemapper-print render tests/fixtures/maps/luebeck.omap --out luebeck-a3.pdf --paper a3 --orientation portrait
   ```

2. Auf A3 in 100 % ausdrucken, "An Seite anpassen" aus. Im Druckdialog von macOS heißt das "Tatsächliche
   Größe". Ein A3-Drucker ist nötig; auf A4 läuft die Karte über (das Programm meldet 62,6 mm Überlauf)
   und lässt sich so nicht prüfen. Notfalls im Copyshop drucken.
3. Zwei Gebäudeecken suchen und den Abstand mit einem Lineal messen. Die Ecken sind die Anfangspunkte zweier
   Gebäude (Symbol 521) aus `luebeck.omap`. Ihre Lage auf dem A3-Blatt, gemessen von der linken und oberen
   Papierkante:

   | Ecke | von links | von oben |
   |---|---|---|
   | A (Anfang des Gebäudes bei Karten-Koordinate -80,780 / 20,866 mm) | 68,2 mm | 231,0 mm |
   | B (Anfang des Gebäudes bei Karten-Koordinate -107,548 / -71,778 mm) | 41,5 mm | 138,3 mm |

   Soll-Abstand: 96,4 mm auf dem Papier. Das sind bei 1 : 4000 rund 385,7 m in der Natur.
   Die Lage auf dem Blatt gilt für den Standard-Aufruf oben (Karte zentriert). Die Ecken sind kurze Gebäudekanten
   von etwa 2 bis 3 mm. Wenn du sie nicht eindeutig findest, miss stattdessen von der linken Papierkante bis zum
   Gebäude A (Soll 68,2 mm) und bis zum Gebäude B (Soll 41,5 mm), das ergibt dieselbe Aussage.
4. Ergebnis eintragen. Eine Abweichung bis etwa 0,5 mm (0,5 %) ist Messung und Drucker. Mehr deutet auf
   Skalierung im Druckdialog hin.

Ergebnis des Probedrucks:

- Datum:
- Drucker und Papier:
- Gemessener Abstand A-B (Soll 96,4 mm):
- Bemerkung:
