# Änderungen

[English version](CHANGELOG.en.md) · [Handbuch](docs/manual/de.md)

Hier stehen alle wichtigen Änderungen an freemapper-print, die neueste zuerst. Das Format folgt
[Keep a Changelog](https://keepachangelog.com/de/1.1.0/).

## [Unveröffentlicht]

### Hinzugefügt

- Die Programmtexte gibt es auf Deutsch und Englisch: Hilfe, Zusammenfassung, Hinweise, Warnungen und Fehler.
  Die Sprache kommt aus `--lang de|en` (vor oder nach dem Befehl), dann aus `FREEMAPPER_LANG`, `LC_ALL`,
  `LC_MESSAGES` und `LANG`. Die Texte stehen in Fluent-Dateien unter `crates/freemapper-print-cli/i18n/`.
  Fehlermeldungen der Bibliothek und die Überschriften von clap bleiben vorerst Englisch.

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
