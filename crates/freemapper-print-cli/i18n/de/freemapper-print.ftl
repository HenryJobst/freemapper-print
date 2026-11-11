# Programmtexte von freemapper-print, Deutsch (ADR 0024).
# Die Variablennamen sind in jeder Sprache gleich. Texte aus der Bibliothek, vom Betriebssystem
# und aus der Kartendatei kommen in $error, $names und $chars an und bleiben unverändert.

## Format

# Das Zeichen zwischen ganzen und Nachkommastellen.
decimal-separator = ,

## Hilfe der Befehlszeile (clap)

cli-about = Druckt fertige Orientierungslaufkarten als PDF im exakten Maßstab
help-lang = Sprache der Programmtexte: de oder en. Ohne Angabe entscheiden FREEMAPPER_LANG, dann LC_ALL, LC_MESSAGES und LANG.
render-about = Schreibt eine Karte als PDF in einem exakten Druckmaßstab.
render-after-size = Im PDF entspricht jeder Millimeter auf dem Papier genau der Kartenstrecke, die der Druckmaßstab vorgibt. In Originalgröße drucken.
render-after-exit-codes = Exit-Codes: 0 bei Erfolg, 1, wenn die Karte nicht gelesen oder gedruckt werden kann, 2, wenn die Befehlszeile falsch ist.
help-map = Die OOM-Kartendatei (.omap).
help-out = Zu schreibende PDF-Datei. Eine vorhandene Datei wird überschrieben; ihr Ordner muss existieren.
help-scale = Nenner des Druckmaßstabs, z. B. 4000 für 1:4000. Ohne Angabe gilt der Maßstab der Karte.
help-paper = Blattformat.
help-orientation = Ausrichtung der Seite: portrait (Hochformat) oder landscape (Querformat). Auto wählt die Ausrichtung, in der weniger von der Karte fehlt.
help-kind = Kartenart. Nur nötig, wenn das Programm sie weder aus freemapper.toml noch aus der Kartendatei erkennt.
help-show-sheet-objects = Zeichnet auch den Informationsblock von freemapper: Titel, Maßstab, Maßstabsleiste und Namensnennung. Er skaliert mit der Karte, bei einem anderen Druckmaßstab kann sein Maßstabstext deshalb falsch sein.
init-about = Legt die Projektdatei einer Veranstaltung für eine Karte an.
init-after-content = Die neue Datei hat ein Layout im Kartenmaßstab, auf A4 oder sonst A3, je nachdem, wo die Karte ohne Hinweise Platz findet, einen Titelblock, eine Kontrollstrecke und einen Vorschlag für die Äquidistanz, den Sie prüfen und bestätigen. Die Kartenart kommt aus --kind, aus der freemapper.toml neben der Karte oder aus der Kartendatei.
init-after-exit-codes = Exit-Codes: 0 bei Erfolg, 1, wenn die Karte nicht gelesen werden kann, die Kartenart unbekannt ist oder die Projektdatei schon existiert, 2, wenn die Befehlszeile falsch ist.
help-init-folder = Ordner für die neue freemapper-print.toml. Fehlt er, wird er angelegt.
help-init-map = Die OOM-Kartendatei (.omap). Die Projektdatei speichert ihren Pfad relativ zu sich selbst.
help-init-title = Titel der Veranstaltung auf den Blättern. Ohne Angabe gilt der Dateiname der Karte.
help-init-kind = Kartenart. Nur nötig, wenn keine freemapper.toml neben der Karte sie nennt.
check-about = Zeichnet jedes Layout eines Projekts, mit Bahndatei jede Bahn, ohne eine Datei zu schreiben, und listet die Hinweise auf.
check-after-exit-codes = Exit-Codes: 0, wenn kein Layout und keine Bahn Hinweise hat (Hinweise „Zur Information“ zählen nicht), 1, wenn das Projekt oder seine Bahndatei nicht geladen werden kann oder ein Layout, eine Bahn oder die Bahndatei Hinweise hat, 2, wenn die Befehlszeile falsch ist.
help-project = Die Projektdatei freemapper-print.toml oder der Ordner, in dem sie liegt.
export-about = Schreibt je Layout ein PDF, benannt nach der Layout-ID. Mit Bahndatei schreibt es je Bahn ein PDF, benannt nach der Bahn, auf dem Layout, das ihre Zuordnung nennt.
export-after-exit-codes = Exit-Codes: 0, wenn alle PDFs geschrieben sind, auch mit Hinweisen, 1, wenn das Projekt oder seine Bahndatei nicht geladen oder ein PDF nicht gezeichnet oder geschrieben werden kann, 2, wenn die Befehlszeile falsch ist oder ein unbekanntes Layout oder eine unbekannte Bahn nennt.
help-layout = Nur dieses Layout exportieren, ohne Bahnen. Für mehrere wiederholen. Ohne Angabe werden alle Layouts exportiert, bei einem Projekt mit Bahndatei alle Bahnen.
help-course = Nur diese Bahn exportieren. Für mehrere wiederholen. Ohne Angabe werden alle Bahnen der Bahndatei exportiert.
help-single-file = Alle Bahnen in ein PDF schreiben, eine Seite je Bahn, benannt nach dem Titel der Veranstaltung.
help-out-dir = Ordner für die PDFs. Er muss existieren. Ohne Angabe gilt der Ordner out neben der Projektdatei, der bei Bedarf angelegt wird. Ein PDF gleichen Namens wird überschrieben.

## Fehler

error-line = Fehler: { $message }
error-unknown-lang = unbekannte Sprache „{ $lang }“; verfügbar: { $available }
error-read-map = Die Karte { $path } konnte nicht gelesen werden
error-kind-unknown = Die Kartenart lässt sich aus der Datei nicht erkennen; --kind sprint, --kind schoolyard oder --kind forest angeben
error-out-folder-missing = Der Ordner { $path } für das PDF existiert nicht; zuerst anlegen
error-out-is-map = --out zeigt auf die Kartendatei { $path }; einen PDF-Pfad wählen, damit die Karte unversehrt bleibt
error-print = Die Karte konnte nicht gedruckt werden
error-load-project = Das Projekt { $path } konnte nicht geladen werden
error-base-part-hidden = { $path }: Feld `{ $field }`: Der Kartenteil „base“ ist die Grundfläche der Sprintkarte (belaufbare Fläche) und wird immer gedruckt; „base“ aus hide_parts entfernen
error-project-exists = { $path } gibt es schon; init überschreibt die Datei nicht
error-create-folder = Der Ordner { $path } konnte nicht angelegt werden
error-title-control = --title darf keine Zeilenumbrüche, Tabulatoren oder anderen Steuer- oder unsichtbaren Formatzeichen enthalten
error-unknown-layout = Das Projekt hat kein Layout { $id }; vorhanden sind { $known }
error-out-dir-missing = Der Ordner { $path } für die PDFs existiert nicht; zuerst anlegen
error-sheet = Layout { $layout } konnte nicht gezeichnet werden
error-course-sheet = Bahn „{ $course }“ konnte nicht gezeichnet werden
error-courses = Die Bahndatei lässt sich nicht verwenden
error-unknown-course = Die Bahndatei hat keine Bahn „{ $name }“; vorhanden sind { $known }
error-no-course-file = --course und --single-file brauchen eine Bahndatei, das Projekt nennt aber keine unter [sources] courses
error-disk-full = nicht genug Speicherplatz für { $path }: Nötig sind { $needed } MB samt 50 MB Reserve, frei sind { $available } MB
error-write-file = { $path } konnte nicht geschrieben werden
error-outside-project = { $path }: Ein Link führt aus dem Projektordner hinaus, deshalb wird dort nichts geschrieben; den Link durch einen echten Ordner ersetzen oder den Zielordner mit --out-dir angeben

## Hinweise und Warnungen nach dem Druck

