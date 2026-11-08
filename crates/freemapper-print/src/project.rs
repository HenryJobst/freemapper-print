//! The event project file `freemapper-print.toml` (ADR 0025): event, map source, shared sheet elements and any
//! number of layouts. Editor, CLI and MCP server use this one file (ADR 0008).

use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::print::{Orientation, Paper};
use crate::scale::MapKind;

/// The only format version this program reads and writes.
pub const FORMAT: u32 = 1;

/// Default file name of a project file.
pub const FILE_NAME: &str = "freemapper-print.toml";

/// The whole project file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub format: u32,
    pub event: Event,
    pub sources: Sources,
    pub map: MapSettings,
    #[serde(default)]
    pub print: PrintSettings,
    /// Sheet elements that every layout shows unless it hides them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shared: Vec<ElementSpec>,
    #[serde(default)]
    pub layouts: Vec<LayoutSpec>,
}

/// Language of the printed sheet: number formats and fixed sheet words. Independent of the program language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SheetLang {
    #[default]
    De,
    En,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<toml::value::Date>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organiser: Option<String>,
    #[serde(default)]
    pub lang: SheetLang,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sources {
    /// Path of the map, relative to the project file.
    pub map: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapSettings {
    pub kind: MapKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contour_interval_m: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrintSettings {
    #[serde(default = "yes")]
    pub credit_line: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copyright: Option<String>,
}

impl Default for PrintSettings {
    fn default() -> Self {
        PrintSettings {
            credit_line: true,
            copyright: None,
        }
    }
}

/// Whether north lines are added when the map has none (ADR 0023).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NorthLines {
    #[default]
    Auto,
    Never,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutSpec {
    pub id: String,
    #[serde(default = "default_paper")]
    pub paper: Paper,
    #[serde(default = "default_orientation")]
    pub orientation: Orientation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub print_scale: Option<u32>,
    #[serde(default)]
    pub map_frame: MapFrameSpec,
    #[serde(default)]
    pub north_lines: NorthLines,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hide_parts: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elements: Vec<ElementSpec>,
    /// Ids of shared elements this layout does not show.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hide_shared: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapFrameSpec {
    /// Map point (mm on the map sheet) in the centre of the frame; `None` centres the printed area.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub centre_map_mm: Option<[f64; 2]>,
    /// Passe-partout width in mm.
    #[serde(default = "default_margin_mm")]
    pub margin_mm: f64,
    /// Frame line width in mm; 0 draws no frame.
    #[serde(default = "default_border_mm")]
    pub border_mm: f64,
}

impl Default for MapFrameSpec {
    fn default() -> Self {
        MapFrameSpec {
            centre_map_mm: None,
            margin_mm: 8.0,
            border_mm: 0.25,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementKind {
    TitleBlock,
    Text,
    Logo,
    ScaleCheckBar,
    CreditLine,
    CopyrightLine,
}

/// A sheet element. One flat struct: which optional fields apply depends on `kind` (see `validate`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ElementSpec {
    pub id: String,
    pub kind: ElementKind,
    pub anchor: Anchor,
    /// title_block, text
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<String>,
    /// title_block, text, credit_line, copyright_line
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_pt: Option<f64>,
    /// title_block, text: white box behind the text.
    #[serde(default = "yes")]
    pub box_white: bool,
    /// logo: image file, relative to the project file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
    /// logo: printed width in mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width_mm: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorTarget {
    Page,
    MapFrame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Corner {
    TopLeft,
    Top,
    TopRight,
    Left,
    Centre,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

/// Where an element hangs: a corner or edge of the page or of the map frame, plus an offset in mm measured
/// inward from that corner (x to the right, y down).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Anchor {
    pub to: AnchorTarget,
    pub corner: Corner,
    #[serde(default)]
    pub offset_mm: [f64; 2],
}

fn yes() -> bool {
    true
}
fn default_paper() -> Paper {
    Paper::A4
}
fn default_orientation() -> Orientation {
    Orientation::Auto
}
fn default_margin_mm() -> f64 {
    8.0
}
fn default_border_mm() -> f64 {
    0.25
}

/// A project file that was read, with the place it lies in.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedProject {
    /// Path of the project file as given.
    pub path: PathBuf,
    /// Absolute directory of the project file; relative paths inside it resolve against this.
    pub dir: PathBuf,
    pub project: Project,
}

impl LoadedProject {
    /// Absolute path of the map file.
    pub fn map_path(&self) -> PathBuf {
        self.resolve(&self.project.sources.map)
    }

    /// Resolves a path from the project file (logo, ...) against the file's directory.
    pub fn resolve(&self, p: &Path) -> PathBuf {
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.dir.join(p)
        }
    }
}

/// Why a project file could not be read, checked or written. The messages name the file and, where known, the
/// line, column and field.
#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("cannot read project file {file}: {source}")]
    Io {
        file: String,
        source: std::io::Error,
    },
    #[error("{file}:{line}:{column}: {message}")]
    Parse {
        file: String,
        line: usize,
        column: usize,
        message: String,
    },
    #[error(
        "{file}: unsupported project file format {found}; this program supports format {FORMAT}"
    )]
    UnsupportedFormat { file: String, found: u32 },
    #[error("{file}: field `{field}`: {message}")]
    Invalid {
        file: String,
        field: String,
        message: String,
    },
    #[error("{file}: field `{field}`: the id `{id}` is used more than once")]
    DuplicateId {
        file: String,
        field: String,
        id: String,
    },
    #[error("{file}: field `sources.map`: the map file {map} does not exist")]
    MissingMap { file: String, map: String },
    #[error("{file}: field `layouts`: the project needs at least one layout")]
    NoLayout { file: String },
    #[error("{file} exists already; it was not overwritten")]
    Exists { file: String },
    #[error("cannot write the project file: {0}")]
    Serialize(String),
}

