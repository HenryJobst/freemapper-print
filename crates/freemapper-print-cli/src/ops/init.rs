//! `init`: writes a starter project file for a map, optionally with the courses of a course file.

use std::path::{Path, PathBuf};

use freemapper_print::course::SourceFormat;
use freemapper_print::omap::MapDocument;
use freemapper_print::output;
use freemapper_print::print::Paper;
use freemapper_print::project::{self, KeyComment, LayoutSpec, LoadedProject, SheetLang};
use freemapper_print::scale::{MapKind, infer_kind, read_sidecar};
use freemapper_print::sheet::LogoCache;
use serde::Serialize;

use super::check::{SheetCheck, check_layout};
use super::{OpError, is_format, read_courses, read_map};

/// What `init` takes.
#[derive(Debug, Clone)]
pub struct InitArgs {
    /// The project folder; created if missing.
    pub folder: PathBuf,
    /// The map file.
    pub map: PathBuf,
    /// The event title; `None` for the title of the map's `freemapper.toml`, else the map's file name.
    pub title: Option<String>,
    /// The map kind; `None` for `map_kind` of the map's `freemapper.toml`, else inferred.
    pub kind: Option<MapKind>,
    /// A course file (`.ppen` or IOF XML): one layout per print scale, an assignment per course.
    pub courses: Option<PathBuf>,
    /// The language of the texts the starter puts on the sheet.
    pub sheet_lang: SheetLang,
}

/// Where the map kind came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KindSource {
    /// The caller named it.
    Given,
    /// `map_kind` in the map's `freemapper.toml`.
    Sidecar,
    /// Inferred from the symbol set and scale (ADR 0021).
    Inferred,
}

/// What `init` reports while it works, before the file is written.
#[derive(Debug)]
pub enum InitProgress<'a> {
    /// The map's `freemapper.toml` could not be read or parsed, so it counts as missing. `error` is the chain of
    /// causes as the library and the operating system word it (ADR 0024).
    SidecarUnusable { error: &'a str },
    /// The map kind and where it came from.
    Kind { kind: MapKind, source: KindSource },
    /// The course file was read: `path` as the project file names it (relative to the project folder).
    CoursesRead { path: &'a Path, count: usize },
    /// A layout at this print scale is being fitted to the map; it takes a moment on a large map.
    Fitting { scale: u32 },
}

/// Comments `init` writes into the new project file, in the user's language.
#[derive(Debug, Clone)]
pub struct FileComments {
    /// Above `contour_interval_m` (when the starter proposes one, else where it would be).
    pub contour: String,
    /// Above the shared elements: where the footer goes.
    pub footer: String,
}

/// A layout of the new project file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InitLayout {
    pub id: String,
    pub paper: Paper,
    pub margin_mm: f64,
    pub print_scale: u32,
    /// The map frame takes the map (or, with a course file, the courses) at this scale.
    pub fits: bool,
    /// The courses assigned to the layout, in file order.
    pub courses: Vec<String>,
}

/// What `init` read from the course file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InitCourses {
    /// The course file as the project file names it: relative to the project folder.
    pub path: PathBuf,
    /// The number of courses.
    pub count: usize,
    /// The file is IOF XML: no print scales, no control descriptions.
    pub iof: bool,
}

/// The project file `init` wrote.
#[derive(Debug, Serialize)]
pub struct InitResult {
    /// The project file, as named by the folder given.
    pub path: PathBuf,
    pub kind: MapKind,
    pub kind_source: KindSource,
    /// The scale of the map file.
    pub map_scale: u32,
    pub layouts: Vec<InitLayout>,
    /// The number of assignments written.
    pub assignments: usize,
    /// `None` without a course file.
    pub course_file: Option<InitCourses>,
    /// The contour interval the starter proposes for the map kind; `None` when it proposes none.
    pub contour_interval_m: Option<f64>,
    /// Without a course file, when the layout does not fit: what `check` says about its sheet.
    pub check: Option<SheetCheck>,
}

impl InitResult {
    /// Every layout fits its map frame.
    pub fn fits(&self) -> bool {
        self.layouts.iter().all(|l| l.fits)
    }
}

/// What `init --courses` learned from the course file before it wrote anything.
struct CoursePlan {
    /// The course file, absolute.
    file: PathBuf,
    /// Each course and the print scale of its layout, in file order.
    courses: Vec<(String, u32)>,
    /// The file is IOF XML: no print scales, no descriptions.
    iof: bool,
}

