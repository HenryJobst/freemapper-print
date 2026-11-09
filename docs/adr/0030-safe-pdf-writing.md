# 0030. Safe PDF writing: free-space check, `.part` file, 50 MB reserve

Date: 2026-11-09
Status: Accepted

## Context

A full disk must not leave a half-written PDF that looks finished (spec §5.10, suite rule from freemapper). An
export writes one PDF per layout. freemapper checks free space before it fetches data and keeps 1 GB free per
volume (its ADR 0045), because its downloads reach hundreds of megabytes.

## Decision

- Before writing, the program asks the volume for its free space and refuses when it holds less than the PDF
  plus a reserve. The error names the file, the megabytes needed and the megabytes free.
- The reserve is 50 MB (`DISK_RESERVE_BYTES`). A PDF here is a few megabytes; a 1 GB reserve would refuse a small
  export on a nearly full disk that can hold it with room to spare.
- The bytes go to `<name>.pdf.part` next to the target, are synced, and the file is renamed to the target. An
  existing PDF stays untouched until then. On any error the `.part` file is removed. A disk that fills up while
  writing is reported as a full disk, with 0 MB free if the second query fails too.
- A file that already has the `.part` name is overwritten and removed. Such a name is unlikely next to a PDF.
- If the free space cannot be read, the check is skipped and the write itself still catches a full disk.
- The query uses `fs4` 1.1.0, released 2026-04-28 and so older than ADR 0006 asks. freemapper pins
  `fs4 =0.13.1`. Task 1 had already reviewed 1.1.0, so freemapper-print keeps it.

## Consequences

- A reader of the output folder sees either the old PDF or the complete new one.
- A panic between creating and renaming the `.part` file leaves it behind; there is no drop guard.
- Cost if wrong: different reserves and `fs4` versions across the suite until a shared crate settles them.

Source: spec §5.10; M2 plan, Task 1; ledger rulings of Task 1 and Task 7 (50 MB reserve, fs4 1.1.0).
