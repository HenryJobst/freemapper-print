# Images of the user manual

The figures in `docs/manual/de.md` and `docs/manual/en.md` show sheets that freemapper-print 0.8.0 (plus the
unreleased window gap, ADR 0063) made from two maps and the browser editor working on one of them.

## Source and permission

- Maps: "Gleisdreieck" and "Rummelsburg" (Berlin, sprint, 1:4000, OpenOrienteering Mapper files).
- Author: Remo Madella, MOO Challenge Berlin 2025.
- Permission: Remo Madella confirmed by e-mail on 2026-11-12 that the maps may be used as example maps in the
  public freemapper project, inside the suite. The figures need no further permission. The maps carry no licence
  of their own beyond that, so for any other use ask the author. Only the finished images are in this repository,
  not the `.omap` files.
- Courses: the courses on the sheets (A with 8 controls and 2.38 km, B with 5 controls and 1.40 km, both on the
  Gleisdreieck map) were made with freemapper-courseplan 0.14.1. They carry no date and no organiser; the event
  date 2027-05-08 and the organiser "OL-Verein Musterstadt" come from the print project and are made up.

Every caption in the manual names the map author.

## How they were made

Nothing here is a photograph or a drawing. Each sheet is a PDF from `freemapper-print export` (or `render` for the
first figure), converted with `pdftoppm` at 110 to 200 dpi and saved as JPEG (quality 80 to 84) or PNG. The detail
figures are crops of such pages. The two editor figures are 1400 × 900 pixel screenshots of
`freemapper-print edit --no-browser` with the German interface, driven with `playwright-cli`.

The project files and the course file are not in the repository. The settings that matter:

- Event: title "Training Gleisdreieck", `date = 2027-05-08`, `organiser = "OL-Verein Musterstadt"`,
  `contour_interval_m = 2.5`, sheet language German. The title block of the course sheets has four lines:
  `{event}`, `Bahn {course} · {length_km} km · {controls} Posten`, `{date} · {organiser}` and
  `1:{print_scale} · Äquidistanz {contour_interval}`. A logo (a small compass mark drawn for this purpose as SVG,
  not a real club logo) sits at the bottom left, the scale check bar at the bottom right and the description box
  at the top right.
- Course sheet of course "A": A4 portrait at 1:4000, passe-partout 8 mm, `map_frame` with
  `centre_map_mm = [47.5, 158.0]` and `crop = true`. The number of control 38 was moved with `numbers accept`
  (direction 57.9°, gap 3 mm). The description box and the detail figure are crops of this sheet at 200 dpi.
- Window figure: the same sheet with `centre_map_mm = [36.5, 160.0]` and `masks window --course A --assign`,
  no other option. Radius 15 mm, gap 3 mm; the course has no pair of points closer than 170 m, so no window got
  smaller.
- Corridor figure: course "B" on a sheet of `page_mm = [160, 190]` in landscape, 8 mm passe-partout,
  `centre_map_mm = [57.0, 17.0]`. The legs 1 to 2 and 4 to 5 cross, so the corridor is made of two pieces, each
  from `masks corridor` with `--width 12`: start to control 44, and control 44 to the finish, the outer ends a few
  millimetres beyond start and finish. The second hole is copied into the polygon of the first.
- Scale figure: the same section at `print_scale` 4000 and 3000, each cropped to 80 mm × 60 mm on paper.
- Plain map figure: `render rummelsburg.omap --paper a3` without a project.
- Layout figures (`sheet-passe-partout.jpg`, `sheet-elements.jpg`) show layout sheets without a course. Their
  event names ("Park am Gleisdreieck", "Stadtsprint Rummelsburg") and the logo are made up.

## Files

| File | Chapter | Content |
|---|---|---|
| `render-map.jpg` | 9 | `render`, whole Rummelsburg map on A3 |
| `sheet-passe-partout.jpg` | 6 | passe-partout, frame line and credit line |
| `sheet-elements.jpg` | 7 | title block, logo, scale check bar, credit line |
| `scale-comparison.jpg` | 9 | 1:4000 next to 1:3000 |
| `course-sheet.jpg` | 8 | course "A" sheet with overprint and description box |
| `description-box.png` | 8 | the description box of course "A" |
| `course-detail.jpg` | 8 | circles, legs and control numbers 4 to 6 |
| `mask-window.jpg` | 11 | window mask, default windows, course "A" |
| `mask-corridor.jpg` | 11 | corridor mask, course "B" |
| `editor-element.jpg` | 12 | the editor with an element form |
| `editor-window-mask.png` | 12 | the editor after adding a window mask |
