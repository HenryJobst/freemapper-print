// SPDX-License-Identifier: AGPL-3.0-or-later

//! Paper frames (ADR 0079): the sheets freemapper keeps in `templates/paper-frame.omap` next to a map, and the
//! layouts that print them. A frame is a closed line, the whole sheet at the scale of the map, centred on a map
//! point (suite contract, freemapper-core ADR 0046). Each frame becomes one layout of the same paper and
//! orientation, centred on the frame and printed at the map scale, so the sheet of the frame is the sheet printed.

use std::path::Path;

use freemapper_contract::tags::{
    ORIENTATION_KEY, ORIENTATION_LANDSCAPE, ORIENTATION_PORTRAIT, PAPER_FRAME_FILE, PAPER_KEY,
    SOURCE_PAPER_FRAME,
};

use crate::omap::read::read_map_file;
use crate::omap::{MapObject, OmapError};
use crate::print::{Orientation, Paper};
use crate::project::{LayoutSpec, MapFrameSpec, NorthLines};

/// Tag of the information source on a map object (`freemapper:source`).
const SOURCE_KEY: &str = "freemapper:source";
/// Map coordinates per millimetre.
const UM_PER_MM: f64 = 1000.0;
/// Two frames or layouts whose centres differ by less than this (mm) are the same place.
const SAME_PLACE_MM: f64 = 0.01;

/// The papers a frame names, with their size in portrait.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramePaper {
    A6,
    A5,
    A4,
    A3,
    A2,
}

impl FramePaper {
    /// The word of the contract (`freemapper:paper`).
    pub fn name(self) -> &'static str {
        match self {
            FramePaper::A6 => "A6",
            FramePaper::A5 => "A5",
            FramePaper::A4 => "A4",
            FramePaper::A3 => "A3",
            FramePaper::A2 => "A2",
        }
    }

    fn parse(word: &str) -> Option<Self> {
        [Self::A6, Self::A5, Self::A4, Self::A3, Self::A2]
            .into_iter()
            .find(|p| p.name() == word)
    }

    /// The paper of a layout that prints this frame.
    pub fn paper(self) -> Paper {
        match self {
            FramePaper::A6 => Paper::A6,
            FramePaper::A5 => Paper::A5,
            FramePaper::A4 => Paper::A4,
            FramePaper::A3 => Paper::A3,
            FramePaper::A2 => Paper::A2,
        }
    }

    /// Width and height in portrait (mm).
    pub fn size_mm(self) -> [f64; 2] {
        let (w, h) = self.paper().size_mm();
        [w, h]
    }
}

/// A paper frame of the template map.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaperFrame {
    pub paper: FramePaper,
    /// `Portrait` or `Landscape`, never `Auto`.
    pub orientation: Orientation,
    /// The middle of the frame as `centre_map_mm` takes it: map mm, y pointing up.
    pub centre_map_mm: [f64; 2],
}

/// What reading the template map found.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PaperFrames {
    /// The usable frames in file order.
    pub frames: Vec<PaperFrame>,
    /// Frames left out: one without both tags, with a value the contract does not know, or without a closed line.
    pub skipped: usize,
}

/// The template map cannot be read.
#[derive(Debug, thiserror::Error)]
pub enum FramesError {
    #[error("cannot read the paper frames in {path}")]
    Read {
        path: std::path::PathBuf,
        #[source]
        source: OmapError,
    },
}

/// Reads the paper frames next to the map at `map`: `templates/paper-frame.omap` in the map's folder. A map
/// without that file has no frames and gives an empty result.
pub fn read_paper_frames(map: &Path) -> Result<PaperFrames, FramesError> {
    let Some(dir) = map.parent() else {
        return Ok(PaperFrames::default());
    };
    let file = dir.join(PAPER_FRAME_FILE);
    if !file.is_file() {
        return Ok(PaperFrames::default());
    }
    let doc = read_map_file(&file).map_err(|source| FramesError::Read { path: file, source })?;
    let mut found = PaperFrames::default();
    for object in doc.parts.iter().flat_map(|p| &p.objects) {
        if tag(object, SOURCE_KEY) != Some(SOURCE_PAPER_FRAME) {
            continue;
        }
        match frame_of(object) {
            Some(frame) => found.frames.push(frame),
            None => found.skipped += 1,
        }
    }
    Ok(found)
}