fn line_col(text: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(text.len());
    let before = &text[..offset];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    (line, column)
}

fn parse_error(file: &str, text: &str, e: &toml::de::Error) -> ProjectError {
    let (line, column) = e.span().map_or((1, 1), |s| line_col(text, s.start));
    ProjectError::Parse {
        file: file.to_string(),
        line,
        column,
        message: e.message().to_string(),
    }
}

/// Reads and checks a project file. Paths inside stay as written; `LoadedProject::resolve` makes them absolute.
pub fn load(path: &Path) -> Result<LoadedProject, ProjectError> {
    let file = path.display().to_string();
    let text = std::fs::read_to_string(path).map_err(|source| ProjectError::Io {
        file: file.clone(),
        source,
    })?;

    #[derive(Deserialize)]
    struct Probe {
        format: u32,
    }
    let probe: Probe = toml::from_str(&text).map_err(|e| parse_error(&file, &text, &e))?;
    if probe.format != FORMAT {
        return Err(ProjectError::UnsupportedFormat {
            file,
            found: probe.format,
        });
    }
    let project: Project = toml::from_str(&text).map_err(|e| parse_error(&file, &text, &e))?;

    let abs = std::path::absolute(path).map_err(|source| ProjectError::Io {
        file: file.clone(),
        source,
    })?;
    let dir = abs.parent().map(Path::to_path_buf).unwrap_or_default();
    let loaded = LoadedProject {
        path: path.to_path_buf(),
        dir,
        project,
    };
    validate(&file, &loaded)?;
    Ok(loaded)
}

fn invalid(file: &str, field: String, message: &str) -> ProjectError {
    ProjectError::Invalid {
        file: file.to_string(),
        field,
        message: message.to_string(),
    }
}

