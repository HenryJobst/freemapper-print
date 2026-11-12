# freemapper-print for AI agents

This file tells an AI agent how to install freemapper-print and how to use it for a person who wants a finished
orienteering map on paper. It ships in every release archive next to the program. People read
[README.en.md](README.en.md) (English) or [README.md](README.md) (German); the [user manual](docs/manual/en.md)
has the details.

freemapper-print turns a finished OpenOrienteering Mapper map (`.omap`) and, if there are any, the courses from
Purple Pen (`.ppen`) or from any program that writes IOF XML 3.0 into exact-scale vector PDFs: 1 mm on paper is the
exact map distance at the print scale. One project file per event, `freemapper-print.toml`, holds the layouts
(paper, frame, passe-partout), the sheet elements (title block, texts, logos, scale check bar, credit line), the
mask variants for window and corridor orienteering and the assignment of each course to a layout. It also puts
the course overprint on the map, one sheet per course, with the control description from Purple Pen files.

Tell the person before you start:

- It prints finished maps. It does not draw a map, and it does not design courses; for the first use freemapper
  and Purple Pen. It reads `.omap` maps, not OCAD files.
- Map kinds are `sprint`, `schoolyard` and `forest`. A print scale larger than the map scale enlarges the symbols
  as the standard prescribes. A scale that is not standard for the kind (a reduction) prints with a warning and
  is for training only.
- A course with alternative controls at one stop or with forks and loops (Purple Pen variations) is left out with
  a note. Purple Pen's IOF XML export writes each variation as a course of its own.
- There is no automatic corridor for corridor orienteering, because no rule says where it runs. The person or
  you supply the line.
- The PDF is only exact if it is printed at actual size (100 %, no "fit to page").

## Install

freemapper-print is one executable without an installer and without dependencies. Do not build it from source
unless the user asks; the release archives are ready to run.

1. Find the newest version: `https://api.github.com/repos/HenryJobst/freemapper-print/releases/latest` returns
   JSON with `tag_name`, for example `v0.8.0`. The release page is
   <https://github.com/HenryJobst/freemapper-print/releases/latest>.
2. Pick the archive for the system. The name is `freemapper-print-<tag>-<platform>` plus the extension:

   | System (`uname -s` / `uname -m`) | Platform | Archive |
   |---|---|---|
   | macOS, `arm64` (Apple chip) | `macos-arm64` | `.tar.gz` |
   | macOS, `x86_64` (Intel) | `macos-x86_64` | `.tar.gz` |
   | Linux, `x86_64` | `linux-x86_64` | `.tar.gz` |
   | Linux, `aarch64` | `linux-arm64` | `.tar.gz` |
   | Windows, x64 | `windows-x86_64` | `.zip` |

   Each archive has a `<archive>.sha256` next to it. The Linux builds are static (musl) and run on any
   distribution.
3. Download, check and unpack. macOS and Linux, for example in `~/Applications` or `~/opt`:

   ```sh
   tag=v0.8.0; platform=macos-arm64
   base="https://github.com/HenryJobst/freemapper-print/releases/download/$tag"
   curl -fLO "$base/freemapper-print-$tag-$platform.tar.gz"
   curl -fLO "$base/freemapper-print-$tag-$platform.tar.gz.sha256"
   shasum -a 256 -c "freemapper-print-$tag-$platform.tar.gz.sha256"   # Linux: sha256sum -c
   tar -xzf "freemapper-print-$tag-$platform.tar.gz"
   ```

   Windows (PowerShell):

   ```powershell
   $tag = "v0.8.0"; $name = "freemapper-print-$tag-windows-x86_64.zip"
   $base = "https://github.com/HenryJobst/freemapper-print/releases/download/$tag"
   Invoke-WebRequest "$base/$name" -OutFile $name
   Invoke-WebRequest "$base/$name.sha256" -OutFile "$name.sha256"
   (Get-FileHash $name -Algorithm SHA256).Hash.ToLower()   # compare with the first word of the .sha256 file
   Unblock-File $name
   Expand-Archive $name -DestinationPath .
   ```

