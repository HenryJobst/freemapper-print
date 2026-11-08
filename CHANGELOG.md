# Änderungen

[English version](CHANGELOG.en.md) · [Handbuch](docs/manual/de.md)

Hier stehen alle wichtigen Änderungen an freemapper-print, die neueste zuerst. Das Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/).

## [Unveröffentlicht]

### Hinzugefügt

- Projektgerüst nach dem Vorbild von freemapper: Rust-Workspace mit Bibliothek und Programm `freemapper-print`
  (bisher nur `--version` und `--help`), CI für Linux, macOS und Windows, Release-Workflow.
- Design, Glossar und die ersten 15 Architekturentscheidungen.
- `freemapper-print render <Karte> --out <PDF>` druckt eine OOM-Karte als PDF im exakten Maßstab. Papier (A4, A3),
  Ausrichtung und Druckmaßstab sind wählbar, `--kind` gibt die Kartenart vor, `--show-sheet-objects` zeichnet
  Nordlinien und Informationsblock mit. Das Programm meldet, wie viele Millimeter der Karte nicht auf die Seite
  passen, außerdem nicht gezeichnete, vereinfachte oder abgeschnittene Objekte, ersetzte Schriften und fehlende
  Zeichen. Zum Schluss erinnert es ans Drucken in Originalgröße. Text aus der Karte erscheint nur mit
  maskierten Steuerzeichen in der Ausgabe.
