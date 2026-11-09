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
init-after-content = Die neue Datei hat ein A4-Layout im Kartenmaßstab, einen Titelblock, eine Kontrollstrecke und einen Vorschlag für die Äquidistanz, den Sie prüfen und bestätigen.
init-after-exit-codes = Exit-Codes: 0 bei Erfolg, 1, wenn die Karte nicht gelesen werden kann, die Kartenart unbekannt ist oder die Projektdatei schon existiert, 2, wenn die Befehlszeile falsch ist.
help-init-folder = Ordner für die neue freemapper-print.toml. Fehlt er, wird er angelegt.
help-init-map = Die OOM-Kartendatei (.omap). Die Projektdatei speichert ihren Pfad relativ zu sich selbst.
help-init-title = Titel der Veranstaltung auf den Blättern. Ohne Angabe gilt der Dateiname der Karte.
help-init-kind = Kartenart. Nur nötig, wenn keine freemapper.toml neben der Karte sie nennt.
check-about = Zeichnet jedes Layout eines Projekts, ohne eine Datei zu schreiben, und listet die Hinweise auf.
check-after-exit-codes = Exit-Codes: 0, wenn kein Layout Hinweise hat (ergänzte Nordlinien zählen nicht), 1, wenn das Projekt nicht geladen werden kann oder ein Layout Hinweise hat, 2, wenn die Befehlszeile falsch ist.
help-project = Die Projektdatei freemapper-print.toml oder der Ordner, in dem sie liegt.
export-about = Schreibt je Layout ein PDF, benannt nach der Layout-ID.
export-after-exit-codes = Exit-Codes: 0, wenn alle PDFs geschrieben sind, auch mit Hinweisen, 1, wenn das Projekt nicht geladen oder ein PDF nicht gezeichnet oder geschrieben werden kann, 2, wenn die Befehlszeile falsch ist oder ein unbekanntes Layout nennt.
help-layout = Nur dieses Layout exportieren. Für mehrere wiederholen. Ohne Angabe werden alle Layouts exportiert.
help-out-dir = Ordner für die PDFs. Er muss existieren. Ohne Angabe gilt der Ordner out neben der Projektdatei, der bei Bedarf angelegt wird. Ein PDF gleichen Namens wird überschrieben.

## Fehler

error-line = Fehler: { $message }
error-unknown-lang = unbekannte Sprache „{ $lang }“; verfügbar: { $available }
error-read-map = die Karte { $path } konnte nicht gelesen werden
error-kind-unknown = die Kartenart lässt sich aus der Datei nicht erkennen; --kind sprint, --kind schoolyard oder --kind forest angeben
error-out-folder-missing = der Ordner { $path } für das PDF existiert nicht; zuerst anlegen
error-out-is-map = --out zeigt auf die Kartendatei { $path }; einen PDF-Pfad wählen, damit die Karte unversehrt bleibt
error-print = die Karte konnte nicht gedruckt werden
error-write = das PDF konnte nicht geschrieben werden
error-load-project = das Projekt { $path } konnte nicht geladen werden
error-project-exists = { $path } gibt es schon; init überschreibt die Datei nicht
error-create-folder = der Ordner { $path } konnte nicht angelegt werden
error-init-kind-unknown = keine freemapper.toml neben der Karte nennt die Kartenart; --kind sprint, --kind schoolyard oder --kind forest angeben
error-unknown-layout = das Projekt hat kein Layout { $id }; vorhanden sind { $known }
error-out-dir-missing = der Ordner { $path } für die PDFs existiert nicht; zuerst anlegen
error-sheet = Layout { $layout } konnte nicht gezeichnet werden
error-disk-full = nicht genug Speicherplatz für { $path }: nötig sind { $needed } MB samt 50 MB Reserve, frei sind { $available } MB
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
warning-overflow = Warnung: die Karte passt nicht auf die Seite. Sie ist { $cut } für den bedruckbaren Bereich. Versuchen Sie { $hints } oder { $last }.
warning-overflow-single = Warnung: die Karte passt nicht auf die Seite. Sie ist { $cut } für den bedruckbaren Bereich. Versuchen Sie { $last }.
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
note-fonts = Hinweis: die Schrift Arimo ersetzt { $names }.
note-missing-glyphs = Hinweis: die eingebettete Schrift kann diese Zeichen nicht darstellen, deshalb fehlen sie im PDF: { $chars }
reminder-actual-size = In Originalgröße drucken: 100 % („Tatsächliche Größe“), „An Seite anpassen“ ausgeschaltet. Jede andere Größe zerstört den Maßstab.

