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
| Projektvorlage | starter project | Was `init` für eine Karte schreibt: ein Layout im Kartenmaßstab, `a4` oder `a3` auf dem ersten Blatt, das die Karte ohne Hinweise zeigt (A4, dann A3, das Passepartout bei Bedarf in Schritten von 2 mm bis 24 mm breiter für die Fußzeile), ein Titelblock mit `{event}` und `1:{print_scale} · {contour_interval}`, eine Kontrollstrecke und die vorgeschlagene Äquidistanz (Sprintkarte 2 m, Waldkarte 5 m, Schulhofkarte keine), die ein Kommentar in der Datei zum Bestätigen markiert. Quellenangaben und Namensnennung kommen aus der Fußzeile. | `project::starter`, `project::fit_starter`, `MapKind::proposed_contour_interval_m` |
| Prüfung | check | Der Befehl `check`: zeichnet jedes Layout im Speicher, ohne Dateien zu schreiben, und listet Blatthinweise und Druckbericht. Exit-Code 1, sobald ein Hinweis außer „Nordlinien ergänzt“ auftritt. | `check`, `export::sheet_pdf` |
| Export | export | Der Befehl `export`: je Layout ein PDF `<Layout-ID>.pdf` im Ausgabeordner, die Layouts nacheinander gezeichnet und geschrieben, sodass immer nur eine Seite im Speicher liegt. | `export::export_layouts` |
| Ausgabeordner | output folder | Ordner, in den `export` die PDFs schreibt: `--out-dir` (muss existieren) oder der Ordner `out` neben der Projektdatei, der bei Bedarf angelegt wird. | `ExportArgs::out_dir` |
| Kartennotizen | map notes | Der Text im Element `<notes>` der `.omap`. freemapper schreibt dort die Datenquellen hinein, eine je Zeile; freemapper-print macht daraus die Quellenangaben der Fußzeile, wenn `print.copyright` fehlt. | `MapDocument::notes` |
| Begleitdatei | sidecar file | Die `freemapper.toml` neben einer Karte von freemapper. freemapper-print liest daraus nur `map_kind` (beim Druck und bei `init`); eine kaputte Datei gibt einen Hinweis, und die Kartenart kommt dann aus der Kartendatei. | `scale::read_sidecar` |
| Blattsprache | sheet language | Sprache der gedruckten Blätter (`event.lang`, `de` oder `en`, Standard `de`): Dezimalzeichen, Datumsformat, Namensnennung. Unabhängig von der Programmsprache. | `SheetLang` |
| Programmsprache | program language | Sprache der Meldungen und der Hilfe: `--lang`, sonst `FREEMAPPER_LANG`, `LC_ALL`, `LC_MESSAGES`, `LANG`; Deutsch oder Englisch, Englisch als Rückfall (ADR 0024). | `Lang`, `detect_lang` |
| sicheres Schreiben | safe writing | So schreibt `export` (und `render`) ein PDF: freien Platz prüfen (PDF plus 50 MB Reserve), in `<Name>.pdf.part` schreiben, synchronisieren, umbenennen. Ein voller Datenträger hinterlässt kein halbes PDF (ADR 0030). | `write_pdf_file`, `DISK_RESERVE_BYTES` |
| Zuordnung | assignment | Welche Bahn mit welchem Layout und welcher Abdeckungsvariante gedruckt wird. | `Assignment` |

## Layout & Blatt / layout & sheet

