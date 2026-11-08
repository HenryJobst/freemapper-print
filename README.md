# freemapper-print

[English version](README.en.md) · [Handbuch](docs/manual/de.md) · [Glossar](docs/glossary.md) · [Änderungen](CHANGELOG.md)

freemapper-print macht aus einer fertigen OL-Karte und den Bahnen aus einem Bahnlegungsprogramm die
druckfertigen Kartenblätter, ein Blatt je Bahn. Es ist das Gegenstück zu
[freemapper](https://github.com/HenryJobst/freemapper): freemapper liefert den Ausgangsstand der Karte,
gezeichnet wird in OpenOrienteering Mapper (OOM), und freemapper-print bringt die fertige Karte aufs Papier.

Geplant ist:

- die Karte aus OOM (`.omap`) im exakten Druckmaßstab als Vektor-PDF, später auch OCAD-Karten;
- Bahnen aus Purple Pen (`.ppen`) und aus jedem Programm, das IOF-XML 3.0 schreibt (Purple Pen, Condes,
  OCAD-Bahnlegung), mit Bahnaufdruck und Postenbeschreibung;
- Passepartout, Titel, Nordlinien, Vereins- und Sponsorenlogos, eine Kontrollstrecke zum Nachmessen;
- Abdeckungen für Fenster- und Korridor-OL in Varianten, die man einzelnen Bahnen zuordnet;
- mehrere Layouts je Veranstaltung, etwa 1:4000 für die Älteren und 1:5000 für die Jüngeren, wobei die Symbole
  ihre vorgeschriebene Größe behalten;
- Bedienung über einen Editor im Browser, über die Kommandozeile oder über einen KI-Assistenten (MCP).

Das Programm braucht kein OCAD und läuft auf macOS, Linux und Windows.

## Status

freemapper-print druckt seit Meilenstein M1 eine fertige OOM-Karte im exakten Maßstab als Vektor-PDF
(`freemapper-print render`). Layouts mit Titel und Logos folgen in M2, Bahnen aus Purple Pen in M3, Abdeckungen
für Fenster- und Korridor-OL in M4. Alles Weitere steht im
[Design](docs/superpowers/specs/2026-11-08-freemapper-print-design.md), im [Glossar](docs/glossary.md) und in den
[Architekturentscheidungen](docs/adr/README.md). Die erste Version richtet sich an Vereinstrainings in Stadt und
Park: Sprintkarten auf A4, gedruckt am eigenen Drucker oder im Copyshop.

## Selbst bauen (für Entwickler)

```sh
git clone https://github.com/HenryJobst/freemapper-print
cd freemapper-print
cargo build --release
```

`rust-toolchain.toml` legt die Rust-Version fest; `rustup` installiert sie beim ersten Bauen. Vor jedem Commit
prüft [lefthook](https://github.com/evilmartians/lefthook) Formatierung und clippy (`lefthook install`).

## Benutzung

```sh
freemapper-print render luebeck.omap --out luebeck-5000-a4.pdf --paper a4 --scale 5000
```

Das schreibt die Karte als PDF auf eine A4-Seite im Maßstab 1:5000. Die wichtigsten Optionen:

- `--paper a4|a3` wählt das Papier (Standard A4), `--orientation auto|portrait|landscape` die Ausrichtung. Bei
  `auto` nimmt das Programm die Ausrichtung, die weniger von der Karte abschneidet.
- `--scale <Zahl>` setzt den Druckmaßstab, zum Beispiel 4000 für 1:4000. Ohne die Option gilt der Maßstab der
  Karte.
- `--kind sprint|schoolyard|forest` gibt die Kartenart vor, falls das Programm sie weder aus `freemapper.toml`
  noch aus der Kartendatei erkennt.
- `--show-sheet-objects` zeichnet auch die Nordlinien und den Informationsblock der Karte.

Passt die Karte nicht auf die Seite, schreibt das Programm das PDF trotzdem und meldet, wie viele Millimeter
fehlen. Es nennt auch Objekte, die es nicht oder nur vereinfacht gezeichnet hat.

Drucken Sie das PDF in Originalgröße (100 %, "Tatsächliche Größe") und schalten Sie "An Seite anpassen" aus.
Sonst stimmt der Maßstab nicht. Ein fertiges Beispiel mit denselben Seiten in 1:4000 und 1:5000 liegt in
[examples/luebeck-scales](examples/luebeck-scales/README.md). Das Handbuch erklärt das Drucken im Kapitel
[Drucken](docs/manual/de.md#9-drucken-maßstabsgetreu-am-eigenen-drucker-und-im-copyshop).

## Mit einem KI-Agenten

Ein MCP-Server, über den ein KI-Agent die Karten drucken kann, kommt in einem späteren Meilenstein. Bis dahin
läuft alles über die Kommandozeile.

## Lizenz und Namensnennung

freemapper-print steht unter der [GPL-3.0-or-later](LICENSE), wie freemapper. Es übernimmt Code aus freemapper
und darf Code aus OOM übernehmen (ADR 0002).

Das Programm bettet die Schrift Arimo ein (Arial-kompatibel, SIL Open Font License 1.1). Ihre Lizenz liegt in
den Release-Archiven als `FONT-LICENSE.txt`.
