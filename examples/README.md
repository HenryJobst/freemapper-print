# Beispielprojekte

[English](README.en.md)

Hier liegen fertige Veranstaltungen, gebaut auf Karten aus den
[Beispielen von freemapper](https://github.com/HenryJobst/freemapper/tree/main/examples). Jedes Beispiel hat
die Projektdatei, die Eingabedateien, die erzeugten PDFs und ein Vorschaubild. Für jede Art von Veranstaltung
gibt es ein Beispiel, sobald das Programm sie drucken kann:

| Beispiel | Zeigt | Ab |
|---|---|---|
| [`luebeck-scales`](luebeck-scales/) | dieselbe Sprintkarte in 1:4000 auf A4 und 1:3000 auf A3: genauer Maßstab, Symbole wachsen nach Norm auf 133 % | M1 |
| [`luebeck-competition-layouts`](luebeck-competition-layouts/) | ein Wettkampf mit zwei Layouts, A4 1:4000 für die älteren Klassen, A3 1:3000 für die jüngsten, mit Passepartout, Titelblock, zwei Logos, Kontrollstrecke und Fußzeile, gebaut nur mit `init`, `check` und `export` | M2 |
| `schoolyard` | eine Schulhofkarte in 1:1000 auf A4; wartet auf eine Schulhofkarte in den Beispielen von freemapper (Version 0.8.0 hat keine) | geplant |
| [`luebeck-competition-courses`](luebeck-competition-courses/) | derselbe Wettkampf mit drei Bahnen aus Purple Pen, Postenbeschreibung, Postennummern (eine mit `numbers accept` korrigiert), Sperrgebiet, verbotener Querung und markierter Strecke zum Ziel, dazu dieselben Bahnen als IOF-XML 3.0 ohne Beschreibung | M3 |
| [`berlin-viktoriapark-training`](berlin-viktoriapark-training/) | ein Vereinstraining im Berliner Viktoriapark: eine Bahn aus Purple Pen auf A4 in 1:4000 mit Postenbeschreibung, auf der Karte aus Version 0.8.0 von freemapper | M3 |
| `luebeck-training-window` | Fenster-OL mit einer Abdeckungsvariante | M4 |
| `luebeck-training-corridor` | Korridor-OL mit einer Abdeckungsvariante | M4 |

Alle Karten stammen aus freemapper 0.8.0 (Commit `a2b41b4`). Als Sprintkarten liegen sie auf der hellbraunen
Grundfläche (ADR 0044), und die Parks sind gelb.

Später kommen Postensetzer-Blätter, Mannschafts-OL und eine Waldkarte dazu.