word-landscape = Querformat
word-portrait = Hochformat
report-summary = { $out } geschrieben: { $paper } { $orientation }, Druckmaßstab 1:{ $print_scale }, Kartenmaßstab 1:{ $map_scale }, Geometriefaktor { $geometry }, Symbolfaktor { $symbol }.
note-sidecar = Hinweis: { $error }. Die Kartenart kommt stattdessen aus der Kartendatei.
warning-non-standard-scale = Warnung: 1:{ $print_scale } ist kein normgerechter Maßstab für { $kind }. Die Karte wird verkleinert, die Symbole behalten ihre Größe im Maßstab 1:{ $base_scale }. Dieser Druck taugt nur fürs Training.
word-kind-sprint = Sprintkarten
word-kind-schoolyard = Schulhofkarten
word-kind-forest = Waldkarten
overflow-too-wide = { $mm } mm zu breit
overflow-too-tall = { $mm } mm zu hoch
overflow-and = { $first } und { $second }
hint-paper-a3 = Papier im Format A3
hint-other-orientation = die andere Ausrichtung
hint-scale-larger = einen Druckmaßstab mit größerer Zahl
hint-scale-example = einen Druckmaßstab mit größerer Zahl, zum Beispiel 1:{ $next }
hint-scale-example-non-standard = einen Druckmaßstab mit größerer Zahl, zum Beispiel 1:{ $next } (nicht normgerecht, nur fürs Training)
warning-overflow = Warnung: Die Karte passt nicht auf die Seite. Sie ist { $cut } für den bedruckbaren Bereich. Versuchen Sie { $hints } oder { $last }.
warning-overflow-single = Warnung: Die Karte passt nicht auf die Seite. Sie ist { $cut } für den bedruckbaren Bereich. Versuchen Sie { $last }.
note-skipped = Hinweis: { $count ->
        [one] 1 Objekt wird
       *[other] { $count } Objekte werden
    } nicht gezeichnet, weil das Symbol unbekannt oder ausgeblendet ist.
note-unknown-type = Hinweis: { $count ->
        [one] 1 Objekt wird
       *[other] { $count } Objekte werden
    } nicht gezeichnet, weil das Programm ihren Objekttyp nicht kennt.
note-truncated = Hinweis: { $count ->
        [one] 1 Objekt wurde
       *[other] { $count } Objekte wurden
    } vereinfacht oder gekürzt, weil die Datei mehr Details verlangt, als der Renderer zeichnet.
note-page-limit = Hinweis: { $count ->
        [one] 1 Objekt wird
       *[other] { $count } Objekte werden
    } nicht gezeichnet, weil die Seite ihre Zeichengrenze erreicht hat.
note-dropped-pattern = Hinweis: { $count ->
        [one] 1 Objekt wird
       *[other] { $count } Objekte werden
    } ohne Flächenmuster gezeichnet, dessen Abstand unter 0,05 mm liegt oder keine Zahl ist.
note-fonts = Hinweis: Die Schrift Arimo ersetzt { $names }.
note-missing-glyphs = Hinweis: Die eingebettete Schrift kann diese Zeichen nicht darstellen, deshalb fehlen sie im PDF: { $chars }
reminder-actual-size = In Originalgröße drucken: 100 % („Tatsächliche Größe“), „An Seite anpassen“ ausgeschaltet. Jede andere Größe zerstört den Maßstab.

## Projektbefehle: init, check, export

init-written = { $path } geschrieben, mit einem Layout { $layout }: { $paper }, Passepartout { $margin } mm, Druckmaßstab 1:{ $scale }, für { $kind }.
init-no-fit = Warnung: Weder auf A4 noch auf A3 zeigt ein Blatt die Karte im Maßstab 1:{ $scale } ohne Hinweise oder Fehler, deshalb bleibt das Projekt bei A4. check meldet Folgendes:
init-contour-hint = Das Projekt schlägt eine Äquidistanz von { $value } m vor, üblich für { $kind }. Sehen Sie auf der Karte nach und bestätigen Sie den Wert oder ändern Sie map.contour_interval_m.
init-no-contour-hint = Für Schulhofkarten schlägt das Projekt keine Äquidistanz vor. Hat die Karte Höhenlinien, tragen Sie map.contour_interval_m ein; bis dahin meldet check sie als fehlend.
init-next = Als Nächstes freemapper-print check { $path } aufrufen.
init-kind-flag = Kartenart: { $kind }, wie mit --kind angegeben.
init-kind-sidecar = Kartenart: { $kind }, wie in der freemapper.toml neben der Karte angegeben.
init-kind-inferred = Kartenart: { $kind }, abgeleitet aus Symbolsatz und Maßstab der Karte. Stimmt das nicht, ändern Sie map.kind in der Projektdatei.
init-fitting = Suche ein A4- oder A3-Blatt, das die Karte in 1:{ $scale } zeigt …
init-next-with-notes = Als Nächstes freemapper-print check { $path } aufrufen. Es meldet die Hinweise oben, bis das Layout geändert ist.
# Kommentare in der neuen Projektdatei.
init-comment-contour = Vorschlag für { $kind }. An der Karte prüfen und bestätigen.
init-comment-no-contour = Für Schulhofkarten gibt es keinen Vorschlag. Hat die Karte Höhenlinien, contour_interval_m mit der Äquidistanz in Metern ergänzen.
init-comment-footer = Quellenangaben und Namensnennung stehen in der Fußzeile links unten, unter dem Kartenausschnitt, wenn das Passepartout breit genug ist, sonst im Kartenausschnitt. Ein Element copyright_line oder credit_line verschiebt sie.
check-summary = Layout { $layout }: { $paper } { $orientation }, Druckmaßstab 1:{ $print_scale }, Kartenmaßstab 1:{ $map_scale }, Geometriefaktor { $geometry }, Symbolfaktor { $symbol }.
check-clean = { $count ->
        [one] 1 Layout
       *[other] { $count } Layouts
    } geprüft, keine Hinweise.
check-with-notes = Layouts mit Hinweisen: { $count } von { $total }.
report-course-summary = { $out } für Bahn „{ $course }“ auf Layout { $layout } geschrieben: { $paper } { $orientation }, Druckmaßstab 1:{ $print_scale }, Kartenmaßstab 1:{ $map_scale }, Geometriefaktor { $geometry }, Symbolfaktor { $symbol }.
report-course-page = Seite { $page }, Bahn „{ $course }“ auf Layout { $layout }: { $paper } { $orientation }, Druckmaßstab 1:{ $print_scale }, Kartenmaßstab 1:{ $map_scale }, Geometriefaktor { $geometry }, Symbolfaktor { $symbol }.
report-single-file = { $out } mit { $count ->
        [one] 1 Seite
       *[other] { $count } Seiten
    } geschrieben.
check-course-summary = Bahn „{ $course }“ auf Layout { $layout }: { $paper } { $orientation }, Druckmaßstab 1:{ $print_scale }, Kartenmaßstab 1:{ $map_scale }, Geometriefaktor { $geometry }, Symbolfaktor { $symbol }.
check-courses-clean = { $count ->
        [one] 1 Bahn
       *[other] { $count } Bahnen
    } und { $layouts ->
        [one] 1 Layoutblatt
       *[other] { $layouts } Layoutblätter
    } geprüft, keine Hinweise.
check-courses-with-notes = Mit Hinweisen: { $count } von { $total ->
        [one] 1 Bahn
       *[other] { $total } Bahnen
    }, { $layouts_with_notes } von { $layouts ->
        [one] 1 Layoutblatt
       *[other] { $layouts } Layoutblättern
    }.
check-course-file-notes = Die Bahndatei selbst hat Hinweise, siehe die Zeilen über der ersten Bahn.

## Blatthinweise

list-and = { $first } und { $last }
list-more = { $count } weitere
note-element-outside-page = Hinweis: Das Element { $id } ragt über den Blattrand und wird dort abgeschnitten.
note-footer-outside-page = Hinweis: Die Fußzeile { $content ->
        [copyright] mit den Quellenangaben
        [credit] mit der Namensnennung
       *[both] mit Quellenangaben und Namensnennung
    } ragt über den Blattrand und wird dort abgeschnitten.
note-element-overlaps-map = Hinweis: Das Element { $id } hängt am Blatt und deckt einen Teil des Kartenausschnitts ab, die Karte darunter ist nicht zu sehen.
note-footer-overlaps-map = Hinweis: Das Passepartout ist zu schmal für die Fußzeile { $content ->
        [copyright] mit den Quellenangaben
        [credit] mit der Namensnennung
       *[both] mit Quellenangaben und Namensnennung
    }. Sie steht deshalb auf einem weißen Kasten im Kartenausschnitt und deckt einen Teil der Karte ab. Ein breiteres margin_mm oder ein Element copyright_line oder credit_line verschiebt sie.
note-larger-than-a3 = Hinweis: Das Papier ist größer als A3, das nehmen die meisten Bürodrucker nicht.
note-missing-scale-or-contour = Hinweis: Im Layout { $layout } { $count ->
        [one] fehlt
       *[other] fehlen
    } { $missing }.
missing-print-scale-text = ein Text mit { $name } für den Druckmaßstab
missing-contour-text = ein Text mit { $name } für die Äquidistanz
missing-contour-value = ein Wert für { $name }, die Äquidistanz in Metern
note-north-lines-added = Zur Information: Die Karte hat keine Nordlinien, deshalb { $count ->
        [one] wurde 1 Linie
       *[other] wurden { $count } Linien
    } ergänzt. Sie laufen ohne Unterbrechung über den Kartenausschnitt. Sie an kleinen Objekten wie Steinen oder Wegenden zu unterbrechen, ist Aufgabe des Kartografen in OOM.