4. The archive unpacks to a folder `freemapper-print-<tag>-<platform>/` with `freemapper-print` (Windows:
   `freemapper-print.exe`), the READMEs, `LICENSE`, `FONT-LICENSE.txt`, `THIRD_PARTY.md`, this file and
   `docs/manual/`. The program is not signed:
   - macOS refuses to start it until the quarantine flag is gone:
     `xattr -d com.apple.quarantine freemapper-print-<tag>-<platform>/freemapper-print`. Tell the user you did
     this and why.
   - Linux: `chmod +x freemapper-print` if the executable bit got lost.
   - Windows may show "Windows protected your PC" on the first start. `Unblock-File $name` before
     `Expand-Archive` avoids it; otherwise the user confirms with *More info* > *Run anyway*.
5. Check: `<folder>/freemapper-print --version` prints `freemapper-print <version>`.

Remember the absolute path of the executable; the MCP registration needs it. Project files stay valid.

From 0.8.0 on (0.7.0 and earlier have no `update` command), you update an installed release build with
`freemapper-print update` instead of unpacking a newer archive: it checks the signature, replaces the program at the
same path (so the registration keeps working) and keeps the old version as `freemapper-print.previous` (on Windows
`freemapper-print.previous.exe`). Until then, unpack the new archive. Ask the person first, then add `--yes` (without a terminal it refuses to install
otherwise). `update --check` only looks and exits 10 when a newer version exists. `update --rollback` goes back one
version. A build from source, or a release built without the signing key, cannot update itself; `update` says so
and names the release page. If the program folder is not writable, `update` names it and changes nothing; then
unpack the new archive there by hand, and point the registration at a new path only if it changed.

Do not add the user's e-mail address, name or other personal data to any request, file name, project title or
setting. The program contacts GitHub only for the update check and for `update` itself, which downloads the new
program. The check runs at most once a day. Its request names the program, its version and the repository, and
sends no data about the person or the project. `freemapper-print update --mode off` or
`FREEMAPPER_PRINT_UPDATE=off` stops the check. Apart from that the program makes no network requests.

## Use it from the command line (preferred)

If you can run commands (Claude Code, Codex), use the command line. One command does one blocking job and
prints the result; with MCP every tool call is another turn that re-reads the whole conversation, which costs
tokens. Pass `--lang en` when you parse the output.

Do not repeat the switches from memory: `freemapper-print --help` lists the commands and
`freemapper-print <command> --help` the switches, in the language of `--lang`.

```sh
freemapper-print init <folder> --map /path/to/map.omap --courses /path/to/courses.ppen --title "Training"
freemapper-print check <folder>
freemapper-print export <folder> --out-dir /path/to/pdfs
```

| Command | Purpose |
|---|---|
| `render <map> --out <file.pdf>` | The map alone as one PDF. `--scale`, `--paper a4\|a3`, `--orientation auto\|portrait\|landscape`, `--kind`, `--show-sheet-objects`. |
| `init <folder> --map <file>` | Creates the project file. `--title`, `--kind sprint\|schoolyard\|forest`, `--courses <.ppen or .xml>`. |
| `check <project>` | Draws every layout or course without writing a file and lists the notes. |
| `export <project>` | One PDF per layout, or per course with a course file. `--layout <id>`, `--course <name>` (repeatable), `--single-file`, `--out-dir <dir>` (default: `out` next to the project file, created if missing). |
| `masks window <project> --course <name>` | `--controls`, `--radius`, `--radius-for CODE=MM`, `--no-start`, `--no-finish`, `--id`, `--assign`. |
| `masks corridor <project> --points "x,y;x,y"` | `--width`, `--id`, `--course`, `--assign`. |
| `numbers accept <project> --course <name> --control <code> --layout-wide\|--sheet` | With `--direction` and `--gap`, or the proposal of `check`. |
| `edit <project>` | The browser editor, for people. `--port <n>`, `--no-browser`. |
| `update` | From 0.8.0 on: installs the newest signed release over this program. `--check` (exit 10 when a newer version exists), `--yes`, `--version <X.Y.Z>`, `--rollback`, `--mode notify\|install\|off`. |
| `mcp` | The MCP server. |