fn validate(file: &str, lp: &LoadedProject) -> Result<(), ProjectError> {
    let p = &lp.project;
    if p.layouts.is_empty() {
        return Err(ProjectError::NoLayout {
            file: file.to_string(),
        });
    }
    if !lp.map_path().is_file() {
        return Err(ProjectError::MissingMap {
            file: file.to_string(),
            map: lp.map_path().display().to_string(),
        });
    }
    let mut layout_ids = HashSet::new();
    for (i, l) in p.layouts.iter().enumerate() {
        if !layout_ids.insert(l.id.as_str()) {
            return Err(ProjectError::DuplicateId {
                file: file.to_string(),
                field: format!("layouts[{i}].id"),
                id: l.id.clone(),
            });
        }
    }
    let mut shared_ids = HashSet::new();
    for (i, e) in p.shared.iter().enumerate() {
        if !shared_ids.insert(e.id.as_str()) {
            return Err(ProjectError::DuplicateId {
                file: file.to_string(),
                field: format!("shared[{i}].id"),
                id: e.id.clone(),
            });
        }
        check_element(file, &format!("shared[{i}]"), e)?;
    }
    for (i, l) in p.layouts.iter().enumerate() {
        let mut ids = shared_ids.clone();
        for (j, e) in l.elements.iter().enumerate() {
            if !ids.insert(e.id.as_str()) {
                return Err(ProjectError::DuplicateId {
                    file: file.to_string(),
                    field: format!("layouts[{i}].elements[{j}].id"),
                    id: e.id.clone(),
                });
            }
            check_element(file, &format!("layouts[{i}].elements[{j}]"), e)?;
        }
        for (j, h) in l.hide_shared.iter().enumerate() {
            if !shared_ids.contains(h.as_str()) {
                return Err(invalid(
                    file,
                    format!("layouts[{i}].hide_shared[{j}]"),
                    &format!("there is no shared element with the id `{h}`"),
                ));
            }
        }
        if l.print_scale == Some(0) {
            return Err(invalid(
                file,
                format!("layouts[{i}].print_scale"),
                "must be larger than 0",
            ));
        }
    }
    Ok(())
}

fn check_element(file: &str, at: &str, e: &ElementSpec) -> Result<(), ProjectError> {
    if e.kind == ElementKind::Logo {
        if e.file.is_none() {
            return Err(invalid(file, format!("{at}.file"), "a logo needs a `file`"));
        }
        match e.width_mm {
            None => {
                return Err(invalid(
                    file,
                    format!("{at}.width_mm"),
                    "a logo needs a `width_mm`",
                ));
            }
            Some(w) if w <= 0.0 => {
                return Err(invalid(
                    file,
                    format!("{at}.width_mm"),
                    "must be larger than 0",
                ));
            }
            Some(_) => {}
        }
    }
    Ok(())
}

