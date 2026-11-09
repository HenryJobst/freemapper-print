# Handbuch zu freemapper-print

[English](en.md) · [README](../../README.md)

Dieses Handbuch wächst mit dem Programm. Geschrieben sind die Kapitel 5, 6, 7, 9 und 10. Am Ende soll es
Schritt für Schritt von der fertigen OOM-Karte und der Purple-Pen-Datei zu den gedruckten Kartenblättern führen,
auch ohne Erfahrung mit der Kommandozeile.

Kapitel:

1. Was freemapper-print macht
2. Installation
3. Mit einem KI-Assistenten
4. Ohne Assistent: Editor und Kommandozeile
5. [Eine Veranstaltung anlegen](#5-eine-veranstaltung-anlegen)
6. [Layouts: Blattformat, Maßstab, Kartenausschnitt](#6-layouts-blattformat-maßstab-kartenausschnitt)
7. [Titel, Logos und andere Blattelemente](#7-titel-logos-und-andere-blattelemente)
8. Abdeckungen für Fenster- und Korridor-OL
9. [Drucken: maßstabsgetreu am eigenen Drucker und im Copyshop](#9-drucken-maßstabsgetreu-am-eigenen-drucker-und-im-copyshop)
10. [Meldungen und was zu tun ist](#10-meldungen-und-was-zu-tun-ist)

Das Programm spricht Deutsch oder Englisch. Es nimmt die Sprache aus `--lang de` oder `--lang en`, sonst aus
den Umgebungsvariablen `FREEMAPPER_LANG`, `LC_ALL`, `LC_MESSAGES` und `LANG`. Die Meldungen in diesem Handbuch
sind die deutschen.

## 5. Eine Veranstaltung anlegen

Zu jeder Veranstaltung gehört ein Ordner mit einer Projektdatei, `freemapper-print.toml`. Darin stehen die
Veranstaltung, die Karte, die Layouts und die Blattelemente. Bahnen kommen mit einer späteren Version dazu (M3);
bis dahin druckt das Programm die Karte mit Titel, Logos und Kontrollstrecke, aber ohne Bahnaufdruck.

Der Ablauf hat drei Befehle:

1. `init` legt die Projektdatei an.
2. Sie bearbeiten die Datei mit einem Texteditor und rufen `check` auf, bis die Blätter so aussehen, wie sie
   sollen.
3. `export` schreibt je Layout ein PDF.

Das Beispiel [examples/luebeck-competition-layouts](../../examples/luebeck-competition-layouts/README.md) zeigt
den ganzen Weg mit allen Befehlen und Ausgaben.

### Die Projektdatei anlegen: `init`

```sh
freemapper-print init stadtsprint --map karten/luebeck.omap --title "Stadtsprint Lübeck"
```

Das legt den Ordner `stadtsprint` an, falls es ihn noch nicht gibt, und darin `freemapper-print.toml`. Eine
vorhandene Projektdatei überschreibt `init` nicht. Den Pfad zur Karte speichert die Datei relativ zu sich
selbst. Sie können den Ordner also verschieben, solange die Karte mitwandert.

Die neue Datei enthält:

- den Titel aus `--title`, ohne die Option den Dateinamen der Karte;
- die Kartenart. Sie kommt aus `--kind`, sonst aus der `freemapper.toml` neben der Karte, sonst aus dem
  Symbolsatz und dem Maßstab der Karte, und `init` sagt, woher. Findet das Programm keine, bricht es ab und
  fragt nach `--kind sprint`, `--kind schoolyard` oder `--kind forest`;
- einen Vorschlag für die Äquidistanz: 2 m für Sprintkarten, 5 m für Waldkarten, keinen für Schulhofkarten.
  Ein Kommentar in der Datei markiert den Wert. Sehen Sie auf der Karte nach, welche Äquidistanz sie hat, und
  korrigieren Sie ihn;
- einen Titelblock mit `{event}` und `1:{print_scale} · {contour_interval}` oben links im Kartenausschnitt
  und eine Kontrollstrecke unten rechts;
- ein Layout im Maßstab der Karte. `init` probiert A4 mit 8 mm Passepartout, dann A4 mit breiterem
  Passepartout (in Schritten von 2 mm bis 24 mm), dann A3 und nimmt das erste Blatt, auf dem `check` nichts zu
  melden hat. Für die Lübecker Sprintkarte ist das A3 mit 14 mm. Passt keines, bleibt die Datei bei A4 mit
  8 mm; `init` beginnt seine Meldung dann mit „Warnung:“ und zeigt, was `check` melden wird.

Die Blattsprache, also die Sprache von Datum, Dezimalzeichen und Namensnennung auf dem Blatt, setzt `init` auf
die Programmsprache. Sie steht in `event.lang`.

### Die Projektdatei

Die Datei ist [TOML](https://toml.io/de/). Jede Zeile hat die Form `schlüssel = wert`, Abschnitte stehen in
eckigen Klammern. Ein Schlüssel, den das Programm nicht kennt, ist ein Fehler mit Datei, Zeile und Spalte, damit
ein Tippfehler nicht still übergangen wird. Ein Wert außerhalb seines Bereichs ist ein Fehler mit Zeile und
Spalte des Werts. Pfade gelten relativ zur Projektdatei.

```toml
format = 1

[event]
title = "Stadtsprint Lübeck"
date = 2027-05-15                  # optional, für {date}
organiser = "OL-Verein Musterstadt" # optional, für {organiser}
lang = "de"                        # Blattsprache: de oder en, Standard de

[sources]
map = "luebeck.omap"

[map]
kind = "sprint"                    # sprint, schoolyard oder forest
contour_interval_m = 2.0           # Äquidistanz in Metern, für {contour_interval}

[print]
copyright = """
© OpenStreetMap contributors, ODbL
© GeoBasis-DE/LVermGeo SH/CC BY 4.0 (Quelle verändert)
"""                                # optional, ersetzt die Quellen aus der Karte

[[shared]]                         # gemeinsame Blattelemente, Kapitel 7
id = "title"
kind = "title_block"
lines = ["{event}", "1:{print_scale} · Äquidistanz {contour_interval}"]
anchor = { to = "map_frame", corner = "top_left", offset_mm = [2.0, 2.0] }

[[layouts]]                        # ein oder mehrere Layouts, Kapitel 6
id = "a4-4000"
paper = "A4"
print_scale = 4000
```

| Abschnitt | Schlüssel | Bedeutung |
|---|---|---|
| oben | `format` | immer `1`. Eine andere Zahl lehnt das Programm ab und nennt die Version, die es kennt. |
| `[event]` | `title` | Name der Veranstaltung, für `{event}` |
| | `date` | Datum als `JJJJ-MM-TT` ohne Anführungszeichen, für `{date}` |
| | `organiser` | Ausrichter, für `{organiser}` |
| | `lang` | Blattsprache `de` oder `en`: „15.05.2027“ oder „15 May 2027“, „2,5 m“ oder „2.5 m“, „Erstellt mit“ oder „Created with“ |
| `[sources]` | `map` | Pfad zur OOM-Karte (`.omap`) |
| `[map]` | `kind` | Kartenart: `sprint`, `schoolyard` oder `forest`. Sie bestimmt, wie die Symbole bei einem anderen Druckmaßstab wachsen (Kapitel 9) und wie weit Nordlinien auseinanderliegen. |
| | `contour_interval_m` | Äquidistanz in Metern, größer als 0 und höchstens 100 |
| `[print]` | `copyright` | Quellenangaben, eine je Zeile. Ohne den Schlüssel nimmt das Programm die Kartennotizen. |
| `[[shared]]` | | ein gemeinsames Blattelement; der Abschnitt darf mehrfach vorkommen |
| `[[layouts]]` | | ein Layout; mindestens eines, beliebig viele |

### Prüfen: `check`

```sh
freemapper-print check stadtsprint
```

`check` nimmt die Projektdatei oder ihren Ordner. Es zeichnet jedes Layout im Speicher, schreibt aber keine
Datei, und gibt je Layout eine Zeile mit Papier, Ausrichtung, Maßstab und Faktoren aus, danach die Hinweise.
Zum Schluss steht „1 Layout geprüft, keine Hinweise.“ oder „Layouts mit Hinweisen: 1 von 2.“

Der Exit-Code ist 0, wenn kein Layout etwas zu melden hat, und 1, wenn eines einen Hinweis hat oder sich nicht
zeichnen lässt. Zeilen, die mit „Zur Information:“ beginnen, für ergänzte Nordlinien und einen gewollten
Beschnitt (`map_frame.crop`), zählen nicht. So kann ein Skript `check` vor `export` setzen. Ein Hinweis hält den
Export nicht auf. Manche Hinweise sind gewollt, etwa der Überstand einer Karte, die größer
ist als das Blatt. Kapitel 10 erklärt jeden Hinweis.

### Die PDFs schreiben: `export`

```sh
freemapper-print export stadtsprint
freemapper-print export stadtsprint --layout a4-4000
freemapper-print export stadtsprint --out-dir ~/Desktop/druck
```

`export` schreibt je Layout ein PDF und benennt es nach der Layout-ID, etwa `a4-4000.pdf`. Ohne `--out-dir`
landen die PDFs im Ordner `out` neben der Projektdatei, den das Programm bei Bedarf anlegt. Ein Ordner hinter
`--out-dir` muss schon existieren. `--layout` exportiert nur dieses Layout und lässt sich wiederholen. Ein PDF
gleichen Namens wird überschrieben.

Für jedes Layout gibt `export` dieselben Zeilen aus wie `check`, mit dem Pfad des PDFs in der ersten. Die
Layouts entstehen nacheinander, es liegt immer nur ein Blatt im Speicher. Der Exit-Code ist 0, sobald alle
PDFs geschrieben sind, auch wenn es Hinweise gab.

Vor dem Schreiben prüft das Programm den freien Platz: Es braucht die Größe des PDFs und 50 MB Reserve. Das PDF
entsteht zuerst als `<Name>.pdf.part` und bekommt erst seinen Namen, wenn es vollständig auf dem Datenträger
steht. Ist der Datenträger voll, bleibt ein älteres PDF gleichen Namens unverändert, und es bleibt kein halbes
PDF zurück; ebenso bei jedem anderen Fehler. Die Meldung nennt, wie viel Platz nötig und wie viel frei ist, oder
dass der Datenträger das nicht meldet. Lassen Sie nicht zwei Exporte gleichzeitig in denselben Ordner
schreiben: Sie teilten sich die `.part`-Datei.

## 6. Layouts: Blattformat, Maßstab, Kartenausschnitt

Ein Layout ist eine Vorlage für ein Kartenblatt. Meist hat eine Veranstaltung ein bis fünf: zum Beispiel A4 in
1:4000 für die älteren Klassen und A3 in 1:3000 für die jüngsten. Jedes Layout wird zu einem PDF.

```toml
[[layouts]]
id = "a3-3000"
paper = "A3"
orientation = "landscape"
print_scale = 3000
north_lines = "auto"
hide_parts = []
hide_shared = []
map_frame = { margin_mm = 10.0, border_mm = 0.25 }
```

| Schlüssel | Standard | Bedeutung |
|---|---|---|
| `id` | | Name des Layouts und des PDFs (`a3-3000.pdf`), Pflicht |
| `paper` | `"A4"` | `"A4"` oder `"A3"` |
| `orientation` | `"auto"` | `"portrait"` (hoch), `"landscape"` (quer) oder `"auto"`: die Ausrichtung, in der weniger von der Karte fehlt |
| `print_scale` | Maßstab der Karte | Nenner des Druckmaßstabs, `3000` für 1:3000 |
| `north_lines` | `"auto"` | `"auto"` ergänzt Nordlinien, wenn die Karte keine hat; `"never"` nie |
| `hide_parts` | leer | Kartenteile, die das Blatt nicht zeigt, mit ihrem genauen Namen aus OOM |
| `hide_shared` | leer | IDs gemeinsamer Elemente, die dieses Layout nicht zeigt |
| `elements` | leer | eigene Blattelemente dieses Layouts, Kapitel 7 |
| `map_frame.margin_mm` | 8 | Breite des Passepartouts in mm, auf allen Seiten gleich; kleiner als die halbe kürzere Papierseite |
| `map_frame.border_mm` | 0,25 | Breite der Rahmenlinie in mm, höchstens 20; 0 zeichnet keine |
| `map_frame.centre_map_mm` | Mitte der Karte | Kartenpunkt `[x, y]` in der Mitte des Kartenausschnitts, so wie OOM ihn anzeigt |
| `map_frame.crop` | `false` | `true`: Der Kartenausschnitt beschneidet die Karte mit Absicht; der Überstand ist nur ein Hinweis |

Die Layout-ID wird zum Dateinamen. Sie darf deshalb keines der Zeichen `/ \ : * ? " < > |` enthalten, nicht
mit einem Punkt beginnen oder enden und kein Gerätename von Windows sein (`con`, `nul`, `com1` …). Zwei IDs, die
sich nur in Groß- und Kleinschreibung unterscheiden, lehnt das Programm ab, weil ihre PDFs unter macOS und
Windows dieselbe Datei wären.

### Kartenausschnitt und Passepartout

Der Kartenausschnitt ist das Blatt ohne das Passepartout. Die Karte wird auf ihn beschnitten und nie verkleinert,
denn das würde den Maßstab verfälschen. Ragt sie hinaus, meldet `check`, um wie viele Millimeter (Kapitel 9).
Die Rahmenlinie liegt mittig auf der Kante des Kartenausschnitts: Die eine Hälfte deckt die Schnittkante der
Karte, die andere liegt auf dem Passepartout.

Ohne `centre_map_mm` liegt die Mitte dessen, was gedruckt wird, in der Mitte des Ausschnitts. Mit
`centre_map_mm = [x, y]` liegt dort dieser Punkt der Karte. Die Zahlen sind Millimeter auf der Karte im
Kartenmaßstab, genau so, wie OOM sie in der Statusleiste anzeigt, wenn die Koordinatenanzeige auf
Kartenkoordinaten steht: Zeigen Sie in OOM auf die Stelle und übernehmen Sie beide Zahlen. OOM zeigt y nach oben
(sein Quelltext gibt in `MapWidget::updateCursorposLabel` `x` und `-y` aus); das Programm rechnet das in die
Koordinaten der `.omap` mit y nach unten um. Jede Zahl darf höchstens 2 000 000 mm vom Ursprung der Karte
entfernt sein. Bei einer Karte, die größer ist als das Blatt, legen Sie so den Ausschnitt auf das
Wettkampfgebiet.

Soll das Blatt nur einen Teil der Karte zeigen, setzen Sie dazu `crop = true`
(`map_frame = { centre_map_mm = [120.0, -80.0], crop = true }`). Der Überstand ist dann ein Hinweis zur
Information, und `check` endet trotzdem mit 0.

Wie breit das Passepartout sein muss, hängt vor allem von der Fußzeile ab (Kapitel 7): Sie steht nur dann unter
dem Kartenausschnitt, wenn sie dort ganz Platz hat. Für zwei Zeilen reichen 8 mm, für fünf braucht es etwa
14 mm.

### Druckmaßstab

Ohne `print_scale` druckt das Layout im Maßstab der Karte. Ein anderer Maßstab ändert die Symbolgrößen so, wie
die Norm es vorsieht (Kapitel 9). Eine Sprintkarte lässt sich in 1:3000, 1:4000 und 1:5000 drucken, eine
Waldkarte in 1:5000 bis 1:20000. Blattelemente behalten ihre Größe in Millimetern: Ein Titel in 14 pt ist auf
dem A3-Blatt in 1:3000 genauso groß wie auf dem A4-Blatt in 1:4000.

### Nordlinien

Nordlinien (Symbol 601) gehören zur Karte. Hat die Karte welche, druckt das Programm sie so, wie sie gezeichnet
sind, mit ihren Lücken an kleinen Objekten. Hat sie keine und steht `north_lines` auf `"auto"`, ergänzt das
Programm senkrechte Linien über den ganzen Kartenausschnitt, im Bodenabstand der Kartenart: 120 m bei
Sprintkarten (30 mm in 1:4000, 40 mm in 1:3000), 300 m bei Waldkarten, 37,5 m bei Schulhofkarten. Sie nehmen
das Symbol 601 der Karte und liegen deshalb wie gezeichnete unter Mauern und Zäunen. Ohne Lücken laufen sie
trotzdem durch; die setzt der Kartograf in OOM. Verbirgt ein Layout mit `hide_parts` den Kartenteil, in dem die
Nordlinien der Karte liegen, ergänzt das Programm keine.

### Kartenteile ausblenden

`hide_parts = ["peripheral information"]` blendet den Kartenteil dieses Namens aus. Groß- und Kleinschreibung
zählt. Ein Name, den die Karte nicht hat, gibt einen Hinweis. Den Informationsblock von freemapper (Titel,
Maßstab, Maßstabsleiste) druckt ein Layout ohnehin nie, die Blattelemente treten an seine Stelle.

## 7. Titel, Logos und andere Blattelemente

Blattelemente stehen auf dem Blatt, gehören aber nicht zur Karte. Gemeinsame Elemente stehen unter
`[[shared]]` und erscheinen auf jedem Blatt. Ein Layout blendet einzelne mit `hide_shared` aus und hat unter
`[[layouts.elements]]` eigene. Die IDs der gemeinsamen Elemente und der Elemente eines Layouts müssen
zusammen eindeutig sein. `footer` ist vergeben.

Jedes Element hat:

| Schlüssel | Bedeutung |
|---|---|
| `id` | Name, mit dem Hinweise das Element nennen |
| `kind` | `title_block`, `text`, `logo`, `scale_check_bar`, `copyright_line` oder `credit_line` |
| `anchor.to` | `"page"` (das Blatt) oder `"map_frame"` (der Kartenausschnitt) |
| `anchor.corner` | eine von neun Stellen: `top_left`, `top`, `top_right`, `left`, `centre`, `right`, `bottom_left`, `bottom`, `bottom_right` |
| `anchor.offset_mm` | `[x, y]`, Abstand in mm von dieser Stelle nach innen |

Jeder Abstand liegt zwischen -2000 und 2000 mm. Er zählt von einer linken oder oberen Kante nach rechts oder unten,
von einer rechten oder unteren nach links oder oben, von einer Mitte nach rechts oder unten. Das Element liegt
innen an seiner Stelle: `top_right` mit `[2, 2]` setzt seine rechte obere Ecke 2 mm links und 2 mm unter die rechte
obere Ecke.

Ein Element am Kartenausschnitt liegt auf der Karte und deckt sie ab, das ist gewollt. Ein Element am Blatt,
das in den Kartenausschnitt ragt, bekommt einen Hinweis. Ebenso ein Element, das über den Blattrand ragt.

Andere Schlüssel gelten nur für bestimmte Arten. Steht einer bei der falschen Art, ist das ein Fehler.

### Titelblock und Text

```toml
[[shared]]
id = "title"
kind = "title_block"
lines = ["{event}", "{date} · {organiser}", "1:{print_scale} · Äquidistanz {contour_interval}"]
anchor = { to = "map_frame", corner = "top_left", offset_mm = [2.0, 2.0] }
```

Ein Titelblock setzt die erste Zeile fett in 14 pt (`size_pt` ändert das, bis 200) und die übrigen in 9 pt. Ein
`text` setzt alle Zeilen in 9 pt oder in `size_pt`. Beide stehen auf einem weißen Kasten mit 1 mm Rand; `box_white
= false` lässt ihn weg. Die Schrift ist Arimo, dieselbe wie für Kartentexte.

In den Zeilen setzt das Programm diese Platzhalter ein:

| Platzhalter | wird zu |
|---|---|
| `{event}` | `event.title` |
| `{date}` | `event.date`, auf Deutsch „15.05.2027“, auf Englisch „15 May 2027“ |
| `{organiser}` | `event.organiser` |
| `{print_scale}` | Druckmaßstab des Layouts, ab fünf Stellen mit schmalem Leerzeichen: „4000“, „10 000“ |
| `{map_scale}` | Maßstab der Karte |
| `{contour_interval}` | `map.contour_interval_m` mit Einheit: „2 m“, „2,5 m“ |
| `{layout}` | die Layout-ID |

Ein unbekannter Platzhalter, etwa ein Tippfehler wie `{evnet}`, bleibt so stehen und gibt einen Hinweis.
`{course}` für den Bahnnamen kommt mit den Bahnen.

Die IOF verlangt Maßstab und Äquidistanz auf jedem Blatt. Fehlt in allen Texten eines Layouts
`{print_scale}` oder `{contour_interval}`, oder fehlt `map.contour_interval_m`, meldet `check` das.

### Logo

```toml
[[shared]]
id = "club_logo"
kind = "logo"
file = "logos/verein.svg"
width_mm = 35.0
anchor = { to = "map_frame", corner = "top_right", offset_mm = [2.0, 2.0] }
```

`width_mm` ist die Breite auf dem Papier, höchstens 2000; die Höhe folgt aus dem Seitenverhältnis. Das Programm
nimmt PNG, JPEG und SVG:

- **PNG und JPEG** bleiben Pixelbilder, ein PNG behält seine Transparenz. Für den Druck sollte das Bild etwa
  300 Pixel je Zoll haben, für 35 mm Breite also rund 400 Pixel.
- **SVG** wird zu Flächen und Linien in CMYK und bleibt in jeder Größe scharf. Das ist die beste Wahl, wenn
  das Logo nur aus Flächen, Linien und Verläufen besteht.

Nicht jedes SVG lässt sich drucken. Ein SVG kann viel mehr als Formen: Masken, Filter, Muster, eingebettete
Bilder, Schrift, Stilvorlagen. Manche Dateien nutzen das, um mit wenigen Kilobyte Minuten an Rechenzeit oder
Gigabyte an Speicher zu verlangen. Deshalb nimmt das Programm nur eine feste Liste einfacher Elemente an: `svg`,
`g`, `path`, `rect`, `circle`, `ellipse`, `line`, `polyline`, `polygon`, `defs`, `linearGradient`,
`radialGradient`, `stop`, `use`, `title`, `desc`, `metadata` und `clipPath` (nur in Nutzerkoordinaten). Enthält
ein Logo etwas anderes, lässt das Programm es weg und meldet das. Typische Gründe:

- Text als `<text>`: Wandeln Sie die Schrift in Pfade um (Inkscape: „Pfad › Objekt in Pfad umwandeln“,
  Illustrator: „Schrift › In Pfade umwandeln“).
- Masken, Filter, Schatten, Muster, eingebettete Pixelbilder, Stilvorlagen in `<style>`: Speichern Sie das
  Logo ohne diese Effekte.
- Eine Dokumenttypdeklaration (DTD), die mehr enthält als Kommentare und kurze Text-Entitäten, wie Illustrator
  sie schreibt.
- Zu groß oder zu tief verschachtelt: über 10 MB, über 20 000 Pfadsegmente, über 256 Ebenen.

Die Meldung rät dann, das Logo als einfaches SVG zu exportieren, in Inkscape etwa mit „Speichern unter ›
Normales SVG“. Den Rest des Blatts druckt das Programm trotzdem.

Was das Blatt nicht genau so zeichnen kann, wie das SVG es beschreibt, zeichnet es vereinfacht und meldet es:
ein Verlauf erscheint in seiner ersten Farbe, Transparenz wird deckend, Strichelung wird durchgezogen, und
Beschneidungspfade wirken nicht. Ein PNG oder JPEG über 25 Megapixel oder über 65 535 Pixel Kantenlänge, eine
Datei über 10 MB und eine fehlende oder kaputte Datei werden ausgelassen und gemeldet. Die genauen Grenzen
stehen im [Glossar](../glossary.md) unter „Logo“ und in ADR 0026.

Das Programm liest und dekodiert jede Logodatei einmal je `check` oder `export`, egal wie viele Elemente und
Layouts sie nutzen, und das PDF enthält sie einmal. Ein Blatt trägt höchstens 50 Megapixel an PNG- und
JPEG-Logos und 1 000 000 Einheiten SVG-Arbeit, jedes Logo so oft gezählt, wie es platziert ist. Ein Logo
darüber hinaus fehlt mit einem Hinweis. Zwei Logos der größten erlaubten Größe passen; ein Vereinslogo von
einigen hundert Pixeln braucht fast nichts davon.

### Kontrollstrecke

```toml
[[shared]]
id = "scale_check_bar"
kind = "scale_check_bar"
anchor = { to = "map_frame", corner = "bottom_right", offset_mm = [2.0, 2.0] }
```

Ein schwarz-weißer Balken auf weißem Kasten, beschriftet etwa „200 m = 50 mm“. Seine Länge ist die größte
runde Strecke (5, 10, 20, 50, 100, 200, 250, 500 oder 1000 m), die im Druckmaßstab höchstens 50 mm lang ist:
200 m bei 1:4000, 100 m (33,3 mm) bei 1:3000. Mit einem Lineal am Ausdruck sehen Sie, ob der Drucker skaliert
hat (Kapitel 9).

### Fußzeile: Quellenangaben und Namensnennung

Jedes Blatt hat unten links eine Fußzeile in 5 pt, auf einem weißen Kasten, damit nichts darunter sie verdeckt;
unter dem Kartenausschnitt ist der Kasten weiß auf weißem Papier. Oben stehen die Quellenangaben, eine je Zeile,
aus `print.copyright` oder aus den Kartennotizen. Darunter steht die Namensnennung „Erstellt mit freemapper-print –
https://github.com/HenryJobst/freemapper-print“ (auf englischen Blättern „Created with …“). Die Namensnennung lässt
sich nicht abschalten.

Die Fußzeile steht unter dem Kartenausschnitt, wenn sie dort ganz Platz hat. Sonst steht sie links unten im
Kartenausschnitt, und `check` meldet, dass sie einen Teil der Karte abdeckt. An beiden Stellen nennt `check`
jedes Element, das die Fußzeile abdeckt. Abhilfe: ein breiteres Passepartout (`margin_mm`) oder weniger Zeilen in
`print.copyright`. Mehr als sechs Zeilen Quellenangaben zeigt das Programm nicht: fünf und eine sechste, die den
Rest zählt. Eine Zeile, die breiter ist als der Kartenausschnitt, kürzt es mit „…“, und `check` nennt sie.

Ein Element `copyright_line` oder `credit_line` holt diesen Teil aus der Fußzeile und setzt ihn an seinen eigenen
Anker, in 5 pt oder `size_pt`. Es verschiebt die Zeilen nur; die Namensnennung bleibt auf dem Blatt, auf einem
eigenen weißen Kasten. Ein
`credit_line` braucht mindestens 5 pt. Läge es nicht ganz auf dem Blatt, bleibt die Namensnennung in der
Fußzeile, und `check` meldet das. Die Namensnennung wird nach allem anderen auf dem Blatt gezeichnet, kein
Element kann sie verdecken.

## 9. Drucken: maßstabsgetreu am eigenen Drucker und im Copyshop

Eine Orientierungslaufkarte muss den Maßstab haben, der darauf steht. Das PDF von freemapper-print hat ihn
genau: 1 mm auf dem Papier sind die Strecke im Gelände, die der Druckmaßstab vorgibt. Ob der Ausdruck ihn
behält, entscheidet der Druckdialog.

### Das PDF erzeugen

Für eine Veranstaltung schreibt `export` die PDFs (Kapitel 5). Für einen schnellen Druck der nackten Karte
ohne Projektdatei gibt es `render`:

```sh
freemapper-print render luebeck.omap --out luebeck-4000-a4.pdf --paper a4
```

Ohne `--scale` druckt das Programm im Maßstab der Karte. `--scale 3000` druckt in 1:3000. Zum Drucken wählen Sie A4
oder A3 (`--paper`) und, wenn nötig, die Ausrichtung (`--orientation`). Bei `auto` nimmt das Programm die
Ausrichtung, die weniger von der Karte abschneidet. `render` lässt rundum 5 mm frei und druckt weder Titel noch
Kontrollstrecke. Die Fußzeile druckt es aber: die Quellenangaben aus den Kartennotizen und die Namensnennung in der
Programmsprache, auf einem weißen Kasten unten links im Druckbereich. Unter der Karte ist dafür bei 5 mm Rand kein
Platz. Deckt der Kasten einen Teil der Karte ab, meldet `render` das; ein Projekt mit breiterem Passepartout setzt
die Fußzeile unter die Karte. Zuerst schreibt das Programm eine Zeile mit Papier, Druckmaßstab und Faktoren, danach
folgen Hinweise und eine Erinnerung ans Drucken in Originalgröße.

### In Originalgröße drucken

Öffnen Sie das PDF und wählen Sie im Druckdialog 100 % oder „Tatsächliche Größe“. Schalten Sie „An Seite
anpassen“ und „Verkleinern auf Druckbereich“ aus. Jede andere Einstellung ändert den Maßstab, und die Karte
stimmt nicht mehr. Im Copyshop sagen Sie dasselbe: ohne Skalierung, 100 %, Papier wie im PDF.

### A4 oder A3

Ist die Karte größer als der Kartenausschnitt (bei `render` die Seite ohne 5 mm Rand), schneidet das Programm
sie ab und meldet es, zum Beispiel:

```
Warnung: Die Karte passt nicht auf die Seite. Sie ist 4,0 mm zu hoch für den bedruckbaren Bereich. Versuchen Sie ...
```

Die Zahl sagt, um wie viele Millimeter die Karte hinausragt, oben und unten zusammen. Gemessen wird an dem, was
tatsächlich gedruckt würde, also mit Linienbreiten, Punktsymbolen und Texten. Die Karte wird nicht verkleinert,
denn das würde den Maßstab verfälschen. Sie können ein größeres Papier nehmen (A3), einen Druckmaßstab mit größerer
Zahl oder in einem Layout mit `centre_map_mm` den Ausschnitt wählen und den Schnitt mit `map_frame.crop = true` als
gewollt kennzeichnen (Kapitel 6). Haben Sie die Ausrichtung selbst gewählt, nennt die Meldung auch die andere. Das
PDF entsteht auch bei einem Überstand, damit Sie nachsehen können, was fehlt.

### Wie groß die Symbole sind

Die Vorschrift der Kartenart legt die Symbolgrößen für einen Basismaßstab fest: 1:4000 für Sprintkarten
(ISSprOM), 1:15000 für Waldkarten (ISOM). Druckt man größer, wächst alles im selben Verhältnis, das Gelände
und jedes Symbol. Eine Sprintkarte in 1:3000, wie sie die Norm für die jüngsten Altersklassen empfiehlt, hat
Symbole in 133 %: Ein schmaler Fußweg ist dann 0,36 mm statt 0,27 mm breit. Eine Waldkarte wächst in 1:10000
auf 150 %, in 1:7500 auf 200 % und in 1:5000 auf 300 %. Die Karte braucht dabei entsprechend mehr Papier.

Kleiner als im Basismaßstab sieht die Norm keinen Druck vor. Das Programm druckt ihn trotzdem, etwa eine
Sprintkarte in 1:5000. Die Symbole behalten dann ihre Größe aus 1:4000, nur das Gelände schrumpft. Das Programm
warnt dabei:

```
Warnung: 1:5000 ist kein normgerechter Maßstab für Sprintkarten. Die Karte wird verkleinert, die Symbole
behalten ihre Größe im Maßstab 1:4000. Dieser Druck taugt nur fürs Training.
```

Das Programm kennt dafür die Kartenart (Sprintkarte, Schulhofkarte, Waldkarte). Bei `render` erkennt es sie aus
`freemapper.toml` oder aus der Kartendatei, sonst geben Sie sie mit `--kind` an. Im Projekt steht sie in
`map.kind`.

### Den Maßstab mit dem Lineal prüfen

Ein Layout zeigt eine Kontrollstrecke, wenn es ein Element `scale_check_bar` hat (Kapitel 7). `init` legt eines
an, und `check` meldet ein Layout ohne. Legen Sie das Lineal an den Balken: Bei
„200 m = 50 mm“ muss er genau 50 mm lang sein. Bei `render` fehlt die Kontrollstrecke. Messen Sie dann zwischen
zwei Punkten, deren Abstand Sie kennen, etwa zwei Gebäudeecken oder den Enden eines geraden Straßenstücks. Für
100 m muss das Lineal diese Werte zeigen:

| Druckmaßstab | 100 m auf der Karte |
|---|---|
| 1:4000 | 25 mm |
| 1:3000 | 33,3 mm |

Weicht das Maß ab, hat der Drucker skaliert. Prüfen Sie dann den Druckdialog. Die Beispiele
[examples/luebeck-scales](../../examples/luebeck-scales/README.md) und
[examples/luebeck-competition-layouts](../../examples/luebeck-competition-layouts/README.md) enthalten dieselbe
Karte in 1:4000 auf A4 und 1:3000 auf A3.

### Hinweise zum Druck der Karte

Diese Hinweise kommen von `render`, `check` und `export`:

- „… nicht gezeichnet, weil das Symbol unbekannt oder ausgeblendet ist“: Objekte, deren Symbol die Kartendatei
  ausblendet oder nicht kennt. Sie fehlen im PDF.
- „… vereinfacht oder gekürzt“: Ein Objekt braucht mehr Zeichenarbeit, als das Programm je Objekt zulässt. Es
  erscheint vereinfacht oder abgeschnitten.
- „… nicht gezeichnet, weil die Seite ihre Zeichengrenze erreicht hat“: Die ganze Seite braucht mehr
  Zeichenarbeit, als das Programm je Seite zulässt. Das passiert nur bei kaputten oder absichtlich überladenen
  Dateien.
- „… nicht gezeichnet, weil das Programm ihren Objekttyp nicht kennt“: Die Datei enthält Objekte einer Art,
  die OOM nicht schreibt. Sie fehlen im PDF.
- „… ohne Flächenmuster gezeichnet, dessen Abstand unter 0,05 mm liegt oder keine Zahl ist“: Die Kartendatei
  verlangt ein Flächenmuster, das dichter als 0,05 mm ist oder ungültige Zahlen hat. Das Programm lässt das
  Muster weg und druckt den Rest der Fläche.
- „Hinweis: … Die Kartenart kommt stattdessen aus der Kartendatei.“: Die Datei `freemapper.toml` neben der
  Karte ist kaputt. Das Programm bestimmt die Kartenart dann aus der Kartendatei.
- „Die Schrift Arimo ersetzt …“: Das PDF enthält nur die Schrift Arimo. Andere Schriften der Karte ersetzt das
  Programm durch sie.
- „Die eingebettete Schrift kann diese Zeichen nicht darstellen …“: Ein Zeichen der Karte fehlt in Arimo und
  wird nicht gedruckt.
- „Die Fußzeile mit Quellenangaben und Namensnennung steht auf einem weißen Kasten … und deckt einen Teil der
  Karte ab“ (nur `render`): Die Fußzeile liegt unten links auf der Karte. Ein Projekt setzt sie unter die Karte.
- Den Informationsblock von freemapper (Titel, Maßstab, Maßstabsleiste, Namensnennung) druckt `render` nur mit
  `--show-sheet-objects`. Bei einem anderen Druckmaßstab skaliert er mit der Karte, ein darin gedruckter
  Maßstab kann also falsch sein. Die Nordlinien gehören zur Karte und stehen immer im PDF.

## 10. Meldungen und was zu tun ist

Dieses Kapitel erklärt die Hinweise zu den Blättern eines Layouts. Die Hinweise zum Druck der Karte selbst
stehen in Kapitel 9. Keiner hält den Export auf. Jede Zeile, die mit „Hinweis:“ beginnt, lässt `check` mit
Exit-Code 1 enden; eine Zeile, die mit „Zur Information:“ beginnt, nicht.

| Hinweis (gekürzt) | Bedeutung und Abhilfe |
|---|---|
| „Die Karte passt nicht auf die Seite. Sie ist … zu hoch/zu breit …“ | Die Karte ist größer als der Kartenausschnitt und wird abgeschnitten. A3, ein anderer Maßstab, ein schmaleres Passepartout oder `centre_map_mm` für den gewünschten Ausschnitt. Ist der Schnitt gewollt, `map_frame.crop = true` setzen. |
| „Zur Information: Die Karte ist … für den Kartenausschnitt und wird dort mit Absicht beschnitten“ | Nur zur Information: Das Layout hat `map_frame.crop = true`. |
| „Das Passepartout ist zu schmal für die Fußzeile …“ | Die Fußzeile steht auf weißem Kasten in der Karte. `margin_mm` erhöhen, `print.copyright` kürzen oder die Zeilen mit `copyright_line`/`credit_line` woanders hinsetzen. |
| „Die Fußzeile … unten links deckt einen Teil des Elements … ab“ | Die Fußzeile mit ihrem weißen Kasten überdeckt ein Element unten links, unter oder im Kartenausschnitt. Das Element versetzen oder die Zeilen der Fußzeile mit einem Element `copyright_line` oder `credit_line` verschieben. Der Hinweis nennt, was die Fußzeile enthält: Quellenangaben, Namensnennung oder beides. |
| „Das Element … läge mit der Namensnennung nicht ganz auf dem Blatt“ | Das Element `credit_line` würde über den Blattrand ragen, deshalb bleibt die Namensnennung in der Fußzeile. Anker, Abstand und `size_pt` prüfen. |
| „Die Fußzeile … ragt über den Blattrand“ | Die Fußzeile reicht über das Blatt hinaus. Passepartout und Quellenangaben prüfen. |
| „Das Element … ragt über den Blattrand“ | Anker oder Abstand des Elements prüfen; ein Logo eventuell schmaler machen. |
| „Das Element … hängt am Blatt und deckt einen Teil des Kartenausschnitts ab“ | Ein Element am Blatt (`to = "page"`) liegt über der Karte. Ans Passepartout rücken oder an den Kartenausschnitt hängen, wenn es auf der Karte stehen soll. |
| „Das Papier ist größer als A3“ | Nur bei Papierformaten über A3; derzeit kennt das Programm nur A4 und A3. |
| „Im Layout … fehlt ein Text mit {print_scale} …“ | Maßstab oder Äquidistanz stehen auf keinem Blattelement, oder `map.contour_interval_m` fehlt. Einen Titelblock oder Text mit dem Platzhalter ergänzen, den Wert eintragen. |
| „Layout … hat keine Kontrollstrecke“ | Das Layout zeigt kein Element `scale_check_bar`. Eines ergänzen (gemeinsam oder im Layout), damit ein Lineal am Ausdruck zeigt, ob der Drucker skaliert hat. |
| „Zur Information: Die Karte hat keine Nordlinien, deshalb wurden … Linien ergänzt“ | Nur zur Information, siehe Kapitel 6. `north_lines = "never"` schaltet das ab. |
| „… weder Nordlinien noch ein brauchbares Liniensymbol 601 …“ | Die Karte hat kein Symbol für Nordlinien. Das Programm zieht schwarze Linien von 0,14 mm über die Karte; sie liegen über allem. Besser: Nordlinien in OOM zeichnen. |
| „Layout … zeigt … von … Quellenangaben“ | Mehr als sechs Quellenzeilen. In `print.copyright` zusammenfassen. |
| „Die Quellenangabe ‚…‘ ist breiter als der Kartenausschnitt und wird mit ‚…‘ gekürzt“ | Die Zeile in den Kartennotizen oder in `print.copyright` kürzen oder auf zwei Zeilen verteilen. |
| „Das Logo … fehlt auf dem Blatt: …“ | Datei fehlt, ist kaputt, zu groß oder ein SVG mit nicht erlaubten Teilen, oder das Blatt trägt schon 50 Megapixel an Logos oder hat sein SVG-Budget aufgebraucht; der Grund steht dahinter. Siehe „Logo“ in Kapitel 7. |
| „Das Logo … wird vereinfacht gezeichnet: …“ | Verlauf, Transparenz, Strichelung oder Beschneidung im SVG. Das Logo erscheint, aber nicht ganz wie im Grafikprogramm. |
| „Der Text … enthält {…}. Das ist kein Platzhalter …“ | Tippfehler im Platzhalter. Die Liste steht in Kapitel 7. |
| „Layout … blendet den Kartenteil … aus, die Karte hat aber keinen Teil dieses Namens“ | Namen in `hide_parts` genau so schreiben wie in OOM, mit Groß- und Kleinschreibung. |

Fehler in der Projektdatei halten `check` und `export` an. Die Meldung nennt Datei, Zeile, Spalte und den
Schlüssel, etwa ``freemapper-print.toml:24:1: field `layouts[1].elements[0].width_mm`: a logo needs a `width_mm`
``. Fehlt der Schlüssel, nennt sie die Zeile der Tabelle, in die er gehört. Diese Texte kommen aus der Bibliothek
und sind vorerst Englisch.