`<project>` is `freemapper-print.toml` or the folder that holds it. Results and notes go to stdout; errors go to
stderr. Exit codes: 0 on success; 1 when a file cannot be loaded, drawn or written, when `check` finds notes
(notes marked Info do not count) or when `init` finds an existing project file; 2 when the command line is wrong,
for example an unknown layout, course or control; 10 when `update --check` finds a newer version. `export` exits 0 when every PDF is written, even with notes.

After `init`, `check`, `export` and `render` the program may print one line on stderr that a newer version exists. Pass
it on to the person in your own words and ask before you run `update --yes`; do not update on your own. In mode `install` the person has asked for updates
in advance: after a command that exits 0 the program installs by itself, and the progress and result lines go to
stdout.

`edit` starts a local server and a page in a browser for a person who wants to see the sheet and move things with
the mouse. It blocks until the person clicks Done or presses Ctrl+C and needs someone at the browser, so an agent
does not use it; an agent does the same work with the commands above or the MCP tools. The editor writes the same
project file through the same functions.

### What the command line does not cover

The MCP tools `project_info`, `list_courses`, `set_layout`, `set_element` and `assign_mask` have no command. Over
the command line you do their work in the project file `freemapper-print.toml`, which is plain text with
comments: read it for the layouts, elements, masks and assignments; edit it for layout, element and assignment
changes. [The manual](docs/manual/en.md) (chapters 5 to 8) lists every key. Edit with care, keep the comments,
and run `check` afterwards: it loads the file again and names the line of a mistake. Courses and their
controls are listed by `check` and `export`, or read them from the course file.

### Workflow

1. Ask for the map file (`.omap`) and, if there is one, the course file. Both must be on the computer; you need
   their absolute paths.
2. New event: `init` with `--map` and `--courses`. Existing project: read `freemapper-print.toml`.
3. `check`. Read every note to the person in plain words; do not paste the list. Warnings about a number that
   sits badly, a control under a mask or a map that does not fit the courses are worth a decision. Do not
   export yet if `check` exits 1, unless the person says so.
4. Adjust: `masks window`, `masks corridor` (with `--assign`), `numbers accept`, or edit the project file for
   layouts and elements. Run `check` again.
5. `export` into a folder the person names (it must exist). Tell them where the PDFs are.

An export of many courses can take a while. A command that runs longer than your default command timeout needs
a longer timeout, or run it in the background and wait for it to end.

## Use it through MCP

For assistants that cannot run commands (Claude Desktop, for example), `freemapper-print mcp` is an MCP server on
stdin/stdout. Register it once with the absolute path:

```sh
claude mcp add freemapper-print -- /absolute/path/to/freemapper-print mcp          # Claude Code
```

```toml
# Codex: ~/.codex/config.toml
[mcp_servers.freemapper-print]
command = "/absolute/path/to/freemapper-print"
args = ["mcp"]
```

```json
{ "mcpServers": { "freemapper-print": { "command": "/absolute/path/to/freemapper-print", "args": ["mcp"] } } }
```

`--lang de|en` sets the language of the texts the tools return (default: `FREEMAPPER_LANG`, then `LC_ALL`,
`LC_MESSAGES`, `LANG`). Every tool also takes an optional `lang` argument.

### Tools

Every tool takes `project`, the absolute path of `freemapper-print.toml` or of the folder that holds it (except
`init_project`, which takes `folder`, and `check_update` and `install_update`, which take neither). All tools also take the optional `lang`. Lists are capped at 200 entries;
the `more_*` fields count the rest.

