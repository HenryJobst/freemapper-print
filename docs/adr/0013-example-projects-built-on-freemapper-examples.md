# 0013. Example projects built on freemapper's examples

Date: 2026-11-08
Status: Accepted

## Context

freemapper ships one example per German state (its ADR 0020). Prospective users want to see a finished
training sheet, not only a feature list.

## Decision

`examples/` holds finished events built on maps from freemapper's examples: a project file, the course file
(`.ppen` or IOF XML), the generated PDFs and a preview image. The map itself is referenced or copied from
freemapper. The first example is a sprint training with a window and a corridor variant.

## Consequences

- Each new feature that changes the printed sheet updates at least one example.
- Cost if wrong: the examples age when freemapper's maps change.

Source: freemapper ADR 0020; brainstorming 2026-11-08.