note-north-lines-without-symbol = Hinweis: Die Karte hat weder Nordlinien noch ein brauchbares Liniensymbol 601, deshalb { $count ->
        [one] wurde 1 schlichte schwarze Linie
       *[other] wurden { $count } schlichte schwarze Linien
    } über die Karte gezeichnet.
note-copyright-truncated = Hinweis: Layout { $layout } zeigt { $shown } von { $total } Quellenangaben. Eine weitere Zeile zählt die übrigen.
note-logo-skipped = Hinweis: Das Logo { $id } fehlt auf dem Blatt: { $reason }
note-logo-simplified = Hinweis: Das Logo { $id } wird vereinfacht gezeichnet: { $reason }
note-unknown-placeholder = Hinweis: Der Text { $id } enthält { $name }. Das ist kein Platzhalter und wird so gedruckt, wie es dasteht.
note-unknown-part = Hinweis: Layout { $layout } blendet den Kartenteil { $name } aus, die Karte hat aber keinen Teil dieses Namens. Groß- und Kleinschreibung zählt.
note-credit-line-moved = Hinweis: Das Element { $id } läge mit der Namensnennung nicht ganz auf dem Blatt. Die Namensnennung bleibt deshalb in der Fußzeile unten links.
note-render-footer-on-map = Hinweis: Die Fußzeile { $content ->
        [copyright] mit den Quellenangaben
        [credit] mit der Namensnennung
       *[both] mit Quellenangaben und Namensnennung
    } steht auf einem weißen Kasten unten links im Druckbereich und deckt einen Teil der Karte ab. In einem Projekt (init, dann export) kann sie unter den Kartenausschnitt rücken.
note-render-copyright-truncated = Hinweis: Die Fußzeile zeigt { $shown } von { $total } Quellenangaben. Eine weitere Zeile zählt die übrigen.

## Warum ein Logo fehlt oder vereinfacht ist (nach „Das Logo … fehlt auf dem Blatt:“ oder „wird vereinfacht gezeichnet:“)

logo-reason-needs-file-and-width = Es braucht file und width_mm.
logo-reason-not-found = { $file } gibt es nicht.
logo-reason-unreadable = { $file } lässt sich nicht lesen ({ $error }).
logo-reason-not-a-file = { $file } ist keine normale Datei.
logo-reason-file-too-large = { $file } ist größer als 10 MB.
logo-reason-unknown-format = { $file } ist keine PNG-, JPEG- oder SVG-Datei.
logo-reason-too-many-pixels = { $file } hat { $width } × { $height } Pixel, mehr als 25 Megapixel. Speichern Sie es mit weniger Pixeln.
logo-reason-too-wide = { $file } ist { $width } × { $height } Pixel groß, breiter oder höher als 65 535 Pixel.
logo-reason-undecodable = { $file } lässt sich nicht dekodieren ({ $error }).
logo-reason-decoder-limit = { $file } ist zu groß zum Dekodieren. Speichern Sie es als 8-Bit-Bild oder mit weniger Pixeln.
logo-reason-svg-not-utf8 = { $file } ist kein UTF-8-Text.
logo-reason-svg-unreadable = { $file } lässt sich nicht als SVG lesen ({ $error }).
logo-reason-svg-unsupported = { $file } nutzt etwas, das ein Logo nicht haben kann: { $detail }. Exportieren Sie das Logo als einfaches SVG mit Formen, Pfaden und Verläufen.
logo-reason-svg-too-complex = { $file } ist zu aufwendig für ein Logo: { $detail }. Vereinfachen Sie das Logo oder exportieren Sie es als einfaches SVG.
logo-reason-cannot-size = Breite oder Höhe von { $file } lassen sich nicht bestimmen.
logo-reason-pixel-budget = Das Blatt trägt schon 50 Megapixel an Logos ({ $file }). Nehmen Sie kleinere Bilder oder weniger Logos.
logo-reason-svg-budget = Die SVG-Logos des Blatts haben ihr Budget von 1 000 000 Einheiten schon aufgebraucht ({ $file }). Vereinfachen Sie sie oder nehmen Sie weniger.
logo-simplified-gradient = Ein Farbverlauf wird in seiner ersten Farbe gezeichnet.
logo-simplified-pattern = Eine Musterfüllung wird schwarz gezeichnet.
logo-simplified-opacity = Transparenz wird ignoriert, das Logo wird deckend gezeichnet.
logo-simplified-dashes = Strichelungen werden als durchgehende Linien gezeichnet.
logo-simplified-image = Ein eingebettetes Bild fehlt.
logo-simplified-text = Text fehlt; wandeln Sie ihn in Pfade um.
logo-simplified-clip-path = Ein Beschneidungspfad wird ignoriert.
note-missing-scale-check-bar = Hinweis: Layout { $layout } hat keine Kontrollstrecke. Ein Element vom Typ scale_check_bar zeigt am Ausdruck, ob der Drucker skaliert hat.
note-footer-covers-element = Hinweis: Die Fußzeile { $content ->
        [copyright] mit den Quellenangaben
        [credit] mit der Namensnennung
       *[both] mit Quellenangaben und Namensnennung
    } unten links deckt einen Teil des Elements { $id } ab. Versetzen Sie das Element, oder verschieben Sie die Zeilen der Fußzeile mit einem Element copyright_line oder credit_line.
error-disk-full-unknown = nicht genug Speicherplatz für { $path }: Nötig sind { $needed } MB samt 50 MB Reserve, wie viel frei ist, meldet der Datenträger nicht
note-copyright-line-cut = Hinweis: Die Quellenangabe „{ $line }“ ist breiter als der Kartenausschnitt und wird mit „…“ gekürzt. Kürzen Sie sie in den Kartennotizen oder in print.copyright.
hint-crop = Für ein Blatt, das nur einen Teil der Karte zeigt, wählt map_frame.centre_map_mm den Teil, und map_frame.crop = true kennzeichnet den Schnitt als gewollt.
note-map-cropped = Zur Information: Die Karte ist { $cut } für den Kartenausschnitt und wird dort mit Absicht beschnitten (map_frame.crop = true).

## Bahnblätter

note-course-outside-frame = Hinweis: Auf dem Blatt der Bahn „{ $course }“ { $count ->
        [one] liegt Posten { $controls }
       *[other] liegen die Posten { $controls }
    } außerhalb des Kartenausschnitts oder auf seinem Rand. Ohne map_frame.centre_map_mm wird der Ausschnitt auf die Bahn zentriert; größeres Papier oder ein Druckmaßstab mit größerer Zahl zeigt mehr.
note-default-layout = Zur Information: Die Bahn „{ $course }“ hat keine Zuordnung und wird auf dem ersten Layout gedruckt, { $layout }.
note-no-description-box-iof-xml = Zur Information: Die Bahn „{ $course }“ kommt aus IOF-XML. Das Format kennt keine Postenbeschreibungen, das Blatt hat deshalb keine.
note-no-description-box-no-element = Zur Information: Das Layout der Bahn „{ $course }“ hat kein Element control_descriptions, das Blatt hat deshalb keine Postenbeschreibung.
note-no-description-box-outside-page = Hinweis: Die Postenbeschreibung der Bahn „{ $course }“ würde über den Blattrand reichen und fehlt deshalb. Versetzen Sie sie über ihren Anker oder verkleinern Sie sie mit cell_mm.
note-no-description-box-no-descriptions = Hinweis: Kein Posten der Bahn „{ $course }“ hat in der Bahndatei eine Beschreibung, das Blatt hat deshalb keine Postenbeschreibung. Tragen Sie die Beschreibungen in Purple Pen ein.
note-element-covers-course = Hinweis: Das Element { $id } deckt { $count ->
        [one] den Posten { $controls }
       *[other] die Posten { $controls }
    } der Bahn ab. Versetzen Sie das Element über seinen Anker, damit die Bahn sichtbar bleibt.
