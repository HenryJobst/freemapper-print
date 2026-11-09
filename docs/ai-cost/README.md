# KI-Aufwand (Claude API)

Das Badge in der README zeigt, was alle Claude-Code-Sessions an diesem Repository zu API-Listenpreisen
gekostet hätten, Subagenten und Worktree-Sessions eingeschlossen. Wer Claude über ein Abo nutzt, zahlt diesen
Betrag nicht; die Zahl ist ein Gegenwert, keine Rechnung.

Ein Skript aus dem Nachbarprojekt claude-settings-helper liest stündlich die Session-Protokolle, rechnet jeden
Request mit den aktuellen Preisen der offiziellen
[Preisseite](https://platform.claude.com/docs/en/about-claude/pricing.md) um und schreibt das Ergebnis hierher:

- `latest.json`: der aktuelle Stand, mit Quelle und Abrufzeit der Preise;
- `history.csv`: eine Zeile für jeden Lauf, bei dem sich der Stand geändert hat.

Die Summe wird bei jedem Lauf mit den dann gültigen Preisen neu berechnet. Ältere Zeilen in `history.csv` können
deshalb von einer späteren Rechnung abweichen.

## English

The badge in the README shows what all Claude Code sessions on this repository would have cost at API list
prices, subagents and worktree sessions included. With a subscription nobody pays this amount; it is an
equivalent, not an invoice.

A script from the sibling project claude-settings-helper reads the session logs every hour, prices each request
with the current rates from the official [pricing page](https://platform.claude.com/docs/en/about-claude/pricing.md)
and writes the result here:

- `latest.json`: the current figure, with the source and fetch time of the prices;
- `history.csv`: one line for each run in which the figure changed.

Each run recomputes the total at the prices valid then, so older lines in `history.csv` can differ from a later
calculation.
