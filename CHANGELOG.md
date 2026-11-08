# Änderungen

[English version](CHANGELOG.en.md) · [Handbuch](docs/manual/de.md)

Hier stehen alle wichtigen Änderungen an freemapper-print, die neueste zuerst. Das Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/).

## [Unveröffentlicht]

### Hinzugefügt

- Projektgerüst nach dem Vorbild von freemapper: Rust-Workspace mit Bibliothek und Programm `freemapper-print`
  (bisher nur `--version` und `--help`), CI für Linux, macOS und Windows, Release-Workflow.
- Design, Glossar und die ersten 15 Architekturentscheidungen.
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
