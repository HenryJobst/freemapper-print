# 0062. Manual figures come from two released maps

Date: 2026-11-12
Status: Accepted

## Context

The manual had no images. Sheets, masks and the editor are easier to explain with a picture. The sample maps in
`examples/` are small and made by the project; for the manual the suite wants real sprint maps. Remo Madella
released "Gleisdreieck" and "Rummelsburger Bucht" (MOO Challenge Berlin 2025) on 2026-11-12 for use inside the
freemapper suite. The user asked for the screenshots on 2026-11-12 and named these maps.

## Decision

- All map figures of the manual are made from these two maps. The maps themselves stay out of the repository;
  only finished images are committed, under `docs/manual/img/`, each with a caption that names the author.
- `docs/manual/img/README.md` names source, date, permission and how the images were made. No open licence is
  stated: the images are used with permission, not freely reusable.
- The courses on the sheets are made up for the manual and say so in the captions.

## Consequences

- The release archives carry `docs/manual/` and so the images (about 2 MB). Cost if wrong: if the permission does
  not cover redistribution, the images go out of the manual and the archives of the next release.
- Whether the permission covers the `.omap` files in the public repository is open (freemapper-courseplan asks
  the author); this ADR does not rely on it.
- Figures show the program of 0.8.0 and the German editor; they age with the interface.