fn tag<'a>(object: &'a MapObject, key: &str) -> Option<&'a str> {
    object
        .tags
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn frame_of(object: &MapObject) -> Option<PaperFrame> {
    let paper = FramePaper::parse(tag(object, PAPER_KEY)?)?;
    let orientation = match tag(object, ORIENTATION_KEY)? {
        ORIENTATION_LANDSCAPE => Orientation::Landscape,
        ORIENTATION_PORTRAIT => Orientation::Portrait,
        _ => return None,
    };
    // A closed line has at least three points; the box of its points is the sheet.
    if object.coords.len() < 3 {
        return None;
    }
    let (mut min, mut max) = ([i64::MAX; 2], [i64::MIN; 2]);
    for c in &object.coords {
        for (i, v) in [i64::from(c.x), i64::from(c.y)].into_iter().enumerate() {
            min[i] = min[i].min(v);
            max[i] = max[i].max(v);
        }
    }
    // Map coordinates point down; `centre_map_mm` points up, as OOM's status bar shows them.
    let x = (min[0] + max[0]) as f64 / 2.0 / UM_PER_MM;
    let y = -((min[1] + max[1]) as f64) / 2.0 / UM_PER_MM;
    Some(PaperFrame {
        paper,
        orientation,
        centre_map_mm: [round_um(x), round_um(y)],
    })
}

/// Rounds to 0.0001 mm, so that a half micrometre from a box middle does not show as a long decimal.
fn round_um(mm: f64) -> f64 {
    (mm * 10_000.0).round() / 10_000.0
}

/// The id a frame's layout gets when nothing else is taken: `frame-a4-portrait`.
fn base_id(frame: &PaperFrame) -> String {
    let orientation = if frame.orientation == Orientation::Portrait {
        "portrait"
    } else {
        "landscape"
    };
    format!("frame-{}-{orientation}", frame.paper.name().to_lowercase())
}

/// The layout that prints `frame` at `scale`: its paper in its orientation, the map centred on the frame, the
/// frame cutting the map on purpose. `margin_mm` is the passe-partout; the caller takes the one its first layout has, which leaves room for
/// the footer.
pub fn layout_for(frame: &PaperFrame, id: String, scale: u32, margin_mm: f64) -> LayoutSpec {
    LayoutSpec {
        id,
        paper: frame.paper.paper(),
        page_mm: None,
        orientation: frame.orientation,
        print_scale: Some(scale),
        map_frame: MapFrameSpec {
            centre_map_mm: Some(frame.centre_map_mm),
            crop: true,
            margin_mm,
            ..MapFrameSpec::default()
        },
        north_lines: NorthLines::Auto,
        hide_parts: Vec::new(),
        view: Vec::new(),
        elements: Vec::new(),
        hide_shared: Vec::new(),
    }
}

/// Whether `layout` already prints `frame` at `scale`: same page, orientation and centre.
fn prints(layout: &LayoutSpec, frame: &PaperFrame, scale: u32) -> bool {
    let want = layout_for(frame, String::new(), scale, layout.map_frame.margin_mm);
    layout.portrait_mm() == want.portrait_mm()
        && layout.orientation == want.orientation
        && layout.print_scale.unwrap_or(scale) == scale
        && layout
            .map_frame
            .centre_map_mm
            .zip(want.map_frame.centre_map_mm)
            .is_some_and(|(a, b)| {
                (a[0] - b[0]).abs() < SAME_PLACE_MM && (a[1] - b[1]).abs() < SAME_PLACE_MM
            })
}

/// What [`plan_layouts`] decided.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FramePlan {
    /// The layouts to add, in frame order.
    pub add: Vec<LayoutSpec>,
    /// The ids of the layouts that print a frame already; nothing is added for those frames.
    pub present: Vec<String>,
}

