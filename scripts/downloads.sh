#!/usr/bin/env bash
# Counts the downloads of the program itself and prints a shields.io endpoint document (freemapper ADR 0177).
#
#   scripts/downloads.sh owner/repo
#
# Only the release archives count: `<repo>-<tag>-<platform>.tar.gz` and `.zip`. The checksums, the
# update manifest and its signature and the `.gz` files of `update` do not, because one installation
# fetches several of them or none of its own.
set -euo pipefail

repo=${1:?usage: downloads.sh owner/repo}
name=${repo#*/}

total=$(gh api --paginate "repos/$repo/releases" --jq "
  [.[].assets[]
   | select(.name | test(\"^$name-v[0-9][^/]*-(macos|linux|windows)-[a-z0-9_]+\\\\.(tar\\\\.gz|zip)\$\"))
   | .download_count]
  | add // 0" | paste -sd+ - | bc)

printf '{"schemaVersion":1,"label":"%s","message":"%s","color":"blue"}\n' "${DOWNLOADS_LABEL:-Downloads}" "$total"
