# freemapper-print

[English version](README.en.md) · [Handbuch](docs/manual/de.md) · [Glossar](docs/glossary.md) · [Änderungen](CHANGELOG.md)

<!-- claude-cost:start -->
[![KI-Aufwand (Claude API): 832 $](https://img.shields.io/badge/KI--Aufwand%20%28Claude%20API%29-832%20%24-8A2BE2)](docs/ai-cost/history.csv)
<!-- claude-cost:end -->
[Was die Zahl bedeutet](docs/ai-cost/README.md)

<p align="center"><a href="examples/README.md"><img src="docs/manual/img/sheet-elements.jpg" alt="Ein A4-Blatt in 1:4000 auf der Karte Gleisdreieck mit Titelblock links oben, Kompass-Logo links unten, Kontrollstrecke rechts unten und der Namensnennung unter dem Rahmen" width="760"></a><br><sub>Ein Blatt auf der Karte „Gleisdreieck“ von Remo Madella (MOO Challenge Berlin 2025): A4 quer in 1:4000 mit Titelblock, Logo, Kontrollstrecke und Namensnennung. Ein Klick öffnet die <a href="examples/README.md">Beispiele</a>.</sub></p>

freemapper-print macht aus einer fertigen OL-Karte und den Bahnen aus einem Bahnlegungsprogramm die
druckfertigen Kartenblätter, ein Blatt je Bahn. Es ist das Gegenstück zu
[freemapper](https://github.com/HenryJobst/freemapper): freemapper liefert den Ausgangsstand der Karte,
gezeichnet wird in OpenOrienteering Mapper (OOM), und freemapper-print bringt die fertige Karte aufs Papier.

Das kann es schon:

- eine fertige Karte aus OOM (`.omap`) im exakten Druckmaßstab als Vektor-PDF ausgeben, auf A4, A3 oder einer frei gewählten Seitengröße, auch in
  einem anderen Maßstab mit den Symbolgrößen, die die Norm vorschreibt (zum Beispiel 1:3000 für die jüngsten
  Klassen mit Symbolen auf 133 %);
- eine Projektdatei je Veranstaltung mit beliebig vielen Layouts: angelegt mit `init`, geprüft mit `check`, als
  PDFs geschrieben mit `export`;
- auf jedem Blatt Passepartout, Rahmenlinie und eine Fußzeile mit Quellenangaben und Namensnennung; als Elemente,
  die man selbst platziert, Titelblock und Texte mit Platzhaltern, Vereins- und Sponsorenlogos (PNG, JPEG, SVG)
  und eine Kontrollstrecke zum Nachmessen;
- Nordlinien immer mitdrucken, auch ergänzen, wenn die Karte keine hat;
- die Grundfläche der Sprintkarten von freemapper mitdrucken, immer und ohne Schalter (am besten in Farbe);
- Bahnen aus Purple Pen (`.ppen`) und aus jedem Programm, das IOF-XML 3.0 schreibt (Condes, OCAD-Bahnlegung),
  in der Projektdatei verknüpfen und bei jedem Lauf neu lesen;
- ein Blatt je Bahn, mit dem Bahnaufdruck in Violett, aufgeteilt in unteres und oberes Violett, und der
  Postenbeschreibung aus Purple-Pen-Dateien (IOF-XML kennt keine Beschreibungen);
- Postennummern dort, wo die Bahndatei sie hinsetzt, oder vom Programm so platziert, dass sie Strecken, Kreise und
  die Punkt- und Liniensymbole der Karte frei lassen; `check` schlägt einen besseren Platz vor, `numbers accept`
  schreibt ihn ins Projekt;
- `init --courses` legt ein Projekt aus einer Bahndatei an; `export` schreibt ein PDF je Bahn, mit `--course` nur
  gewählte Bahnen, mit `--single-file` ein PDF mit allen Bahnen;
- die Bahnsymbole in der Größe, die die Norm für den Druckmaßstab vorgibt, etwa Postenkreise von 7,5 mm auf einer
  Waldkarte in 1:10000;
- prüfen, ob die Bahn zur Karte passt: Das Programm liest Purple Pen mit der y-Achse nach oben und erkennt sie bei
  IOF-XML, prüft den Kartenmaßstab, vergleicht die Posten mit Breite und Länge und hält bei einer zweifelhaften
  Einpassung an;
- Abdeckungen für Fenster- und Korridor-OL: Eine Abdeckung verbirgt die Karte auf dem Blatt einer Bahn bis auf
  Fenster oder einen Korridor. Bahnaufdruck, Postenbeschreibung, Titel und Fußzeile bleiben überall stehen, auch
  die Strecken zwischen den Fenstern;
- `masks window` legt runde Fenster um die Posten einer Bahn, mit einem Radius in Millimetern auf dem Papier und
  auf Wunsch einem eigenen Radius je Posten. Die Fenster überlappen sich nie: Zwischen zwei Fenstern bleiben
  mindestens 3 mm Papier, und wo Posten nah beieinander liegen, werden die Fenster kleiner. `masks corridor` macht aus einer Linie, die Sie vorgeben, einen
  Korridor mit einstellbarer Breite. Einen automatischen Korridor gibt es nicht, denn kein Regelwerk sagt, wo er
  verläuft;
- Abdeckungen auch von Hand in die Projektdatei schreiben; das White-out aus Purple Pen druckt das Programm als
  Abdeckung der Bahnen, für die es gilt;
- `check` warnt vor Posten, Pflichtstrecken, Sperrgebieten und anderem Bahnaufdruck, die unter einer Abdeckung
  liegen;
- einen MCP-Server (`freemapper-print mcp`) mit dreizehn Werkzeugen, über den ein KI-Agent wie Claude Code ein Projekt
  liest, prüft, Abdeckungen, Postennummern, Layouts und Blattelemente einträgt und die PDFs schreibt;
- Meldungen und Hilfe auf Deutsch und Englisch; ein volles Laufwerk hinterlässt kein halbes PDF, und das Programm
  schreibt nie durch einen Link am Ziel.
- einen Editor im Browser (`freemapper-print edit`, ab Version 0.7.0): Blattvorschau, genau so gezeichnet wie im PDF, Elemente und Postennummern
  ziehen, Layouts und Elemente in Formularen ändern, Fenster- und Korridormasken anlegen, prüfen und PDFs
  schreiben; jede Änderung steht sofort in der Projektdatei, mit Rückgängig und Wiederholen;
- `freemapper-print update` installiert das neueste signierte Release über das Programm, `--rollback` kehrt zurück, und eine tägliche Prüfung meldet eine neuere Version (ab Version 0.8.0; siehe Installation).

Geplant ist:

- später OCAD-Karten.

Das Programm braucht kein OCAD und läuft auf macOS, Linux und Windows.

## Teil der freemapper-Suite

freemapper-print ist eines von mehreren Programmen rund um dieselben Karten. Der Code, den sie gemeinsam brauchen,
liegt in [freemapper-core](https://github.com/HenryJobst/freemapper-core), einer Bibliothek aus kleinen
Rust-Crates, die jedes Programm in einer festen Version einbindet, statt den Code zu kopieren. freemapper-print
legt alle auf einen Tag fest, jetzt v0.22.0. Es nutzt daraus `freemapper-text` (Programmtexte),
`freemapper-rules` (Regelprofile), `freemapper-fs` (sicheres Schreiben von Dateien), `freemapper-omap`
(Kartenmodell und Kartenleser) und `freemapper-course` (Bahnen, Bahndateien und die Lage der Postennummern). Der
Renderer (`freemapper-render`) liegt seit core v0.9.0 auch dort, der Seitenaufbau seit v0.13.0, der Bahnaufdruck seit
v0.15.0 und die Druckregeln samt Maßstabstabellen (`freemapper-printrules`, wie stark Symbole bei einem anderen
Druckmaßstab wachsen) seit v0.16.0. Auch der Editor-Server (`freemapper-web`) und die Selbstaktualisierung
(`freemapper-update`) kommen aus core.

## Beispiele

Die Bilder stammen aus dem [Handbuch](docs/manual/de.md) und zeigen zwei Sprintkarten von Remo Madella, Gleisdreieck
und Rummelsburg (MOO Challenge Berlin 2025), die hier mit seiner Erlaubnis stehen. Die Bahnen auf Gleisdreieck sind
mit freemapper-courseplan 0.14.1 gemacht, Datum und Verein auf den Blättern sind erfunden. Ein Klick auf ein Bild
öffnet das Kapitel, das es erklärt.

<a href="docs/manual/de.md#fenster-um-die-posten-masks-window"><img src="docs/manual/img/mask-window.jpg" width="440"
alt="Fenster-OL auf der Karte Gleisdreieck, A4 hochkant in 1:4000: die Karte nur in zehn getrennten runden Fenstern um Start, Posten und Ziel, dazwischen weißes Papier mit den violetten Strecken"></a>

**Fenster-OL** (Gleisdreieck, Remo Madella): `masks window --course A --assign` mit den Voreinstellungen legt zehn
Fenster von 15 mm Radius auf dem Papier um Start, Posten und Ziel. Zwischen zwei Fenstern bleiben mindestens 3 mm
weißes Papier; wo zwei Posten nah beieinander liegen, werden die Fenster kleiner. Hier liegt kein Postenpaar so nah.
Mehr im Kapitel [Fenster um die Posten](docs/manual/de.md#fenster-um-die-posten-masks-window).

<a href="docs/manual/de.md#ein-korridor-entlang-einer-linie-masks-corridor"><img src="docs/manual/img/mask-corridor.jpg" width="640"
alt="Korridor-OL auf der Karte Gleisdreieck: ein schmaler Streifen Karte entlang der Strecken durch alle fünf Posten, sonst weißes Papier, oben rechts die Postenbeschreibung"></a>

**Korridor-OL** (Gleisdreieck, Remo Madella): Bahn B mit fünf Posten (1,4 km) in einem Korridor von 12 mm Breite
entlang ihrer Strecken. Zwei Strecken kreuzen sich, deshalb entsteht der Korridor aus zwei Stücken. Mehr im Kapitel
[Ein Korridor entlang einer Linie](docs/manual/de.md#ein-korridor-entlang-einer-linie-masks-corridor).

<a href="docs/manual/de.md#wie-groß-die-symbole-sind"><img src="docs/manual/img/scale-comparison.jpg" width="640"
alt="Dieselbe Stelle der Karte Gleisdreieck in 1:4000 links und in 1:3000 rechts; rechts sind Symbole und Linien größer"></a>

**Zwei Maßstäbe** (Gleisdreieck, Remo Madella): dieselbe Stelle in 1:4000 und in 1:3000, je 80 mm × 60 mm in
Originalgröße. Bei 1:3000 wachsen alle Symbole nach ISSprOM auf 133 %. Mehr im Kapitel
[Wie groß die Symbole sind](docs/manual/de.md#wie-groß-die-symbole-sind).

<a href="docs/manual/de.md#das-pdf-erzeugen"><img src="docs/manual/img/render-map.jpg" width="640"
alt="Die ganze Karte Rummelsburg auf einer A3-Seite quer in 1:4000, in der Mitte, mit der Namensnennung in kleiner Schrift unten links"></a>

**Nur die Karte** (Rummelsburg, Remo Madella): `render` schreibt die ganze Karte als PDF auf eine A3-Seite in 1:4000,
ohne Projektdatei. Mehr im Kapitel [Das PDF erzeugen](docs/manual/de.md#das-pdf-erzeugen).

<a href="docs/manual/de.md#12-der-editor-im-browser"><img src="docs/manual/img/editor-window-mask.png" width="640"
alt="Der Editor im Browser: links der Projektbaum, in der Mitte die Blattvorschau mit der Karte Gleisdreieck in runden Fenstern, rechts die Eigenschaften der Abdeckung window-A"></a>

**Der Editor** (Gleisdreieck, Remo Madella): die Blattvorschau ist genau das spätere PDF. Hier steht die Fenstermaske
gerade im Baum. Jede Änderung steht sofort in der Projektdatei. Mehr im Kapitel
[Der Editor im Browser](docs/manual/de.md#12-der-editor-im-browser).

Die vollständigen Beispielprojekte mit Projektdatei, Eingabedateien, PDFs und Vorschaubild liegen in
[`examples/`](examples/README.md): [`gleisdreieck-scales`](examples/gleisdreieck-scales/README.md),
[`rummelsburg-competition-layouts`](examples/rummelsburg-competition-layouts/README.md),
[`rummelsburg-competition-courses`](examples/rummelsburg-competition-courses/README.md),
[`gleisdreieck-training`](examples/gleisdreieck-training/README.md),
[`gleisdreieck-training-window`](examples/gleisdreieck-training-window/README.md),
[`gleisdreieck-training-corridor`](examples/gleisdreieck-training-corridor/README.md) und
[`rummelsburg-mcp-session`](examples/rummelsburg-mcp-session/README.md).

## Status

Die aktuelle Version ist [v0.8.0](https://github.com/HenryJobst/freemapper-print/releases/tag/v0.8.0) vom 12. November 2026 (was neu ist, steht in den [Änderungen](CHANGELOG.md)). Sie druckt eine fertige
OOM-Karte maßstabsgetreu (`freemapper-print render`) und macht aus einer Projektdatei fertige Blätter mit
Layouts, Titelblock, Logos und Kontrollstrecke (`init`, `check`, `export`). Bahnen, Bahnaufdruck, Postenbeschreibung und
Postennummern (M3) gehören dazu. Der Meilenstein M4, Abdeckungen für Fenster- und Korridor-OL, gehört
dazu. Der Meilenstein M5, der MCP-Server für KI-Agenten, gehört seit dieser Version dazu. Der Editor im Browser (M6, `freemapper-print edit`) gehört seit dieser Version dazu. Die erste Version richtet sich an Vereinstrainings in Stadt und
Park: Sprintkarten auf A4, gedruckt am eigenen Drucker oder im Copyshop. Mehr steht im
[Design](docs/superpowers/specs/2026-11-08-freemapper-print-design.md), im [Glossar](docs/glossary.md) und in den
[Architekturentscheidungen](docs/adr/README.md).

Seit v0.8.0 hat `main` Änderungen, die noch nicht veröffentlicht sind, darunter den Abstand zwischen Fenstern
(Fenster überlappen sich nicht und halten 3 mm Abstand) und die neuen Bilder im Handbuch. Sie stehen in den
[Änderungen](CHANGELOG.md) unter „Unveröffentlicht“.

## Installation

freemapper-print ist ein einzelnes Programm ohne Installer:

1. Auf der [Release-Seite](https://github.com/HenryJobst/freemapper-print/releases/latest) das Archiv für das
   eigene System herunterladen:
   - Windows: `freemapper-print-…-windows-x86_64.zip`
   - macOS mit Apple-Chip (M1 und neuer): `freemapper-print-…-macos-arm64.tar.gz`
   - macOS mit Intel-Prozessor: `freemapper-print-…-macos-x86_64.tar.gz`
   - Linux: `freemapper-print-…-linux-x86_64.tar.gz` (ARM-Rechner: `linux-arm64`)
2. Das Archiv entpacken. Der Ordner enthält `freemapper-print` bzw. `freemapper-print.exe`.
3. Beim ersten Start warnt das System, weil das Programm nicht signiert ist. Unter Windows „Weitere
   Informationen“ und dann „Trotzdem ausführen“ wählen. Unter macOS in „Systemeinstellungen › Datenschutz &
   Sicherheit“ auf „Trotzdem öffnen“ klicken, oder im Terminal im entpackten Ordner
   `xattr -d com.apple.quarantine freemapper-print` ausführen. Unter Linux reicht notfalls
   `chmod +x freemapper-print`.
4. Im Terminal im entpackten Ordner prüfen: `./freemapper-print --version` (Windows:
   `.\freemapper-print.exe --version`).

Ein KI-Agent, der Befehle ausführen kann (Claude Code, Codex), übernimmt Herunterladen, Prüfen und Anmelden auch selbst: Sie sagen ihm „Lies AGENTS.md von github.com/HenryJobst/freemapper-print und richte freemapper-print ein.“ Das Archiv enthält neben dem Programm die READMEs, die Lizenz, das Handbuch und die `AGENTS.md`.

### Aktualisieren

Ab Version 0.8.0 aktualisiert sich ein Release-Build selbst: `freemapper-print update` installiert nach Rückfrage das neueste Release an Ort und Stelle, prüft vorher dessen minisign-Signatur und hebt die alte Version als `freemapper-print.previous` auf; `freemapper-print update --rollback` kehrt zurück. Die tägliche Prüfung nach `init`, `check`, `export` und `render` sowie beim Start von `mcp` und `edit` meldet in einer Zeile, wenn es eine neuere Version gibt. Die Prüfung fragt höchstens einmal am Tag bei GitHub das Aktualisierungsmanifest und dessen Signatur ab, mit einem User-Agent, der Programm, Version und Repository nennt, und sendet keine Daten über Sie oder Ihr Projekt. `freemapper-print update --mode off` oder `FREEMAPPER_PRINT_UPDATE=off` schaltet sie ab. Ein selbst gebautes Programm sagt stattdessen, wie man es aktualisiert. Kapitel 2 des [Handbuchs](docs/manual/de.md#2-installation) nennt die Schalter und Ordner.

## Selbst bauen (für Entwickler)

```sh
git clone https://github.com/HenryJobst/freemapper-print
cd freemapper-print
cargo build --release
```

`rust-toolchain.toml` legt die Rust-Version fest; `rustup` installiert sie beim ersten Bauen. Vor jedem Commit
prüft [lefthook](https://github.com/evilmartians/lefthook) Formatierung und clippy (`lefthook install`).

freemapper-print hängt von [freemapper-core](https://github.com/HenryJobst/freemapper-core) ab, in einer festen
Version (ein Git-Tag). Zum normalen Bauen genügt das, Cargo holt die Crates selbst. Wer gleichzeitig an
freemapper-core arbeitet, legt es neben freemapper-print ab (`../freemapper-core`) und trägt in der `Cargo.toml`
vorübergehend den lokalen Ordner ein:

```toml
[patch."https://github.com/HenryJobst/freemapper-core"]
freemapper-text = { path = "../freemapper-core/crates/freemapper-text" }
freemapper-rules = { path = "../freemapper-core/crates/freemapper-rules" }
freemapper-fs = { path = "../freemapper-core/crates/freemapper-fs" }
```

Dieser Eintrag gilt nur zum Entwickeln. Auf `main` steht nie ein `[patch]`. Die Einzelheiten stehen in der README
von freemapper-core.

## Benutzung

### Eine Veranstaltung

```sh
freemapper-print init stadtsprint --map rummelsburg.omap --title "Stadtsprint Rummelsburg"
freemapper-print check stadtsprint
freemapper-print export stadtsprint
```

`init` legt im Ordner `stadtsprint` die Projektdatei `freemapper-print.toml` an: ein Layout im Maßstab der
Karte auf A4 oder A3, einen Titelblock, eine Kontrollstrecke und einen Vorschlag für die Äquidistanz, den Sie an
der Karte prüfen. In der Datei ergänzen Sie weitere Layouts, Logos und Texte. `check` zeichnet jedes Layout,
ohne etwas zu schreiben, und listet die Hinweise: Karte zu groß für den Kartenausschnitt, Passepartout zu
schmal für die Fußzeile, Logo nicht lesbar und so weiter. `export` schreibt je Layout ein PDF in den Ordner
`out` neben der Projektdatei, benannt nach der Layout-ID. Das [Handbuch](docs/manual/de.md#5-eine-veranstaltung-anlegen)
beschreibt die Projektdatei Schlüssel für Schlüssel.

### Bahnen

```sh
freemapper-print init stadtsprint --map rummelsburg.omap --title "Stadtsprint Rummelsburg" --courses courses.ppen
freemapper-print check stadtsprint
freemapper-print numbers accept stadtsprint --course A --control 49 --sheet
freemapper-print export stadtsprint
```

`--courses` nimmt eine `.ppen`-Datei oder IOF-XML 3.0 (`.xml`). `init` liest sie zuerst und passt sie an die Karte
an, eine Datei, die nicht zur Karte gehört, stoppt `init` also vor dem Schreiben. Danach legt es die Layouts an,
eine Zuordnung für jede Bahn und, bei einer `.ppen`, einen Kasten für die Postenbeschreibung. Die Projektdatei
speichert den Pfad zur Bahndatei, und jedes `check` und `export` liest sie neu: Eine Änderung in Purple Pen
braucht kein neues `init`.

`check` geht jede Bahn durch und listet die Hinweise: Die Bahn passt nicht zur Karte, ein Posten liegt außerhalb
des Ausschnitts, ein Blattelement deckt einen Posten ab, eine Postennummer liegt auf einer Strecke oder einem
Kartensymbol und so weiter. Bei einer Nummer im Konflikt schlägt der Hinweis einen Platz vor. `numbers accept`
übernimmt den Vorschlag für eine Bahn (`--sheet`) oder für alle Bahnen auf ihrem Layout (`--layout-wide`) und hängt
ihn als Tabelle `[[numbers]]` an die Projektdatei an; alles davor bleibt unverändert. Mit `--direction` und `--gap`
setzen Sie einen eigenen Platz.

`export` schreibt je Bahn ein PDF in den Ordner `out`, benannt nach der Bahn. `--course <NAME>` (wiederholbar)
exportiert nur diese Bahnen, `--single-file` schreibt alle Bahnen in ein PDF mit einer Seite je Bahn, und
`--layout` exportiert ein Layout ohne Bahnen. Das [Handbuch](docs/manual/de.md#8-bahnen) geht die Schlüssel der
Projektdatei, die Einpassungsfehler und die Hinweise durch.

### Fenster- und Korridor-OL

```sh
freemapper-print masks window stadtsprint --course A --radius 15 --assign
freemapper-print masks corridor stadtsprint --points "-92,2.3;-60,4.4;-28,-2.5;-15.5,58.5" --width 12 --assign --course B
freemapper-print check stadtsprint
```

`masks window` legt um jeden Posten der Bahn A ein rundes Fenster, auch um Start und Ziel, und `--assign` druckt
die Bahn damit. Der Radius gilt in Millimetern auf dem Papier; `--controls` wählt Posten aus, `--radius-for 35=20`
gibt einem Posten ein größeres Fenster. Zwischen zwei Fenstern bleiben mindestens 3 mm Papier (`--gap`); wo zwei
Posten dafür zu nah beieinander liegen, werden beide Fenster kleiner, und der Befehl nennt sie. `masks corridor` legt einen Korridor von 12 mm Breite entlang der Punkte,
in Karten-mm mit der y-Achse nach oben, wie die Statusleiste von OOM sie zeigt. Beide hängen eine Tabelle
`[[masks]]` an die Projektdatei an; alles davor bleibt unverändert. `check` meldet danach Posten und
Bahnaufdruck, die unter der Abdeckung liegen. Das [Handbuch](docs/manual/de.md#11-abdeckungen-für-fenster--und-korridor-ol)
beschreibt die Abdeckungen, ihre Grenzen und das White-out aus Purple Pen.

### Nur die Karte

```sh
freemapper-print render rummelsburg.omap --out rummelsburg-3000-a3.pdf --paper a3 --scale 3000
```

Das schreibt die Sprintkarte als PDF auf eine A3-Seite im Maßstab 1:3000, ohne Titel und Kontrollstrecke. Die
wichtigsten Optionen:

- `--paper a4|a3` wählt das Papier (Standard A4), `--orientation auto|portrait|landscape` die Ausrichtung. Bei
  `auto` nimmt das Programm die Ausrichtung, die weniger von der Karte abschneidet.
- `--scale <Zahl>` setzt den Druckmaßstab, zum Beispiel 4000 für 1:4000. Ohne die Option gilt der Maßstab der
  Karte. Bei einem größeren Maßstab wachsen die Symbole so, wie es die Norm vorschreibt: eine Sprintkarte in
  1:3000 auf 133 %, eine Waldkarte in 1:10000 auf 150 %. Einen kleineren Maßstab als den der Norm (Sprint
  1:5000) druckt das Programm mit Symbolen in Normgröße und warnt, dass der Druck nur fürs Training taugt.
- `--kind sprint|schoolyard|forest` gibt die Kartenart vor, falls das Programm sie weder aus `freemapper.toml`
  noch aus der Kartendatei erkennt.
- `--show-sheet-objects` druckt auch den Informationsblock von freemapper: Titel, Maßstab, Maßstabsleiste und
  Namensnennung. Die Nordlinien gehören zur Karte und stehen immer im PDF.
- `--lang de|en` wählt die Sprache der Meldungen; ohne die Option entscheiden `FREEMAPPER_LANG` und
  die Spracheinstellung des Systems.

Passt die Karte nicht auf die Seite, schreibt das Programm das PDF trotzdem und meldet, wie viele Millimeter
fehlen. Es nennt auch Objekte, die es nicht oder nur vereinfacht gezeichnet hat.

Die Grundfläche einer Sprintkarte von freemapper (Kartenteil `base`) gehört zur Karte und steht immer im PDF. Für
den Überstand zählt sie nicht, denn sie reicht über die Karte hinaus. Solche Karten am besten farbig drucken:
In Graustufen ist ihr helles Braun kaum vom Papier zu unterscheiden.

Drucken Sie das PDF in Originalgröße (100 %, „Tatsächliche Größe“) und schalten Sie „An Seite anpassen“
aus. Sonst stimmt der Maßstab nicht. Das Handbuch erklärt das Drucken im Kapitel
[Drucken](docs/manual/de.md#9-drucken-maßstabsgetreu-am-eigenen-drucker-und-im-copyshop).

### Der Editor im Browser

```sh
freemapper-print edit meine-veranstaltung
```

Öffnet das Projekt in Ihrem Browser. Das Programm gibt eine Adresse auf 127.0.0.1 mit einem zufälligen
Kennzeichen aus; mit `--no-browser` öffnet es den Browser nicht, mit `--port` legt man den Port fest. Das Blatt
auf dem Bildschirm ist genau so gezeichnet wie im PDF. Sie verschieben Elemente mit der Maus oder den Pfeiltasten,
ziehen Postennummern oder setzen sie in einem Formular, ändern Layouts und Elemente in Formularen, legen Fenster- und Korridormasken an, prüfen und exportieren. Es gibt keinen
Speichern-Knopf: Jede Änderung steht sofort in der Projektdatei. Strg+C beendet den Editor. Das Handbuch erklärt
alles im Kapitel [Der Editor im Browser](docs/manual/de.md#12-der-editor-im-browser).

## Mit einem KI-Agenten

`freemapper-print mcp` startet einen MCP-Server auf Standardein- und -ausgabe. Ein KI-Agent wie Claude Code ruft
darüber dieselben Schritte auf wie die Kommandozeile: `project_info`, `check_project` und `list_courses` lesen,
`init_project`, `masks_window`, `masks_corridor`, `numbers_accept`, `set_layout`, `set_element` und `assign_mask`
schreiben in die Projektdatei, `export_project` schreibt die PDFs. Für Claude Code genügt eine Datei `.mcp.json`
im Ordner, in dem Sie Claude Code starten:

```json
{
  "mcpServers": {
    "freemapper-print": {
      "command": "/home/user/bin/freemapper-print",
      "args": ["--lang", "de", "mcp"]
    }
  }
}
```

`command` ist der absolute Pfad zum entpackten Programm. Auch in den Aufrufen gelten nur absolute Pfade, denn der
Server kennt den Arbeitsordner des Agenten nicht. Die Werkzeuge, die schreiben, kommen sich nicht in die Quere,
und die Kommentare der Projektdatei bleiben bis auf wenige Fälle stehen, die das Handbuch nennt. Bricht der Agent einen langen Export ab, endet
er nach dem Blatt, an dem er gerade zeichnet. Das [Handbuch](docs/manual/de.md#3-mit-einem-ki-agenten) beschreibt jedes Werkzeug, das Beispiel
[`rummelsburg-mcp-session`](examples/rummelsburg-mcp-session/README.md) zeigt eine echte Sitzung: Fenster-OL aus einer
Trainingsbahn, geprüft und als PDF geschrieben.

Für KI-Agenten gibt es eine [AGENTS.md](AGENTS.md) (englisch), die auch in jedem Release-Archiv liegt: Sie erklärt einem Agenten, wie er das Programm installiert und aktualisiert und über MCP oder die Kommandozeile bedient.

## Lizenz und Namensnennung

freemapper-print steht unter der [GPL-3.0-or-later](LICENSE), wie freemapper. Es übernimmt Code aus freemapper
und darf Code aus OOM übernehmen (ADR 0002).

Das Programm bettet die Schrift Arimo ein (Arial-kompatibel, SIL Open Font License 1.1). Ihre Lizenz liegt in
den Release-Archiven als `FONT-LICENSE.txt`.
