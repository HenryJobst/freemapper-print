# Images of the user manual

The figures in `docs/manual/de.md` and `docs/manual/en.md` show sheets that freemapper-print 0.8.0 made from two
maps and the browser editor working on one of them.

## Source and permission

- Maps: "Gleisdreieck" and "Rummelsburg" (Berlin, sprint, 1:4000, OpenOrienteering Mapper files).
- Author: Remo Madella, MOO Challenge Berlin 2025.
- Date of release: 2026-11-12. Remo Madella released the two maps for use inside the freemapper suite.
- No other licence is stated. The images are used with his permission and are not under an open licence. Ask the
  author before you reuse them elsewhere. The maps themselves are not in this repository; only the finished images
  are.

Every caption in the manual names the map author.

## How they were made

Nothing here is a photograph or a drawing. Each sheet is a PDF from `freemapper-print export` (or `render` for the
first figure), converted with `pdftoppm` at 110 to 200 dpi and saved as JPEG (quality 82 to 86) or PNG. The detail
figures are crops of such pages. The two editor figures are 1400 × 900 pixel screenshots of `freemapper-print edit`
with the German interface.

The project files and the course file are not in the repository. The settings that matter:

- Sheets: A4 landscape at 1:4000, `map_frame` with `centre_map_mm` and `crop = true` so that a sheet shows a
  section of the map. Passe-partout 10 mm, in the passe-partout figure 30 mm. Layout sheet and course sheets:
  title block top left, scale check bar bottom right, a logo (a small compass mark drawn for this purpose, not a
  real club logo). The event, club and date are made up.
- Courses: two courses written by hand as a Purple Pen file with controls on the point features of the map (8
  controls for course "A", 5 for course "B"), with the description symbols of column D. A control number was moved
  with `numbers accept`.
- Window figure: `masks window --course A --radius 14 --assign`.
- Corridor figure: `masks corridor --width 12 --course B --assign` along ten points, on a sheet of
  `page_mm = [150, 210]` in landscape, 8 mm passe-partout.
- Scale figure: the same section at `print_scale` 4000 and 3000, each cropped to 80 mm × 60 mm on paper.
- Plain map figure: `render rummelsburg.omap --paper a3` without a project.

## Files

| File | Chapter | Content |
|---|---|---|
| `render-map.jpg` | 9 | `render`, whole Rummelsburg map on A3 |
| `sheet-passe-partout.jpg` | 6 | passe-partout, frame line and credit line |
| `sheet-elements.jpg` | 7 | title block, logo, scale check bar, credit line |
| `scale-comparison.jpg` | 9 | 1:4000 next to 1:3000 |
| `course-sheet.jpg` | 8 | course sheet with overprint and description box |
| `description-box.png` | 8 | the description box |
| `course-detail.jpg` | 8 | circles, legs and control numbers |
| `mask-window.jpg` | 11 | window mask |
| `mask-corridor.jpg` | 11 | corridor mask |
| `editor-element.jpg` | 12 | the editor with an element form |
| `editor-window-mask.png` | 12 | the editor after adding a window mask |