| Deutsch | English | Definition | Code |
|---|---|---|---|
| Layout | layout | Vorlage für Kartenblätter: Blattformat, Ausrichtung, Druckmaßstab, Kartenausschnitt und Blattelemente. Eine Veranstaltung hat meist 1 bis 5. Die Layout-ID benennt die PDF-Datei; sie darf daher kein `/ \ : * ? " < > |` und keine Steuerzeichen enthalten, nicht mit einem Punkt beginnen oder enden, kein Windows-Gerätename sein und sich von den anderen IDs nicht nur in Groß- und Kleinschreibung unterscheiden. | `Layout` |
| Kartenblatt, Blatt | sheet | Eine gedruckte Seite: eine Bahn mit ihrem Layout und ihrer Abdeckungsvariante. | `Sheet` |
| Blattformat | paper size | Papiergröße, z. B. A4 oder A3. | `Paper` |
| Ausrichtung | orientation | Hoch- oder Querformat. | `Orientation` |
| Kartenausschnitt | map frame | Der Teil der Karte, der auf dem Blatt erscheint, und das Rechteck, das er auf dem Blatt füllt: das Blatt ohne das Passepartout. Die Karte wird darauf beschnitten; der Überstand wird an ihm gemessen. Mit `centre_map_mm` liegt dieser Kartenpunkt in seiner Mitte, in den Koordinaten, die OOM in der Statusleiste zeigt (mm, y nach oben), sonst die Mitte dessen, was gezeichnet wird. | `MapFrameSpec`, `RenderedPage::clip`, `frame_box` |
| gewollter Beschnitt | intended crop | `map_frame.crop = true`: Der Kartenausschnitt schneidet die Karte mit Absicht ab. Der Überstand ist dann ein Hinweis zur Information und lässt `check` nicht scheitern. | `MapFrameSpec::crop`, `SheetNote::MapCropped` |
| Rahmenlinie | frame border | Schwarze Linie um den Kartenausschnitt, `border_mm` breit (Standard 0,25 mm), mittig auf seiner Kante, in der Blattebene; die eine Hälfte deckt die Schnittkante der Karte (ADR 0028). 0 zeichnet keine. | `MapFrameSpec::border_mm`, `frame_border` |
| Passepartout | passe-partout | Weißer Rand zwischen Kartenausschnitt und Blattrand, auf dem Blattelemente stehen können. Auf jeder Seite gleich breit (`margin_mm`, Standard 8 mm); es wird nichts darauf gemalt. | `MapFrameSpec::margin_mm`, `PrintOptions::frame_margin_mm` |
| Blattelement | sheet element | Etwas, das auf dem Blatt steht, aber nicht zur Karte gehört: Titelblock, Logo, Text, Postenbeschreibung, Kontrollstrecke, Namensnennung. | `SheetElement` |
| gemeinsames Element | shared element | Blattelement, das einmal gepflegt wird und in allen Layouts erscheint. Ein Layout darf es verschieben oder ausblenden. | `SharedElement` |
| Anker | anchor | Ecke oder Kante des Blatts (`page`) oder des Kartenausschnitts (`map_frame`), an der ein Blattelement hängt: eine von neun Positionen (`top_left`, `top`, `top_right`, `left`, `centre`, `right`, `bottom_left`, `bottom`, `bottom_right`) mit Abstand `offset_mm = [x, y]` in mm, von der Position nach innen gemessen (x nach rechts, y nach unten). Von einer rechten oder unteren Kante zählt der Abstand nach links oder oben, von der Mitte nach rechts oder unten; das Element liegt innen an seiner Position. | `Anchor`, `anchor_point` |
| Textelement | text element | Blattelement `text` mit eigenen Zeilen (9 pt, Platzhalter erlaubt), auf Wunsch auf weißem Kasten (`box_white`, Standard an). | `ElementKind::Text` |
| Titelblock | title block | Blattelement mit Veranstaltung, Bahn, Maßstab und Äquidistanz. | `TitleBlock` |
| Platzhalter | placeholder | Text in geschweiften Klammern, der beim Zeichnen ausgefüllt wird: `{event}`, `{date}`, `{organiser}`, `{print_scale}`, `{map_scale}`, `{contour_interval}`, `{layout}`; ab M3 auch `{course}`. Ein unbekannter bleibt stehen und wird gemeldet. | `Placeholder` |
| Logo | logo | Vereins- oder Sponsorenlogo als PNG, JPEG oder SVG, `width_mm` breit; die Höhe folgt dem Seitenverhältnis. PNG und JPEG bleiben Pixelbilder und behalten ihren Alphakanal. Ein SVG darf nur einfache Formen, Verläufe, `use` und Beschneidungspfade im Nutzerkoordinatensystem enthalten (Liste in ADR 0026); alles andere, etwa Masken, Muster, Filter, Marker, eingebettete Bilder oder Text, lässt das Logo mit dem Rat aus, es als einfaches SVG zu exportieren. Ebenso ein SVG mit mehrdeutigen Attributen: zwei Attribute gleichen lokalen Namens an einem Element, wenn `usvg` diesen Namen liest (`href` neben `xlink:href` nur, wenn beide dasselbe Ziel haben; `version` neben `inkscape:version` ist erlaubt) oder Attribute fremder Namensräume außer denen von Inkscape, Sodipodi und Illustrator. Weitere Grenzen: höchstens 256 Ebenen tief, 1000 Attribute je Element, 64 Namensraumdeklarationen, 256 Stopps je Verlauf, ein `style`-Attribut höchstens 2048 Byte und 64 Deklarationen, alle zusammen höchstens 256 KiB, jeder andere Attributwert außer Pfaddaten höchstens 1 MiB, alle Attributwerte zusammen höchstens 4 MiB; die beiden Summen zählen jede Kopie durch `use` mit. Eine Dokumenttypdeklaration (DTD) darf nur Kommentare und höchstens 64 Entitäten mit je höchstens 1024 Byte reinem Text enthalten, wie Illustrator sie schreibt; jede andere DTD lässt das Logo aus. Das SVG wird zu Flächen und Linien in CMYK (einfache Umrechnung aus RGB), jede Fläche nach ihrer Füllregel. Was das Blatt nicht wie geschrieben zeichnen kann, wird vereinfacht und gemeldet: ein Verlauf in seiner ersten Farbe, Transparenz deckend, Strichelung durchgezogen, Beschneidungspfade wirken nicht. Die Gehrungsgrenze (`stroke-miterlimit`) wird nicht übernommen. Eine Datei über 10 MB, ein Pixelbild über 25 Megapixel oder über 65 535 Pixel Kantenlänge, ein SVG über 20 000 Pfadsegmente sowie eine fehlende oder kaputte Datei werden ausgelassen und gemeldet (ADR 0019, ADR 0026). | `Logo`, `sheet::logo` |
| Logo-Zwischenspeicher | logo cache | Hält die Logos eines `check`- oder `export`-Laufs: jede Datei einmal gelesen und dekodiert, von allen Elementen und Layouts geteilt, sodass das PDF sie einmal enthält. Merkt sich auch, warum eine Datei nicht geht (ADR 0026). | `LogoCache` |
| Logo-Budget | logo budget | Was ein Blatt an Logos trägt: höchstens 50 Megapixel PNG und JPEG und 1 000 000 Einheiten SVG-Arbeit, jedes Logo so oft gezählt, wie es platziert ist. Ein Logo darüber hinaus fehlt mit Hinweis (ADR 0026). | `LogoBudget`, `MAX_PAGE_LOGO_PIXELS`, `MAX_PAGE_SVG_UNITS` |
| Kontrollstrecke | scale check bar | Balken mit Länge in Metern und Millimetern, mit dem man am Ausdruck nachmessen kann, ob der Drucker skaliert hat. Die Länge ist die größte runde Strecke (5 m bis 1000 m), die im Druckmaßstab höchstens 50 mm lang wird. | `ScaleCheckBar` |
| Namensnennung | credit line | Hinweis „Erstellt mit freemapper-print – <Repository>“ (englisch „Created with …“) in der Fußzeile jedes PDFs, auch von `render`, und in den PDF-Metadaten. Lässt sich nicht abschalten; ein Element `credit_line` (mindestens 5 pt) verschiebt sie nur, und nur, wenn es ganz auf dem Blatt liegt. Sie wird nach allem anderen gezeichnet. | `CREDIT`, `credit_line` |
| Fußzeile | footer | Die Quellenangaben und die Namensnennung als ein Block auf weißem Kasten, links unten auf dem Blatt unter dem Kartenausschnitt (5 pt, Grundlinie 4 mm über dem Papierrand), sofern der ganze Block dort Platz hat, sonst im Kartenausschnitt auf weißem Kasten (ADR 0029). `render` setzt sie mit den Kartennotizen in den 5-mm-Rahmen, also auf den Kasten. Hinweise nennen sie `footer`; kein Blattelement darf diese ID tragen. | `sheet::text::default_footer`, `FOOTER_ID` |
| Quellenangabe | copyright line | Eine Zeile mit einer Quelle der Karte (etwa „© OpenStreetMap contributors“), je Zeile der Kartennotizen oder von `print.copyright`. Steht über der Namensnennung; ein Element `copyright_line` verschiebt sie. | `copyright_line` |
| Kartenteil | map part | Gruppe von Kartenobjekten in OOM (`<part>`), z. B. „peripheral information“. Ein Layout blendet Kartenteile nach ihrem genauen Namen aus (`hide_parts`, Groß- und Kleinschreibung zählt). | `MapPart`, `LayoutSpec::hide_parts` |
| Blattbericht | sheet report | Was ein Kartenblatt zeigt: der Druckbericht der Karte im Kartenausschnitt und die Blatthinweise. | `SheetReport` |
| Blatthinweis | sheet note | Hinweis zu einem Blatt, der den Export nicht aufhält: Element außerhalb des Blatts, Element über der Karte, Fußzeile über einem Element, Papier größer als A3, fehlender Maßstab oder fehlende Äquidistanz, keine Kontrollstrecke, ergänzte Nordlinien, gewollter Beschnitt, gekürzte oder zu viele Quellenangaben, verschobene Namensnennung, ausgelassenes oder vereinfachtes Logo, unbekannter Platzhalter, unbekannter Kartenteil. Gründe sind Werte, keine englischen Sätze, damit jede Sprache ihren Text hat. Ergänzte Nordlinien und gewollter Beschnitt informieren nur („Zur Information:“) und zählen nicht für den Exit-Code von `check`. | `SheetNote`, `LogoSkipReason`, `LogoSimplification`, `MissingItem` |
| Blattebene | sheet layer | Die Elemente eines Blatts (Titel, Texte, Logos, Maßstabsleiste, Fußzeile), die nach der Karte gemalt werden, in ihrer Reihenfolge und nicht auf den Kartenrahmen beschnitten. Jedes Element ist ein `SheetItem`. | `SheetItem` |
| Füllregel | fill rule | Welche Punkte eine Fläche bedeckt, wo sich ihre Ringe kreuzen oder ineinander liegen. Gerade-Ungerade (`EvenOdd`): innen, wo ein Strahl die Ringe ungerade oft kreuzt; so füllen Kartenflächen wie in OOM und die übrigen Blattelemente. Nicht-Null (`NonZero`): innen, wo die Ringe den Punkt insgesamt mindestens einmal umlaufen; SVG-Logos nehmen die Regel aus dem SVG. | `FillRule`, `Shape::Fill` |
| Direktfarbe | direct colour | Farbe eines Blattelements, als CMYK-Werte von 0 bis 1 angegeben statt als Kartenfarbe nach Priorität. | `DirectColour` |
| Rasterbild | raster image | Ein Bild aus Pixeln (8 Bit RGB, optional mit Alpha), zum Beispiel ein Logo. Nur `RasterImage::new` erzeugt eines, geprüft auf Größe und Datenlänge. | `RasterImage` |

