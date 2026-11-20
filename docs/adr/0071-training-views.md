# 0071. Training views

Date: 2026-11-20
Status: Accepted

## Context
A trainer wants the finished training map with a class of symbols left out: an Ultimate map without paths, a
contour orienteering map in brown only, a map without north lines. freemapper writes such variants itself and
freemapper-core v0.30.1 holds the rule (`freemapper_omap::view`, ADR 0039 there: `View`, `SymbolColours`, the frame
test, `file_token`). freemapper-print prints any `.omap`, also one the user edited, so the view applies here too.

## Decision
- `LayoutSpec` gets `view`, a list of strings in the project file (`view = ["no-paths", "no:101"]`). The entries are
  parsed with `View::parse` when the project loads; a bad entry names its place (`layouts[0].view[1]`). The entries
  of a layout combine: an object stays when every one keeps it.
- The sheet applies the view to the objects in memory after `hide_parts` and before the page is drawn
  (`sheet::without_view`). The frame stays: the objects of the part "peripheral information" and those tagged
  `freemapper:source=sheet` (`View::keeps_frame`). So do the sheet elements of print itself (title block, scale
  check bar, credit line), the course overprint and the masks, which are no objects of the map.
- The part `base`, the walkable area under a sprint map, is left as it is (ADR 0044). A view works on the symbols
  drawn over it, so `brown-only` on a sprint map keeps the base fill.
- Generated north lines (ADR 0027) follow the view: when it does not keep symbol 601, `north_lines = "auto"`
  generates none. The detection of north lines on the map still looks at the whole file.
- `render` and `export` take `--view` (repeatable, combining). `render` applies it to the map before drawing; it
  generates no north lines anyway. `export` adds the entries to every layout. Without `--out-dir` it writes to
  `out/<canonical view>` (`View::file_token`, for example `out/no-paths` or `out/brown-only+no-505_506`), so that a
  view never replaces the full sheets; with `--out-dir` the person has chosen the folder.
- The editor and the MCP tools take no view yet; a project file with `view` loads, prints and is kept by them.

## Consequences
- A layout of a project can carry a view for good, for a training set that always prints without paths.
- A map whose symbol set lacks a code (an edited set, an unknown symbol) is kept by `no:` lists and dropped by
  `only:` lists, as the core's `keeps` answers for an empty code.
- Cost if wrong: `brown-only` keeps the white base of a sprint map; a person who wants the bare brown on white
  leaves the base fill out by hand until we decide to treat it differently.
