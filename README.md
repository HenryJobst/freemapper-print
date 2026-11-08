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

freemapper-print ist in der Planung. Es gibt noch kein lauffähiges Programm, nur das Projektgerüst, das
[Design](docs/superpowers/specs/2026-11-08-freemapper-print-design.md), das [Glossar](docs/glossary.md) und die
[Architekturentscheidungen](docs/adr/README.md). Die erste Version richtet sich an Vereinstrainings in Stadt
und Park: Sprintkarten auf A4, gedruckt am eigenen Drucker oder im Copyshop.

## Selbst bauen (für Entwickler)

```sh
git clone https://github.com/HenryJobst/freemapper-print
cd freemapper-print
cargo build --release
```

`rust-toolchain.toml` legt die Rust-Version fest; `rustup` installiert sie beim ersten Bauen. Vor jedem Commit
prüft [lefthook](https://github.com/evilmartians/lefthook) Formatierung und clippy (`lefthook install`).

## Lizenz und Namensnennung

freemapper-print steht unter der [GPL-3.0-or-later](LICENSE), wie freemapper. Es übernimmt Code aus freemapper
und darf Code aus OOM übernehmen (ADR 0002).