| Tool | Parameters (besides `project`, `lang`) | What it does |
|---|---|---|
| `project_info` | none | Title, map kind and scale, the map and course file, layouts with their elements, courses with layout and masks, mask variants, assignments. |
| `check_project` | none | Draws every sheet in memory and returns the notes (`kind`, `severity` `info`/`warning`/`error`, `course`, `message`) and `needs_attention`. |
| `list_courses` | none | The courses in file order with number of controls, layout and masks. |
| `init_project` | `folder`, `map`; optional `courses`, `kind` (`sprint`, `schoolyard`, `forest`), `title` | Writes a new `freemapper-print.toml`. Refuses a folder that has one. With a course file: one layout per print scale and an assignment per course. |
| `masks_window` | `course`; optional `controls`, `radius`, `radius_for`, `no_start`, `no_finish`, `id`, `assign` | Round windows around the controls of a course. `controls`: codes `"31,32"` or places `"2-5"`. `radius` in mm on paper (1 to 100, default 15), `radius_for`: `["35=20"]`. |
| `masks_corridor` | `points` (`"x,y;x,y;…"`, map mm, y up, at least 2); optional `width` (mm on paper, 1 to 100, default 10), `course`, `id`, `assign` | A corridor along a line. `course` gives the print scale for the width; `assign` needs it. |
| `assign_mask` | `course`; optional `mask`, `layout` | Sets the mask a course prints with (a `[[masks]]` id or `whiteout-<n>`); without `mask` the course prints without one. |
| `numbers_accept` | `course`, `control`, `scope` (`sheet` or `layout_wide`); optional `direction` (degrees, 0 = east, counter-clockwise) with `gap` (mm on paper, above 0 up to 10) | Fixes where a control number is printed. Without `direction` and `gap` it takes the proposal of `check_project`. |
| `set_layout` | `id`; optional `paper` (`A4`, `A3`), `page_mm` (`[width, height]` in portrait, 10 to 5080 mm each, instead of `paper`; `null` goes back), `orientation` (`auto`, `portrait`, `landscape`), `print_scale`, `margin_mm`, `border_mm`, `centre_map_mm`, `north_lines` (`auto`, `never`), `crop`, `hide_parts`, `hide_shared` | Adds a layout or changes the one with this id; keys left out keep their value. |
| `set_element` | `id`, `kind`, `anchor`; optional `layout`, `lines`, `size_pt`, `file`, `width_mm`, `cell_mm`, `box_white` | Adds or replaces a sheet element. `kind`: `title_block`, `text`, `logo`, `scale_check_bar`, `credit_line`, `copyright_line`, `control_descriptions`. Without `layout` it is shared by all layouts. `anchor`: `to` (`page` or `map_frame`), `corner`, `offset_mm`. |
| `check_update` | none | Whether a newer version exists, with the version numbers, the release date and the release notes. |
| `install_update` | optional `confirm` | Installs the newest release over this program. Without `confirm: true` it returns an error and installs nothing. The running server keeps the old version until the assistant restarts it; tell the person to restart it. |
| `export_project` | `out_dir`; optional `courses`, `layouts`, `single_file` | Writes the PDFs into `out_dir`, which must exist. Replaces PDFs of the same name. Returns files with size, notes and the reminder to print at actual size. |

### Workflow

The same steps as over the command line, with the tools:

1. Ask for the map file (`.omap`) and, if there is one, the course file (absolute paths).
2. New event: `init_project` with `map` and `courses`. Existing project: `project_info`.
3. `check_project`. Read every note to the person in plain words; do not paste the list. Warnings about a number
   that sits badly, a control under a mask or a map that does not fit the courses are worth a decision. When
   `needs_attention` is true, do not export yet unless the person says so.
4. Adjust: `set_layout`, `set_element`, `masks_window` or `masks_corridor` (with `assign`), `numbers_accept`.
   Run `check_project` again after changes.
5. `export_project` into a folder the person names (it must exist). Tell them where the PDFs are.

Rules:

- The first tool result of a session may carry a note that a newer version exists. Pass it on to the person and ask
  before you call `install_update` with `confirm: true`; never set `confirm` without their yes. A release the update
  cannot install, or one whose signature does not verify, comes with a note that says so; do not install it some
  other way.
