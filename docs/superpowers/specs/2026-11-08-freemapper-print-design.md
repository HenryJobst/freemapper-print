# freemapper-print – Design

Status: for approval · Date: 2026-11-08 · Terms follow [`docs/glossary.md`](../../glossary.md).

Decisions are recorded as ADRs in `docs/adr/`. Statements marked *(user)* were decided by the user in the
brainstorming on 2026-11-08. Statements marked *(Claude's call)* are proposals the user has not confirmed yet.

## 1. Intent

[freemapper](https://github.com/HenryJobst/freemapper) produces OOM starter projects from official geodata.
The mapper finishes the map in OpenOrienteering Mapper (OOM). Until now the next step, turning the finished map
into a printed sheet with a passe-partout, title, north lines, logos, course overprint, control descriptions and
masks for window or corridor orienteering, was done by club members with OCAD. OCAD does not run on the
author's MacBook, and the author has no Windows machine *(user)*.

OOM can draw frames, texts and north lines, but it has no course overprint, no control descriptions, no
per-course masks and no headless export. Purple Pen runs on macOS only as a beta and is the course-setting tool
the author uses today *(user)*.

**freemapper-print** closes that gap. It takes a finished map and the courses from a course-setting program and
produces the print PDF for each course: the map at the exact print scale, the course overprint, the control
descriptions, the masks and the sheet elements of the chosen layout.

Users: the author and club members who set courses for training and small events. They use the program through
a local browser editor, the CLI or an AI agent via MCP *(user)*.

## 2. Goals, non-goals, anti-goals

**First use case** *(user)*: club training in town and park terrain. That means sprint maps (ISSprOM 2019-2,
1:4000 or 1:5000) on A4, printed on a home printer or in a copy shop, with window and corridor exercises.

**Goals (first usable version)**

- Read a finished OOM map (`.omap`, `.xmap`) and render it to vector PDF at the exact print scale.
- Import courses from IOF XML 3.0 (Purple Pen, Condes, OCAD course setting) *(user)* and from Purple Pen
  projects (`.ppen`).
- Several layouts per event, 1 to 5 in practice *(user)*. A layout fixes paper size, orientation, print scale,
  map frame and the placement of sheet elements. Different age classes need different scales *(user)*.
- Mask variants for window and corridor orienteering, assigned to one or more courses *(user)*.
- Sheet elements: passe-partout, title block, north lines, logos, free texts, control description box, scale
  check bar, credit and copyright line. Shared elements are maintained once and used by every layout.
- Texts with placeholders (course name, length, climb, scale, contour interval, event title, date) that are
  filled per course.
- One readable project file per event, used by the editor, the CLI and the MCP server alike *(user)*.
- Three interfaces over one library: CLI, MCP server (stdio), local browser editor *(user)*.
- Office print PDF: correct page size, exact scale, course overprint that lets the map show through.

**Later**

- OCAD map files (`.ocd`) as map source *(user: maps also come from OCAD)*.
- Press PDF for print shops: PDF/X, bleed, crop marks, course overprint as a spot colour *(user: both
  print paths are used)*.
- Hiding symbol groups (for example a contour-only exercise) *(Claude's call: deferred)*.
- Legibility warnings for symbols that move too close after a scale change *(Claude's call: deferred)*.
- Entering control descriptions for courses imported from IOF XML *(user: `.ppen` only at first)*.
- freemapper switching to this project's renderer *(user)*.
- Maps from the author's own future map editor (separate project, not part of this design) *(user)*.

**Non-goals**

- Course setting. Courses come from Purple Pen, Condes or OCAD *(user)*.
- Editing the map itself. That stays in OOM, OCAD or the future editor.
- Pixel-identical output compared with OOM or OCAD *(user)*.

**Anti-goals** *(user)*. The program fails even if it technically works when:

- the printed scale is wrong, or
- symbols are illegible.

## 3. Constraints and decided choices

| Topic | Choice | Source |
|---|---|---|
| Language | Rust, edition 2024, pure-Rust core, one static binary per platform | ADR 0001 |
| Licence | GPL-3.0-or-later | ADR 0002 |
| Terms and languages | glossary first, English code, bilingual user docs | ADR 0003 |
| Interfaces | CLI, MCP over stdio, local browser editor | ADR 0004, 0005 |
| Dependencies | at least two weeks old, pinned where newer releases are younger | ADR 0006 |
| Toolchain | pinned in `rust-toolchain.toml` | ADR 0007 |
| Project file | one file for editor, CLI and MCP | ADR 0008 |
| Renderer | own renderer, conformant to ISSprOM/ISOM, not pixel-identical | ADR 0009 |
| Renderer origin | copied from freemapper, developed here, freemapper moves onto it later | ADR 0014 |
| Courses | IOF XML 3.0 plus `.ppen`, masks owned by freemapper-print | ADR 0010 |
| First target | club training, office print PDF | ADR 0011 |
| Scale change | symbols keep their printed size | ADR 0012 |
| Course setting vs. sheet | course setting decides what is the same on every sheet, freemapper-print the rest | ADR 0015 |
| User texts | unslop and humanizer skills on READMEs, manual, changelog, ADRs and long messages | `CLAUDE.md` |

## 4. Concepts

```
Event (project file)
├── map source        sanssouci.omap (map scale 1:4000)
├── course source     training.ppen  or  courses.xml (IOF XML 3.0)
├── shared elements   title block, logos, credit and copyright line
├── layouts           "A4 1:4000"  "A4 1:5000"   … (1–5)
├── mask variants     "window 3–5"  "corridor north" …
└── assignments       course → layout, course → mask variant (0 or 1)
```

- **Layout.** Paper size and orientation, print scale, map frame (the map extent in map coordinates and the
  rectangle it fills on the sheet), passe-partout, and the elements placed on the sheet. A layout may move or
  hide a shared element.
- **Sheet element.** Anchored to a sheet corner, edge or the map frame with an offset in millimetres, so the
  same element works on A4 and A3.
- **Map coordinates** *(Claude's call)*. Map frames, masks and element anchors on the map use OOM's map
  coordinates (millimetres on the map at map scale, y down), the same units the `.omap` stores. Maps without
  georeferencing therefore work. Projected coordinates appear only where a georeferenced map and a
  georeferenced course file are matched (§5.3).
- **Mask variant.** One or more polygons in map coordinates, each with holes. The masked area is painted white
  above the map and below the course overprint. Window orienteering uses holes around legs; corridor
  orienteering uses a hole along the corridor.
- **Sheet.** One course printed with its layout and its mask variant. Export produces one PDF per course or one
  multi-page PDF for all courses.

## 5. Architecture

Workspace with two crates, as in freemapper:

- `crates/freemapper-print`: library with all domain logic (readers, model, layout, renderer, PDF).
- `crates/freemapper-print-cli`: binary `freemapper-print` with the CLI, the MCP server and the editor's web
  server and page assets.

### 5.1 Map input

- `.omap`/`.xmap` reader for colours, symbols, objects, georeferencing and the map scale. freemapper only reads
  symbol sets from `.omap` today, so the object reader is new. The MIT crate `omap` (omap-rs) and freemapper's
  `omap/model.rs` are candidates to start from *(Claude's call: the builder chooses)*.
- `.ocd` reader later. OCAD 6–12 is documented in the OCAD wiki; OCAD 2018 and newer is not. Test early with
  real files.

### 5.2 Renderer

freemapper already renders its own `MapDocument` to PDF (`crates/freemapper/src/render/`, pdf-writer): lines
with caps, joins and dash patterns, borders, mid/start/end symbols, point symbols, combined symbols and texts,
following `docs/render/oom-render-reference.md`. Missing there and needed here: area patterns (line and point
hatching), pointed caps, dash symbols, text framing, embedded fonts, raster and vector images.

The renderer is copied from freemapper into this project and developed further here *(user, ADR 0014)*.
freemapper moves onto it later. The copy keeps a note of the freemapper commit it came from, so fixes can be
carried across until then. Since both projects are GPL-3.0, the builder may consult and port OOM's rendering
code (`src/core/renderables/`, `src/core/symbols/`) where the reference document falls short.

Rendering rules:

- **Exact scale.** One map millimetre at print scale equals one PDF millimetre. Page size equals the paper
  size. No implicit fitting.
- **Scale change (ADR 0012).** The map geometry scales by `map_scale / print_scale`. Symbol dimensions
  (line widths, dash lengths, point symbols, area pattern spacing, map texts) are multiplied by a symbol factor
  that depends on the map kind and the print scale. The table is data, not code *(Claude's call; the builder
  verifies each row against the standard's text before M1 ends)*:

  | Map kind | Base scale | Print scale | Symbol factor |
  |---|---|---|---|
  | Sprint (ISSprOM 2019-2) | 1:4000 | 1:3000, 1:4000, 1:5000 | 1.0 |
  | Forest (ISOM 2017-2) | 1:15000 | 1:15000 | 1.0 |
  | Forest (ISOM 2017-2) | 1:15000 | 1:10000, 1:7500, 1:5000 | 1.5 |
  | Schoolyard (IOF school set 2019) | 1:1000 | 1:1000 | 1.0 |

  The map kind is a project setting. `init` proposes it from the `.omap` symbol set name and map scale, and the
  user confirms it. A print scale outside the table is refused. The course overprint follows the overprint
  dimensions of the same standard at the print scale, independent of the map's symbol factor.
- **Legibility warnings** *(Claude's call: later)*. Checking minimum gaps between symbols after a scale change
  is a large geometry module. The first version reports the symbol factor and the geometry factor on export
  and leaves the gap check for later.
- **Course overprint.** Purple with the standard's widths. On the office PDF the overprint lets the map show
  through (multiply blend or overprint simulation). The map stays readable under circles and lines.
- **Colour order.** OOM colour priority, as in freemapper. The mask lies above the map and below the course
  overprint and the sheet elements.

### 5.3 Course input

Research on 2026-11-08 found:

- IOF XML 3.0 `CourseData` carries controls (map position in mm and lat/lng), courses, control order, control
  number position, leg lengths, scale and forkings. It has **no** control description columns C–H, no masks,
  no forbidden areas, no leg bends and no print areas.
- Purple Pen, Condes and OCAD write the y axis upward, against the standard. The importer must detect it.
- Purple Pen's `.ppen` (XML) has everything: description symbols per column, white-out and other specials per
  course, leg bends and gaps, number placement, print area and print scale per course.
- OCAD and Condes deliver control descriptions only separately (OCAD file, PDF or text).

Hence:

- IOF XML 3.0 is the common import *(user)*. In the first version, control descriptions come only from
  `.ppen` *(user)*. Courses from IOF XML print without a description box, and `check` says so. Entering
  descriptions in freemapper-print comes later.
- `.ppen` is a second import with descriptions, specials and bends *(Claude's call)*. Its white-out objects
  become mask variants.
- **Live link** *(Claude's call)*. The project file names the course file; `check` and `export` read it
  again each time, because courses change until shortly before a training. Data the user adds in
  freemapper-print survives a changed course file: assignments are keyed by course name, entered control
  descriptions by control code. A course or control that disappeared is reported, and its data is kept until
  the user removes it. Print scale and print area in a `.ppen` are ignored for printing; the layout decides.
  When a layout is created, `.ppen` values are offered as defaults.
- **Overprint objects in the first version** *(Claude's call)*. From `.ppen`: start, controls, finish, legs
  with bends and gaps, numbers, marked routes, mandatory crossing points, out-of-bounds and dangerous areas,
  forbidden routes, first aid, water, texts, lines and white-out (as mask variants). Sprint training runs next
  to roads, so the safety-related objects cannot wait. From IOF XML only what the format carries; entering
  further objects is later.
- Matching courses to the map: map positions in mm relative to the map's origin, checked against lat/lng when
  both the map and the file are georeferenced. A mismatch is an error, not a silent shift.
- Control description symbols: Purple Pen's `symbols.xml` (201 symbols as vector geometry, 2004 and 2018
  standards, texts in several languages) is the most complete open source. Its licence follows the Purple Pen
  repository (BSD-style per file headers, no LICENSE file); the builder confirms it before use.

### 5.4 Output

- **Office print PDF** (first): page size equals paper size, DeviceCMYK or RGB, overprint simulated,
  fonts embedded. On export, CLI, MCP and editor remind the user to print at "actual size".
- **Scale check bar** on every sheet by default: a bar with its length in metres and millimetres, so a ruler
  shows whether the printer scaled the page.
- **Press PDF** (later): PDF/X, bleed, crop marks, course overprint as a Separation colour with overprint flags.
- **Credit** *(Claude's call)*: the PDF metadata names freemapper-print and its repository; a visible credit
  line on the sheet is on by default and can be switched off per event.

### 5.5 Project file

TOML, as in freemapper *(Claude's call: format)*. A sketch, not a schema:

```toml
[event]
title = "Vereinstraining Sanssouci"
date = 2026-11-14

[sources]
map = "maps/sanssouci.omap"
courses = "courses/training.ppen"

[[shared]]
id = "title"
kind = "title_block"
lines = ["{event}", "{course} · {length_km} km", "1:{print_scale} · Äquidistanz {contour_interval} m"]
anchor = { to = "map_frame", corner = "top_left", offset_mm = [2, 2] }

[map]
kind = "sprint"          # proposed by init, confirmed by the user

[[layouts]]
id = "a4-4000"
paper = "A4"
orientation = "portrait"
print_scale = 4000
map_frame = { centre_map_mm = [12.5, -40.0], margin_mm = 8 }

[[masks]]
id = "window-3-5"
# Polygons in map coordinates (mm, y down); the first ring is the outline, the others are holes.
polygons = [[[[-60.0, -90.0], [80.0, -90.0], [80.0, 20.0], [-60.0, 20.0]],
             [[0.0, -50.0], [30.0, -50.0], [30.0, -20.0], [0.0, -20.0]]]]

[[assignments]]
course = "D"
layout = "a4-4000"
mask = "window-3-5"

# Corrections of control number positions (§5.7): direction in degrees (0 = east, counter-clockwise),
# gap in print mm. The base positions come from the course file.
[[numbers]]
control = "31"
layout = "a4-5000"             # every sheet printed with this layout
direction = 90
gap_mm = 1.0

[[numbers]]
control = "31"
course = "D"                   # only the sheet of course D (its layout and mask)
direction = 315
```

Paths are relative to the project file. The editor writes the same file and keeps comments it did not create
*(Claude's call)*.

### 5.6 Interfaces

- **CLI** (clap): `init`, `check`, `export` (one course, several or all), `edit` (opens the
  editor).
- **MCP** (rmcp over stdio, ADR 0004): tools mirror the CLI, plus tools to list layouts, elements, masks and
  assignments and to change them. Results are compact JSON; relative paths are refused.
- **Editor** (ADR 0005): local web page served by the binary. Left: event with sources, layouts, mask variants,
  shared elements and assignments. Centre: the sheet of one chosen course, with elements that can be selected
  and moved and masks that can be drawn on the map. Right: properties of the selected element. Export lives in
  the menu, not as buttons on the main view *(user)*.

### 5.7 Course setting and the sheet (ADR 0015)

Rule *(user, 2026-11-08)*: what is the same on every sheet of a course belongs to course setting; what depends
on the sheet (print scale, mask, sheet elements, map frame) belongs to freemapper-print.

- **Course setting** (read from the course file, never changed here): controls, order, legs and their bends,
  marked routes (taped routes), crossing points, forbidden areas and routes, out-of-bounds areas, control
  descriptions, and the control number positions. The course-setting program is Purple Pen today and later a
  sibling course-setting application that replaces it *(user)*.
- **freemapper-print**: masks, map frame, sheet elements, and corrections of number positions for a layout or a
  sheet. Gaps in circles and legs come from the `.ppen` in the first version.

**Control number placement** *(user, agreed with the course-setting session)*. Levels, the more specific one
winning:

1. automatic placement in freemapper-print, only for numbers the course file gives no position for (for
   example IOF XML without `MapTextPosition`);
2. the course file: the central position per control and the per-course position, both set in course setting
   (`.ppen` number positions, IOF XML `MapTextPosition`);
3. a correction per layout and control code, in the project file;
4. a correction per sheet (course, with its layout and mask) and control code, in the project file. This is the
   exception for a mask that only one course on a layout has.

A position is stored as a direction and a gap in print millimetres, so a number keeps its distance from the
circle at every print scale. In the editor the user drags a number and chooses whether the correction applies to
the layout or only to this sheet. freemapper-print never writes back to the course file. When the course file
changes a position that a correction overrides, `check` reports it.

**Check and proposals.** `check` scores every number on the finished sheet. Hard conflicts: outside the map
frame, under a sheet element, under a mask, on a circle, leg or number of the same course. Soft costs *(Claude's
call)*: covering map detail weighted by symbol (buildings, walls and impassable lines high, open areas low). For
each conflict `check` proposes a position; the user accepts it as a layout or sheet correction in the editor, by
command or through MCP. freemapper-print never moves a number silently.

**Shared placement module** *(agreed with the course-setting session)*. The scoring and search live in a module
of their own with no dependency on the rest of freemapper-print, so the course-setting application can use the
same algorithm. Callers add their own obstacles: course setting knows circles, legs and map detail;
freemapper-print adds masks, sheet elements and the map frame. Like the renderer (ADR 0014), the module is
developed here first and extracted into a shared crate later.

**Overprint and masks.** The course overprint lies above the masks, so a mask never hides a marked route, a
crossing point, a forbidden route or an out-of-bounds area. `check` warns when such an object lies inside a
masked area, because the runner then sees it without the map underneath *(Claude's call)*.

## 6. Testing and quality

- Readers (`.omap`, IOF XML, `.ppen`) test-first with small fixtures and real files from Purple Pen, Condes and
  OCAD exports.
- Renderer: golden PDFs for fixed inputs, plus a raster comparison against OOM's PDF export of the freemapper
  examples with a tolerance. Pixel identity is not the goal (ADR 0009).
- Scale: an automated check that the scale check bar and a known map distance measure correctly in the PDF.
- fmt, clippy `-D warnings` and tests in CI on Linux, macOS and Windows, lefthook pre-commit, Claude Code hooks,
  as in freemapper.

## 7. Milestones *(Claude's call)*

1. **M0** Repository, docs, glossary, ADRs, CI. (This commit.)
2. **M1** `.omap` reader and shared renderer with area patterns and embedded fonts: one map rendered to an
   office PDF at exact scale, checked against the freemapper examples.
3. **M2** Project file, layouts and sheet elements (passe-partout, title block, north lines, logos, texts,
   scale check bar, credit), CLI `export`.
4. **M3** Course import (IOF XML 3.0, `.ppen`), course overprint, control description box, control number
   positions from the course file with layout and sheet corrections, the placement module, `check` with
   proposals.
5. **M4** Mask variants and assignments.
6. **M5** MCP server.
7. **M6** Browser editor.
8. **Later** OCAD reader, press PDF, symbol group hiding.

The builder may reorder M5 and M6 or start the editor earlier if layout placement through the CLI turns out
too awkward.

## 8. Left to the builder

- Module layout inside the crates, crate choices within ADR 0006, and the exact project file schema.
- Whether to start the `.omap` reader from omap-rs or from freemapper's model.
- How the editor draws masks (polygon tool, buffer around legs, or both).
- Font choice for sheet elements among openly licensed fonts.
- Which freemapper example and which training format the first example project shows.

## 9. Questions answered after the review

- Renderer location: copied into this project, freemapper moves onto it later (§5.2, ADR 0014).
- Control descriptions in the first version: only from `.ppen` (§5.3).