impl CoursePlan {
    /// The distinct print scales, in the order the courses first use them.
    fn scales(&self) -> Vec<u32> {
        let mut out: Vec<u32> = Vec::new();
        for (_, scale) in &self.courses {
            if !out.contains(scale) {
                out.push(*scale);
            }
        }
        out
    }
}

/// The map kind: `given`, else `map_kind` in `freemapper.toml` next to the map, else inferred from the symbol
/// set and scale (ADR 0021), and where it came from. A `freemapper.toml` that cannot be read or parsed goes to
/// `sidecar_unusable` with the chain of its causes.
pub fn detect_kind(
    map: &MapDocument,
    path: &Path,
    given: Option<MapKind>,
    sidecar_unusable: &mut dyn FnMut(&str),
) -> Result<(MapKind, KindSource), OpError> {
    if let Some(kind) = given {
        return Ok((kind, KindSource::Given));
    }
    match read_sidecar(path) {
        Ok(Some(kind)) => return Ok((kind, KindSource::Sidecar)),
        Ok(None) => {}
        // The whole chain: the operating system's error is the source.
        Err(err) => sidecar_unusable(&format!("{:#}", anyhow::Error::new(err))),
    }
    let kind = infer_kind(map, path).ok_or(OpError::KindUnknown)?;
    Ok((kind, KindSource::Inferred))
}

