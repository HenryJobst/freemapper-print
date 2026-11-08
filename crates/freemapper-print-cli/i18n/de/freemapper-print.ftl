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
render-after-exit-codes = Exit-Codes: 0 bei Erfolg, 1 wenn die Karte nicht gelesen oder gedruckt werden kann, 2 wenn die Befehlszeile falsch ist.
help-map = Die OOM-Kartendatei (.omap).
help-out = Zu schreibende PDF-Datei. Eine vorhandene Datei wird überschrieben; ihr Ordner muss existieren.
help-scale = Nenner des Druckmaßstabs, z. B. 4000 für 1:4000. Ohne Angabe gilt der Maßstab der Karte.
help-paper = Blattformat.
help-orientation = Ausrichtung der Seite. Auto nimmt die, die weniger von der Karte abschneidet.
help-kind = Kartenart. Nur nötig, wenn das Programm sie weder aus freemapper.toml noch aus der Kartendatei erkennt.
help-show-sheet-objects = Zeichnet auch den Informationsblock von freemapper: Titel, Maßstab, Maßstabsleiste und Namensnennung. Er skaliert mit der Karte, bei einem anderen Druckmaßstab kann sein Maßstabstext deshalb falsch sein.

## Fehler

error-line = Fehler: { $message }
error-unknown-lang = unbekannte Sprache "{ $lang }"; verfügbar: { $available }
error-read-map = die Karte { $path } konnte nicht gelesen werden
error-kind-unknown = die Kartenart lässt sich aus der Datei nicht erkennen; --kind sprint, --kind schoolyard oder --kind forest angeben
error-out-folder-missing = der Ordner { $path } für das PDF existiert nicht; bitte zuerst anlegen
error-out-is-map = --out zeigt auf die Kartendatei { $path }; einen PDF-Pfad wählen, damit die Karte unversehrt bleibt
error-print = die Karte konnte nicht gedruckt werden
error-write = das PDF konnte nicht geschrieben werden

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
note-fonts = Hinweis: die Schrift Arimo ersetzt { $names }.
note-missing-glyphs = Hinweis: Die eingebettete Schrift hat kein Zeichen für diese Buchstaben, deshalb fehlen sie im PDF: { $chars }
reminder-actual-size = In Originalgröße drucken: 100 %, "An Seite anpassen" ausgeschaltet. Jede andere Größe zerstört den Maßstab.
