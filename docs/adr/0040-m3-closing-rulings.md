# 0040. M3 closing rulings: German state names and the slow probe

Date: 2026-11-09
Status: Accepted

## Context

Two rulings of the M3 review had no ADR: a wording rule for German texts (ruling R34) and the acceptance of a
slow worst case (ruling R41).

## Decision

- German user texts (READMEs, manual, messages, changelog) say "Bundesland" and "Bundesländer" for the states of
  Germany. "Land" and "Länder" only mean countries. This is the suite's term rule from freemapper. The M3 branch
  has no occurrence of the wrong use, and later texts follow the rule.
- `check` on an extreme file (1000 courses of 200 stops each) takes about 65 s. That is accepted: real course
  files hold fewer than 50 courses, and the number search budget (ADR 0038) bounds the search itself. The time
  goes into drawing every sheet again. A drawing cache per layout goes on the M4 list.

## Consequences

- A file with hundreds of courses makes `check` slow, and nothing warns about it.
- Cost if wrong: a cache per layout in M4, with no change to the output.

Source: M3 rulings R34 and R41.
