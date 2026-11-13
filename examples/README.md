# Beispielprojekte

[English](README.en.md)

Hier liegen sieben fertige Veranstaltungen auf zwei Karten von Remo Madella, gezeichnet für die MOO Challenge
Berlin 2025: „Gleisdreieck“ und „Rummelsburger Bucht“. Jedes Beispiel hat die Projektdatei, die Eingabedateien,
die erzeugten PDFs und ein Vorschaubild. Die README jedes Beispiels erklärt, was auf dem Blatt zu sehen ist und
wie es entstand.

| Beispiel | Zeigt | Karte |
|---|---|---|
| [`gleisdreieck-scales`](gleisdreieck-scales/) | derselbe Kartenausschnitt in 1:4000 auf A4 und 1:3000 auf A3: genauer Maßstab, Symbole wachsen nach Norm auf 133 % | Gleisdreieck |
| [`rummelsburg-competition-layouts`](rummelsburg-competition-layouts/) | ein Wettkampf mit zwei Layouts, A4 1:4000 für die älteren Klassen, A3 1:3000 für die jüngsten, mit Passepartout, Titelblock, zwei Logos, Maßstabsbalken und Fußzeile, gebaut nur mit `init`, `check` und `export` | Rummelsburger Bucht |
| [`rummelsburg-competition-courses`](rummelsburg-competition-courses/) | derselbe Wettkampf mit drei Bahnen (Kurz, Mittel, Lang) aus einer Purple-Pen-Datei, Postenbeschreibung, Postennummern (eine mit `numbers accept` korrigiert), Sperrgebiet, verbotener Querung und markierter Strecke zum Ziel, dazu dieselben Bahnen als IOF-XML 3.0 ohne Beschreibung | Rummelsburger Bucht |
| [`gleisdreieck-training`](gleisdreieck-training/) | ein Vereinstraining: zwei Bahnen auf A4 in 1:4000, je ein Blatt mit Titelblock, Vereinszeichen, Maßstabsbalken und Postenbeschreibung | Gleisdreieck |
| [`gleisdreieck-training-window`](gleisdreieck-training-window/) | ein Fenster-OL: Karte nur in runden Fenstern um Start, Posten und Ziel, gebaut mit `masks window`, ein Fenster größer als die anderen | Gleisdreieck |
| [`gleisdreieck-training-corridor`](gleisdreieck-training-corridor/) | ein Korridor-OL: Karte nur in einem 12 mm breiten Korridor, die Linie von Hand gewählt und mit `masks corridor` gebaut | Gleisdreieck |
| [`rummelsburg-mcp-session`](rummelsburg-mcp-session/) | eine aufgezeichnete Sitzung mit dem MCP-Server: Ein KI-Agent macht aus einer Trainingsbahn einen Fenster-OL mit Fenstern von 12 mm, prüft das Blatt und schreibt das PDF, in sechs Werkzeugaufrufen | Rummelsburger Bucht |

## Karten und Bahnen

Remo Madella hat beide Karten am 12. November 2026 für die freemapper-Suite freigegeben und der Verwendung als
Beispielkarten in diesem öffentlichen Projekt zugestimmt. Das ist eine Erlaubnis für die Suite, keine offene
Lizenz. Die `ATTRIBUTION.txt` in jedem Beispiel hat den Wortlaut, deutsch und englisch. Wer die Karten anders
nutzen will, fragt Remo Madella.

Die Bahnen sind erfunden, ebenso Veranstaltungen, Daten und Vereine. Sie wurden mit freemapper-courseplan 0.14.1
gemacht und liegen als Purple-Pen-Datei oder als IOF-XML bei. Die Vereinszeichen sind für die Beispiele gezeichnet
und stehen unter CC0.

Jedes Beispiel bringt seine Karte mit, damit der Ordner für sich läuft. Git speichert gleiche Dateien nur einmal.
Die Beispiele wiegen zusammen etwa 35 MB. Ein PDF der Karte Gleisdreieck hat 2,1 bis 3,2 MB, eines der Karte
Rummelsburg 1,1 MB. Seit freemapper-core v0.22.0 lässt ein PDF weg, was der Rahmen nicht zeigen kann. Ein Blatt mit
`crop = true` enthält also nur den sichtbaren Teil der Karte.

## Die alten Beispiele

Die früheren Beispiele auf Lübecker und Berliner Karten aus freemapper 0.8.0 sind nach
[`tests/fixtures/examples/`](../tests/fixtures/examples/README.md) umgezogen. Sie sind Testdaten, und die Tests
vergleichen ihre PDFs Byte für Byte.

Blätter für Postensetzer, Mannschafts-OL, eine Schulhofkarte und eine Waldkarte folgen später.
