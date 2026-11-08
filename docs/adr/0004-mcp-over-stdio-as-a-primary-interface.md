# 0004. MCP over stdio as a primary interface

Date: 2026-11-08
Status: Accepted

## Context

The user wants to drive freemapper-print through an AI agent as well as from the command line. freemapper's
MCP server (its ADR 0005) works with Claude Code and other MCP clients.

## Decision

`freemapper-print mcp` serves MCP over stdio with `rmcp`, the official SDK, in the same way as freemapper. The
tools mirror the CLI and add tools that list and change layouts, elements, masks and assignments in the project
file. Results are compact JSON text. Relative paths are refused, because the agent does not know the server's
working folder.

## Consequences

- An agent can prepare a whole event: sources, layouts, masks, assignments and export.
- Cost if wrong: clients that need HTTP transport or structured output schemas need another transport later.

Source: brainstorming 2026-11-08; freemapper ADR 0005.