/// The layouts missing for `frames` next to the `existing` layouts of a project whose map has the scale `scale`,
/// with the passe-partout `margin_mm`.
/// A frame an existing layout prints already adds nothing; an id that is taken gets `-2`, `-3`, … The result
/// never replaces a layout.
pub fn plan_layouts(
    frames: &[PaperFrame],
    existing: &[LayoutSpec],
    scale: u32,
    margin_mm: f64,
) -> FramePlan {
    let mut plan = FramePlan::default();
    let mut taken: Vec<String> = existing.iter().map(|l| l.id.clone()).collect();
    for frame in frames {
        let already = existing
            .iter()
            .chain(&plan.add)
            .find(|l| prints(l, frame, scale));
        if let Some(layout) = already {
            if existing.iter().any(|l| l.id == layout.id) && !plan.present.contains(&layout.id) {
                plan.present.push(layout.id.clone());
            }
            continue;
        }
        let base = base_id(frame);
        let mut id = base.clone();
        let mut n = 2;
        while taken.contains(&id) {
            id = format!("{base}-{n}");
            n += 1;
        }
        taken.push(id.clone());
        plan.add.push(layout_for(frame, id, scale, margin_mm));
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    use freemapper_contract::fixture;

    fn read_fixture() -> (tempfile::TempDir, PaperFrames, u32) {
        let fixture = fixture("paper-frames").expect("the fixture exists");
        let dir = tempfile::tempdir().unwrap();
        let map = fixture.write_into(dir.path()).unwrap();
        let frames = read_paper_frames(&map).unwrap();
        (dir, frames, fixture.expected().scale_denominator)
    }

    #[test]
    fn the_frames_of_the_contract_fixture_are_read_with_their_centres() {
        let (_dir, found, _) = read_fixture();
        let expected = fixture("paper-frames").unwrap().expected().paper_frames;
        assert_eq!(found.skipped, 0);
        assert_eq!(found.frames.len(), expected.len());
        for (frame, want) in found.frames.iter().zip(&expected) {
            let [x0, y0, x1, y1] = want.bbox.map(f64::from);
            assert_eq!(frame.paper.name(), want.paper);
            assert_eq!(
                frame.orientation == Orientation::Portrait,
                want.orientation == "portrait"
            );
            assert_eq!(
                frame.centre_map_mm,
                [(x0 + x1) / 2000.0, -(y0 + y1) / 2000.0]
            );
        }
    }

    #[test]
    fn a_map_without_a_template_has_no_frames() {
        let dir = tempfile::tempdir().unwrap();
        let found = read_paper_frames(&dir.path().join("map.omap")).unwrap();
        assert_eq!(found, PaperFrames::default());
    }

    #[test]
    fn every_frame_paper_is_a_paper_of_its_size() {
        let all = [
            (FramePaper::A6, Paper::A6, "A6"),
            (FramePaper::A5, Paper::A5, "A5"),
            (FramePaper::A4, Paper::A4, "A4"),
            (FramePaper::A3, Paper::A3, "A3"),
            (FramePaper::A2, Paper::A2, "A2"),
        ];
        for (frame, paper, name) in all {
            assert_eq!((frame.paper(), frame.name()), (paper, name));
            let (w, h) = paper.size_mm();
            assert_eq!(frame.size_mm(), [w, h]);
        }
    }

    #[test]
    fn a_frame_becomes_a_layout_of_its_paper_at_the_map_scale() {
        let (_dir, found, scale) = read_fixture();
        let plan = plan_layouts(&found.frames, &[], scale, 12.0);
        assert_eq!(plan.add.len(), 2);
        let a4 = &plan.add[0];
        assert_eq!(a4.id, "frame-a4-portrait");
        assert_eq!(a4.paper, Paper::A4);
        assert_eq!(a4.page_mm, None);
        assert_eq!(a4.orientation, Orientation::Portrait);
        assert_eq!(a4.print_scale, Some(scale));
        assert!(a4.map_frame.crop);
        assert_eq!(a4.map_frame.margin_mm, 12.0);
        assert_eq!(
            a4.map_frame.centre_map_mm,
            Some(found.frames[0].centre_map_mm)
        );
        let a5 = &plan.add[1];
        assert_eq!(a5.id, "frame-a5-landscape");
        assert_eq!((a5.paper, a5.page_mm), (Paper::A5, None));
        assert_eq!(a5.orientation, Orientation::Landscape);
        assert_eq!(a5.portrait_mm(), (148.0, 210.0));
    }

    #[test]
    fn a_frame_a_layout_prints_already_adds_nothing() {
        let (_dir, found, scale) = read_fixture();
        let first = plan_layouts(&found.frames, &[], scale, 12.0);
        let again = plan_layouts(&found.frames, &first.add, scale, 12.0);
        assert!(again.add.is_empty());
        assert_eq!(again.present, ["frame-a4-portrait", "frame-a5-landscape"]);
    }

    #[test]
    fn a_free_page_of_version_0_14_counts_as_the_paper() {
        let (_dir, found, scale) = read_fixture();
        let mut old = plan_layouts(&found.frames[1..], &[], scale, 12.0).add;
        old[0].paper = Paper::A4;
        old[0].page_mm = Some([148.0, 210.0]);
        let again = plan_layouts(&found.frames, &old, scale, 12.0);
        assert_eq!(again.present, ["frame-a5-landscape"]);
        assert_eq!(again.add.len(), 1);
    }

    #[test]
    fn two_equal_frames_name_their_layout_once() {
        let (_dir, found, scale) = read_fixture();
        let first = plan_layouts(&found.frames[..1], &[], scale, 12.0);
        let twice = [found.frames[0], found.frames[0]];
        let again = plan_layouts(&twice, &first.add, scale, 12.0);
        assert_eq!(again.present, ["frame-a4-portrait"]);
    }

    #[test]
    fn a_taken_id_gets_a_number_and_is_never_replaced() {
        let (_dir, found, scale) = read_fixture();
        let mut taken = layout_for(&found.frames[0], "frame-a4-portrait".into(), scale, 12.0);
        taken.map_frame.centre_map_mm = Some([1.0, 1.0]);
        let plan = plan_layouts(&found.frames[..1], &[taken], scale, 12.0);
        assert_eq!(plan.add.len(), 1);
        assert_eq!(plan.add[0].id, "frame-a4-portrait-2");
    }

    #[test]
    fn every_paper_of_the_contract_has_a_size() {
        for word in freemapper_contract::tags::PAPERS {
            let paper = FramePaper::parse(word).expect(word);
            let [w, h] = paper.size_mm();
            assert!(w < h, "{word} is given in portrait");
        }
    }
}
