# 0003. Glossary first, English code, bilingual user docs

Date: 2026-11-08
Status: Accepted

## Context

The users and most orienteering terms in daily use are German. Code, specs and the IOF standards are English.
freemapper fixed this with a glossary (its ADR 0007), and the two projects share many terms.

## Decision

- `docs/glossary.md` holds the German term, the English term, a definition and the code identifier for every
  domain term. A new term gets its row before or with its code.
- Terms that freemapper's glossary already defines keep freemapper's names.
- Source code is entirely English.
- The READMEs, the user manual, the examples README and the changelog are bilingual (German and English).
  Specs, plans and ADRs are English.

## Consequences

- One concept has one name in code, docs and messages.
- Cost if wrong: each user-facing change is written twice, and the two languages can drift apart.

Source: brainstorming 2026-11-08; freemapper ADR 0007.
