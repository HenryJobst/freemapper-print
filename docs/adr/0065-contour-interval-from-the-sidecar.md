# 0065. The contour interval comes from the sidecar when it has one

Date: 2026-11-13
Status: Accepted

## Context

`init` proposed a contour interval by map kind (2 m sprint, 5 m forest, none for schoolyard) and left it to the
person to look it up. freemapper computes the contours itself and knows the interval it used. Its sidecar
`freemapper.toml` now carries it as `[contours] interval_m` when it differs from the standard of the kind
(freemapper-core v0.25.0, contract ADR 0018: `Sidecar::contour_interval_m()`; `None` means the standard applies).
The user asked on 2026-11-13 that print reads it through the contract.

## Decision

- `init` reads the value with `scale::read_sidecar_contour_interval` and writes it as `map.contour_interval_m`. It
  beats the proposal for the kind; a sidecar without the key, or one that cannot be used, leaves the proposal.
- The comment above the key and the hint after `init` say which of the two it is, so that a person knows whether
  freemapper or the program guessed.
- A value from the sidecar needs no extra confirmation text. `map.contour_interval_m` stays a normal key that a person
  may edit; the sidecar is read once, by `init`, not on every `check` or `export`.

## Consequences

- A forest map at 2.5 m prints `1:10000 · 2,5 m` without a hand edit.
- A project made before this change keeps its value; nothing is read again from the sidecar.
- Only `init` changed; `contour_interval_m` at the top level of the sidecar (freemapper's own setting) is not read.