## Projektbefehle: init, check, export

init-written = { $path } geschrieben, mit einem A4-Layout { $layout } im Maßstab 1:{ $scale } für { $kind }.
init-contour-hint = Das Projekt schlägt eine Äquidistanz von { $value } m vor, üblich für { $kind }. Sehen Sie auf der Karte nach und bestätigen Sie den Wert oder ändern Sie map.contour_interval_m.
init-no-contour-hint = Für Schulhofkarten schlägt das Projekt keine Äquidistanz vor. Hat die Karte Höhenlinien, tragen Sie map.contour_interval_m ein; bis dahin meldet check sie als fehlend.
init-next = Als Nächstes freemapper-print check { $path } aufrufen.
# Kommentare in der neuen Projektdatei.
init-comment-contour = Vorschlag für { $kind }. An der Karte prüfen und bestätigen.
init-comment-no-contour = Für Schulhofkarten gibt es keinen Vorschlag. Hat die Karte Höhenlinien, contour_interval_m mit der Äquidistanz in Metern ergänzen.
init-comment-footer = Quellenangaben und Namensnennung stehen in der Fußzeile links unten, unter dem Kartenausschnitt, wenn das Passepartout breit genug ist, sonst in ihm. Ein Element copyright_line oder credit_line verschiebt sie.
check-summary = Layout { $layout }: { $paper } { $orientation }, Druckmaßstab 1:{ $print_scale }, Kartenmaßstab 1:{ $map_scale }, Geometriefaktor { $geometry }, Symbolfaktor { $symbol }.
check-clean = { $count ->
        [one] 1 Layout
       *[other] { $count } Layouts
    } geprüft, keine Hinweise.
check-with-notes = Layouts mit Hinweisen: { $count } von { $total }.

## Blatthinweise

list-and = { $first } und { $last }
note-element-outside-page = Hinweis: das Element { $id } ragt über den Blattrand und wird dort abgeschnitten.
note-footer-outside-page = Hinweis: die Fußzeile mit Quellenangaben und Namensnennung ragt über den Blattrand und wird dort abgeschnitten.
note-element-overlaps-map = Hinweis: das Element { $id } hängt am Blatt und deckt einen Teil des Kartenausschnitts ab, die Karte darunter ist nicht zu sehen.
note-footer-overlaps-map = Hinweis: das Passepartout ist zu schmal für die Fußzeile mit Quellenangaben und Namensnennung. Sie steht deshalb auf einem weißen Kasten im Kartenausschnitt und deckt einen Teil der Karte ab. Ein breiteres margin_mm oder ein Element copyright_line oder credit_line verschiebt sie.
note-larger-than-a3 = Hinweis: das Papier ist größer als A3, das nehmen die meisten Bürodrucker nicht.
note-missing-scale-or-contour = Hinweis: im Layout { $layout } { $count ->
        [one] fehlt
       *[other] fehlen
    } { $missing }.
missing-print-scale-text = ein Text mit { $name } für den Druckmaßstab
missing-contour-text = ein Text mit { $name } für die Äquidistanz
missing-contour-value = ein Wert für { $name }, die Äquidistanz in Metern
note-north-lines-added = Hinweis: die Karte hat keine Nordlinien, deshalb { $count ->
        [one] wurde 1 Linie
       *[other] wurden { $count } Linien
    } ergänzt. Sie laufen ohne Unterbrechung über den Kartenausschnitt. Sie an kleinen Objekten wie Steinen oder Wegenden zu unterbrechen, ist Aufgabe des Kartografen in OOM.
note-north-lines-without-symbol = Hinweis: die Karte hat weder Nordlinien noch ein brauchbares Liniensymbol 601, deshalb { $count ->
        [one] wurde 1 schlichte schwarze Linie
       *[other] wurden { $count } schlichte schwarze Linien
    } über die Karte gezeichnet.
note-copyright-truncated = Hinweis: Layout { $layout } zeigt { $shown } von { $total } Quellenangaben. Eine weitere Zeile zählt die übrigen.
note-logo-skipped = Hinweis: das Logo { $id } fehlt auf dem Blatt: { $reason }
note-logo-simplified = Hinweis: das Logo { $id } wird vereinfacht gezeichnet: { $reason }
note-unknown-placeholder = Hinweis: der Text { $id } enthält { $name }. Das ist kein Platzhalter und wird so gedruckt, wie es dasteht.
note-unknown-part = Hinweis: Layout { $layout } blendet den Kartenteil { $name } aus, die Karte hat aber keinen Teil dieses Namens. Groß- und Kleinschreibung zählt.
