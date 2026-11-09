# 0041. Writes go through freemapper-fs

Date: 2026-11-09
Status: Accepted

## Context

Up to 0.3.0, freemapper-print wrote its files with its own code. A PDF went to `<name>.pdf.part` and was renamed
to its name (ADR 0030). A file with that fixed name, or a link of that name, was opened and written. `numbers
accept` resolved a project file that was a symbolic link and replaced the file the link pointed to (ADR 0025, M3
Task 11). `init` created the project file in place.

freemapper had the same weaknesses and fixed them in its ADR 0060, which moved to freemapper-core as core ADR 0009
"Writes never follow links". Its crate `freemapper-fs` writes a new partial file next to the target, flushes it and
renames it over the target, so a link at the target name is replaced and never written through. Its `*_inside`
functions refuse a file whose folder a link leads out of the project folder. Project folders are shared and
downloaded, so freemapper-print needs the same rules.

## Decision

- freemapper-print depends on `freemapper-fs` from freemapper-core, tag `v0.2.0`. Every file it writes goes
  through that crate.
- Project files are the files inside the project folder by name: the project file `freemapper-print.toml` and
  the PDFs in `out/` below it, or in an `--out-dir` inside the project folder. They go through
  `write_inside(project folder, file, bytes)`. "Inside by name" means the path, made absolute and with `.` and
  `..` taken by name, starts with the project folder. Links are not resolved for that decision;
  `write_inside` checks them and refuses a folder that leads out with core's text `OUTSIDE_PROJECT`.
- Files outside the project folder are not project files: the PDF of `render --out`, and the PDFs in an
  `--out-dir` outside the project folder. They go through `write_cache_file`. It replaces a link at the target
  name like the other writes, but the folders on the way may be links, since the user named that folder.
- `init` and `export` call `check_folder_inside` for the `out` folder once, before anything is written. `export`
  also checks an `--out-dir` that lies inside the project folder by name. A refused folder stops the command
  with exit 1 and a message in German or English that names the folder: `<path>: a link leads out of the project
  folder, so nothing is written there; replace the link with a real folder`. A refused write later on gets the
  same message with the file's path. `check` writes nothing and checks nothing here.
- A project file that is a symbolic link is now replaced by a regular file and never written through (ruling R1).
  This reverses the M3 behaviour of `numbers accept`, which resolved the link and replaced its target. Writing
  through a link would let a shared folder make freemapper-print replace any file the link names, which core ADR
  0009 rules out for every app of the suite. The new file gets the permissions of the old one, or of the file the
  link pointed to: freemapper-print reads them before the write and sets them on the new file afterwards,
  because `freemapper-fs` creates its partial file with the default permissions.
- `init` first claims the project file's name with `create_new`, as before, so an existing file or link of that
  name is refused with "exists already". The text then replaces the empty file through `write_inside`. A failed
  write removes the claimed file again.
- The free-space check stays in freemapper-print (ADR 0030): before the write, the volume must hold the
  PDF plus 50 MB, or the command stops with `DiskFull`. Only writing the partial file and the rename moved to
  core. A disk that fills up while `freemapper-fs` writes still becomes `DiskFull`.
- Partial files are named `<file>.<process id>.<counter>.part` and created with `create_new`. A taken name, a
  link included, is skipped and left alone. A failed write removes its partial file, so freemapper-print no longer
  needs its own guard. `export` removes partial files older than one hour that killed runs left in the project's
  output folder (`remove_stale_parts_inside`). It leaves an output folder outside the project alone, because
  other programs' `.part` files may live there.
- Errors of `freemapper-fs` are `(path, io::Error)`; freemapper-print turns them into `OutputError::Io { path,
  source }` for PDFs and `ProjectError::Write` for the project file.

## Consequences

- A shared or downloaded project folder can no longer make freemapper-print write outside it, through a link at a
  PDF's name, at the old partial-file name or at the `out` folder.
- Whoever linked their project file to a file elsewhere sees the link replaced by a regular file after `numbers
  accept`. The file it pointed to keeps the old content.
- A project whose `out` folder is a link to another drive stops `init` and `export`. `--out-dir` with a folder
  outside the project still writes there.
- The library's export functions now create a missing output folder, as `freemapper-fs` creates folders on the
  way. The CLI still refuses a missing `--out-dir` before it writes.
- After a replace, the project file has the default permissions for a moment until the old ones are set again.
- Every example PDF is byte-identical.
- Cost if wrong: users who keep their project file behind a link have to link it again, and `out` folders on
  another drive need `--out-dir`.

Source: core ADR 0009 (freemapper ADR 0060); core-switch plan Task 3; ruling R1.
