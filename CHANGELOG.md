# Änderungen

[English version](CHANGELOG.en.md) · [Handbuch](docs/manual/de.md)

Hier stehen alle wichtigen Änderungen an freemapper-print, die neueste zuerst. Das Format folgt
[Keep a Changelog](https://keepachangelog.com/de/1.1.0/).

## [Unveröffentlicht]

### Behoben

- Beispiel `berlin-viktoriapark-training`: neue Bahn von 1,8 km mit 11 Posten auf Gebäudeecken, Ecken offener Flächen und Pfaden statt neun Posten an Gebäudeecken.

- Die Maßstabsleiste aus freemapper v0.9.0 (Tag `freemapper:source=scale-bar`) zählt wie die übrigen Randangaben und wird nicht mehr als Karteninhalt mitgedruckt.
- `init` nimmt den Titel aus der `freemapper.toml` neben der Karte, bevor es auf den Dateinamen zurückfällt.

### Geändert

- Die Beschreibungssymbole aus Purple Pen kommen aus freemapper-core v0.6.0 (ADR 0049); die Ausgabe bleibt gleich.
- Die gemeinsamen Bausteine kommen jetzt aus freemapper-core v0.6.0 (Purple-Pen-Schreiber, Objekt-IDs). Die Ausgabe bleibt unverändert.
- Abdeckflächen aus Purple Pen heißen `whiteout-<n>` mit Purple Pens eigener Nummer statt der Stelle in der Datei, so ändert Löschen oder Umsortieren die Bedeutung anderer nicht. Projektdateien mit `whiteout-1` und so weiter prüfen Sie mit `check` (ADR 0048).

## [0.5.0] - 2026-11-10

### Hinzugefügt

- Abdeckungen für Fenster- und Korridor-OL in der Projektdatei (ADR 0045). Eine Tabelle `[[masks]]` hat eine ID
  und Polygone aus einem Umriss und Löchern, den Fenstern. Die Punkte sind Karten-mm mit der y-Achse nach oben,
  wie OOM sie anzeigt und wie `centre_map_mm` sie nimmt. `mask = "…"` in `[[assignments]]` gibt einer Bahn ihre
  Abdeckung, mehrere Bahnen dürfen dieselbe nutzen. Beim Laden prüft das Programm jede Abdeckung und nennt einen
  Fehler mit Datei, Zeile und Feld, etwa ein Loch außerhalb seines Umrisses oder einen Ring, der sich kreuzt.
- Abdeckungen werden gedruckt. Auf dem Blatt einer Bahn mit Abdeckung fehlt die Karte in der abgedeckten Fläche;
  außerhalb des Umrisses und in den Fenstern ist sie zu sehen, und überlappende Fenster ergeben ein Fenster.
  Bahnaufdruck, Beschreibungskasten, Blattelemente und Fußzeile bleiben ganz sichtbar, auch die Strecken zwischen
  den Fenstern. Ohne eigene Abdeckung gelten die Abdeckflächen (White-out) aus Purple Pen, jede für sich, auch wenn
  sie sich überlappen. Ein Blatt wendet höchstens 1000 Abdeckungen an; hat eine Bahn mehr, fehlen die übrigen, und
  `check` warnt. Blätter ohne Abdeckung bleiben Byte für Byte gleich.
- `masks window` schreibt eine Abdeckung mit runden Fenstern um die Posten einer Bahn (ADR 0047). Der Radius gilt
  in Millimetern auf dem Papier, Vorgabe 15 mm; `--radius-for` gibt einzelnen Posten einen eigenen, `--controls`
  wählt Posten nach Code oder Nummer aus, und `--assign` druckt die Bahn damit. Start und Ziel bekommen ebenfalls
  ein Fenster, außer mit `--no-start` oder `--no-finish`. Die Ausgabe und der Kommentar in der Projektdatei nennen
  Start und Ziel beim Namen, nicht mit den Codes `STA1` und `FIN1`. Die Abdeckung behält die Fenster in Karten-mm;
  nach einem neuen Druckmaßstab ruft man den Befehl noch einmal auf.
- `masks corridor` macht aus einer Linie, die Sie mit `--points` vorgeben, einen Korridor mit der Breite aus
  `--width` (Vorgabe 10 mm auf dem Papier). Einen automatischen Korridor gibt es nicht. Einen Knick, der für die
  Breite zu scharf ist, oder eine Linie, die sich kreuzt, lehnt der Befehl ab und nennt die Stelle, bei einer
  Kreuzung den Punkt, an dem sich die Ränder schneiden.
- Beide Befehle hängen die Tabelle als Text an die Projektdatei an, mit einem Kommentar, wie sie entstand; alles
  davor bleibt unverändert. Eine vorhandene Abdeckung ändern sie nie. Schreibt die Datei ihre Zuordnungen als
  `assignments = [...]`, bricht `--assign` mit Exit-Code 2 ab, und die Datei bleibt, wie sie war.
- `check` prüft Bahnblätter mit Abdeckung (ADR 0046). Es warnt, wenn ein Posten, der Start oder das Ziel dort
  liegt, wo die Abdeckung die Karte verbirgt, und zählt je Art, was vom Bahnaufdruck dort liegt: Pflichtstrecken,
  Querungsstellen, verbotene Strecken, Sperrgebietsgrenzen, Sperr- und Gefahrengebiete, Texte. Gedruckt wird der
  Aufdruck trotzdem. Es warnt auch, wenn eine Abdeckung den ganzen Kartenausschnitt verbirgt, und sagt zur
  Information, wenn sie im Ausschnitt nichts verbirgt oder keine Bahn sie nutzt; Abdeckflächen aus Purple Pen, die
  nichts verbergen, stehen in einer Zeile je Bahn. Es warnt, wenn ein Blattelement oder die Fußzeile einen Teil
  eines Fensters abdeckt. Nennt die Projektdatei eine Abdeckfläche mit ihrer Nummer (`whiteout-<n>`), sagt `check`,
  wo diese Fläche liegt, denn die Nummer folgt der Reihenfolge in der Bahndatei und kann nach einer Änderung in
  Purple Pen eine andere Fläche meinen. Erreicht die Prüfung ihre Grenze, sagt `check`, dass es nur einen Teil
  geprüft hat.
- Zwei Beispiele: `luebeck-training-window` (Fenster-OL mit acht Posten) und `luebeck-training-corridor`
  (Korridor-OL entlang dreier Straßen der Lübecker Altstadt). Das Handbuch hat ein Kapitel 11 über Abdeckungen.

### Geändert

- Der Hinweis auf Abdeckflächen in der Bahndatei beginnt jetzt mit „Zur Information:“ und zählt nicht mehr für den
  Exit-Code von `check`, denn die Abdeckflächen werden gedruckt.
- Das Programm liest eine Projektdatei von höchstens 16 MB. Eine größere lehnt es ab, bevor es sie liest; eine
  Datei innerhalb der Grenzen für Abdeckungen braucht höchstens etwa 4 MB.
- Die letzte Zeile von `check` zählt in der richtigen Zahl: „Mit Hinweisen: 1 von 1 Bahn, 0 von 1 Layoutblatt.“

## [0.4.0] - 2026-11-10

### Geändert

- Programmtexte, Regelprofile und das Schreiben von Dateien kommen jetzt aus freemapper-core
  (`freemapper-text`, `freemapper-rules`, `freemapper-fs`; ADR 0042). Die gedruckten Blätter und alle Ausgaben
  bleiben gleich, die Beispiel-PDFs sind Byte für Byte dieselben.
- Auch Kartenleser, Bahndateien, Einpassung der Bahnen und die Lage der Postennummern kommen jetzt aus
  freemapper-core (`freemapper-omap`, `freemapper-course`, alle Crates in Version v0.3.0; ADR 0043). Das ist
  eine interne Änderung. Für Nutzer ändert sich nichts, die Beispiel-PDFs sind Byte für Byte dieselben.
- Eine Projektdatei, die ein Link (symbolischer Link) ist, ersetzt `numbers accept` beim Schreiben durch eine
  normale Datei. Bisher schrieb es durch den Link in die Zieldatei. Die Datei, auf die er zeigte, behält ihren
  Inhalt; die Rechte der Projektdatei bleiben erhalten (ADR 0041).
- `export` und auch `init` lehnen einen Ordner `out` oder ein `--out-dir` im Projekt ab, der ein Link aus dem
  Projektordner hinaus ist. Das Programm schreibt nichts dorthin und meldet: „…: Ein Link führt aus dem
  Projektordner hinaus, deshalb wird dort nichts geschrieben; den Link durch einen echten Ordner ersetzen oder
  den Zielordner mit --out-dir angeben“. Ein `--out-dir` auf einen Ordner außerhalb des Projekts, auch auf einem
  anderen Laufwerk, geht weiter.
- Die Teildatei beim Schreiben eines PDFs heißt jetzt `<Name>.pdf.<Prozess-ID>.<Zähler>.part` statt
  `<Name>.pdf.part`. Zwei Läufe im selben Ordner teilen sich keine Teildatei mehr. `export` löscht Teildateien im
  `out`-Ordner des Projekts, die älter als eine Stunde sind.
- Ein fehlender `--out-dir` wird wie bisher gemeldet (der Ordner muss vorher existieren).
- Die Karten in den Tests und in allen vier Beispielen stammen jetzt aus freemapper 0.8.0 (Commit `a2b41b4`). Sie
  liegen auf der Grundfläche der Sprintkarte, und Parks sind gelb. Die PDFs und Vorschaubilder der Beispiele sind
  neu erzeugt und zeigen beides. freemapper zeichnet Wege aus festem Kies nicht mehr als Wege der Karte, deshalb
  stehen sechs Posten an neuen Stellen: Posten 36 der Lübecker Bahn A an einer Gebäudeecke (die Bahn ist jetzt
  2,5 km lang statt 2,6 km) und fünf Posten des Trainings im Viktoriapark an Gebäudeecken und an einer Ecke der
  offenen Fläche. Die Kandidaten von freemapper, darunter Postenstandorte und unerreichbare Durchgänge, liegen in
  einer eigenen Vorlage und kommen nicht aufs Blatt. Ein neuer Test prüft das.

### Hinzugefügt

- Die Grundfläche neuerer Sprintkarten von freemapper: eine Fläche in der Farbe „belaufbare Fläche“ im Kartenteil
  `base` unter der ganzen Karte. freemapper-print druckt sie immer mit, beschnitten am Kartenausschnitt. Für die
  Mitte des Kartenausschnitts, den Überstand, die ergänzten Nordlinien und die Lage der Postennummern zählt sie
  nicht, denn sie reicht über die Karte hinaus. `hide_parts = ["base"]` lehnt das Programm ab, mit einer Meldung
  auf Deutsch oder Englisch (ADR 0044). Solche Karten farbig drucken: In Graustufen ist das helle Braun kaum von
  Papierweiß und befestigten Flächen zu unterscheiden.

## [0.3.0] - 2026-11-09

### Hinzugefügt

- Druckregeln als Regelprofil im Format von freemapper-courseplan (ADR 0031). Das eingebettete Profil `iof-print`
  enthält die Tabelle der Symbolgrößen je Maßstab, den Abstand der Nordlinien und die vorgeschlagene Äquidistanz,
  dazu für den Bahnaufdruck die Größen der Bahnsymbole aus ISSprOM 2019-2 und ISOM 2017-2, das Violett 35/85/0/0
  und die Kästchengröße der Postenbeschreibung. Jeder Wert nennt seine Quelle. Die Projektdatei kann in `[print]`
  ein anderes Profil (`rule_profile`) und Aufsätze (`rule_overlays`) nennen; ein unbekannter Name ist ein Fehler
  mit Zeile und Spalte. Die gedruckten Blätter ändern sich nicht.
- Bahnen aus Purple Pen und IOF-XML 3.0 (ADR 0033). Das Programm liest `.ppen`-Dateien (Posten, Bahnen, Knickpunkte
  und Lücken der Strecken, Kreislücken je Maßstab, Beschreibungen, Sperrgebiete, verbotene Strecken,
  Pflichtstrecken, Lage der Nummern, Druckmaßstäbe) und IOF-XML 3.0 `CourseData` (Posten, Bahnen, Positionen, Lage
  der Nummern, Länge und Anstieg). IOF-XML kennt keine Beschreibungen, Sonderobjekte, Knickpunkte und
  Druckmaßstäbe, seine Blätter zeigen deshalb nur Posten, Strecken und Nummern. Der Leser wird nach der Dateiendung
  gewählt. Was das Programm noch nicht drucken kann, lässt es weg und meldet es: Bahnen mit Varianten (Gabelungen
  und Schleifen) oder alternativen Posten und unbekannte Elemente. Das White-out aus Purple Pen wird gelesen und
  gemeldet, aber nicht gedruckt; es wartet auf die Abdeckungen (M4).
- Eine Bahn muss zur Karte passen (ADR 0032). Eine Purple-Pen-Datei liest das Programm mit der y-Achse nach oben,
  wie Purple Pen sie schreibt; passt sie so nicht, ist das ein Fehler, gespiegelt gedruckt wird nie. Bei IOF-XML
  probiert es beide Lesarten der y-Achse und nimmt die, bei der alle Posten auf der Karte liegen. Stehen in der
  Datei Breite und Länge und ist die Karte in UTM georeferenziert, entscheiden sie. Passen beide und entscheidet
  nichts, nimmt es „oben“ an und warnt, bis `course_y_axis` die Achse bestätigt. Ein `course_y_axis = "down"` bei
  einer Purple-Pen-Datei gibt eine Warnung. Ein Kartenmaßstab in der Bahndatei muss dem der Karte entsprechen. Ein
  Posten, der mehr als 20 m von dem Ort entfernt liegt, den seine Breite und Länge angeben, hält die Einpassung an.
  Jeder Fehlschlag ist ein Fehler, der sagt, was zu prüfen ist; aus einer Bahn, die nicht passt, wird nichts
  gedruckt.
- Der Bahnaufdruck (ADR 0034). Das Programm zeichnet Start, Posten, Strecken, Ziel, Sperrgebiete, verbotene
  Strecken, Pflichtstrecken, Querungsstellen, Kartenwechsel, Texte und Nummern im Violett der Bahn, in zwei
  Schichten. Das untere Violett liegt unter Schwarz, Braun und Blau 100 %, das obere über der ganzen Karte. Beide
  werden im Mischmodus `Multiply` gemalt. Die Größen kommen aus dem Regelprofil und haben die Größe, die die Norm
  für den Druckmaßstab vorgibt, den Bahnfaktor: Eine Waldkarte in 1:10000 bekommt Postenkreise von 7,5 mm, in
  1:15000 von 5,0 mm, gleich in welchem Maßstab die Kartendatei gezeichnet ist. Eine verbotene Strecke ist eine
  Reihe von Kreuzen, wie ISOM 2017-2 das Symbol 711 zeichnet.
- Die Projektdatei kann Bahnen einbinden. `[sources] courses` nennt die Bahndatei (`.ppen` oder IOF-XML, relativ
  zur Projektdatei); sie wird bei jeder Prüfung und jedem Export neu gelesen. `course_y_axis` („up“ oder „down“)
  legt die y-Achse der Datei fest (ein Purple-Pen-Projekt braucht das nicht), `course_georef_check = false`
  schaltet die Prüfung mit Breite und Länge ab. `[[assignments]]` ordnet einer Bahn ein Layout zu, ohne Zuordnung
  gilt das erste Layout. `[[numbers]]` korrigiert die Lage einer Postennummer je Layout oder je Bahn. Gibt es eine
  Bahn oder einen Posten der Projektdatei in der Bahndatei nicht mehr, bleibt der Eintrag stehen und `check` meldet
  ihn; die übrigen Bahnen werden trotzdem gelesen.
- Ein Blatt je Bahn. Hat das Projekt eine Bahndatei, schreibt `export` je Bahn ein PDF auf dem Layout, das ihre
  Zuordnung nennt, mit dem Bahnaufdruck in Violett. Die Datei heißt wie die Bahn; Zeichen, die in keinem Dateinamen
  stehen dürfen, werden zu `-`, und Namen, die sich nur in Groß- und Kleinschreibung unterscheiden, bekommen `
  (2)`. Ohne `centre_map_mm` wird der Kartenausschnitt auf die Bahn zentriert. `--course <NAME>` (mehrfach möglich)
  exportiert nur diese Bahnen. `--single-file` schreibt alle Bahnen in ein PDF, „<Veranstaltung> - alle Bahnen.pdf“
  auf deutschen Blättern, eine Seite je Bahn in ihrem Papierformat. Dass die Karte über den Kartenausschnitt
  hinausreicht, ist auf einem Bahnblatt erwartet und nur eine Information. `--layout` exportiert weiter ein Layout
  ohne Bahnen. Texte können `{course}`, `{length_km}`, `{climb_m}` und `{controls}` nutzen. `check` prüft jede Bahn
  und meldet Posten außerhalb des Kartenausschnitts, danach die Layoutblätter, die `export --layout` schreibt.
  Projekte ohne Bahndatei exportieren wie bisher (ADR 0035).
- Die Postenbeschreibung auf Bahnblättern. Ein Layout-Element vom Typ `control_descriptions` zeichnet die
  IOF-Postenbeschreibung der Bahn aus einer `.ppen`-Datei: Veranstaltung, Bahn mit Länge und Steigung, Start, je
  Posten eine Zeile mit Nummer, Code und Symbolen, Zeilen für ausgeflaggte Strecken und Kartenwechsel mit ihrer
  Länge, und das Ziel. Der Zweittitel einer Bahn aus Purple Pen bekommt eine eigene Zeile, das Wenden der Karte
  seine Zeile 15.6, und Score-Bahnen listen ihre Posten wie Purple Pen nach Code. Symbole und Linienstärken folgen
  Purple Pen. Ein Kästchen ist 6 mm groß, `cell_mm` stellt 4 bis 10 mm ein, und die Größe bleibt bei jedem
  Druckmaßstab gleich. Die Beschreibung hängt wie jedes Element an ihrem Anker und liegt auf weißem Grund. Bahnen
  aus IOF-XML haben keine Beschreibungen, ihre Blätter bleiben ohne, und `check` sagt das zur Information. `check`
  warnt, wenn die Beschreibung über den Blattrand reichen würde (das Blatt lässt sie dann weg), wenn kein Posten
  der Bahn eine Beschreibung hat und wenn die Bahndatei ein Symbol nutzt, das der Beschreibungsstandard nicht kennt
  (sein Kästchen bleibt leer). Auf jedem Bahnblatt warnt `check` außerdem, wenn ein Blattelement einen Posten, den
  Start oder das Ziel verdeckt (ADR 0037).
- Postennummern auf Bahnblättern. Eine Nummer steht, wo die Bahndatei sie hinsetzt: bei Purple Pen die Lage je
  Bahn, umgerechnet mit Druckmaßstab und Größenanpassung der Bahn, bei IOF-XML `MapTextPosition`. Eine Korrektur in
  `[[numbers]]` für das Layout oder für die Bahn geht vor; ihr Abstand `gap_mm` liegt über 0 und höchstens bei 10
  mm. Hat die Datei keine Lage, setzt freemapper-print die Nummer selbst: hinter den Posten, in Verlängerung der
  hereinführenden Strecke, und nicht auf Kreise, Strecken, andere Nummern, Blattelemente sowie Punkt- und
  Liniensymbole der Karte. `check` prüft jede Nummer. Liegt sie auf dem Bahnaufdruck, einem Blattelement, über dem
  Rand des Kartenausschnitts oder auf einem Punkt- oder Liniensymbol der Karte, nennt die Warnung, was sie
  verdeckt, Kartensymbole mit ihrem Code, und schlägt Richtung und Abstand vor. Gibt es keinen freien Platz, sagt
  sie das und schlägt den am wenigsten verdeckten vor. Gedruckt wird die Nummer trotzdem an ihrer Stelle,
  verschoben wird nichts. Das Regelprofil legt mit der Regel `number_gap` 0,5 bis 5 mm Abstand fest, wie in
  freemapper-courseplan auf 0,01 mm gerundet; `check` warnt bei Abständen außerhalb. Die Suche nach Plätzen hat
  eine Grenze je Blatt; erreicht ein Blatt sie, sagt es das, und seine übrigen Nummern bleiben ohne Suche. Hat die
  Bahndatei eine Lage geändert, über der eine Korrektur mit `base_seen` steht, meldet `check` das. Bewertung und
  Suche sitzen in einem eigenen Modul, das freemapper-courseplan übernehmen wird (ADR 0038).
- `init --courses <DATEI>` legt ein Projekt aus einer Bahndatei an. Es liest die Datei zuerst und passt sie an die
  Karte an, eine Datei, die nicht zur Karte passt, stoppt `init` also vor dem Schreiben. Aus einer `.ppen`
  entstehen ein Layout je Druckmaßstab der Datei, eine Zuordnung für jede Bahn, ein Beschreibungskasten in der
  oberen rechten Ecke des Kartenausschnitts und `map_frame.crop = true`; eine Bahn ohne Druckmaßstab bekommt den
  Kartenmaßstab. Aus IOF-XML entsteht ein Layout im Kartenmaßstab ohne Beschreibungskasten. Jede Layout-ID nennt
  ihren Druckmaßstab, etwa `a4-4000`. Der Titelblock beginnt mit `{event} · {course}`, so nennt jedes Bahnblatt
  seine Bahn, auch eines aus IOF-XML ohne Beschreibungskasten. `init` sagt, woher jeder Wert stammt.
- `numbers accept <PROJEKT> --course <NAME> --control <CODE> (--layout-wide | --sheet)` trägt eine Nummernkorrektur
  in die Projektdatei ein. Ohne `--direction` und `--gap` nimmt es den Vorschlag, den `check` zeigt. Es hängt eine
  Tabelle `[[numbers]]` an und lässt alles davor unverändert, auch die Kommentare, und lädt die Datei danach neu;
  schlägt das fehl, stellt es den alten Inhalt wieder her. Ein Link auf die Projektdatei bleibt ein Link, die Datei
  behält Rechte und Zeilenenden. Eine Korrektur für denselben Posten und Geltungsbereich lehnt es ab.
  `--layout-wide` übernimmt den Vorschlag nur, wenn er auf jeder Bahn des Layouts frei ist, die die Nummer druckt
  (ADR 0025, ADR 0038).
- Beispiel `examples/luebeck-competition-courses`: der Lübecker Sprint mit drei Bahnen aus einer von Hand
  geschriebenen Purple-Pen-Datei, A und B auf A4 in 1:4000, C auf A3 in 1:3000, mit Postenbeschreibung,
  Sperrgebiet, verbotener Querung, markierter Strecke zum Ziel und einer mit `numbers accept` versetzten Nummer.
  Der Ordner `iof/` druckt dieselben Bahnen aus IOF-XML 3.0, ohne Beschreibung. Der Start steht auf der Straße im
  Westen der Altstadt.
- Beispiel `examples/berlin-viktoriapark-training`: ein Vereinstraining von 1,7 km auf der Viktoriapark-Karte von
  freemapper (Version 0.6.0), eine Bahn aus Purple Pen auf A4 in 1:4000 mit Postenbeschreibung.
- Handbuch-Kapitel 8 „Bahnen“ und die Hinweise und Fehler zu Bahnen in Kapitel 10, auf Deutsch und Englisch. Die
  READMEs nennen, was M3 bringt.

### Behoben

- Erzeugte Nordlinien ohne Nordlinien-Symbol in der Karte (die schlichten schwarzen Linien von 0,14 mm) wuchsen mit
  dem Symbolfaktor und wurden deshalb auf einer Karte, die in einem vergrößerten Maßstab gezeichnet ist, zu dünn
  gedruckt. Sie haben jetzt die Größe, die die Norm für den Druckmaßstab vorgibt, wie die Bahnsymbole (ADR 0018).

## [0.2.0] - 2026-11-09

### Hinzugefügt

- Eine Projektdatei je Veranstaltung, `freemapper-print.toml`, mit drei Befehlen. `init` legt sie für eine
  Karte an: ein Layout im Maßstab der Karte auf A4 oder A3 (das erste, auf dem die Karte ohne Hinweise Platz
  hat), einen Titelblock, eine Kontrollstrecke und einen Vorschlag für die Äquidistanz zum Bestätigen (2 m für
  Sprint-, 5 m für Waldkarten); es sagt, woher die Kartenart kommt. `check`
  zeichnet jedes Layout, ohne etwas zu schreiben, listet die Hinweise und endet mit Exit-Code 1, wenn es welche
  gibt. Zeilen, die nur informieren (ergänzte Nordlinien, gewollter Beschnitt), beginnen mit „Zur Information:“
  und zählen nicht. `export` schreibt je Layout ein PDF, benannt nach der Layout-ID, in den Ordner `out` oder nach
  `--out-dir`; `--layout` wählt einzelne Layouts. Ein Tippfehler in der Projektdatei ist ein Fehler mit Zeile
  und Spalte und wird nicht still übergangen, ebenso ein Wert außerhalb seines Bereichs (etwa eine Schrift über
  200 pt oder ein Passepartout von halber Papierbreite).
- Layouts: beliebig viele je Veranstaltung, jedes mit Papier (A4, A3), Ausrichtung, Druckmaßstab, Passepartout,
  Rahmenlinie und auf Wunsch einem Kartenpunkt in der Mitte des Kartenausschnitts, so angegeben, wie die
  Statusleiste von OOM ihn zeigt. Mit `map_frame.crop = true` beschneidet ein Layout die Karte mit Absicht: Der
  Überstand ist dann nur ein Hinweis, und `check` geht durch. Ein Layout kann Kartenteile und gemeinsame
  Blattelemente ausblenden und eigene Elemente haben.
- Blattelemente, jedes an einer von neun Stellen des Blatts oder des Kartenausschnitts verankert: Titelblock und
  Texte mit Platzhaltern (`{event}`, `{date}`, `{organiser}`, `{print_scale}`, `{map_scale}`,
  `{contour_interval}`, `{layout}`), Logos und eine Kontrollstrecke. Fehlt auf einem Blatt der Maßstab oder
  die Äquidistanz, gibt es einen Hinweis, denn die IOF verlangt beide; ebenso bei einem Layout ohne
  Kontrollstrecke.
- Fußzeile auf jedem Blatt: die Quellenangaben der Karte (oder aus `print.copyright`), eine je Zeile, und
  darunter die Namensnennung „Erstellt mit freemapper-print“ mit der Adresse des Projekts. Die Namensnennung
  lässt sich nicht abschalten: Ein Element `credit_line` verschiebt sie nur, wenn es mindestens 5 pt groß ist und
  ganz auf dem Blatt liegt, und sie liegt auf weißem Kasten über allem anderen. Auch die ganze Fußzeile steht auf
  weißem Kasten, ein dunkles Element darunter kann sie nicht verdecken. Sie steht unter dem Kartenausschnitt,
  wenn sie dort ganz Platz hat, sonst in der Karte, und `check` meldet das und jedes Element, das sie abdeckt,
  und nennt, was die Fußzeile enthält. Höchstens sechs Quellenzeilen erscheinen, eine zu lange wird gekürzt und
  gemeldet.
- Logos als PNG, JPEG oder SVG, so breit wie `width_mm`. PNG und JPEG behalten ihre Transparenz. Ein SVG wird
  in CMYK gedruckt und bleibt in jeder Größe scharf; Verläufe erscheinen in ihrer ersten Farbe, Transparenz
  deckend, und das Programm meldet jede Vereinfachung. Zum Schutz vor Dateien, die Minuten rechnen oder
  Gigabyte Speicher verlangen, nimmt das Programm nur einfache SVGs an: Formen, Verläufe, `use` und einfache
  Beschneidungspfade. Ein Logo mit Text, Masken, Filtern, Mustern, eingebetteten Bildern oder Stilvorlagen, mit
  mehrdeutigen Attributen oder einer aufwendigen DTD wird ausgelassen, und der Hinweis rät, es als einfaches SVG
  zu exportieren. Ebenso eine fehlende, kaputte oder zu große Datei (über 10 MB, 25 Megapixel oder 20 000
  Pfadsegmente). Der Rest des Blatts erscheint trotzdem. Jede Logodatei wird je `check` oder `export` einmal
  dekodiert und je PDF einmal eingebettet; ein Blatt trägt höchstens 50 Megapixel an Logos und 1 000 000
  Einheiten SVG-Arbeit.
- Kontrollstrecke: ein Balken in runder Länge (5 m bis 1000 m, höchstens 50 mm im Druck), beschriftet etwa
  „200 m = 50 mm“. Am Ausdruck lässt sich nachmessen, ob der Drucker skaliert hat.
- Nordlinien für Karten ohne welche: Hat die Karte kein Symbol 601 und das Layout `north_lines = "auto"`,
  ergänzt das Programm senkrechte Linien im Bodenabstand der Kartenart (Sprint 120 m, Wald 300 m, Schulhof
  37,5 m, noch ungeprüft), mit dem Symbol 601 der Karte, sodass sie wie gezeichnete unter Mauern und Zäunen
  liegen. Lücken an kleinen Objekten setzt weiter der Kartograf in OOM. Ohne Nordlinien-Symbol gibt es schwarze
  Linien von 0,14 mm und einen Hinweis.
- Meldungen und Hilfe auf Deutsch und Englisch. Die Sprache kommt aus `--lang de|en`, sonst aus `FREEMAPPER_LANG`
  und der Spracheinstellung des Systems. Auch die Hinweise zu Logos kommen in der Programmsprache.
  Fehlermeldungen zur Projektdatei, Einzelheiten vom System oder einem Bilddecoder in einem Hinweis (etwa der
  Name eines SVG-Elements) und die Überschriften der Hilfe bleiben vorerst Englisch. Die Sprache auf dem Blatt
  (Datum, Dezimalzeichen, Namensnennung) stellt `event.lang` ein.
- Sicheres Schreiben: Vor jedem PDF prüft das Programm, ob die Größe des PDFs und 50 MB Reserve frei sind. Das
  PDF entsteht als `.part`-Datei und bekommt seinen Namen erst, wenn es vollständig geschrieben ist. Ein voller
  Datenträger hinterlässt kein halbes PDF, und die Meldung nennt den nötigen und den freien Platz. Auch ein
  anderer Fehler oder ein Absturz beim Schreiben lässt keine `.part`-Datei zurück.
- Beispiel `examples/luebeck-competition-layouts`: ein Stadtsprint mit einem A4-Blatt in 1:4000 für die
  älteren Klassen und einem A3-Blatt in 1:3000 für die jüngsten, mit Titelblock, zwei erfundenen Logos,
  Kontrollstrecke und Fußzeile, gebaut nur mit `init`, `check` und `export`.

### Geändert

- `render` setzt die Fußzeile nun auch auf sein PDF: die Quellenangaben aus den Kartennotizen und die
  Namensnennung „Erstellt mit freemapper-print“, in der Programmsprache, auf einem weißen Kasten unten links.
  So trägt eine Karte aus OpenStreetMap-Daten die Quellenangabe, die ihre Lizenz verlangt. Die Karte selbst
  verschiebt sich nicht; die PDFs von `luebeck-scales` sind neu erzeugt.
- Die Lübecker Karte in den Tests und im Beispiel `luebeck-scales` stammt jetzt aus freemapper 0.5.0. Sie hat
  die IOF-Farben nach Rev. 4 (etwa Braun 25/75/100/0 statt 0/56/100/18) und Gebäude in Schwarz 50 % statt 60 %.
  Ihre Nordlinien liegen nun im Kartenteil „default part“; „peripheral information“ enthält nur noch den
  Informationsblock. Die PDFs von `luebeck-scales` sind neu erzeugt, Maße und Überstand (4,0 mm auf A4) sind
  gleich geblieben.

### Behoben

- Eine Fehlermeldung wiederholt die Meldung des Systems nicht mehr, wie in „No such file or directory (os
  error 2): No such file or directory (os error 2)“.
- Der PDF-Schreiber lehnt eine Seite mit einer unendlichen oder ungültigen Zahl ab, statt ein ungültiges PDF zu
  schreiben.

## [0.1.0] - 2026-11-08

### Hinzugefügt

- Projektgerüst nach dem Vorbild von freemapper: Rust-Workspace mit Bibliothek und Programm `freemapper-print`
  (bisher nur `--version` und `--help`), CI für Linux, macOS und Windows, Release-Workflow.
- Design, Glossar und Architekturentscheidungen.
- `freemapper-print render <Karte> --out <PDF>` druckt eine OOM-Karte als PDF im exakten Maßstab. Papier (A4,
  A3), Ausrichtung und Druckmaßstab sind wählbar, `--kind` gibt die Kartenart vor, `--show-sheet-objects`
  druckt den Informationsblock von freemapper mit. Die Nordlinien gehören zur Karte und stehen immer im PDF.
  Das Programm meldet, wie viele Millimeter der Karte nicht auf die Seite passen, außerdem nicht gezeichnete,
  vereinfachte oder abgeschnittene Objekte, ersetzte Schriften und fehlende Zeichen. Zum Schluss erinnert es
  ans Drucken in Originalgröße. Steuerzeichen aus der Kartendatei gibt das Programm nur maskiert aus,
  Fehlermeldungen eingeschlossen.
- Die Karte aus OOM (`.omap`) wird gelesen, mit allen Kartenteilen.
  Hintergrundbilder (Vorlagen) bleiben unberücksichtigt.
- Flächenmuster, spitze Linienenden, Strichpunktsymbole, Textrahmen und gedrehter Text werden wie in OOM
  gezeichnet. Kartentexte stehen in der eingebetteten Schrift Arimo, auch mit Umlauten und anderen
  Unicode-Zeichen.
- Die Symbolgrößen folgen den IOF-Normen. Bei einem größeren Druckmaßstab wächst alles im selben Verhältnis:
  eine Sprintkarte in 1:3000 auf 133 %, eine Waldkarte in 1:10000 auf 150 %, in 1:7500 auf 200 % und in 1:5000
  auf 300 %. Einen kleineren Maßstab als den Basismaßstab (Sprint 1:5000, Wald 1:20000) druckt das Programm
  mit Symbolen in Normgröße und warnt, dass der Druck nicht der Norm entspricht und nur fürs Training taugt.
- Beispiel `examples/luebeck-scales`: dieselbe Sprintkarte in 1:4000 auf A4 und in 1:3000 auf A3, mit
  Anleitung zum Nachmessen.
- Ein Test vergleicht die Ausgabe mit dem PDF-Export von OOM.
- Die Release-Archive enthalten die Lizenz der Schrift Arimo als `FONT-LICENSE.txt`.
- Texte in einem Textfeld brechen wie in OOM an der Feldbreite um und richten sich nach den Feldkanten aus.
  Der Absatzabstand der Textsignatur gilt nach jedem Absatz.
- Der Überstand misst, was tatsächlich gedruckt würde, also mit Linienbreiten, Punktsymbolen und Texten. Die
  Seite wird auch danach zentriert.
- Kaputte oder überladene Kartendateien halten das Programm nicht mehr auf: Zu tief verschachteltes XML ist
  ein Fehler, und eine Seite hat ein Zeichenlimit. Das Programm meldet Objekte, die es deshalb nicht oder nur
  zum Teil zeichnet, außerdem Objekte unbekannter Art, verworfene Flächenmuster und eine kaputte
  `freemapper.toml`. Das Limit zählt jeden Punkt, den eine Seite behält, und jeden Umriss, den das PDF
  schreibt. Die schlimmste gemessene präparierte Datei, 500 000 kurze Linien in 134 KB, brauchte etwa 200 MB
  Speicher.
- Eine Textfeldgröße oder Drehung, die keine endliche Zahl ist, und eine negative Textfeldgröße sind
  Lesefehler. Alte Textfelder, deren Koordinaten als `<coord>`-Elemente gespeichert sind, behalten ihr Feld.
