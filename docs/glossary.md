# Glossar / Glossary

Verbindliche Begriffe für Code und Dokumentation. Neue Fachbegriffe werden **zuerst hier** ergänzt,
dann verwendet. Code-Bezeichner sind Englisch (Rust-Konventionen: `TypeName`, `fn_name`, `CONST_NAME`).
Begriffe, die das [Glossar von freemapper](https://github.com/HenryJobst/freemapper/blob/main/docs/glossary.md)
schon festlegt (Maßstab, Symbol, Symbolsatz, Kartenart, Sprintkarte, Äquidistanz, Nordlinien, Missweisung …), gelten hier
unverändert und stehen nur dann noch einmal hier, wenn freemapper-print sie anders braucht.

Binding terms for code and documentation. New domain terms are added **here first**, then used.
Code identifiers are English (Rust conventions: `TypeName`, `fn_name`, `CONST_NAME`). Terms already fixed by
freemapper's glossary keep their meaning here and appear again only where freemapper-print needs more.

Abkürzungen / abbreviations: **ISSprOM** = International Specification for Sprint Orienteering Maps (IOF,
2019-2), **ISOM** = International Specification for Orienteering Maps (IOF, 2017-2), **IOF-XML 3.0** = IOF Data
Standard 3.0, **OOM** = OpenOrienteering Mapper.

## Veranstaltung & Projekt / event & project

| Deutsch | English | Definition | Code |
|---|---|---|---|
| Veranstaltung | event | Ein Training oder Wettkampf. Für jede Veranstaltung gibt es ein Projekt. | `Event` |
| Projektdatei | project file | TOML-Datei einer Veranstaltung. Editor, CLI und MCP-Server lesen und schreiben nur diese Datei (ADR 0008). | `ProjectFile` |
| Kartenquelle | map source | Die fertige Karte, auf die die Projektdatei verweist (`.omap`, später `.ocd`). | `MapSource` |
| Bahnquelle | course source | Die Datei mit den Bahnen (IOF-XML 3.0 oder `.ppen`). Wird bei jeder Prüfung und jedem Export neu gelesen. | `CourseSource` |
| Zuordnung | assignment | Welche Bahn mit welchem Layout und welcher Abdeckungsvariante gedruckt wird. | `Assignment` |

## Layout & Blatt / layout & sheet

| Deutsch | English | Definition | Code |
|---|---|---|---|
| Layout | layout | Vorlage für Kartenblätter: Blattformat, Ausrichtung, Druckmaßstab, Kartenausschnitt und Blattelemente. Eine Veranstaltung hat meist 1 bis 5. | `Layout` |
| Kartenblatt, Blatt | sheet | Eine gedruckte Seite: eine Bahn mit ihrem Layout und ihrer Abdeckungsvariante. | `Sheet` |
| Blattformat | paper size | Papiergröße, z. B. A4 oder A3. | `PaperSize` |
| Ausrichtung | orientation | Hoch- oder Querformat. | `Orientation` |
| Kartenausschnitt | map frame | Der Teil der Karte, der auf dem Blatt erscheint, und das Rechteck, das er auf dem Blatt füllt. | `MapFrame` |
| Passepartout | passe-partout | Weißer Rand zwischen Kartenausschnitt und Blattrand, auf dem Blattelemente stehen können. | `Passepartout` |
| Blattelement | sheet element | Etwas, das auf dem Blatt steht, aber nicht zur Karte gehört: Titelblock, Logo, Text, Postenbeschreibung, Kontrollstrecke, Namensnennung. | `SheetElement` |
| gemeinsames Element | shared element | Blattelement, das einmal gepflegt wird und in allen Layouts erscheint. Ein Layout darf es verschieben oder ausblenden. | `SharedElement` |
| Anker | anchor | Ecke oder Kante des Blatts oder des Kartenausschnitts, an der ein Blattelement mit Abstand in mm hängt. | `Anchor` |
| Titelblock | title block | Blattelement mit Veranstaltung, Bahn, Maßstab und Äquidistanz. | `TitleBlock` |
| Platzhalter | placeholder | Text in geschweiften Klammern, z. B. `{course}`, der je Bahn ausgefüllt wird. | `Placeholder` |
| Logo | logo | Vereins- oder Sponsorenlogo als Bild (SVG, PDF, PNG). | `Logo` |
| Kontrollstrecke | scale check bar | Balken mit Länge in Metern und Millimetern, mit dem man am Ausdruck nachmessen kann, ob der Drucker skaliert hat. | `ScaleCheckBar` |
| Namensnennung | credit line | Hinweis auf freemapper-print und sein Repository im Blatt und in den PDF-Metadaten. | `CREDIT` |

## Maßstab & Darstellung / scale & rendering

| Deutsch | English | Definition | Code |
|---|---|---|---|
| Kartenmaßstab | map scale | Maßstab, in dem die Kartenquelle gezeichnet ist. | `map_scale` |
| Druckmaßstab | print scale | Maßstab, in dem ein Layout die Karte druckt. | `print_scale` |
| Geometriefaktor | geometry factor | Kartenmaßstab geteilt durch Druckmaßstab; so stark wird die Kartengeometrie skaliert. | `geometry_factor` |
| Symbolfaktor | symbol factor | Faktor für Symbolmaße beim Druck, abhängig von Kartenart und Druckmaßstab (ADR 0012). | `symbol_factor` |
| Kartenkoordinaten | map coordinates | Koordinaten in Millimetern auf der Karte im Kartenmaßstab, y nach unten, wie in der `.omap`. | `MapCoord` |
| Renderer | renderer | Teil des Programms, der Kartenobjekte mit ihren Symbolen ins PDF zeichnet. | `render` |
| Farbpriorität | colour priority | Reihenfolge, in der Farben übereinander gedruckt werden, wie in OOM. | `ColorPriority` |

## Bahnen / courses

| Deutsch | English | Definition | Code |
|---|---|---|---|
| Bahn | course | Folge von Start, Posten und Ziel für eine Gruppe von Läufern. | `Course` |
| Bahnlegungsprogramm | course-setting program | Purple Pen, Condes oder OCAD-Bahnlegung. | – |
| Posten | control | Punkt, den der Läufer anlaufen muss; hat einen Code. | `Control` |
| Postencode | control code | Kennung eines Postens, z. B. „31“. | `ControlCode` |
| Postennummer | control number | Laufende Nummer eines Postens in einer Bahn, auf der Karte neben dem Kreis. | `control_number` |
| Strecke | leg | Verbindung zwischen zwei aufeinanderfolgenden Posten. | `Leg` |
| Knickpunkt | bend | Punkt, an dem eine Strecke abknickt, z. B. um ein Sperrgebiet herum. | `bends` |
| Bahnaufdruck | course overprint | Die violetten Bahnsymbole über der Karte. | `CourseOverprint` |
| Postenbeschreibung | control descriptions | Tabelle der Posten nach IOF-Postenbeschreibung (Spalten A bis H). | `ControlDescriptions` |
| Gabelung | forking | Varianten einer Bahn, z. B. in Staffeln. | `Forking` |
| Purple-Pen-Projekt | Purple Pen project | Native XML-Datei von Purple Pen (`.ppen`). | `PurplePenProject` |

## Abdeckungen / masks

| Deutsch | English | Definition | Code |
|---|---|---|---|
| Abdeckung | mask | Weiße Fläche über der Karte mit Löchern; liegt unter Bahnaufdruck und Blattelementen. In Purple Pen „White-out“. | `Mask` |
| Abdeckungsvariante | mask variant | Benannte Menge von Abdeckungen, die einer oder mehreren Bahnen zugeordnet wird. | `MaskVariant` |
| Fenster-OL | window orienteering | Training, bei dem die Karte nur in Fenstern um Posten oder Strecken sichtbar ist. | – |
| Korridor-OL | corridor orienteering | Training, bei dem nur ein schmaler Streifen entlang der Strecke sichtbar ist. | – |

## Ausgabe / output

| Deutsch | English | Definition | Code |
|---|---|---|---|
| Druck-PDF | print PDF | PDF eines oder mehrerer Kartenblätter. | – |
| Büro-PDF | office print PDF | Für eigenen Drucker und Copyshop: Seitengröße gleich Blattformat, Bahnaufdruck mit durchscheinender Karte simuliert. | `OutputProfile::Office` |
| Druckerei-PDF | press PDF | Für Druckereien: PDF/X, Beschnittzugabe, Schnittmarken, Bahnaufdruck als Sonderfarbe (später). | `OutputProfile::Press` |
| Sonderfarbe | spot colour | Druckfarbe, die als eigene Farbe gedruckt wird, z. B. das Violett des Bahnaufdrucks. | `SpotColor` |
| Überdrucken | overprinting | Drucktechnik, bei der eine Farbe die darunterliegende nicht ausspart. Nicht verwechseln mit Bahnaufdruck. | `overprint_mode` |