note-unknown-description-symbol = Hinweis: Die Postenbeschreibung der Bahn „{ $course }“ nutzt das Symbol { $id }, das der Beschreibungsstandard der Bahndatei nicht kennt. Sein Kästchen bleibt leer.
note-overprint-cut = Hinweis: Der Bahnaufdruck der Bahn „{ $course }“ verlangt mehr Schraffurlinien, Striche oder Kreuze, als ein Blatt zeichnet, deshalb fehlen einige.
note-masks-capped = Hinweis: Die Bahn „{ $course }“ hat { $total } Abdeckungen, ein Blatt wendet höchstens { $applied } an. Die übrigen fehlen, und wo nur sie abdecken würden, ist die Karte zu sehen.
note-overprint-in-masked-area = Hinweis: Auf dem Blatt der Bahn „{ $course }“ { $count ->
        [one] liegt
       *[other] liegen
    } { $objects } ganz oder teilweise dort, wo die Abdeckung die Karte verbirgt. Der Bahnaufdruck wird dort trotzdem gedruckt, nur ohne Karte darunter. Ergänzen oder vergrößern Sie ein Fenster, oder prüfen Sie, ob die Bahn die richtige Abdeckung hat.
masked-marked-route = { $count ->
        [one] eine Pflichtstrecke
       *[other] { $count } Pflichtstrecken
    }
masked-crossing-point = { $count ->
        [one] eine Querungsstelle
       *[other] { $count } Querungsstellen
    }
masked-forbidden-route = { $count ->
        [one] eine verbotene Strecke
       *[other] { $count } verbotene Strecken
    }
masked-boundary = { $count ->
        [one] eine Sperrgebietsgrenze
       *[other] { $count } Sperrgebietsgrenzen
    }
masked-out-of-bounds-area = { $count ->
        [one] ein Sperrgebiet
       *[other] { $count } Sperrgebiete
    }
masked-dangerous-area = { $count ->
        [one] ein Gefahrengebiet
       *[other] { $count } Gefahrengebiete
    }
masked-text = { $count ->
        [one] ein Text aus der Bahndatei
       *[other] { $count } Texte aus der Bahndatei
    }
note-control-in-masked-area = Hinweis: Auf dem Blatt der Bahn „{ $course }“ { $count ->
        [one] liegt
       *[other] liegen
    } { $controls } dort, wo die Abdeckung die Karte verbirgt. Die Läufer finden dort keine Karte. Ergänzen Sie ein Fenster, oder prüfen Sie, ob die Bahn die richtige Abdeckung hat.
note-mask-covers-nothing = Zur Information: Die Abdeckung „{ $mask }“ verbirgt im Kartenausschnitt der Bahn „{ $course }“ nichts. Falls sie es soll, prüfen Sie ihre Punkte. Sie sind Karten-mm mit der y-Achse nach oben, wie OOM sie anzeigt.
note-white-outs-cover-nothing = Zur Information: Im Kartenausschnitt der Bahn „{ $course }“ { $count ->
        [one] verbirgt die Abdeckfläche { $ids } aus der Bahndatei nichts.
       *[other] verbergen die { $count } Abdeckflächen { $ids } aus der Bahndatei nichts.
    }
note-element-covers-window = Hinweis: Auf dem Blatt der Bahn „{ $course }“ deckt das Element { $id } einen Teil der Fenster der Abdeckung „{ $mask }“ ab. Auf einem Blatt mit Abdeckung zeigen nur die Fenster die Karte, und das Element verbirgt einen Teil davon. Versetzen Sie das Element über seinen Anker, oder legen Sie das Fenster anders.
note-mask-leaves-no-window = Hinweis: Die Abdeckung „{ $mask }“ verbirgt den ganzen Kartenausschnitt der Bahn „{ $course }“, das Blatt zeigt also gar keine Karte. Prüfen Sie ihre Fenster und ihre Punkte. Sie sind Karten-mm mit der y-Achse nach oben, wie OOM sie anzeigt.
note-mask-check-limited = Hinweis: Auf dem Blatt der Bahn „{ $course }“ hat die Prüfung gegen die Abdeckung ihre Grenze erreicht, ein Teil des Bahnaufdrucks blieb ungeprüft. Suchen Sie auf dem Blatt nach Bahnsymbolen ohne Karte darunter.
note-course-value-unknown = Zur Information: Der Text { $id } nutzt { $name }, die Bahndatei hat für die Bahn „{ $course }“ aber keinen Wert. Der Platzhalter bleibt leer.
note-number-conflict = Hinweis: Auf dem Blatt der Bahn „{ $course }“ überdeckt die Nummer von Posten { $control } { $conflicts }. Sie wird trotzdem dort gedruckt. { $proposal } Um diese Lage zu nutzen, rufen Sie numbers accept mit --course „{ $course }“, --control { $control } und --sheet oder --layout-wide auf.
note-number-gap-out-of-range = Hinweis: Auf dem Blatt der Bahn „{ $course }“ steht die Nummer von Posten { $control } { $gap } mm vom Kreis entfernt. Das Regelprofil erlaubt { $min } bis { $max } mm. Sie wird trotzdem mit diesem Abstand gedruckt; eine Korrektur unter [[numbers]] in der Projektdatei kann ihn ändern.
number-conflict-symbol = das Symbol von Posten { $control }
number-conflict-leg = die Strecke von { $from } nach { $to }
number-conflict-number = die Nummer von Posten { $control }
number-conflict-element = das Element { $id }
number-conflict-outside = den Rand des Kartenausschnitts
number-conflict-point-symbol = { $count ->
        [one] das Punktsymbol { $code }
       *[other] { $count } Objekte des Punktsymbols { $code }
    } der Karte
number-conflict-line-symbol = { $count ->
        [one] das Liniensymbol { $code }
       *[other] { $count } Objekte des Liniensymbols { $code }
    } der Karte
note-number-conflict-unsearched = Hinweis: Auf dem Blatt der Bahn „{ $course }“ überdeckt die Nummer von Posten { $control } { $conflicts }. Sie wird trotzdem dort gedruckt. Das Blatt hat seine Suchgrenze erreicht, deshalb gibt es keinen Vorschlag. Eine Lage setzen Sie mit numbers accept, --direction und --gap.
note-number-search-limited = Hinweis: Auf dem Blatt der Bahn „{ $course }“ hat die Suche nach Plätzen für die Postennummern ihre Grenze erreicht. Die Nummern, zu denen sie nicht mehr kam, bleiben, wo ihre Ebene sie hinsetzt, automatisch gesetzte rechts oben am Kreis, und bekommen keinen Vorschlag. Prüfen Sie das Blatt.
number-proposal-free = Frei ist Richtung { $direction }° mit { $gap } mm Abstand.
number-proposal-none = Einen freien Platz gibt es nicht; am wenigsten verdeckt Richtung { $direction }° mit { $gap } mm Abstand.

## Hinweise zur Bahndatei

note-course-unknown-element = Hinweis: Die Bahndatei enthält das Element { $name }, das diese Version nicht liest. Es fehlt auf den Blättern.
note-course-forked = Zur Information: Die Bahnen { $variants } sind die Varianten der Gabelung „{ $family }“. Jede wird als eigenes Blatt gedruckt.
note-course-alternative-controls = Hinweis: Die Bahn „{ $course }“ bietet an einer Stelle alternative Posten an. Das kann diese Version nicht drucken, die Bahn fehlt deshalb.
note-course-notes-truncated = Zur Information: Die Bahndatei hat mehr Hinweise ergeben, als hier stehen.
note-course-white-out = Zur Information: Die Bahndatei hat Abdeckflächen (White-out). Jede deckt die Karte auf den Bahnen ab, für die sie gilt, außer wenn die Zuordnung einer Bahn eine eigene Abdeckung nennt.
note-course-no-descriptions = Zur Information: Die Bahndatei hat keine Postenbeschreibungen.
note-course-map-issue-as-start = Zur Information: Die Kartenausgabe { $code } dient als Start.
note-course-variations = Hinweis: Die Bahn „{ $course }“ hat Gabelungen oder Schleifen (Varianten in Purple Pen). Diese Version kann sie nicht drucken, die Bahn fehlt deshalb. Der IOF-XML-Export von Purple Pen schreibt jede Variante als eigene Bahn.
note-fit-y-up = Zur Information: Die y-Achse der Bahndatei zeigt nach oben; das Programm hat sie zur Karte passend gedreht.
note-fit-orientation-assumed = Hinweis: Die Bahn passt mit der y-Achse nach oben und nach unten auf die Karte, und nichts in der Datei entscheidet. Das Programm hat „oben“ genommen. Prüfen Sie ein Blatt und tragen Sie dann in der Projektdatei unter [sources] course_y_axis = "up" oder "down" ein.
note-fit-axis-against-format = Hinweis: Die Projektdatei setzt unter [sources] course_y_axis = "{ $set }", aber Purple Pen schreibt die y-Achse immer nach oben. So steht die Bahn gespiegelt auf dem Blatt. Stammt die Datei aus Purple Pen, löschen Sie die Zeile.
note-fit-scale-unchecked = Zur Information: Die Bahndatei nennt keinen Kartenmaßstab, deshalb wurde er nicht mit dem der Karte verglichen.
note-fit-georef-checked = Zur Information: { $count ->
        [one] 1 Posten hat
       *[other] { $count } Posten haben
    } eine Kartenposition und geografische Koordinaten. Beide weichen höchstens { $metres } m voneinander ab.
