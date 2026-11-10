# 0048: White-outs are keyed by Purple Pen's own ids

Status: accepted, amends ADR 0045

## Context

ADR 0045 named white-outs `whiteout-<n>` after their place in the course file. Deleting or reordering one in Purple
Pen silently changed what an assignment meant. freemapper-core v0.4.x carries `id: Option<u32>` on every special
object, filled by the `.ppen` reader.

## Decision

- `n` is Purple Pen's special-object id when every white-out of the file has one that is above zero and unique.
- Otherwise `n` is the place in the file, counting from 1, for all white-outs of the file (no mixing, so ids cannot
  collide).
- Ids, notes and the `check` Info lines are unchanged; only the number behind `whiteout-` differs.

## Consequences

Project files from v0.5.0 that name `whiteout-1` and so on stay valid only if Purple Pen's ids happen to equal the
positions; otherwise `check` reports the missing mask (`Stale::Mask`) and the course prints without it. The manual
and the glossary say that the number is Purple Pen's own.
