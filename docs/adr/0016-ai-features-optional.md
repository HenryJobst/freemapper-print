# 0016. AI features inside the program are optional

Date: 2026-11-08
Status: Accepted

## Context

The user set a rule for the whole suite (freemapper, freemapper-print, freemapper-courseplan): features that
call a language model inside an application must not be required. Club course setters may have no model
access, and print layouts must come out the same with or without one. The MCP server is a different case: an
external assistant drives the program there, and the program itself calls no model.

## Decision

- freemapper-print is fully usable without any AI model: editor, CLI and MCP server.
- A feature that calls a model is active only when a model is configured, and the same task stays possible
  without it, by hand or by a deterministic default.
- The MCP server never calls a model itself.

## Consequences

- Automatic number placement, legibility checks and layout defaults are deterministic code.
- A later AI feature (for example a layout suggestion) needs a configuration switch and a fallback.
- Cost if wrong: if a future feature only makes sense with a model, it stays an optional extra.

Source: user rule for the suite, passed on by the freemapper-courseplan session on 2026-11-08.