note-fit-georef-skipped = Zur Information: Die geografischen Koordinaten wurden nicht geprüft, denn { $reason }.
fit-skip-not-georeferenced = die Karte ist nicht georeferenziert
fit-skip-unsupported-crs = die Karte nutzt das Koordinatensystem { $crs }, das Programm unterstützt aber nur UTM
fit-skip-no-lat-lng = kein Posten hat sowohl eine Kartenposition als auch geografische Koordinaten
fit-skip-check-off = die Projektdatei schaltet die Prüfung ab
note-fit-georef-check-off = Zur Information: Die Projektdatei schaltet die Prüfung mit geografischen Koordinaten ab (course_georef_check = false), das Programm hat sie deshalb nicht beachtet.
note-fit-placed-by-georef = Zur Information: { $count ->
        [one] Posten { $controls } hat
       *[other] Die Posten { $controls } haben
    } nur geografische Koordinaten. Die Georeferenzierung der Karte hat sie platziert.
note-stale-assignment = Hinweis: Die Projektdatei ordnet die Bahn „{ $course }“ einem Layout zu, die Bahndatei hat aber keine solche Bahn. Die Zuordnung bleibt in der Projektdatei.
note-stale-correction = Hinweis: Die Projektdatei korrigiert die Nummer von Posten { $control } für { $scope }, die Bahndatei hat dort aber keinen solchen Posten. Die Korrektur bleibt in der Projektdatei.
stale-scope-layout = Layout { $name }
stale-scope-course = Bahn „{ $name }“
note-stale-changed-base = Hinweis: Die Bahndatei hat die Nummer von Posten { $control } verschoben, seit die Projektdatei sie korrigiert. Prüfen Sie, ob die Korrektur noch passt.
note-stale-mask = Hinweis: Die Projektdatei gibt der Bahn „{ $course }“ die Abdeckung „{ $mask }“, aber die Bahndatei hat keine solche Abdeckfläche (White-out). Die Bahn wird ohne Abdeckung gedruckt; die Zuordnung bleibt in der Projektdatei.
note-mask-unused = Zur Information: Keine Bahn nutzt die Abdeckung „{ $id }“. Nennen Sie sie in einer Zuordnung mit mask = "{ $id }", oder löschen Sie ihren Eintrag [[masks]].
note-white-out-by-number = Zur Information: Die Bahn „{ $course }“ wird mit der Abdeckfläche { $id } aus der Bahndatei gedruckt, die um { $x }, { $y } liegt (Karten-mm, y nach oben). Die Nummer zählt die Abdeckflächen in der Reihenfolge der Bahndatei. Löschen Sie in Purple Pen eine Abdeckfläche oder ändert sich ihre Reihenfolge, meint dieselbe Nummer eine andere Fläche. Prüfen Sie dann die Zuordnung.
note-white-out-stand-in = Zur Information: Auf dem Blatt der Bahn „{ $course }“ tritt die Tabelle [[masks]] mit der ID { $id } an die Stelle der Abdeckfläche aus der Bahndatei, die um { $x }, { $y } liegt (Karten-mm, y nach oben). Die Nummer zählt die Abdeckflächen in der Reihenfolge der Bahndatei. Löschen Sie in Purple Pen eine Abdeckfläche oder ändert sich ihre Reihenfolge, ersetzt die Tabelle eine andere Fläche. Prüfen Sie dann ihre ID.
note-map-cropped-to-course = Zur Information: Die Karte ist { $cut } für den Kartenausschnitt. Ein Bahnblatt zeigt den Teil um die Bahn, der Rest wird mit Absicht abgeschnitten.
note-course-placeholder-not-filled = Zur Information: Der Text { $id } nutzt { $name }, der nur auf Bahnblättern gefüllt wird. Auf diesem Blatt bleibt er leer.
note-no-courses-to-export = Hinweis: Die Bahndatei hat keine Bahn zum Exportieren, deshalb wurde kein PDF geschrieben.
quoted = „{ $text }“

# ---- init --courses, numbers accept (M3 Task 11) ----
help-init-courses = Die Bahndatei, .ppen (Purple Pen) oder .xml (IOF-XML 3.0). Die Projektdatei speichert ihren Pfad relativ zu sich selbst. Aus einer .ppen entstehen ein Layout je Druckmaßstab der Datei, eine Zuordnung je Bahn und ein Beschreibungskasten; aus IOF-XML ein Layout im Kartenmaßstab.
numbers-about = Ändert die Lage der Postennummern in der Projektdatei.
numbers-accept-about = Fügt der Projektdatei eine Korrektur für eine Postennummer hinzu. Ohne --direction und --gap nimmt es den Vorschlag, den check für diesen Posten zeigt.
numbers-accept-after-content = Die neue Tabelle [[numbers]] kommt als Text ans Ende der Datei. Alles davor bleibt unverändert, auch die Kommentare. Danach lädt das Programm die Datei neu; schlägt das fehl, stellt es den alten Inhalt wieder her.
numbers-accept-after-exit-codes = Rückgabewerte: 0, wenn die Korrektur geschrieben ist; 1, wenn sich das Projekt oder seine Bahndatei nicht laden oder schreiben lässt oder die geänderte Datei nicht lädt (der alte Inhalt wird wiederhergestellt); 2, wenn die Befehlszeile falsch ist, die Bahn oder der Posten unbekannt ist, check für den Posten keinen Vorschlag zeigt oder die Datei für denselben Posten und Geltungsbereich schon eine Korrektur hat.
help-numbers-course = Name der Bahn, wie in der Bahndatei.
help-numbers-control = Code des Postens.
help-numbers-layout-wide = Die Korrektur gilt für alle Bahnen, die auf dem Layout dieser Bahn gedruckt werden.
help-numbers-sheet = Die Korrektur gilt nur für das Blatt dieser Bahn. Sie geht einer Layout-Korrektur vor.
help-numbers-direction = Richtung in Grad, 0 ist Osten, gegen den Uhrzeigersinn. Braucht --gap. Wird mit einer Nachkommastelle geschrieben.
help-numbers-gap = Abstand zwischen Nummer und Kreis in Druck-mm, größer als 0 und höchstens 10. Braucht --direction. Wird mit zwei Nachkommastellen geschrieben.
init-courses-read = Bahndatei { $path } gelesen: { $count ->
        [one] 1 Bahn
       *[other] { $count } Bahnen
    }, an die Karte angepasst.
init-courses-scales-ppen = Die Druckmaßstäbe stammen aus der Bahndatei, ein Layout je Maßstab. Eine Bahn ohne Druckmaßstab bekommt den Kartenmaßstab 1:{ $scale }.
init-courses-scales-iof = Die Bahndatei ist IOF-XML. Sie hat keine Druckmaßstäbe, deshalb werden alle Bahnen auf einem Layout im Kartenmaßstab 1:{ $scale } gedruckt. Sie hat auch keine Postenbeschreibungen, das Layout bekommt also keinen Beschreibungskasten.
init-courses-description-box = Jedes Layout hat einen Beschreibungskasten in der oberen rechten Ecke des Kartenausschnitts, mit der Zellengröße des Regelprofils.
init-courses-crop = Ein Bahnblatt schneidet die Karte mit Absicht auf die Bahn zu, deshalb haben die Layouts map_frame.crop = true.
init-written-courses = { $path } geschrieben mit { $layouts ->
        [one] einem Layout
       *[other] { $layouts } Layouts
    } und { $assignments ->
        [one] einer Bahnzuordnung
       *[other] { $assignments } Bahnzuordnungen
    }, für { $kind }.
