# Änderungen

[English version](CHANGELOG.en.md) · [Handbuch](docs/manual/de.md)

Hier stehen alle wichtigen Änderungen an freemapper-print, die neueste zuerst. Das Format folgt
[Keep a Changelog](https://keepachangelog.com/de/1.1.0/).

## [Unveröffentlicht]

### Hinzugefügt

- Eine Projektdatei je Veranstaltung, `freemapper-print.toml`, mit drei Befehlen. `init` legt sie für eine
  Karte an: ein Layout im Maßstab der Karte auf A4 oder A3 (das erste, auf dem die Karte ohne Hinweise Platz
  hat), einen Titelblock, eine Kontrollstrecke und einen Vorschlag für die Äquidistanz zum Bestätigen (2 m für
  Sprint-, 5 m für Waldkarten); es sagt, woher die Kartenart kommt. `check`
  zeichnet jedes Layout, ohne etwas zu schreiben, listet die Hinweise und endet mit Exit-Code 1, wenn es welche
  gibt. Zeilen, die nur informieren (ergänzte Nordlinien, gewollter Beschnitt), beginnen mit „Zur Information:“
  und zählen nicht. `export` schreibt je Layout ein PDF, benannt nach der Layout-ID, in den Ordner `out` oder nach
  `--out-dir`; `--layout` wählt einzelne Layouts. Ein Tippfehler in der Projektdatei ist ein Fehler mit Zeile
  und Spalte und wird nicht still übergangen, ebenso ein Wert außerhalb seines Bereichs (etwa eine Schrift über
  200 pt oder ein Passepartout von halber Papierbreite).
- Layouts: beliebig viele je Veranstaltung, jedes mit Papier (A4, A3), Ausrichtung, Druckmaßstab, Passepartout,
  Rahmenlinie und auf Wunsch einem Kartenpunkt in der Mitte des Kartenausschnitts, so angegeben, wie die
  Statusleiste von OOM ihn zeigt. Mit `map_frame.crop = true` beschneidet ein Layout die Karte mit Absicht: Der
  Überstand ist dann nur ein Hinweis, und `check` geht durch. Ein Layout kann Kartenteile und gemeinsame
  Blattelemente ausblenden und eigene Elemente haben.
- Blattelemente, jedes an einer von neun Stellen des Blatts oder des Kartenausschnitts verankert: Titelblock und
  Texte mit Platzhaltern (`{event}`, `{date}`, `{organiser}`, `{print_scale}`, `{map_scale}`,
  `{contour_interval}`, `{layout}`), Logos und eine Kontrollstrecke. Fehlt auf einem Blatt der Maßstab oder
  die Äquidistanz, gibt es einen Hinweis, denn die IOF verlangt beide; ebenso bei einem Layout ohne
  Kontrollstrecke.
- Fußzeile auf jedem Blatt: die Quellenangaben der Karte (oder aus `print.copyright`), eine je Zeile, und
  darunter die Namensnennung „Erstellt mit freemapper-print“ mit der Adresse des Projekts. Die Namensnennung
  lässt sich nicht abschalten: Ein Element `credit_line` verschiebt sie nur, wenn es mindestens 5 pt groß ist und
  ganz auf dem Blatt liegt, und sie liegt auf weißem Kasten über allem anderen. Auch die ganze Fußzeile steht
  auf weißem Kasten, ein dunkles Element darunter kann sie nicht verdecken. Sie steht unter dem
  Kartenausschnitt, wenn sie dort ganz Platz hat, sonst in der Karte, und `check` meldet das und jedes Element,
  das sie abdeckt, und nennt, was die Fußzeile enthält. Höchstens sechs Quellenzeilen erscheinen, eine zu lange wird gekürzt und gemeldet.
- Logos als PNG, JPEG oder SVG, so breit wie `width_mm`. PNG und JPEG behalten ihre Transparenz. Ein SVG wird
  in CMYK gedruckt und bleibt in jeder Größe scharf; Verläufe erscheinen in ihrer ersten Farbe, Transparenz
  deckend, und das Programm meldet jede Vereinfachung. Zum Schutz vor Dateien, die Minuten rechnen oder
  Gigabyte Speicher verlangen, nimmt das Programm nur einfache SVGs an: Formen, Verläufe, `use` und einfache
  Beschneidungspfade. Ein Logo mit Text, Masken, Filtern, Mustern, eingebetteten Bildern oder Stilvorlagen, mit
  mehrdeutigen Attributen oder einer aufwendigen DTD wird ausgelassen, und der Hinweis rät, es als einfaches SVG
  zu exportieren. Ebenso eine fehlende, kaputte oder zu große Datei (über 10 MB, 25 Megapixel oder 20 000
  Pfadsegmente). Der Rest des Blatts erscheint trotzdem. Jede Logodatei wird je `check` oder `export` einmal
  dekodiert und je PDF einmal eingebettet; ein Blatt trägt höchstens 50 Megapixel an Logos und 1 000 000
  Einheiten SVG-Arbeit.
- Kontrollstrecke: ein Balken in runder Länge (5 m bis 1000 m, höchstens 50 mm im Druck), beschriftet etwa
  „200 m = 50 mm“. Am Ausdruck lässt sich nachmessen, ob der Drucker skaliert hat.
- Nordlinien für Karten ohne welche: Hat die Karte kein Symbol 601 und das Layout `north_lines = "auto"`,
  ergänzt das Programm senkrechte Linien im Bodenabstand der Kartenart (Sprint 120 m, Wald 300 m, Schulhof
  37,5 m, noch ungeprüft), mit dem Symbol 601 der Karte, sodass sie wie gezeichnete unter Mauern und Zäunen
  liegen. Lücken an kleinen Objekten setzt weiter der Kartograf in OOM. Ohne Nordlinien-Symbol gibt es schwarze
  Linien von 0,14 mm und einen Hinweis.
- Meldungen und Hilfe auf Deutsch und Englisch. Die Sprache kommt aus `--lang de|en`, sonst aus
  `FREEMAPPER_LANG` und der Spracheinstellung des Systems. Auch die Hinweise zu Logos kommen in der
  Programmsprache. Fehlermeldungen zur Projektdatei, Einzelheiten vom System oder einem Bilddecoder in einem
  Hinweis (etwa der Name eines SVG-Elements) und die Überschriften der Hilfe bleiben vorerst Englisch. Die Sprache auf dem Blatt (Datum, Dezimalzeichen,
  Namensnennung) stellt `event.lang` ein.
- Sicheres Schreiben: Vor jedem PDF prüft das Programm, ob die Größe des PDFs und 50 MB Reserve frei sind. Das
  PDF entsteht als `.part`-Datei und bekommt seinen Namen erst, wenn es vollständig geschrieben ist. Ein voller
  Datenträger hinterlässt kein halbes PDF, und die Meldung nennt den nötigen und den freien Platz. Auch ein
  anderer Fehler oder ein Absturz beim Schreiben lässt keine `.part`-Datei zurück.
- Beispiel `examples/luebeck-competition-layouts`: ein Stadtsprint mit einem A4-Blatt in 1:4000 für die
  älteren Klassen und einem A3-Blatt in 1:3000 für die jüngsten, mit Titelblock, zwei erfundenen Logos,
  Kontrollstrecke und Fußzeile, gebaut nur mit `init`, `check` und `export`.

### Geändert

- `render` setzt die Fußzeile nun auch auf sein PDF: die Quellenangaben aus den Kartennotizen und die
  Namensnennung „Erstellt mit freemapper-print“, in der Programmsprache, auf einem weißen Kasten unten links.
  So trägt eine Karte aus OpenStreetMap-Daten die Quellenangabe, die ihre Lizenz verlangt. Die Karte selbst
  verschiebt sich nicht; die PDFs von `luebeck-scales` sind neu erzeugt.
- Die Lübecker Karte in den Tests und im Beispiel `luebeck-scales` stammt jetzt aus freemapper 0.5.0. Sie hat
  die IOF-Farben nach Rev. 4 (etwa Braun 25/75/100/0 statt 0/56/100/18) und Gebäude in Schwarz 50 % statt 60 %.
  Ihre Nordlinien liegen nun im Kartenteil „default part“; „peripheral information“ enthält nur noch den
  Informationsblock. Die PDFs von `luebeck-scales` sind neu erzeugt, Maße und Überstand (4,0 mm auf A4) sind
  gleich geblieben.

### Behoben

- Eine Fehlermeldung wiederholt die Meldung des Systems nicht mehr, wie in „No such file or directory (os
  error 2): No such file or directory (os error 2)“.
- Der PDF-Schreiber lehnt eine Seite mit einer unendlichen oder ungültigen Zahl ab, statt ein ungültiges PDF zu
  schreiben.

## [0.1.0] - 2026-11-08

### Hinzugefügt

- Projektgerüst nach dem Vorbild von freemapper: Rust-Workspace mit Bibliothek und Programm `freemapper-print`
  (bisher nur `--version` und `--help`), CI für Linux, macOS und Windows, Release-Workflow.
- Design, Glossar und Architekturentscheidungen.
- `freemapper-print render <Karte> --out <PDF>` druckt eine OOM-Karte als PDF im exakten Maßstab. Papier (A4,
  A3), Ausrichtung und Druckmaßstab sind wählbar, `--kind` gibt die Kartenart vor, `--show-sheet-objects`
  druckt den Informationsblock von freemapper mit. Die Nordlinien gehören zur Karte und stehen immer im PDF.
  Das Programm meldet, wie viele Millimeter der Karte nicht auf die Seite passen, außerdem nicht gezeichnete,
  vereinfachte oder abgeschnittene Objekte, ersetzte Schriften und fehlende Zeichen. Zum Schluss erinnert es
  ans Drucken in Originalgröße. Steuerzeichen aus der Kartendatei gibt das Programm nur maskiert aus,
  Fehlermeldungen eingeschlossen.
- Die Karte aus OOM (`.omap`) wird gelesen, mit allen Kartenteilen.
  Hintergrundbilder (Vorlagen) bleiben unberücksichtigt.
- Flächenmuster, spitze Linienenden, Strichpunktsymbole, Textrahmen und gedrehter Text werden wie in OOM
  gezeichnet. Kartentexte stehen in der eingebetteten Schrift Arimo, auch mit Umlauten und anderen
  Unicode-Zeichen.
- Die Symbolgrößen folgen den IOF-Normen. Bei einem größeren Druckmaßstab wächst alles im selben Verhältnis:
  eine Sprintkarte in 1:3000 auf 133 %, eine Waldkarte in 1:10000 auf 150 %, in 1:7500 auf 200 % und in 1:5000
  auf 300 %. Einen kleineren Maßstab als den Basismaßstab (Sprint 1:5000, Wald 1:20000) druckt das Programm
  mit Symbolen in Normgröße und warnt, dass der Druck nicht der Norm entspricht und nur fürs Training taugt.
- Beispiel `examples/luebeck-scales`: dieselbe Sprintkarte in 1:4000 auf A4 und in 1:3000 auf A3, mit
  Anleitung zum Nachmessen.
- Ein Test vergleicht die Ausgabe mit dem PDF-Export von OOM.
- Die Release-Archive enthalten die Lizenz der Schrift Arimo als `FONT-LICENSE.txt`.
- Texte in einem Textfeld brechen wie in OOM an der Feldbreite um und richten sich nach den Feldkanten aus.
  Der Absatzabstand der Textsignatur gilt nach jedem Absatz.
- Der Überstand misst, was tatsächlich gedruckt würde, also mit Linienbreiten, Punktsymbolen und Texten. Die
  Seite wird auch danach zentriert.
- Kaputte oder überladene Kartendateien halten das Programm nicht mehr auf: Zu tief verschachteltes XML ist
  ein Fehler, und eine Seite hat ein Zeichenlimit. Das Programm meldet Objekte, die es deshalb nicht oder nur
  zum Teil zeichnet, außerdem Objekte unbekannter Art, verworfene Flächenmuster und eine kaputte
  `freemapper.toml`. Das Limit zählt jeden Punkt, den eine Seite behält, und jeden Umriss, den das PDF
  schreibt. Die schlimmste gemessene präparierte Datei, 500 000 kurze Linien in 134 KB, brauchte etwa 200 MB
  Speicher.
- Eine Textfeldgröße oder Drehung, die keine endliche Zahl ist, und eine negative Textfeldgröße sind
  Lesefehler. Alte Textfelder, deren Koordinaten als `<coord>`-Elemente gespeichert sind, behalten ihr Feld.
