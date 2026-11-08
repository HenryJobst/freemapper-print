# 0008. One project file for editor, CLI and MCP

Date: 2026-11-08
Status: Accepted

## Context

An event has 1 to 5 layouts, several mask variants and assignments of courses to both. The same event is
prepared in the editor, from the command line or by an AI agent, sometimes mixed.

## Decision

Each event has one readable TOML project file. It names the map source and the course source by relative path
and holds the shared elements, layouts, mask variants, assignments and user-entered control descriptions. The
editor, the CLI and the MCP server read and write this file and keep nothing elsewhere. The course file stays a
live link and is read again on every check and export.

## Consequences

- An event can be repeated next year by copying the folder and changing the course file.
- The file can be reviewed and versioned like any text file.
- Cost if wrong: two tools writing at the same moment can overwrite each other. As in freemapper, there is no
  lock in the first version.

Source: brainstorming 2026-11-08 (user: "eine Projektdatei für alle").
