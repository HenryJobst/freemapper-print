// SPDX-License-Identifier: AGPL-3.0-or-later

//! `frames`: the layouts for the paper frames of the map (ADR 0079). It reads `templates/paper-frame.omap` next to
//! the map, adds a layout for each frame no layout prints yet and never changes or replaces a layout.

use std::path::PathBuf;

use freemapper_print::frames::{FramePlan, PaperFrames, plan_layouts, read_paper_frames};
use freemapper_print::project::{self, LayoutSpec, MapFrameSpec};
use serde::Serialize;

use super::edit::Change;
use super::{OpError, load_project, read_map};

/// What `frames` takes.
#[derive(Debug, Clone)]
pub struct FramesArgs {
    /// The project file or its folder.
    pub project: PathBuf,
    /// Only report what would be added.
    pub dry_run: bool,
}

/// What `frames` found and did.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FramesResult {
    /// The project file.
    pub path: PathBuf,
    /// The scale of the map; the layouts print at it.
    pub map_scale: u32,
    /// The usable frames in the template map.
    pub found: usize,
    /// Frames left out because their tags or their line are unusable.
    pub skipped: usize,
    /// The layouts added, or with `dry_run` the layouts that would be.
    pub added: Vec<LayoutSpec>,
    /// The layouts that print a frame already.
    pub present: Vec<String>,
    /// Whether the project file was written.
    pub written: bool,
}

/// Adds a layout for every paper frame that no layout of the project prints yet.
pub fn frames(args: &FramesArgs) -> Result<FramesResult, OpError> {
    let loaded = load_project(&args.project)?;
    let map_path = loaded.map_path();
    let map = read_map(&map_path)?;
    let map_scale = map.georef.scale_denominator;
    let PaperFrames { frames, skipped } =
        read_paper_frames(&map_path).map_err(|err| OpError::ReadFrames {
            path: map_path.clone(),
            source: anyhow::Error::new(err),
        })?;
    // The passe-partout of the first layout leaves room for the footer; the frame layouts take it.
    let margin = loaded
        .project
        .layouts
        .first()
        .map_or(MapFrameSpec::default().margin_mm, |l| l.map_frame.margin_mm);
    let FramePlan { add, present } =
        plan_layouts(&frames, &loaded.project.layouts, map_scale, margin);
    let mut added = Vec::new();
    let mut written = false;
    for layout in add {
        if args.dry_run {
            added.push(layout);
            continue;
        }
        let (lp, change) = project::set_layout(&loaded.path, &layout)
            .map_err(|source| super::edit::edit_error(&loaded.path, source))?;
        written |= change != Change::Unchanged;
        added.push(
            lp.project
                .layouts
                .into_iter()
                .find(|l| l.id == layout.id)
                .unwrap_or(layout),
        );
    }
    Ok(FramesResult {
        path: loaded.path,
        map_scale,
        found: frames.len(),
        skipped,
        added,
        present,
        written,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use freemapper_print::print::Paper;
    use std::path::Path;

    use crate::ops::testing::paper_frames_project as project;

    #[test]
    fn init_adds_a_layout_per_frame() {
        let dir = tempfile::tempdir().unwrap();
        let (folder, _) = project(dir.path());
        let loaded = load_project(&folder).unwrap();
        let ids: Vec<_> = loaded
            .project
            .layouts
            .iter()
            .map(|l| l.id.as_str())
            .collect();
        assert_eq!(ids, ["a4", "frame-a4-portrait", "frame-a5-landscape"]);
    }

    #[test]
    fn frames_adds_nothing_when_every_frame_is_printed() {
        let dir = tempfile::tempdir().unwrap();
        let (folder, _) = project(dir.path());
        let result = frames(&FramesArgs {
            project: folder,
            dry_run: false,
        })
        .unwrap();
        assert_eq!((result.found, result.skipped), (2, 0));
        assert!(result.added.is_empty() && !result.written);
        assert_eq!(result.present, ["frame-a4-portrait", "frame-a5-landscape"]);
    }

    #[test]
    fn frames_restores_a_removed_layout_and_a_dry_run_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (folder, _) = project(dir.path());
        let file = project_file_of(&folder);
        crate::ops::testing::remove_layout(&file, "frame-a5-landscape");

        let dry = frames(&FramesArgs {
            project: folder.clone(),
            dry_run: true,
        })
        .unwrap();
        assert_eq!(dry.added.len(), 1);
        assert!(!dry.written);
        assert_eq!(load_project(&folder).unwrap().project.layouts.len(), 2);

        let done = frames(&FramesArgs {
            project: folder.clone(),
            dry_run: false,
        })
        .unwrap();
        assert!(done.written);
        assert_eq!(done.added[0].id, "frame-a5-landscape");
        assert_eq!(
            (done.added[0].paper, done.added[0].page_mm),
            (Paper::A5, None)
        );
        assert_eq!(load_project(&folder).unwrap().project.layouts.len(), 3);
    }

    fn project_file_of(folder: &Path) -> PathBuf {
        crate::ops::project_file(folder)
    }
}
