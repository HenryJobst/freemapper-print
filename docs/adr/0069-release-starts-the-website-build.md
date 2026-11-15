# 0069. A release starts the website build

Date: 2026-11-15
Status: Accepted

## Context
The suite has a website, https://freemapper.de (repository freemapper-site, GitHub Pages). It shows freemapper-print's
download table, manual and changes from the newest release tag, so it must build again after each release. freemapper
does the same since its PR #166.

## Decision
- `release.yml` gets the job `website`. It needs `release`, skips a dry run and has no permissions of its own. It runs
  `gh workflow run deploy.yml -R HenryJobst/freemapper-site` with the token from the secret `SITE_DISPATCH_TOKEN`,
  which reaches the shell only as an environment variable.
- Without the secret the job ends with success; if the start fails it prints a warning. The release is published by
  then, and a website that builds late hurts nobody.
- The READMEs, the manual and this file link to the website. Headings of the manual stay as they are, because the
  website builds its anchors from them.

## Consequences
- The token is one more secret to place and renew; it can start one workflow in one repository and nothing else.
- Cost if wrong: a website that shows the old release until someone starts its build by hand.