/// Writes a new project file. Refuses to replace an existing file.
pub fn save_new(path: &Path, project: &Project) -> Result<(), ProjectError> {
    let file = path.display().to_string();
    let body =
        toml::to_string_pretty(project).map_err(|e| ProjectError::Serialize(e.to_string()))?;
    let mut text = String::new();
    let _ = writeln!(
        text,
        "# freemapper-print event project file, format {FORMAT}"
    );
    let _ = writeln!(text, "# Paths are relative to this file.\n");
    text.push_str(&body);
    use std::io::Write as _;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| {
            if source.kind() == std::io::ErrorKind::AlreadyExists {
                ProjectError::Exists { file: file.clone() }
            } else {
                ProjectError::Io {
                    file: file.clone(),
                    source,
                }
            }
        })?;
    f.write_all(text.as_bytes())
        .map_err(|source| ProjectError::Io { file, source })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/projects/minimal")
    }

    fn minimal_path() -> PathBuf {
        fixture_dir().join(FILE_NAME)
    }

    /// A temp dir with a project file with `extra` appended and a map file next to it.
    fn temp_project(body: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("m.omap"), "x").unwrap();
        let path = dir.path().join(FILE_NAME);
        std::fs::write(&path, body).unwrap();
        (dir, path)
    }

    const HEAD: &str = "format = 1\n[event]\ntitle = \"T\"\n[sources]\nmap = \"m.omap\"\n[map]\nkind = \"forest\"\n";

    #[test]
    fn loads_minimal_project() {
        let lp = load(&minimal_path()).unwrap();
        assert_eq!(lp.project.event.title, "Minimal Sprint");
        assert_eq!(lp.project.map.kind, MapKind::Sprint);
        assert_eq!(lp.project.layouts.len(), 1);
        assert_eq!(lp.project.event.lang, SheetLang::De);
        assert_eq!(lp.project.map.contour_interval_m, None);
    }

    #[test]
    fn paths_relative_to_file() {
        // The test process's cwd is the crate directory, not the fixture directory.
        let lp = load(&minimal_path()).unwrap();
        let map = lp.map_path();
        assert!(map.is_absolute());
        assert!(map.is_file(), "{}", map.display());
        assert_eq!(lp.resolve(Path::new("logo.png")), lp.dir.join("logo.png"));
    }

    #[test]
    fn unknown_key_names_line() {
        let (_d, path) = temp_project(&format!(
            "{HEAD}\n[[layouts]]\nid = \"a\"\npaperr = \"A4\"\n"
        ));
        let msg = load(&path).unwrap_err().to_string();
        assert!(msg.contains(FILE_NAME), "{msg}");
        assert!(msg.contains(":11:1"), "{msg}");
        assert!(msg.contains("paperr"), "{msg}");
    }

    #[test]
    fn unknown_key_in_element_is_rejected() {
        let el = "[[shared]]\nid=\"x\"\nkind=\"scale_check_bar\"\nbogus=1\n[shared.anchor]\nto=\"page\"\ncorner=\"top\"\n";
        let (_d, path) = temp_project(&format!("{HEAD}\n[[layouts]]\nid = \"a\"\n{el}"));
        let msg = load(&path).unwrap_err().to_string();
        assert!(msg.contains("bogus"), "{msg}");
    }

    #[test]
    fn unsupported_format_names_supported() {
        let (_d, path) = temp_project("format = 2\n");
        let msg = load(&path).unwrap_err().to_string();
        assert!(
            msg.contains("format 2") && msg.contains("supports format 1"),
            "{msg}"
        );
    }

    #[test]
    fn duplicate_layout_id() {
        let (_d, path) = temp_project(&format!(
            "{HEAD}\n[[layouts]]\nid = \"a\"\n[[layouts]]\nid = \"a\"\n"
        ));
        let msg = load(&path).unwrap_err().to_string();
        assert!(matches!(load(&path), Err(ProjectError::DuplicateId { .. })));
        assert!(
            msg.contains("layouts[1].id") && msg.contains("`a`"),
            "{msg}"
        );
    }

    #[test]
    fn missing_map_file() {
        let (d, path) = temp_project(&format!("{HEAD}\n[[layouts]]\nid = \"a\"\n"));
        std::fs::remove_file(d.path().join("m.omap")).unwrap();
        let msg = load(&path).unwrap_err().to_string();
        assert!(matches!(load(&path), Err(ProjectError::MissingMap { .. })));
        assert!(
            msg.contains("sources.map") && msg.contains("m.omap"),
            "{msg}"
        );
    }

    #[test]
    fn no_layout() {
        let (_d, path) = temp_project(HEAD);
        assert!(matches!(load(&path), Err(ProjectError::NoLayout { .. })));
    }

    #[test]
    fn many_layouts_are_fine() {
        let mut body = HEAD.to_string();
        for i in 0..40 {
            body.push_str(&format!("[[layouts]]\nid = \"l{i}\"\n"));
        }
        let (_d, path) = temp_project(&body);
        assert_eq!(load(&path).unwrap().project.layouts.len(), 40);
    }

    #[test]
    fn defaults_applied() {
        let lp = load(&minimal_path()).unwrap();
        let l = &lp.project.layouts[0];
        assert_eq!(l.map_frame.margin_mm, 8.0);
        assert_eq!(l.map_frame.border_mm, 0.25);
        assert!(lp.project.print.credit_line);
        assert_eq!(l.north_lines, NorthLines::Auto);
        assert_eq!(l.paper, Paper::A4);
        assert_eq!(l.orientation, Orientation::Auto);
    }

    #[test]
    fn logo_needs_file_and_width() {
        let el =
            "[[shared]]\nid=\"lg\"\nkind=\"logo\"\n[shared.anchor]\nto=\"page\"\ncorner=\"top\"\n";
        let (_d, path) = temp_project(&format!("{HEAD}\n[[layouts]]\nid = \"a\"\n{el}"));
        let msg = load(&path).unwrap_err().to_string();
        assert!(msg.contains("shared[0].file"), "{msg}");
    }

    #[test]
    fn hide_shared_must_exist() {
        let (_d, path) = temp_project(&format!(
            "{HEAD}\n[[layouts]]\nid = \"a\"\nhide_shared = [\"nope\"]\n"
        ));
        let msg = load(&path).unwrap_err().to_string();
        assert!(
            msg.contains("hide_shared[0]") && msg.contains("nope"),
            "{msg}"
        );
    }

    fn full_project() -> Project {
        let anchor = Anchor {
            to: AnchorTarget::MapFrame,
            corner: Corner::BottomRight,
            offset_mm: [3.0, 4.5],
        };
        let el = |id: &str, kind| ElementSpec {
            id: id.into(),
            kind,
            anchor,
            lines: vec![],
            size_pt: None,
            box_white: true,
            file: None,
            width_mm: None,
        };
        let mut title = el("title", ElementKind::TitleBlock);
        title.lines = vec!["Line 1".into(), "Line 2".into()];
        title.size_pt = Some(11.0);
        let mut logo = el("logo", ElementKind::Logo);
        logo.file = Some("logo.png".into());
        logo.width_mm = Some(20.0);
        Project {
            format: 1,
            event: Event {
                title: "Event".into(),
                date: Some(toml::value::Date {
                    year: 2026,
                    month: 11,
                    day: 8,
                }),
                organiser: Some("OC".into()),
                lang: SheetLang::En,
            },
            sources: Sources {
                map: "m.omap".into(),
            },
            map: MapSettings {
                kind: MapKind::Forest,
                contour_interval_m: Some(5.0),
            },
            print: PrintSettings {
                credit_line: false,
                copyright: Some("(c) OC".into()),
            },
            shared: vec![title, el("bar", ElementKind::ScaleCheckBar)],
            layouts: vec![
                LayoutSpec {
                    id: "a3".into(),
                    paper: Paper::A3,
                    orientation: Orientation::Landscape,
                    print_scale: Some(7500),
                    map_frame: MapFrameSpec {
                        centre_map_mm: Some([10.0, 20.0]),
                        margin_mm: 6.0,
                        border_mm: 0.0,
                    },
                    north_lines: NorthLines::Never,
                    hide_parts: vec!["north".into()],
                    elements: vec![logo],
                    hide_shared: vec!["bar".into()],
                },
                LayoutSpec {
                    id: "a4".into(),
                    paper: Paper::A4,
                    orientation: Orientation::Auto,
                    print_scale: None,
                    map_frame: MapFrameSpec::default(),
                    north_lines: NorthLines::Auto,
                    hide_parts: vec![],
                    elements: vec![],
                    hide_shared: vec![],
                },
            ],
        }
    }

    #[test]
    fn round_trip() {
        let (_d, path) = temp_project("");
        std::fs::remove_file(&path).unwrap();
        let project = full_project();
        save_new(&path, &project).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.starts_with("# freemapper-print") && text.contains("format 1"),
            "{text}"
        );
        assert_eq!(load(&path).unwrap().project, project);
    }

    #[test]
    fn save_new_refuses_overwrite() {
        let (_d, path) = temp_project("keep");
        let err = save_new(&path, &full_project()).unwrap_err();
        assert!(matches!(err, ProjectError::Exists { .. }));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "keep");
    }
}
