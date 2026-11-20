# 0076. Example preview images as WebP

Date: 2026-11-20
Status: Accepted

## Context
Each example in `examples/` has a `preview.jpg` of its sheet. The sibling project freemapper uses WebP for its
previews. WebP shows the sharp text and thin lines of a map sheet in a smaller file, and GitHub renders it in a
README.

## Decision
- The eight previews are `preview.webp`, made with `magick … -quality 88 -define webp:method=6`. They were converted
  from the JPEG files, because the PNG sources are not kept; quality 88 keeps the loss small. The files are 20 to 40 %
  smaller than the JPEGs at quality 80 to 82.
- The READMEs and the manual point to the WebP files, and the commands in the READMEs that made the JPEGs now write
  WebP. A new preview is made from its PDF with `pdftoppm` and `magick`, as before.
- The figures of the manual in `docs/manual/img/` stay as they are; this decision covers the example previews.
- Logos in test projects stay PNG and JPEG, because the PDF writer embeds those formats.

## Consequences
- A reader whose viewer does not show WebP sees no preview. Current browsers, GitHub and GitLab do.
- Cost if wrong: converting back is one `magick` call per file.
