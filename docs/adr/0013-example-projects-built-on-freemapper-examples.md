# 0013. Example projects built on freemapper's examples

Date: 2026-11-08
Status: Accepted

## Context

freemapper ships one example per German state (its ADR 0020). Prospective users want to see a finished
training sheet, not only a feature list.

## Decision

`examples/` holds finished events built on maps from freemapper's examples: a project file, the course file
(`.ppen` or IOF XML), the generated PDFs and a preview image. The map itself is referenced or copied from
freemapper. The first example is a sprint training with a window and a corridor variant on
freemapper's Lübeck old town map, as the freemapper session recommended.

## Consequences

- Each new feature that changes the printed sheet updates at least one example.
- Cost if wrong: the examples age when freemapper's maps change.

Source: freemapper ADR 0020; brainstorming 2026-11-08.

## Addendum 2026-11-09 (M2)

- The Lübeck map in the test fixtures and in `examples/` comes from freemapper v0.5.0 (commit `e8b6db9`), the
  newest stable tag. Lübeck's map objects did not change from v0.4.0 (`b4b3e05`); the north-line tags and the
  candidate tags did. The colour and priority notes of freemapper's `oom-render-reference.md` moved along.
- M2 adds no schoolyard example, because freemapper v0.5.0 has no schoolyard map, and no Viktoriapark example,
  because freemapper is about to regenerate that map. Both wait for M3 or later; `examples/README` says so.

Source: ledger rulings of 2026-11-08 and 2026-11-09 (fixture source, examples of Task 11).
