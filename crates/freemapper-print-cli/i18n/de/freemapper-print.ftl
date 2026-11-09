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
    } geprüft, keine Hinweise.
check-courses-with-notes = Bahnen mit Hinweisen: { $count } von { $total }.
check-course-file-notes = Die Bahndatei selbst hat Hinweise. Sie stehen vor der ersten Bahn.

## Blatthinweise

list-and = { $first } und { $last }
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
number-proposal-free = Frei ist Richtung { $direction }° mit { $gap } mm Abstand.
number-proposal-none = Einen freien Platz gibt es nicht; am wenigsten verdeckt Richtung { $direction }° mit { $gap } mm Abstand.

## Hinweise zur Bahndatei

note-course-unknown-element = Hinweis: Die Bahndatei enthält das Element { $name }, das diese Version nicht liest. Es fehlt auf den Blättern.
note-course-forked = Zur Information: Die Bahnen { $variants } sind die Varianten der Gabelung „{ $family }“. Jede wird als eigenes Blatt gedruckt.
note-course-alternative-controls = Hinweis: Die Bahn „{ $course }“ bietet an einer Stelle alternative Posten an. Das kann diese Version nicht drucken, die Bahn fehlt deshalb.
note-course-notes-truncated = Zur Information: Die Bahndatei hat mehr Hinweise ergeben, als hier stehen.
note-course-white-out = Hinweis: Die Bahndatei hat Abdeckflächen (White-out). Diese Version lässt sie weg; spätere Versionen machen Masken daraus.
note-course-no-descriptions = Zur Information: Die Bahndatei hat keine Postenbeschreibungen.
note-course-map-issue-as-start = Zur Information: Die Kartenausgabe { $code } dient als Start.
note-course-variations = Hinweis: Die Bahn „{ $course }“ hat Gabelungen oder Schleifen (Varianten in Purple Pen). Diese Version kann sie nicht drucken, die Bahn fehlt deshalb. Der IOF-XML-Export von Purple Pen schreibt jede Variante als eigene Bahn.
note-fit-y-up = Zur Information: Die y-Achse der Bahndatei zeigt nach oben; das Programm hat sie zur Karte passend gedreht.
note-fit-orientation-assumed = Hinweis: Die Bahn passt mit der y-Achse nach oben und nach unten auf die Karte, und nichts in der Datei entscheidet. Das Programm hat „oben“ genommen. Prüfen Sie ein Blatt und tragen Sie dann in der Projektdatei unter [sources] course_y_axis = "up" oder "down" ein.
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
error-numbers-exists = die Projektdatei hat für Posten { $control } schon eine Korrektur für { $scope }. Ändern Sie sie in der Datei; numbers accept legt nur neue an.
error-numbers-value = --direction braucht eine endliche Zahl, und --gap muss größer als 0 und höchstens { $max } mm sein
numbers-accepted = Korrektur für die Nummer von Posten { $control } für { $scope } in { $path } eingetragen: Richtung { $direction }° mit einem Abstand von { $gap } mm. Mit check sehen Sie das Ergebnis.
error-load-changed-project = das Projekt { $path } lässt sich mit der neuen Korrektur nicht laden, deshalb blieb es unverändert