/// Writes a starter project file into `args.folder`. `comments` makes the comments of the file for the map kind
/// and the contour interval the starter proposes. Nothing is written when an error comes before the file.
pub fn init(
    args: &InitArgs,
    comments: &dyn Fn(MapKind, Option<f64>) -> FileComments,
    progress: &mut dyn FnMut(InitProgress<'_>),
) -> Result<InitResult, OpError> {
    if let Some(title) = &args.title
        && title.chars().any(|c| c.is_control() || is_format(c))
    {
        return Err(OpError::TitleControl);
    }
    // `export` writes into `out` later; a link there that leads out of the project stops `init` already
    // (ADR 0041). A folder that does not exist yet holds no link.
    if args.folder.is_dir() {
        output::check_out_folder(&args.folder, &args.folder.join("out"))
            .map_err(OpError::Output)?;
    }
    let map = read_map(&args.map)?;
    let (kind, source) = detect_kind(&map, &args.map, args.kind, &mut |error| {
        progress(InitProgress::SidecarUnusable { error })
    })?;
    let shown = args.folder.join(project::FILE_NAME);
    if shown.exists() {
        return Err(OpError::ProjectExists { path: shown });
    }
    // The course file is read and fitted before anything is created, so that an error writes nothing.
    let plan = match &args.courses {
        Some(file) => Some(plan_courses(args, &map, kind, file)?),
        None => None,
    };
    let folder_error = |err: std::io::Error| OpError::CreateFolder {
        path: args.folder.clone(),
        source: anyhow::Error::new(err),
    };
    std::fs::create_dir_all(&args.folder).map_err(folder_error)?;
    // Both canonical, so that links and `..` do not spoil the relative path.
    let dir = args.folder.canonicalize().map_err(folder_error)?;
    let map_path = args.map.canonicalize().map_err(|err| OpError::ReadMap {
        path: args.map.clone(),
        source: anyhow::Error::new(err),
    })?;
    // The title of the map's `freemapper.toml` comes before the file name (an unusable sidecar counts as
    // missing).
    let title = args
        .title
        .clone()
        .or_else(|| {
            freemapper_print::scale::read_sidecar_title(&map_path)
                .ok()
                .flatten()
        })
        .unwrap_or_else(|| {
            args.map
                .file_stem()
                .map_or_else(|| "map".into(), |s| s.to_string_lossy().into_owned())
        });
    let map_scale = map.georef.scale_denominator;
    let map_rel = project::relative_path(&dir, &map_path);
    progress(InitProgress::Kind { kind, source });
    let course_file = plan.as_ref().map(|plan| InitCourses {
        path: project::relative_path(&dir, &plan.file),
        count: plan.courses.len(),
        iof: plan.iof,
    });
    if let Some(courses) = &course_file {
        progress(InitProgress::CoursesRead {
            path: &courses.path,
            count: courses.count,
        });
    }
    // One layout per print scale; without a course file, one at the map scale.
    let scales = match &plan {
        Some(plan) if !plan.courses.is_empty() => plan.scales(),
        _ => vec![map_scale],
    };
    // Fitting up to twenty sheets per layout takes a moment on a large map.
    let mut fitted: Vec<(LayoutSpec, bool, u32)> = Vec::new();
    let mut first: Option<project::Project> = None;
    for &scale in &scales {
        let mut starter = project::starter(&title, map_rel.clone(), kind, scale, args.sheet_lang);
        // Course sheets cut the map to the course by design.
        starter.layouts[0].map_frame.crop = plan.is_some();
        progress(InitProgress::Fitting { scale });
        let (new, fits) = project::fit_starter(starter, &dir, &map);
        let mut layout = new.layouts[0].clone();
        // With a course file the id names the print scale too, whether the courses need one layout or several.
        if plan.is_some() {
            layout.id = format!("{}-{scale}", layout.id);
        }
        fitted.push((layout, fits, scale));
        first.get_or_insert(new);
    }
    let mut new = first.expect("at least one scale");
    new.layouts = fitted.iter().map(|(l, _, _)| l.clone()).collect();
    if let Some(plan) = &plan {
        new.sources.courses = Some(project::relative_path(&dir, &plan.file));
        // A sheet without a description box (IOF XML) names its course only in the title block.
        new.shared[0].lines[0] = "{event} \u{b7} {course}".into();
        for (name, scale) in &plan.courses {
            let layout = &fitted
                .iter()
                .find(|(_, _, s)| s == scale)
                .expect("a layout for every scale")
                .0;
            new.assignments.push(project::Assignment {
                course: name.clone(),
                layout: layout.id.clone(),
                mask: None,
            });
        }
        if !plan.iof {
            for layout in &mut new.layouts {
                layout.elements.push(description_box());
            }
        }
    }
    let contour = new.map.contour_interval_m;
    let FileComments {
        contour: contour_comment,
        footer: footer_comment,
    } = comments(kind, contour);
    let key_comments = [
        KeyComment {
            table: "map",
            key: Some("contour_interval_m"),
            text: &contour_comment,
        },
        KeyComment {
            table: "shared",
            key: None,
            text: &footer_comment,
        },
    ];
    let file = dir.join(project::FILE_NAME);
    match project::save_new_with_comments(&file, &new, &key_comments) {
        Ok(()) => {}
        Err(project::ProjectError::Exists { .. }) => {
            return Err(OpError::ProjectExists { path: shown });
        }
        Err(err) => {
            return Err(OpError::WriteFile {
                path: shown,
                source: anyhow::Error::new(err),
            });
        }
    }
    let layouts: Vec<InitLayout> = new
        .layouts
        .iter()
        .zip(&fitted)
        .map(|(layout, (_, fits, scale))| InitLayout {
            id: layout.id.clone(),
            paper: layout.paper,
            margin_mm: layout.map_frame.margin_mm,
            print_scale: *scale,
            fits: *fits,
            courses: plan
                .iter()
                .flat_map(|p| &p.courses)
                .filter(|(_, s)| s == scale)
                .map(|(n, _)| n.clone())
                .collect(),
        })
        .collect();
    // Without a course file, a layout that does not fit gets what `check` will say about it.
    let check = match (&plan, layouts.iter().all(|l| l.fits)) {
        (None, false) => {
            let loaded = LoadedProject {
                path: file.clone(),
                dir: dir.clone(),
                project: new.clone(),
                // `init` writes the default profile.
                rules: freemapper_print::rules::base().clone(),
            };
            Some(check_layout(
                &loaded,
                &map,
                &new.layouts[0],
                &LogoCache::new(),
            ))
        }
        _ => None,
    };
    Ok(InitResult {
        path: shown,
        kind,
        kind_source: source,
        map_scale,
        layouts,
        assignments: new.assignments.len(),
        course_file,
        contour_interval_m: contour,
        check,
    })
}

/// Reads and fits the course file of `init --courses` with the real loader, so that a course file that does
/// not fit the map stops `init` before it writes anything. A `.ppen` gives each course its print scale (the
/// map scale where the file has none); IOF XML has none, so every course takes the map scale.
fn plan_courses(
    args: &InitArgs,
    map: &MapDocument,
    kind: MapKind,
    file: &Path,
) -> Result<CoursePlan, OpError> {
    let file = file.canonicalize().map_err(|err| OpError::ReadCourses {
        path: file.to_path_buf(),
        source: anyhow::Error::new(err),
    })?;
    let map_scale = map.georef.scale_denominator;
    let map_path = args.map.canonicalize().unwrap_or_else(|_| args.map.clone());
    let mut probe = project::starter("probe", map_path, kind, map_scale, SheetLang::En);
    probe.sources.courses = Some(file.clone());
    let loaded = LoadedProject {
        path: args.folder.join(project::FILE_NAME),
        dir: args.folder.clone(),
        project: probe,
        rules: freemapper_print::rules::base().clone(),
    };
    let courses = read_courses(&loaded, map)?.expect("the probe project names a course file");
    let iof = courses.event.source.format == SourceFormat::IofXml;
    let scale_of = |hint: Option<u32>| match hint {
        Some(scale) if !iof && scale > 0 => scale,
        _ => map_scale,
    };
    Ok(CoursePlan {
        file,
        courses: courses
            .event
            .courses
            .iter()
            .map(|c| (c.name.clone(), scale_of(c.print_scale_hint)))
            .collect(),
        iof,
    })
}

/// The control description box of a course layout: top-right corner of the map frame, which the starter's
/// title block (top left) and scale check bar (bottom right) leave free. The cell size is the rule profile's.
fn description_box() -> project::ElementSpec {
    project::ElementSpec {
        id: "descriptions".into(),
        kind: project::ElementKind::ControlDescriptions,
        anchor: project::Anchor {
            to: project::AnchorTarget::MapFrame,
            corner: project::Corner::TopRight,
            offset_mm: [2.0, 2.0],
        },
        lines: vec![],
        size_pt: None,
        box_white: None,
        file: None,
        width_mm: None,
        cell_mm: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::check::SheetCheck;
    use crate::ops::testing::viktoriapark;

    fn args(folder: &Path, courses: Option<PathBuf>) -> InitArgs {
        InitArgs {
            folder: folder.to_path_buf(),
            map: viktoriapark().join("berlin-viktoriapark.omap"),
            title: None,
            kind: None,
            courses,
            sheet_lang: SheetLang::En,
        }
    }

    fn comments(kind: MapKind, contour: Option<f64>) -> FileComments {
        FileComments {
            contour: format!("{kind:?} {contour:?}"),
            footer: "footer".into(),
        }
    }

    #[test]
    fn viktoriapark_with_its_course_file_gets_one_layout_and_one_assignment() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("new");
        let mut seen = Vec::new();
        let result = init(
            &args(&folder, Some(viktoriapark().join("training.ppen"))),
            &comments,
            &mut |p| {
                seen.push(match p {
                    InitProgress::SidecarUnusable { .. } => "sidecar".to_string(),
                    InitProgress::Kind { source, .. } => format!("kind {source:?}"),
                    InitProgress::CoursesRead { count, .. } => format!("courses {count}"),
                    InitProgress::Fitting { scale } => format!("fitting {scale}"),
                })
            },
        )
        .unwrap();
        assert_eq!(seen, ["kind Sidecar", "courses 1", "fitting 4000"]);
        assert_eq!(result.path, folder.join(project::FILE_NAME));
        assert_eq!(result.kind, MapKind::Sprint);
        assert_eq!(result.kind_source, KindSource::Sidecar);
        assert_eq!(result.map_scale, 4000);
        assert_eq!(result.layouts.len(), 1);
        assert_eq!(result.layouts[0].id, "a4-4000");
        assert_eq!(result.layouts[0].print_scale, 4000);
        assert_eq!(result.layouts[0].courses, ["Training"]);
        assert_eq!(result.assignments, 1);
        let courses = result.course_file.as_ref().unwrap();
        assert_eq!((courses.count, courses.iof), (1, false));
        assert_eq!(result.contour_interval_m, Some(2.0));
        assert!(result.check.is_none());
        let text = std::fs::read_to_string(&result.path).unwrap();
        assert!(text.contains("# Sprint Some(2.0)\n"), "{text}");
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["kind_source"], "sidecar");
        assert_eq!(json["layouts"][0]["courses"][0], "Training");
        // A second init into the same folder overwrites nothing.
        let again = init(&args(&folder, None), &comments, &mut |_| {}).unwrap_err();
        assert!(matches!(again, OpError::ProjectExists { .. }), "{again:?}");
    }

    #[test]
    fn viktoriapark_without_course_file_says_what_check_will_say() {
        let dir = tempfile::tempdir().unwrap();
        let result = init(&args(dir.path(), None), &comments, &mut |_| {}).unwrap();
        assert_eq!(result.layouts[0].id, "a4");
        assert!(!result.fits());
        assert!(result.course_file.is_none());
        let Some(SheetCheck::Drawn(sheet)) = &result.check else {
            panic!("{:?}", result.check);
        };
        let kinds: Vec<_> = sheet.notes.iter().map(crate::ops::Notice::kind).collect();
        assert_eq!(
            kinds,
            ["overflow", "copyright_truncated", "footer_overlaps_map"]
        );
    }
}
