# Handbuch zu freemapper-print

[English](en.md) · [README](../../README.md)

Dieses Handbuch entsteht mit der ersten nutzbaren Version; bisher ist nur Kapitel 9 geschrieben. Am Ende soll
es Schritt für Schritt von der fertigen OOM-Karte und der Purple-Pen-Datei zu den gedruckten Kartenblättern
führen, auch ohne Erfahrung mit der Kommandozeile.

Geplante Kapitel:

1. Was freemapper-print macht
2. Installation
3. Mit einem KI-Assistenten
4. Ohne Assistent: Editor und Kommandozeile
5. Eine Veranstaltung anlegen: Karte und Bahnen
6. Layouts: Blattformat, Maßstab, Kartenausschnitt
7. Titel, Logos und andere Blattelemente
8. Abdeckungen für Fenster- und Korridor-OL
9. Drucken: maßstabsgetreu am eigenen Drucker und im Copyshop
10. Meldungen und was zu tun ist

## 9. Drucken: maßstabsgetreu am eigenen Drucker und im Copyshop

Eine Orientierungslaufkarte muss den Maßstab haben, der darauf steht. Das PDF von freemapper-print hat ihn
genau: 1 mm auf dem Papier sind die Strecke im Gelände, die der Druckmaßstab vorgibt. Ob der Ausdruck ihn behält,
entscheidet der Druckdialog.

### Das PDF erzeugen

```sh
freemapper-print render luebeck.omap --out luebeck-4000-a4.pdf --paper a4
```

Ohne `--scale` druckt das Programm im Maßstab der Karte. `--scale 3000` druckt in 1:3000. Zum Drucken wählen
Sie A4 oder A3 (`--paper`) und, wenn nötig, die Ausrichtung (`--orientation`). Bei `auto` nimmt das Programm
die Ausrichtung, die weniger von der Karte abschneidet. Zuerst schreibt das Programm eine Zeile mit Papier,
Druckmaßstab und Faktoren, danach folgen Hinweise und eine Erinnerung ans Drucken in Originalgröße. Das
Programm gibt seine Meldungen auf Englisch aus; die Zitate in diesem Kapitel bleiben deshalb Englisch.

### In Originalgröße drucken

Öffnen Sie das PDF und wählen Sie im Druckdialog 100 % oder "Tatsächliche Größe". Schalten Sie "An Seite
anpassen" und "Verkleinern auf Druckbereich" aus. Jede andere Einstellung ändert den Maßstab, und die Karte
stimmt nicht mehr. Im Copyshop sagen Sie dasselbe: ohne Skalierung, 100 %, Papier wie im PDF.

### A4 oder A3

Das Programm lässt rundum 5 mm Rand frei, weil viele Drucker nicht bis an die Papierkante drucken. Ist die
Karte größer als die Seite, schneidet das Programm sie ab und meldet es, zum Beispiel:

```
Warning: the map does not fit on the page. It is 3.9 mm too tall for the printable area. Try A3 paper, or ...
```

Die Zahl sagt, um wie viele Millimeter die Karte über den bedruckbaren Bereich hinausragt. Gemessen wird an
dem, was tatsächlich gedruckt würde, also mit Linienbreiten, Punktsymbolen und Texten. Die Karte wird nicht
verkleinert, denn das würde den Maßstab verfälschen. Sie können ein größeres Papier nehmen (A3) oder einen
Druckmaßstab mit größerer Zahl. Haben Sie die Ausrichtung selbst gewählt, nennt die Meldung auch die andere
Ausrichtung. Das PDF entsteht auch bei einem Überstand, damit Sie nachsehen können, was fehlt.

### Wie groß die Symbole sind

Die Vorschrift der Kartenart legt die Symbolgrößen für einen Basismaßstab fest: 1:4000 für Sprintkarten
(ISSprOM), 1:15000 für Waldkarten (ISOM). Druckt man größer, wächst alles im selben Verhältnis, das Gelände und
jedes Symbol. Eine Sprintkarte in 1:3000, wie sie die Norm für die jüngsten Altersklassen empfiehlt, hat
Symbole in 133 %: Ein schmaler Fußweg ist dann 0,36 mm statt 0,27 mm breit. Eine Waldkarte wächst in 1:10000
auf 150 %, in 1:7500 auf 200 % und in 1:5000 auf 300 %. Die Karte braucht dabei entsprechend mehr Papier.

Kleiner als im Basismaßstab sieht die Norm keinen Druck vor. Das Programm druckt ihn trotzdem, etwa eine
Sprintkarte in 1:5000. Die Symbole behalten dann ihre Größe aus 1:4000, nur das Gelände schrumpft. Das Programm
warnt dabei:

```
Warning: 1:5000 is not a standard scale for sprint maps. The map is reduced and the symbols keep their 1:4000
size. Use this print for training only.
```

Das Programm kennt dafür die Kartenart (Sprintkarte, Schulhofkarte, Waldkarte). Erkennt es sie weder aus
`freemapper.toml` noch aus der Kartendatei, geben Sie sie mit `--kind` an.

### Den Maßstab mit dem Lineal prüfen

Das Beispiel [examples/luebeck-scales](../../examples/luebeck-scales/README.md) enthält dieselbe Karte in
1:4000 auf A4 und 1:3000 auf A3. Drucken Sie beide in Originalgröße. Messen Sie dann zwischen zwei Punkten,
deren Abstand Sie kennen, etwa zwei Gebäudeecken oder die Enden eines geraden Straßenstücks. Für 100 m muss das
Lineal diese Werte zeigen:

| Druckmaßstab | 100 m auf der Karte |
|---|---|
| 1:4000 | 25 mm |
| 1:3000 | 33,3 mm |

Weicht das Maß ab, hat der Drucker skaliert. Prüfen Sie dann den Druckdialog. Die Karte zeichnet keine eigene
Kontrollstrecke; sie folgt mit M2 und den Layouts.

### Hinweise des Programms

- "not drawn because the symbol is unknown or hidden": Objekte, deren Symbol die Kartendatei ausblendet oder
  nicht kennt. Sie fehlen im PDF.
- "simplified or cut short": Ein Objekt braucht mehr Zeichenbefehle, als das Programm je Objekt zulässt. Es
  erscheint vereinfacht oder abgeschnitten.
- "not drawn because the page reached its drawing limit": Die ganze Seite braucht mehr Zeichenbefehle, als das
  Programm je Seite zulässt. Das passiert nur bei kaputten oder absichtlich überladenen Dateien.
- "the font Arimo replaces ...": Das PDF enthält nur die Schrift Arimo. Andere Schriften der Karte ersetzt das
  Programm durch sie.
- "the embedded font has no glyph for these characters, so they are missing in the PDF": Ein Zeichen der Karte
  fehlt in Arimo und wird nicht gedruckt.
- Den Informationsblock von freemapper (Titel, Maßstab, Maßstabsleiste, Namensnennung) druckt das Programm nur
  mit `--show-sheet-objects`. Bei einem anderen Druckmaßstab skaliert er mit der Karte, ein darin gedruckter
  Maßstab kann also falsch sein. Die Nordlinien gehören zur Karte und stehen immer im PDF.

Die übrigen Kapitel folgen mit späteren Versionen.
