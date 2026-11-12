# 0061. One approval per release

Date: 2026-11-12
Status: Accepted, amends [0060](0060-self-update-from-signed-releases.md) (P2)

## Context

ADR 0060 P2 put a job `gate` and the job `release` in the environment `release`. The environment holds the signing
secrets and has a required reviewer, so every release asked twice: once before the build, once before signing. The
user decided on 2026-11-12 that one click is enough and that the protection belongs in the environment, not in the
workflow code. An approval that only a `needs:` chain enforces can be removed by anyone who may push to `main`;
an approval on the environment cannot.

## Decision

- `gate` is removed. Only `release`, after the builds, uses the environment: one approval per release.
- Every release build is built with `FREEMAPPER_DIST=github-release` and every release is signed. A release without
  `MINISIGN_SECRET_KEY` fails in `release`, after the builds, before anything is published. The unsigned release
  of ADR 0060 is gone; a release that must go out unsigned needs a new decision.
- The environment allows the tag rule `v*` and the branch `main` (for the dry run).

## Consequences

- One click per release. Cost if wrong: none for the secret; a workflow on `main` that names the environment could
  still reach it, which the required reviewer guards.
- A missing secret costs a full build before it shows. The key is set; the dry run proved it.
- Releases up to 0.8.0 were built with the gate and keep their assets.
