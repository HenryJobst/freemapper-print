# 0041. Writes go through freemapper-fs

Date: 2026-11-09
Status: Accepted

## Context

Up to 0.3.0 a PDF went to `<name>.pdf.part` and was renamed (ADR 0030); a file or link of that fixed name was
written to. `numbers accept` wrote through a project file that was a link (ADR 0025, M3 Task 11). Project
folders are shared and downloaded. Core ADR 0009 "Writes never follow links" (from freemapper ADR 0060) fixes
this for the suite with the crate `freemapper-fs`. It writes a new partial file `<file>.<process id>.<counter>.part`
(`create_new`, a taken name is skipped), flushes it and renames it over the target, so a link at the target name
is replaced. A failed write removes its partial file.

## Decision

- Every write of a file's content goes through `freemapper-fs` (tag `v0.2.0`). Project files go through
  `write_inside(project folder, …)`, which refuses a folder that a link leads out of the project: the project file
  and the PDFs in `out/` or in an `--out-dir` inside the project. Inside means the path lies under the project
  folder by name (`..` taken by name), or does once the nearest existing folder above its own folder is resolved,
  so another link to the project folder counts too. The PDF of `render --out` and PDFs in an `--out-dir` outside
  the project go through `write_cache_file`.
- `init` (only when the project folder exists already) and `export` check `out`, and `export` an inside
  `--out-dir`, with `check_folder_inside` before anything is written. The message names the folder: `<path>: a link
  leads out of the project folder, so nothing is written there; replace the link with a real folder, or give the folder it leads to with --out-dir`.
- Plain std, as before: `init` claims the project file's name with `create_new` (an existing file or link is
  "exists already"), and the CLI creates `out` with `create_dir_all`. The library writes create no folder; a
  missing one is `NotFound`. `export` removes partial files older than an hour from the project's own output folder.
- A project file that is a link is replaced by a regular file, never written through (ruling R1). The new file
  gets the old permissions, read before the write. They are set after the rename through an open handle, only
  when they differ, and only when the handle is the regular file at the path (inode on Unix, `same-file` on
  Windows). That step is best effort: a failure leaves the default permissions and reports nothing (ruling R5; the content
  is written, the mode is a convenience, and a 0600 file may become 0644 on exotic filesystems).
- The free-space check with the 50 MB reserve and `DiskFull` stays ours (ADR 0030).
- Core follow-up, to agree with the owner: `write_inside` takes permissions and sets them on the partial file
  before the rename, which removes the window and the handle check here.

## Consequences

- A shared project folder can no longer make freemapper-print write outside it.
- A linked project file becomes a regular file after `numbers accept`, also when the reload fails and the old
  content is written back; the link's target keeps its content. An `out` linked to another drive stops `init` and
  `export`; `--out-dir` with a folder outside the project still works. Every example PDF is byte-identical.
- Cost if wrong: users re-link their project file, and use `--out-dir` for an output folder on another drive.

- The handle for the permission check is opened with `O_NONBLOCK`, so a FIFO swapped in between the look and
  the open cannot block it. `libc` is no direct dependency, so the flag is the numeric constant for macOS, the
  BSDs and Linux (not MIPS or SPARC); on other Unix targets the open stays blocking. The identity check follows
  the open as before.
- Dependencies: ADR 0042 records the tag pin and the owners.

Source: core ADR 0009 (freemapper ADR 0060); core-switch plan Task 3; ruling R1; Task 3 review.
