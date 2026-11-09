# 0030. Safe PDF writing: free-space check, `.part` file, 50 MB reserve

Date: 2026-11-09
Status: Accepted

## Context

A full disk must not leave a half-written PDF that looks finished (spec §5.10, suite rule). freemapper keeps 1 GB
free per volume (its ADR 0045), because its downloads reach hundreds of megabytes.

## Decision

- Before writing, the program asks the volume for its free space and refuses when it holds less than the PDF plus
  a 50 MB reserve (`DISK_RESERVE_BYTES`). A PDF here is a few megabytes; 1 GB would refuse small exports that fit.
- The bytes go to a new partial file `<file>.<process id>.<counter>.part` next to the target
  (`a4.pdf.1234.0.part`), are synced and renamed to the target, so an existing PDF stays until then. Since ADR
  0041 `freemapper-fs` does this step. It creates the partial file new and skips a name that is taken, so two runs
  into one folder no longer share a name. Up to 0.3.0 the name was `a4.pdf.part`.
- A failed write removes its partial file. A disk that fills up while writing is a full disk; when the volume then
  does not say how much is free, the message says so instead of "0 MB".
- If the free space cannot be read before writing, the check is skipped and the write still catches a full disk.
- `OutputError::Io { path, source }` names the file; the OS error is its source and not repeated in its message.
- `fs4` 1.1.0 (2026-04-28, older than ADR 0006 asks) does the query; freemapper pins `=0.13.1`.

## Consequences

- A reader of the output folder sees either the old PDF or the complete new one.
- Cost if wrong: different reserves and `fs4` versions across the suite until a shared crate settles them.

Amended by ADR 0041 (core switch, Task 3): the partial file and the rename come from `freemapper-fs`; the
free-space check and the reserve stay here.

Source: spec §5.10; M2 plan Task 1; ledger rulings of Task 1 and Task 7; final review M2 and Task 1 parked items.
