# 0079. Layouts from paper frames

Date: 2026-11-27
Status: Accepted

## Context
freemapper lets a project keep paper frames: sheets of A6, A5, A4, A3 or A2, landscape or portrait, placed on the
map (freemapper issue 343). Its export writes them as closed lines into `templates/paper-frame.omap` next to the
map, tagged `freemapper:source=paper-frame`, `freemapper:paper` and `freemapper:orientation`. A frame is the whole
sheet at the scale of the map, and its middle is the centre of the sheet (freemapper-core ADR 0046, core v0.33.0).
The frame says what the course setter wants on one sheet; freemapper-print should print exactly that.

## Decision
- The library reads the template map (`frames::read_paper_frames`) and ignores everything in it that is not a frame.
  A frame with a missing or unknown tag, or without a closed line, is counted as skipped and left out. A map without
  the file has no frames. The centre is the middle of the box of the line, in map mm with y up, the way
  `centre_map_mm` takes it.
- Each frame becomes one layout (`frames::layout_for`): the paper and orientation of the frame, `print_scale` the
  map scale, `map_frame.centre_map_mm` the centre of the frame, `crop = true` because the frame cuts the map on
  purpose, and the passe-partout of the project's first layout, which leaves room for the footer. The printed map
  frame is the sheet less the passe-partout, so it is smaller than the helper frame.
- A4 and A3 are papers of the layout. A6, A5 and A2 are free pages (`page_mm`, ADR 0058) with their portrait size, so
  no paper is added to the core and the page size limits of ADR 0058 apply.
- The layout ids are `frame-<paper>-<orientation>`, such as `frame-a4-portrait`, with `-2`, `-3` for a taken id.
- `init` adds the layouts of all frames next to the starter layout, at the map scale. With a course file they stay
  without an assignment. The command `frames <project>` (MCP tool `add_frame_layouts`) adds the layouts a project
  lacks; `--dry-run` only reports. A frame that a layout prints already (same page, orientation, centre and scale) adds
  nothing, and the program never changes or replaces a layout, so a layout the user edited stays as it is.
- The summary of `check`, `export` and `render` names a page by its size: `A5` for 148 × 210 mm, and `148 × 200 mm`
  for a size that is no paper, instead of the paper key, which is `A4` for every free page.
- The editor has a button for it in the layout panel (the edit `add_frame_layouts`). It runs the same operation and
  says what it found and added; undo takes the new layouts back, so it needs no preview step.

## Consequences
- A project made from a map with frames starts with the sheets the course setter chose. Moving a frame in freemapper
  does not move a layout that exists; the user deletes the layout and runs `frames`, or edits `centre_map_mm`.
- Only a frame at the scale of the map is a sheet at its real size. The layouts print at the map scale, so a change of
  `print_scale` leaves the frame behind; the user then sets it on purpose.
- The reader follows the contract: the test `frames` reads the fixture `paper-frames` of freemapper-contract and
  compares the centres with its `expected.toml`.

Source: freemapper issue 343 ("freemapper-print uses the frames"), agreed with the user 2026-11-27 (all papers, layouts at
`init` and on request); freemapper-core ADR 0046.