init-courses-layout = Layout { $layout } bei 1:{ $scale }: { $paper }, Passepartout { $margin } mm, für { $courses }.
init-courses-no-fit = Warnung: Kein A4- oder A3-Blatt zeigt die Karte bei 1:{ $scale } ohne Hinweise oder Fehler, deshalb bleibt Layout { $layout } bei A4. check meldet das.
error-read-courses = die Bahndatei { $path } ließ sich nicht lesen
error-numbers-no-course-file = numbers accept braucht eine Bahndatei, das Projekt nennt aber keine unter [sources] courses
error-unknown-control = die Bahn „{ $course }“ hat keinen Posten { $control }; ihre Posten sind { $known }
error-numbers-no-proposal = für Posten { $control } auf dem Blatt der Bahn „{ $course }“ gibt es keinen Vorschlag, denn seine Nummer hat keinen Konflikt. Mit --direction und --gap setzen Sie eine Lage selbst.
error-numbers-layout-conflict = der Vorschlag für Posten { $control } (Richtung { $direction }° mit { $gap } mm Abstand) stammt vom Blatt der Bahn „{ $course }“ und ist auf dem Layout { $layout } nicht für jede Bahn frei: Er hat einen Konflikt auf den Blättern von { $courses }. Nehmen Sie --sheet für das Blatt der Bahn „{ $course }“ allein, oder geben Sie mit --direction und --gap eine Lage für alle selbst an. Die Projektdatei blieb unverändert.
error-numbers-exists = die Projektdatei hat für Posten { $control } schon eine Korrektur für { $scope }. Ändern Sie sie in der Datei; numbers accept legt nur neue an.
error-numbers-value = --direction braucht eine endliche Zahl, und --gap muss größer als 0 und höchstens { $max } mm sein
numbers-accepted = Korrektur für die Nummer von Posten { $control } für { $scope } in { $path } eingetragen: Richtung { $direction }° mit einem Abstand von { $gap } mm. Mit check sehen Sie das Ergebnis.
error-load-changed-project = das Projekt { $path } lässt sich mit der neuen Korrektur nicht laden, deshalb blieb es unverändert

# ---- masks window, masks corridor (M4 Task 4) ----
masks-about = Fügt der Projektdatei Abdeckungen für Fenster- und Korridor-OL hinzu.
masks-window-about = Fügt eine Abdeckung für Fenster-OL hinzu: Die Karte bleibt nur in einem Kreis um jeden Posten einer Bahn sichtbar.
masks-window-after-content = Der Radius ist in Druck-mm auf dem Blatt angegeben, im Druckmaßstab des Layouts, auf dem die Bahn gedruckt wird. So sieht ein Fenster in jedem Maßstab gleich aus. Die Abdeckung speichert ihn aber in Karten-mm. Ändern Sie später den Druckmaßstab, löschen Sie die Abdeckung und rufen Sie den Befehl noch einmal auf. Ohne --controls bekommt jeder Posten der Bahn ein Fenster. --controls nimmt Postencodes mit Kommas dazwischen (31,32,35) oder zwei Postennummern mit einem Bindestrich (3-6) für die Posten, die auf dem Blatt die Nummern 3 bis 6 tragen; Start und Ziel zählen dabei nicht mit. Start und Ziel bekommen immer ein Fenster, außer mit --no-start und --no-finish. Überlappende Fenster verschmelzen zu einem. Die neue Tabelle [[masks]] kommt als Text ans Ende der Datei, mit einem Kommentar, wie sie entstand; mit --assign bekommt die Zuordnung der Bahn die Abdeckung, oder nach der Tabelle folgt eine neue Zuordnung. Alles davor bleibt unverändert, auch die Kommentare. Danach lädt das Programm die Datei neu; schlägt das fehl, stellt es den alten Inhalt wieder her.
masks-after-exit-codes = Rückgabewerte: 0, wenn die Abdeckung geschrieben ist; 1, wenn sich das Projekt, seine Karte oder seine Bahndatei nicht laden oder schreiben lässt oder die geänderte Datei nicht lädt (der alte Inhalt wird wiederhergestellt); 2, wenn die Befehlszeile falsch ist: eine unbekannte Bahn oder ein unbekannter Posten, ein Wert außerhalb des Bereichs, eine ID, die die Datei schon hat, eine Bahn mit einer anderen Abdeckung oder ein Korridor, der für seine Breite zu scharf abbiegt.
masks-corridor-about = Fügt eine Abdeckung für Korridor-OL hinzu: Die Karte bleibt nur in einem Korridor entlang einer Linie sichtbar, die Sie angeben. Keine Regel legt fest, wo ein Korridor verläuft, deshalb zeichnet das Programm keinen von sich aus.
masks-corridor-after-content = Die Punkte sind Karten-mm mit der y-Achse nach oben, wie OOMs Statusleiste sie zeigt, genau wie centre_map_mm und die Punkte unter [[masks]]. Die Breite ist in Druck-mm auf dem Blatt angegeben: mit --course im Druckmaßstab des Layouts dieser Bahn, sonst im Druckmaßstab, den alle Layouts gemeinsam haben. Die Abdeckung speichert sie aber in Karten-mm. Ändern Sie später den Druckmaßstab, löschen Sie die Abdeckung und rufen Sie den Befehl noch einmal auf. Der Korridor reicht die halbe Breite zu jeder Seite der Linie. Seine Enden sind gerade abgeschnitten; Knicke bis 120° behalten eine spitze Ecke, schärfere werden abgeschrägt. Das Programm lehnt einen Korridor ab, der auf sich selbst zurückläuft, ein Stück hat, das für den Knick an seinem Ende zu kurz ist, oder sich selbst kreuzt; vereinfachen Sie dann die Linie dort oder verringern Sie die Breite. Die neue Tabelle [[masks]] kommt als Text ans Ende der Datei, wie bei masks window.
help-masks-window-course = Name der Bahn, wie in der Bahndatei. Ihre Posten bekommen die Fenster, und ihr Layout gibt den Druckmaßstab vor.
help-masks-controls = Welche Posten ein Fenster bekommen: Codes mit Kommas dazwischen (31,32,35) oder Postennummern als Bereich (3-6). Ohne Angabe jeder Posten. Start und Ziel bekommen ihr Fenster in jedem Fall.
help-masks-radius = Radius jedes Fensters in Druck-mm, von 1 bis 100. Ohne Angabe 15.
help-masks-radius-for = Radius für einen Posten, CODE=MM, etwa 35=20; der Start hat meist den Code STA1, das Ziel FIN1. Darf mehrmals angegeben werden.
help-masks-no-start = Der Start bekommt kein Fenster.
help-masks-no-finish = Das Ziel bekommt kein Fenster.
help-masks-window-id = ID der neuen Abdeckung. Ohne Angabe window-<Bahn>, mit angehängter Nummer, wenn es die schon gibt.
help-masks-window-assign = Die Bahn wird mit der neuen Abdeckung gedruckt: Ihre Zuordnung bekommt sie, oder eine neue Zuordnung kommt hinzu.
help-masks-points = Die Linie als x,y;x,y;… in Karten-mm mit y nach oben, Dezimalstellen mit Punkt, mindestens 2 Punkte.
help-masks-width = Breite des Korridors in Druck-mm, von 1 bis 100. Ohne Angabe 10.
help-masks-corridor-id = ID der neuen Abdeckung. Ohne Angabe corridor-<n> mit der ersten freien Nummer.
help-masks-corridor-assign = Die Bahn aus --course wird mit der neuen Abdeckung gedruckt. Braucht --course.
help-masks-corridor-course = Die Bahn, deren Layout den Druckmaßstab für die Breite vorgibt.
error-masks-no-course-file = masks window und masks corridor --course brauchen eine Bahndatei, das Projekt nennt aber keine unter [sources] courses
error-masks-map-scale = die Karte nennt keinen Maßstab, deshalb lassen sich Druck-mm nicht in Karten-mm umrechnen
error-masks-id = eine Abdeckungs-ID braucht 1 bis { $max } Zeichen und keine Steuerzeichen
error-masks-id-taken = die Projektdatei hat schon eine Abdeckung „{ $id }“. Wählen Sie mit --id eine andere ID, oder ändern Sie die Abdeckung in der Datei.
error-masks-course-has-mask = die Bahn „{ $course }“ wird schon mit der Abdeckung „{ $mask }“ gedruckt. Ändern Sie ihre Zuordnung in der Projektdatei, wenn sie eine andere nutzen soll; das Programm ersetzt sie nicht.
error-masks-assignment-inline = die Zuordnung der Bahn „{ $course }“ steht nicht als eigene Tabelle [[assignments]] in der Datei, deshalb kann das Programm ihr die Abdeckung nicht hinzufügen. Tragen Sie mask = "{ $id }" von Hand ein. Die Projektdatei blieb unverändert.
error-masks-assignments-inline = die Zuordnungen stehen nicht als Tabellen [[assignments]] in der Datei, deshalb kann das Programm für die Bahn „{ $course }“ keine neue anhängen. Tragen Sie eine Zuordnung mit mask = "{ $id }" von Hand ein. Die Projektdatei blieb unverändert.
error-masks-radius-for = --radius-for nimmt CODE=MM, etwa 35=20, und „{ $text }“ ist nicht so geschrieben
error-masks-size = { $value } mm liegt außerhalb von { $min } bis { $max } mm; Radius und Breite sind Druck-mm auf dem Blatt
error-masks-selection = --controls nimmt Postencodes mit Kommas dazwischen (31,32,35) oder einen Bereich von Postennummern (3-6), und „{ $text }“ ist keins von beidem
error-masks-range = die Bahn „{ $course }“ hat die Postennummern 1 bis { $count }, deshalb passt der Bereich { $from }-{ $to } nicht
error-masks-radius-unselected = Posten { $control } bekommt kein Fenster, deshalb kann --radius-for seinen Radius nicht setzen
error-masks-no-controls = die Bahn „{ $course }“ hat keinen Posten, um den ein Fenster passt
error-masks-no-position = Posten { $control } hat keine Lage auf der Karte
error-masks-too-many-points = die Abdeckung hätte mehr als { $max } Punkte. Nehmen Sie weniger Fenster oder einen kleineren Radius.
error-masks-points = --points nimmt Punkte als x,y;x,y;… in Karten-mm mit y nach oben, Dezimalstellen mit Punkt, und „{ $text }“ ist keine solche Liste
error-masks-too-few-points = ein Korridor braucht mindestens 2 verschiedene Punkte
error-masks-too-many-corridor-points = ein Korridor nimmt höchstens { $max } Punkte
error-masks-point-out-of-reach = der Punkt { $x }, { $y } liegt jenseits von ±2 000 000 mm, der Reichweite einer OOM-Karte
error-masks-sharp-turn = der Korridor knickt bei { $x }, { $y } für seine Breite zu scharf ab, seine Ränder würden sich kreuzen. Vereinfachen Sie die Linie dort oder verringern Sie die Breite.
error-masks-self-intersecting = der Korridor kreuzt sich selbst bei { $x }, { $y }. Vereinfachen Sie die Linie oder verringern Sie die Breite.
error-masks-invalid = die erzeugte Abdeckung besteht die Prüfung der Projektdatei nicht ({ $problem }). Das ist ein Fehler im Programm; bitte melden Sie ihn.
error-masks-scales = die Layouts drucken in verschiedenen Maßstäben ({ $scales }), und die Breite ist in Druck-mm angegeben. Geben Sie --course an, damit die Breite dem Layout dieser Bahn folgt.
error-load-changed-mask = das Projekt { $path } lässt sich mit der neuen Abdeckung nicht laden, deshalb blieb es unverändert
stop-start = der Start
stop-start-acc = den Start
stop-finish = das Ziel
stop-controls = { $count ->
        [one] Posten { $codes }
       *[other] die Posten { $codes }
    }
