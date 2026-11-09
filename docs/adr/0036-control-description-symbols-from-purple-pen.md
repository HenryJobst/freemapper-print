# 0036. Control description symbols from Purple Pen

Date: 2026-11-09
Status: Accepted

## Context

The description box (Task 9) needs the IOF pictograms for columns C to H and the instruction rows, plus a German
and an English text per symbol. A `.ppen` stores symbol ids such as `1.10`, the 2004 numbers, even for 2018
descriptions. Purple Pen's `src/PurplePenCore/symbols.xml` (`petergolde/PurplePen` at `201417f9`) defines
201 entries under its BSD licence, which `SymbolDB.cs` and `license.htm` state (ADR 0033).

## Decision

- `crates/freemapper-print/data/purple-pen-symbols.xml` is that file, unchanged and byte-identical (CRLF, BOM).
  `include_str!` embeds it, and `describe::symbols` parses it once into a `OnceLock`. The parser uses the
  course readers' guards (no DOCTYPE, predefined entities only) and limits nesting, points per symbol, ids,
  texts and coordinates.
- Geometry is in cells, x right and y down, 0..1 (instruction rows 0..8 by 0..1); widths and radii too.
  Strokes, fills (even-odd) and circles stay apart; strokes keep Purple Pen's round or flat ends and round or
  sharp corners. Beziers become polylines within 0.005 of a cell.
- `symbol(id, standard)` returns the first entry of the id whose `standard` list holds the standard, or
  that has no list. `Standard::from_description` maps "2004", "2018" and "2024"; Purple Pen knows only 2004
  and 2018, so 2024 uses the 2018 symbols.
- `symbol_text(id, lang)` mirrors `FindBestText`: the language, then English; singular before plural; no
  case first; file order. Gender is ignored. Placeholders such as `{0}` stay. All versions of an id share
  their texts, so the text needs no standard.
- `THIRD_PARTY.md` and `data/README.md` carry the BSD notice, and the release archives ship `THIRD_PARTY.md`.

## Consequences

Purple Pen's pictograms and wording are what Purple Pen users already see. A Purple Pen update needs a new
copy, a new hash and a check that the tests still pass. A replacement id (`replacement-id`) is not followed; a
symbol missing from the standard returns `None`. A real 2024 symbol set would need its own entries.

Source: M3 plan Task 8 and the controller rulings for it (source, licence, standards, geometry, texts,
embedding).