## Maßstab & Darstellung / scale & rendering

| Deutsch | English | Definition | Code |
|---|---|---|---|
| Kartenmaßstab | map scale | Maßstab, in dem die Kartenquelle gezeichnet ist. | `map_scale` |
| Druckmaßstab | print scale | Maßstab, in dem ein Layout die Karte druckt. | `print_scale` |
| Geometriefaktor | geometry factor | Kartenmaßstab geteilt durch Druckmaßstab; so stark wird die Kartengeometrie skaliert. | `geometry_factor` |
| Basismaßstab | base scale | Maßstab, für den die Vorschrift der Kartenart die Symbolgrößen angibt (100 %): Sprintkarte 1:4000, Waldkarte 1:15000, Schulhofkarte 1:1000. | `MapKind::base_scale` |
| Symbolfaktor | symbol factor | Faktor für Symbolmaße beim Druck: die Symbolgröße, die die Vorschrift für den Druckmaßstab festlegt, geteilt durch die für den Kartenmaßstab. Eine Vergrößerung vergrößert alle Symbole im selben Verhältnis, etwa auf 133 % bei einer Sprintkarte in 1:3000 (ADR 0018). | `symbol_factor` |
| nicht normgerechter Maßstab | non-standard scale | Druckmaßstab unter dem Basismaßstab, den die Vorschrift nicht vorsieht (Sprintkarte 1:5000, Waldkarte 1:20000). Die Symbole behalten ihre Größe im Basismaßstab; das Programm warnt, dass der Druck nur fürs Training taugt. | `is_standard`, `PrintReport::non_standard_scale` |
| Regelprofil | rule profile | TOML-Datei mit den Druckregeln, eingebettet ins Programm, im Format von freemapper-courseplan: Symbolgrößen je Maßstab, Nordlinienabstand, Größen der Bahnsymbole, Violett, Kästchengröße der Postenbeschreibung, Gewichte für die Lage der Postennummern, jeder Wert mit Quelle. Ein Basisprofil enthält alle Werte, zurzeit nur `iof-print`; die Projektdatei nennt es in `[print] rule_profile` (ADR 0031). | `RuleProfile`, `rules::base`, `PrintSettings::rule_profile` |
| Regelprofil-Aufsatz, Aufsatz | rule overlay, overlay | Regelprofil, das ein Basisprofil ergänzt: Es fügt Regeln hinzu, ersetzt eine Regel mit derselben `id` oder schaltet sie mit `enabled = false` ab. Die Projektdatei nennt Aufsätze in `[print] rule_overlays`, angewandt in dieser Reihenfolge. M3 bettet noch keinen ein (ADR 0031). | `rule_overlays`, `ProfileKind::Overlay` |
| Regelsatz | rule set | Ein Basisprofil mit seinen Aufsätzen, zusammengeführt. Beim Laden bekommt jede Kartenart jeden Wert genau einmal, sonst ist der Regelsatz unbrauchbar. Er merkt sich, welches Profil eine Regel abgeschaltet hat. Das Projekt lädt seinen Regelsatz einmal beim Laden. | `RuleSet`, `load_rule_set`, `SwitchedOff`, `LoadedProject::rules` |
| Bahnsymbolgrößen | course symbol sizes | Größen der Bahnsymbole 701 bis 710 im Basismaßstab einer Kartenart, aus dem Regelprofil: Postenkreis, Linienbreite, Startdreieck, Zielkreise, Postennummer, Pflichtstrecke, Sperrgebietsgrenze, Durchgang. Sie wachsen mit dem Symbolfaktor. | `CourseSymbolSizes` |
| Platzierungsgewichte | placement weights | Wie stark ein mm² Kartendetail unter einer Postennummer zählt, je Farbklasse: schwarz 10, braun 4, blau 4, übrige 1 (ADR 0031). | `PlacementWeights` |
| gerenderte Seite | rendered page | Eine Seite als Zeichenbefehle in Seitenmillimetern, mit Clip-Rechteck, Seitengröße und Farben. Der PDF-Writer liest nur sie; spätere Ausgaben (SVG, Kacheln) lesen dieselbe. | `RenderedPage` |
| Seitenrand | page margin | Fester weißer Rand von 5 mm an jeder Seitenkante, den die Karte beim Befehl `render` nicht bedruckt. Ein Layout nimmt stattdessen sein Passepartout. | `PAGE_MARGIN_MM` |
| Überstand | overflow | Wie viele Millimeter das, was gedruckt würde (mit Linienbreiten, Punktsymbolen und Texten), in x und y über das Clip-Rechteck hinausragt. Die Karte wird nicht verkleinert, sondern abgeschnitten und der Überstand gemeldet. | `PrintReport::overflow_mm` |
| Druckbericht | print report | Was eine gedruckte Seite zeigt und was nicht: Seitengröße, Druckmaßstab, Faktoren, nicht normgerechter Maßstab, Überstand, übersprungene, gekürzte und wegen des Seitenlimits nicht gezeichnete Objekte, Objekte unbekannter Art, Objekte mit verworfenem Flächenmuster, ersetzte Schriften, fehlende Zeichen. | `PrintReport` |
| verstecktes Symbol | hidden symbol | Symbol, das die Kartendatei mit `is_hidden` ausblendet. Seine Objekte werden nicht gedruckt. | `SymbolSet::is_hidden` |
| übersprungene Objekte | skipped objects | Objekte, die nicht gedruckt werden, weil ihr Symbol unbekannt oder versteckt ist. Der Druckbericht zählt sie. | `PrintReport::skipped_objects` |
| gekürzte Objekte | truncated objects | Objekte, deren Zeichnung vereinfacht oder abgeschnitten wird, weil sie mehr Arbeitseinheiten bräuchte, als der Renderer je Objekt erlaubt, oder weil das Seitenlimit mitten in ihr erreicht ist. Der Druckbericht zählt sie. | `PrintReport::truncated_objects` |
| Arbeitseinheit | work unit | Maß für die Arbeit und den Speicher beim Zeichnen: eine Einheit je Zeichenbefehl und je acht Punkte, die er behält, je acht Punkte eines Umrisses für jeden Clip-Block, den das PDF schreibt, eine je Schritt ohne Ausgabe, eine je 64 kopierte oder geprüfte Punkte. Ein Objekt darf 250 000 Einheiten brauchen (ADR 0019). | `MAX_UNITS_PER_OBJECT`, `KEPT_POINTS_PER_UNIT` |
| Seitenlimit | page limit | Höchstzahl der Arbeitseinheiten einer Seite (500 000). Echte Karten bleiben weit darunter (Lübeck etwa 6500); Objekte nach dem Limit werden nicht gezeichnet und gezählt (ADR 0019). | `MAX_UNITS_PER_PAGE`, `PrintReport::objects_not_drawn_page_limit` |
| verworfenes Flächenmuster | dropped pattern | Flächenmuster, das der Leser verwirft, weil sein Abstand unter 0,05 mm liegt oder eine Zahl ungültig ist. Der Druckbericht zählt die Objekte, denen es fehlt. | `MIN_PATTERN_STEP`, `PrintReport::dropped_pattern_objects` |
| Objekte unbekannter Art | unknown-type objects | Objekte, deren Objektart (`type`) der Leser nicht kennt (nicht Punkt, Linie/Fläche oder Text). Sie werden samt Inhalt übersprungen und gezählt. | `MapDocument::unknown_objects` |
| Malreihenfolge | paint order | Reihenfolge der Zeichenbefehle einer gerenderten Seite: nach Farbpriorität, die höchste Nummer zuerst, innerhalb einer Farbe in der Reihenfolge der Kartendatei. | `RenderedPage::primitives` |
| Randobjekte | sheet objects | Objekte der Karte mit dem freemapper-Tag `freemapper:source=sheet`: der Informationsblock (Titel, Maßstab, Maßstabsleiste, Namensnennung, Symbole 991–994) und die Nordlinien. Den Informationsblock druckt das Programm nur auf Wunsch. Nordlinien (601) gehören zur Karte und werden immer gedruckt (ADR 0023). | `show_sheet_objects` |
| Nordlinien ergänzen | generated north lines | Nordlinien, die das Programm ergänzt, wenn die Karte keine hat und das Layout `north_lines = "auto"` hat: senkrecht in Kartenkoordinaten, im Bodenabstand der Kartenart, mit dem Symbol 601 der Karte, über den Rahmen (ADR 0027). | `NorthLinesAdded` |
| Kartenkoordinaten | map coordinates | Koordinaten in Millimetern auf der Karte im Kartenmaßstab, y nach unten, wie in der `.omap`. | `MapCoord` |
| Renderer | renderer | Teil des Programms, der Kartenobjekte mit ihren Symbolen ins PDF zeichnet. | `render` |
| Farbpriorität | colour priority | Reihenfolge, in der Farben übereinander gedruckt werden, wie in OOM. | `ColorPriority` |
| Strichpunkt | dash point | Koordinate einer Linie mit dem Dash-Point-Flag (32). Strichmuster und Mittelsymbole fangen dort neu an; ein Strichpunktsymbol wird dort gezeichnet. | `FLAG_DASH`, `Part::dash_points` |
| Strichpunktsymbol | dash symbol | Punktsymbol einer Liniensignatur, das an jedem Strichpunkt steht, zur Winkelhalbierenden gedreht. | `DashSymbol` |
| Mittelsymbol | mid symbol | Punktsymbol, das eine Liniensignatur in Abständen entlang der Linie wiederholt (z. B. Zaunstriche); auf gestrichelten Linien je Strich, Strichgruppe oder Lücke. | `Mid`, `MidPlacement` |
| Anfangs- und Endabstand | start and end offset | Strecke, um die eine offene Linie am Anfang und am Ende gekürzt gezeichnet wird. | `LineDef::start_offset`, `LineDef::end_offset` |
| spitzes Linienende | pointed cap | Linienende, das über den Anfangs- oder Endabstand von der vollen Breite bis auf null zuläuft (OOM-Linienende 3). | `LineDef::pointed_caps` |
| Textfeld | box text | Kartentext in einem Rechteck: Der Anker ist die Mitte des Rechtecks, die Zeilen brechen an seiner Breite um und richten sich nach seinen Kanten aus, wie in OOM. | `MapObjectKind::Text::box_size` |
| Absatzabstand | paragraph spacing | Zusätzlicher Abstand nach jedem Absatz eines Kartentexts, aus der Textsignatur. | `TextDef::paragraph_spacing` |
| Textrahmen | text framing | Umrandung einer Textsignatur: die Buchstabenumrisse in einer zweiten Farbe nachgezogen oder als versetzter Schatten gedruckt. | `Framing` |
| Schrift, Schriftschnitt | font, font face | Die Schrift, in der Kartentexte im PDF stehen: Arimo, maßgleich mit Arial, eingebettet als Teilmenge der benutzten Zeichen. Jede Schriftfamilie der Karte wird durch Arimo ersetzt; der Schriftschnitt (normal, fett, kursiv, fett kursiv) folgt der Textsignatur (ADR 0017). | `FontFace`, `fonts` |

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
| Pflichtstrecke | marked route | Abgeflaggter Weg, dem der Läufer folgen muss, z. B. vom letzten Posten zum Ziel. Wird in der Bahnlegung festgelegt. | `MarkedRoute` |
| Nummernlage | number placement | Richtung und Abstand der Postennummer vom Kreis in Druckmillimetern. Die Bahnlegung legt sie fest; freemapper-print korrigiert sie je Layout oder je Blatt (ADR 0015). | `NumberPlacement` |
| automatische Platzierung | automatic placement | Wahl der Nummernlage durch das gemeinsame Platzierungsmodul. In freemapper-print nur, wenn die Bahndatei keine Lage enthält, und als Vorschlag bei Konflikten. | `auto_place` |
| Nummernkorrektur | number correction | Abweichende Nummernlage je Layout oder je Blatt in der Projektdatei; ändert die Bahndatei nicht. | `NumberCorrection` |
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