stop-start-code = Start ({ $code })
stop-finish-code = Ziel ({ $code })
masks-window-comment = Fenster mit { $radius } mm Radius auf dem Papier bei 1:{ $scale } um { $controls } der Bahn „{ $course }“, erzeugt mit masks window.
masks-window-radius-for = { $kind ->
        [start] Der Start hat
        [finish] Das Ziel hat
       *[control] Posten { $control } hat
    } ein Fenster mit { $radius } mm Radius.
masks-window-written = Abdeckung „{ $id }“ in { $path } eingetragen: { $count ->
        [one] ein Fenster
       *[other] { $count } Fenster
    } mit { $radius } mm Radius auf dem Papier bei 1:{ $scale } um { $controls } der Bahn „{ $course }“.
masks-no-start-finish = { $which ->
        [start] Der Start hat kein Fenster, deshalb nennt check ihn. Ohne --no-start bekommt er eins.
        [finish] Das Ziel hat kein Fenster, deshalb nennt check es. Ohne --no-finish bekommt es eins.
       *[both] Start und Ziel haben kein Fenster, deshalb nennt check sie. Ohne --no-start und --no-finish bekommen sie eins.
    }
masks-corridor-comment = Korridor { $width } mm breit auf dem Papier bei 1:{ $scale }, erzeugt mit masks corridor aus diesen Punkten:
masks-corridor-written = Abdeckung „{ $id }“ in { $path } eingetragen: ein Korridor { $width } mm breit auf dem Papier bei 1:{ $scale } entlang { $count } Punkten.
masks-assigned = Die Bahn „{ $course }“ wird jetzt damit gedruckt, auf Layout { $layout }.
masks-not-assigned = Noch nutzt keine Bahn sie. Nennen Sie sie in einer Zuordnung mit mask = "{ $id }", oder rufen Sie den Befehl noch einmal mit --assign auf.
masks-run-check = Mit check sehen Sie das Ergebnis.

# ---- mcp (M5 Task 2) ----
mcp-about = Stellt die Projektwerkzeuge einem KI-Agenten über MCP auf Standardein- und -ausgabe bereit.
mcp-after-content = Der Server liest JSON-RPC-Nachrichten von der Standardeingabe und antwortet auf der Standardausgabe, bis die Standardeingabe endet. Fehler gehen auf die Standardfehlerausgabe. Pfade in Werkzeugaufrufen müssen absolut sein. --lang legt die Sprache der Meldungen in den Ergebnissen fest; ein Werkzeugaufruf kann mit seinem Argument lang eine andere wählen.
mcp-after-exit-codes = Exit-Codes: 0, wenn der Client die Standardeingabe schließt, 1, wenn der Server nicht starten kann oder die Verbindung abbricht, 2, wenn die Befehlszeile falsch ist.
mcp-error-relative-path = { $path } ist ein relativer Pfad; einen absoluten Pfad angeben, denn der Server kennt den Arbeitsordner des Agenten nicht
mcp-error-empty-path = Der Pfad ist leer; einen absoluten Pfad angeben
mcp-error-nul-path = Der Pfad { $path } enthält ein NUL-Zeichen
mcp-error-internal = Interner Fehler: { $error }

# ---- MCP-Werkzeuge, die schreiben (M5 Task 3) ----
error-assign-no-course-file = assign_mask braucht eine Bahndatei, das Projekt nennt aber keine unter [sources] courses
error-unknown-mask = Das Projekt hat keine Abdeckungsvariante „{ $id }“; vorhanden sind { $known }
error-unknown-mask-none = Das Projekt hat keine Abdeckungsvariante „{ $id }“: Die Projektdatei hat keine [[masks]], und die Bahndatei hat keine Abdeckflächen
error-edit-project = Das Projekt { $path } wurde nicht geändert
error-not-editable-layout = Das Projekt { $path } wurde nicht geändert: Das Layout „{ $name }“ steht nicht als eigene Tabelle [[layouts]] in der Datei, deshalb kann das Programm es nicht ändern. Ändern Sie es von Hand.
error-not-editable-elements = Das Projekt { $path } wurde nicht geändert: Die Elemente des Layouts „{ $name }“ stehen nicht als eigene Tabellen [[layouts.elements]] in der Datei, deshalb kann das Programm sie nicht ändern. Ändern Sie sie von Hand.
error-not-editable-shared = Das Projekt { $path } wurde nicht geändert: Die gemeinsamen Elemente stehen nicht als eigene Tabellen [[shared]] in der Datei, deshalb kann das Programm sie nicht ändern. Ändern Sie sie von Hand.
error-not-editable-assignment = Das Projekt { $path } wurde nicht geändert: Die Zuordnung der Bahn „{ $name }“ steht nicht als eigene Tabelle [[assignments]] in der Datei, deshalb kann das Programm sie nicht ändern. Ändern Sie sie von Hand.
mcp-error-corridor-assign = assign braucht eine Bahn; course muss die Bahn nennen, auf deren Blatt der Korridor gedruckt wird
mcp-error-numbers-position = direction und gap gehören zusammen; beide angeben oder beide weglassen, dann gilt der Vorschlag von check_project
mcp-error-project-link = { $path } ist ein Link, und der Server ändert keine Projektdatei über einen Link; den Pfad der Datei angeben, auf die er zeigt
mcp-error-cancelled = Der Client hat den Aufruf abgebrochen, bevor er begann; es wurde nichts geschrieben

# ---- MCP-Export (M5 Task 4) ----
error-export-stopped = { $count ->
    [0] Der Export endete vor dem nächsten Blatt, weil der Client den Aufruf abgebrochen oder die Verbindung geschlossen hat; es wurde kein PDF geschrieben, und keine Teildatei bleibt zurück
    [one] Der Export endete vor dem nächsten Blatt, weil der Client den Aufruf abgebrochen oder die Verbindung geschlossen hat; das zuvor geschriebene PDF bleibt, und keine Teildatei bleibt zurück
   *[other] Der Export endete vor dem nächsten Blatt, weil der Client den Aufruf abgebrochen oder die Verbindung geschlossen hat; die { $count } zuvor geschriebenen PDFs bleiben, und keine Teildatei bleibt zurück
}
mcp-error-export-layouts = layouts exportiert die Layouts ohne Bahnen und passt deshalb nicht zu courses oder single_file; eines davon weglassen
mcp-error-export-written = { $error }. { $count ->
    [one] Das zuvor geschriebene PDF bleibt: { $files }
   *[other] Die { $count } zuvor geschriebenen PDFs bleiben: { $files }
}