- Paths must be absolute. A relative path is refused with a message that says so.
- Writes are serialised in one server. A project file that is a link is not changed; give the path of the file
  the link points to. Comments the file already had stay, except in a few cases the manual lists.
- The program never replaces a mask or a number correction by itself: it adds. To change one, edit the project
  file or use a new id.
- A mask's radius or width is stored in map mm for the layout's print scale. After changing the print scale,
  delete the mask and create it again.
- Errors come back as tool errors with one sentence meant for the user. Relay it; do not retry the same call
  unchanged. The agent can cancel a long export; it stops after the sheet in hand.

## Result files

- `freemapper-print.toml`: the project file. Layouts, elements, masks, assignments, corrections. It is plain text
  with comments; the person may edit it.
- PDFs: one per layout (named after the layout id) or per course (named after the course), in `out/` next to
  the project file or in the folder you named. With `single_file`, one PDF named after the event title.
- The sheets carry a credit line that names freemapper-print and its GitHub link.

## What to tell the user

- Print the PDF at actual size (100 %). The scale check bar on the sheet lets them measure: 200 m is 50 mm at
  1:4000. A print that is scaled to fit is wrong.
- Look at the PDF before printing, and read the notes of `check_project`: a number that crowds a leg, a control
  under a mask, a map that cuts off at the frame.
- Colour printing is best; the base area of sprint maps and the purple overprint rely on it.
- For window and corridor orienteering, say which controls got a window and how large it is on paper.

## Troubleshooting

- "is a relative path; give an absolute path, because the server does not know the working folder of the agent":
  pass absolute paths.
- "exists already; init does not overwrite it": the folder has a project file. Use `project_info` instead, or
  another folder.
- "cannot tell the map kind from the file; pass --kind sprint, --kind schoolyard or --kind forest": pass `kind`.
- "the folder … for the PDFs does not exist; create it first": create `out_dir`.
- "not enough disk space for …": free space or choose another folder. A full disk leaves no half-written PDF.
- "a link leads out of the project folder, so nothing is written there": replace the link with a real folder.
- "the project file has a mask … already" or "prints with the mask … already": the program only adds masks. Use
  another `id` or edit the project file.
- "the project file has a correction for control … already": `numbers_accept` only adds. Edit the file to change.
- macOS "cannot be opened" or "killed": remove the quarantine flag (install step 4).
- "This freemapper-print was built from source, or by a release without signed updates": `update` cannot install
  anything here. Tell the person to get the newest release from the releases page or to `git pull` and
  `cargo build --release --locked`.
- "cannot write to the program folder …": the program sits where the person has no write access. Unpack the new
  archive there by hand with the right permissions, or move the program to a folder they can write to.
- "the release could not be verified … nothing was installed": the signature did not verify. Tell the person and
  do not install that release another way.
- "the input is not a terminal, so freemapper-print cannot ask before installing": ask the person, then run
  `update --yes`.

## Suite

freemapper-print reads the `.omap` and the `freemapper.toml` that
[freemapper](https://github.com/HenryJobst/freemapper) writes (its [AGENTS.md](https://github.com/HenryJobst/freemapper/blob/main/AGENTS.md)
covers making the map). Of `freemapper.toml` it reads only `map_kind`, `scale_denominator`, `crs`, `provider`,
`title` and `[map_area]`. Courses come from Purple Pen or any IOF XML 3.0 program.

## Working on the freemapper-print code

This section is for coding agents in a clone of the repository, not for using the program.

- Build and check: `cargo build --release --locked`, `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --locked`. The toolchain is
  pinned in `rust-toolchain.toml`. Read [`CLAUDE.md`](CLAUDE.md) first.
- Source code and technical docs are English; terms follow [`docs/glossary.md`](docs/glossary.md). User docs are in
  German and English. German text says "Bundesland" for a state.
- Decisions are recorded in [`docs/adr/`](docs/adr/README.md); every change gets a changelog entry in
  `CHANGELOG.md` and `CHANGELOG.en.md`.
- Shared code comes from [freemapper-core](https://github.com/HenryJobst/freemapper-core), pinned by tag. Change a
  core crate there, not here.
