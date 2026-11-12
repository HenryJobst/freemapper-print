# 0062. Manual figures come from two released maps

Date: 2026-11-12
Status: Accepted

## Context

The manual had no images. Sheets, masks and the editor are easier to explain with a picture. The sample maps in
`examples/` are small and made by the project; for the manual the suite wants real sprint maps. Remo Madella
released "Gleisdreieck" and "Rummelsburger Bucht" (MOO Challenge Berlin 2025) on 2026-11-12 for use inside the
freemapper suite, and confirmed by e-mail on the same day that the maps may be used as example maps in the public
project. The user asked for the screenshots on 2026-11-12 and named these maps.

## Decision

- All map figures of the manual are made from these two maps. The maps themselves stay out of the repository;
  only finished images are committed, under `docs/manual/img/`, each with a caption that names the author.
- `docs/manual/img/README.md` names source, date, permission and how the images were made. The maps carry no open
  licence; the permission above is the basis.
- The courses on the sheets (two courses on the Gleisdreieck map) were made with freemapper-courseplan 0.14.1
  and exported as a Purple Pen file. They carry no date and no organiser; the print project sets an event date
  and an organiser, both made up. The caption of the course sheet says who made the courses.

## Consequences

- The release archives carry `docs/manual/` and so the images (about 2 MB). The confirmed permission covers them;
  the figures need no further permission.
- The `.omap` files stay out of the repository because no figure needs them. If a later change wants them as
  example maps, the e-mail covers that use too.
- Figures show the program of 0.8.0 with the window gap of ADR 0063, and the German editor; they age with the
  interface.