# ---- Wortlaut für einen KI-Agenten (M5 Task 5): agent-<id> ersetzt <id> in den Ergebnissen des MCP-Servers, wo
# ---- der CLI-Text Optionen oder Befehle nennt. Nur Variablen von <id>.
agent-error-kind-unknown = Die Kartenart lässt sich aus der Datei nicht erkennen; kind als sprint, schoolyard oder forest angeben
agent-error-title-control = Der Titel darf keine Zeilenumbrüche, Tabulatoren oder anderen Steuer- oder unsichtbaren Formatzeichen enthalten
agent-error-no-course-file = courses und single_file brauchen eine Bahndatei, das Projekt nennt aber keine unter [sources] courses
agent-error-outside-project = { $path }: Ein Link führt aus dem Projektordner hinaus, deshalb wird dort nichts geschrieben; den Link durch einen echten Ordner ersetzen oder den Zielordner als out_dir angeben
agent-error-project-exists = { $path } gibt es schon; init_project überschreibt die Datei nicht
agent-init-kind-flag = Kartenart: { $kind }, wie mit kind angegeben.
agent-init-no-fit = Warnung: Weder auf A4 noch auf A3 zeigt ein Blatt die Karte im Maßstab 1:{ $scale } ohne Hinweise oder Fehler, deshalb bleibt das Projekt bei A4. check_project meldet Folgendes:
agent-init-courses-no-fit = Warnung: Kein A4- oder A3-Blatt zeigt die Karte bei 1:{ $scale } ohne Hinweise oder Fehler, deshalb bleibt Layout { $layout } bei A4. check_project meldet das.
agent-init-next = Als Nächstes check_project für { $path } aufrufen.
agent-init-next-with-notes = Als Nächstes check_project für { $path } aufrufen. Es meldet die Hinweise oben, bis das Layout geändert ist.
agent-note-number-conflict = Hinweis: Auf dem Blatt der Bahn „{ $course }“ überdeckt die Nummer von Posten { $control } { $conflicts }. Sie wird trotzdem dort gedruckt. { $proposal } Um diese Lage zu nutzen, rufen Sie numbers_accept mit course „{ $course }“, control „{ $control }“ und scope sheet oder layout_wide auf.
agent-note-number-conflict-unsearched = Hinweis: Auf dem Blatt der Bahn „{ $course }“ überdeckt die Nummer von Posten { $control } { $conflicts }. Sie wird trotzdem dort gedruckt. Das Blatt hat seine Suchgrenze erreicht, deshalb gibt es keinen Vorschlag. Eine Lage setzen Sie mit numbers_accept, direction und gap.
agent-error-numbers-no-course-file = numbers_accept braucht eine Bahndatei, das Projekt nennt aber keine unter [sources] courses
agent-error-numbers-no-proposal = für Posten { $control } auf dem Blatt der Bahn „{ $course }“ gibt es keinen Vorschlag, denn seine Nummer hat keinen Konflikt. Mit direction und gap setzen Sie eine Lage selbst.
agent-error-numbers-layout-conflict = der Vorschlag für Posten { $control } (Richtung { $direction }° mit { $gap } mm Abstand) stammt vom Blatt der Bahn „{ $course }“ und ist auf dem Layout { $layout } nicht für jede Bahn frei: Er hat einen Konflikt auf den Blättern von { $courses }. Nehmen Sie scope sheet für das Blatt der Bahn „{ $course }“ allein, oder geben Sie mit direction und gap eine Lage für alle selbst an. Die Projektdatei blieb unverändert.
agent-error-numbers-exists = die Projektdatei hat für Posten { $control } schon eine Korrektur für { $scope }. Ändern Sie sie in der Datei; numbers_accept legt nur neue an.
agent-error-numbers-value = direction braucht eine endliche Zahl, und gap muss größer als 0 und höchstens { $max } mm sein
agent-numbers-accepted = Korrektur für die Nummer von Posten { $control } für { $scope } in { $path } eingetragen: Richtung { $direction }° mit einem Abstand von { $gap } mm. Mit check_project sehen Sie das Ergebnis.
agent-error-masks-no-course-file = masks_window, und masks_corridor mit einer Bahn, brauchen eine Bahndatei, das Projekt nennt aber keine unter [sources] courses
agent-error-masks-id-taken = die Projektdatei hat schon eine Abdeckung „{ $id }“. Wählen Sie eine andere id, oder ändern Sie die Abdeckung in der Datei.
agent-error-masks-course-has-mask = die Bahn „{ $course }“ wird schon mit der Abdeckung „{ $mask }“ gedruckt. assign_mask gibt ihr eine andere; masks_window und masks_corridor ersetzen sie nicht.
agent-error-masks-radius-for = jeder Eintrag von radius_for hat die Form CODE=MM, etwa 35=20, und „{ $text }“ ist nicht so geschrieben
agent-error-masks-selection = controls nimmt Postencodes mit Kommas dazwischen (31,32,35) oder einen Bereich von Postennummern (3-6), und „{ $text }“ ist keins von beidem
agent-error-masks-radius-unselected = Posten { $control } bekommt kein Fenster, deshalb kann radius_for seinen Radius nicht setzen
agent-error-masks-points = points nimmt Punkte als x,y;x,y;… in Karten-mm mit y nach oben, Dezimalstellen mit Punkt, und „{ $text }“ ist keine solche Liste
agent-error-masks-scales = die Layouts drucken in verschiedenen Maßstäben ({ $scales }), und die Breite ist in Druck-mm angegeben. Geben Sie course an, damit die Breite dem Layout dieser Bahn folgt.
agent-masks-no-start-finish = { $which ->
        [start] Der Start hat kein Fenster, deshalb nennt check_project ihn. Ohne no_start bekommt er eins.
        [finish] Das Ziel hat kein Fenster, deshalb nennt check_project es. Ohne no_finish bekommt es eins.
       *[both] Start und Ziel haben kein Fenster, deshalb nennt check_project sie. Ohne no_start und no_finish bekommen sie eins.
    }
agent-masks-not-assigned = Noch nutzt keine Bahn sie. assign_mask ordnet sie einer Bahn zu.
agent-masks-run-check = Mit check_project sehen Sie das Ergebnis.
agent-note-mask-unused = Zur Information: Keine Bahn nutzt die Abdeckung „{ $id }“. assign_mask ordnet sie einer Bahn zu; wer sie nicht mehr braucht, löscht ihren Eintrag [[masks]] in der Projektdatei.

# ---- edit (M6 Task 2) ----
edit-about = Öffnet ein Projekt zum Bearbeiten in einer Seite im Browser.
edit-no-browser = Gibt die Adresse aus und öffnet den Browser nicht.
edit-listening = Der Editor läuft unter { $url }. Zum Beenden Strg+C drücken oder im Browser auf „Fertig“ klicken.
help-edit-port = Port auf 127.0.0.1; bei 0 wählt das System einen freien.
error-serve = der Editor konnte nicht gestartet werden oder ist abgebrochen

# ---- editor page (M6 Task 3); the page gets every editor-* and suite-* message through api/texts ----
suite-banner-dismiss = Schließen
editor-page-title = freemapper-print Editor
editor-pane-tree = Projekt
editor-tree-label = Teile des Projekts
editor-pane-sheet = Blattvorschau
editor-pane-props = Eigenschaften
editor-props-heading = Eigenschaften
editor-props-empty = Nichts ausgewählt.
editor-notices-heading = Hinweise
editor-notices-empty = Keine Hinweise.
editor-toolbar-label = Blatt
editor-menu-file = Datei
editor-menu-edit = Bearbeiten
editor-menu-view = Ansicht
editor-menu-export = Export
editor-menu-done = Fertig
editor-menu-undo = Rückgängig
editor-menu-redo = Wiederholen
editor-menu-zoom-in = Vergrößern
editor-menu-zoom-out = Verkleinern
editor-menu-zoom-fit = Ganze Seite
editor-menu-export-sheet = Dieses Blatt exportieren
editor-menu-export-all = Alle Blätter exportieren
editor-status-ready = Bereit.
editor-status-closed = Der Editor ist beendet. Das Fenster kann geschlossen werden.
editor-status-lost = Die Verbindung zum Editor ist abgebrochen.
editor-status-project = Projekt
editor-status-scale = Maßstab
editor-status-page = Seite
