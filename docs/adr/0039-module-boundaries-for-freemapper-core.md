# 0039. Module boundaries for freemapper-core

Date: 2026-11-09
Status: Accepted, amended by 0043

## Context

After M3 the modules `omap`, `render` and `course` move into freemapper-core, the shared library of the suite
(ruling R33). The final review of M3 found two couplings that would hold them back. `render` re-exported the
`Overprint` type of the `overprint` module and painted it, and `overprint` depends on `course` and the rule
profiles. `course::load` read the project file and called `numbers::changed_bases`, which pulls in `overprint`,
`placement`, `render` and the rules. Both are cheaper to cut while the code is fresh.

## Decision

- `render` holds the overprint as plain shapes: `render::page::Overprint` has a lower and an upper list of
  shapes, the priority the lower one goes under, a colour and a flag for shapes a limit left out. The
  `overprint` module builds it and places it on a page with `overprint::on_map`. `render` knows nothing of
  courses (ruling R37).
- The project-aware loading of a course file (`load_courses`, `LoadedCourses`, `Stale`, `layout_for`,
  `base_changed`, `CourseLoadError`) moves from `course::load` to the module `courses`, which sits above
  `project` and `numbers`. `course` keeps the model, the readers, the fit and the transverse Mercator, and uses
  only `omap`.
- Only the streaming XML loop and attribute parsing of `course::xml` are shared outside `course` (with the
  description symbol reader); the note list, the text budget and the length check stay `pub(super)`.
- Unit tests in `lib.rs` read the source files and fail on a forbidden path: `course/` must not name
  `crate::project`, `courses`, `sheet`, `numbers`, `export`, `render`, `overprint`, `placement`, `describe` or
  `print`; `render/` must not name `crate::overprint`, `sheet`, `project`, `course`, `courses`, `numbers`,
  `export`, `describe` or `print`. The test of `placement` (ADR 0038) stays as it is. The check also sees grouped imports (`crate::{project::P}`,
  nested groups too) and `super::super::<module>` paths that name a forbidden module.

## Consequences

- No output changed: every example PDF is byte-identical.
- `course::model::NumberPlacement` still derives `Serialize` and `Deserialize` for the project file. That is
  fine in core as long as serde is a core dependency; the extraction plan has to say so.
- The describe module depends on `course::xml`; when `course` moves, it takes the loop with it.

Amended by ADR 0043: `omap`, `course` and `placement` are the crates `freemapper-omap` and `freemapper-course`
now, and the compiler keeps them apart from the app. The tests for `course/` and `omap/` are gone, and so is
the one for the shared XML loop, since `freemapper-course` keeps the rest of `xml` private. The test for
`render/` stays and also forbids `placement` and `freemapper_course`.

Source: final review of M3 (I2), ruling R37.
