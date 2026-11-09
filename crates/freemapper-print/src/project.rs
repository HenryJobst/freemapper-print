//! The event project file `freemapper-print.toml` (ADR 0025): event, map source, shared sheet elements and any
//! number of layouts. Editor, CLI and MCP server use this one file (ADR 0008).

use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::course::{NumberPlacement, fit::YAxis};
use crate::print::{Orientation, Paper};
use crate::rules::RuleSet;
use crate::scale::MapKind;

/// The only format version this program reads and writes.
pub const FORMAT: u32 = 1;

/// Largest text size (pt) of an element.
pub const MAX_SIZE_PT: f64 = 200.0;
/// Largest logo width and largest anchor offset, in either direction (mm): beyond any paper.
pub const MAX_LENGTH_MM: f64 = 2000.0;
/// Widest frame line (mm).
pub const MAX_BORDER_MM: f64 = 20.0;
/// Smallest and largest printed cell of a control description box (mm). Below 4 mm the symbols are hard to
/// read; IOF events print 5 to 7 mm, and 10 mm leaves room for training sheets in large print.
pub const MIN_CELL_MM: f64 = 4.0;
pub const MAX_CELL_MM: f64 = 10.0;
/// Largest contour interval (m).
pub const MAX_CONTOUR_INTERVAL_M: f64 = 100.0;
/// Farthest map point a frame may centre on, in either direction (mm on the map sheet). OOM maps
/// span at most ±2147 m of map paper (µm in an `i32`).
pub const MAX_CENTRE_MM: f64 = 2_000_000.0;

/// Smallest size (pt) of a `credit_line` element: the credit line must stay readable (ADR 0025).
pub const MIN_CREDIT_PT: f64 = 5.0;

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
    #[serde(default, skip_serializing_if = "PrintSettings::is_empty")]
    pub print: PrintSettings,
    /// Sheet elements that every layout shows unless it hides them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shared: Vec<ElementSpec>,
    #[serde(default)]
    pub layouts: Vec<LayoutSpec>,
    /// Which layout prints which course (ADR 0025). A course without an entry takes the first layout.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assignments: Vec<Assignment>,
    /// Number placements that differ from the course file, per layout or per course.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub numbers: Vec<NumberCorrection>,
}

/// One course printed on one layout. The course name is the course file's; the entry stays in the
/// file when the course file no longer has the course (stale data, ADR 0025).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub course: String,
    pub layout: String,
}

/// A control number placement that overrides the course file's. Exactly one of `layout` and `course`
/// is set. Without `gap_mm`, the gap from the level below is kept.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NumberCorrection {
    pub control: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub course: Option<String>,
    /// Degrees, 0 is east, counter-clockwise; normalised to [0, 360) when the file is read.
    pub direction: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap_mm: Option<f64>,
    /// The course file's placement when the correction was made; `numbers accept` writes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_seen: Option<NumberPlacement>,
}

/// The largest `gap_mm` of a number correction (print mm).
pub const MAX_NUMBER_GAP_MM: f64 = 10.0;

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
    /// Path of the course file (`.ppen` or `.xml`), relative to the project file. It is read again at every
    /// `check` and `export` (ADR 0025).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub courses: Option<PathBuf>,
    /// The course file's y axis when it is set; unset, the fit detects it (ADR 0032).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub course_y_axis: Option<YAxis>,
    /// Whether latitude and longitude in the course file check and place controls (ADR 0032).
    #[serde(
        default = "default_true",
        skip_serializing_if = "std::clone::Clone::clone"
    )]
    pub course_georef_check: bool,
}

fn default_true() -> bool {
    true
}

impl Sources {
    /// Sources with only a map.
    pub fn new(map: PathBuf) -> Self {
        Sources {
            map,
            courses: None,
            course_y_axis: None,
            course_georef_check: true,
        }
    }
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copyright: Option<String>,
    /// The embedded base rule profile the print rules come from (ADR 0031). Left out of a written
    /// file while it is the default `iof-print`.
    #[serde(
        default = "default_rule_profile",
        skip_serializing_if = "is_default_rule_profile"
    )]
    pub rule_profile: String,
    /// Embedded overlays on the base profile, applied in order. Left out while empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rule_overlays: Vec<String>,
}

fn default_rule_profile() -> String {
    crate::rules::BASE_PROFILE.to_string()
}

fn is_default_rule_profile(id: &str) -> bool {
    id == crate::rules::BASE_PROFILE
}

impl Default for PrintSettings {
    fn default() -> Self {
        PrintSettings {
            copyright: None,
            rule_profile: default_rule_profile(),
            rule_overlays: Vec::new(),
        }
    }
}

impl PrintSettings {
    /// Nothing set: the table is left out of a written file.
    pub fn is_empty(&self) -> bool {
        *self == PrintSettings::default()
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
    /// The frame cuts the map on purpose: overflow is information, not a warning, and does not
    /// make `check` fail.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub crop: bool,
}

impl Default for MapFrameSpec {
    fn default() -> Self {
        MapFrameSpec {
            centre_map_mm: None,
            margin_mm: 8.0,
            border_mm: 0.25,
            crop: false,
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
    /// The control description box of the course; drawn on course sheets only.
    ControlDescriptions,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub box_white: Option<bool>,
    /// logo: image file, relative to the project file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
    /// logo: printed width in mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width_mm: Option<f64>,
    /// control_descriptions: printed side of one cell in mm; the rule profile's size when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell_mm: Option<f64>,
}

impl ElementSpec {
    /// Whether a white box sits behind the text; true unless the file says otherwise.
    pub fn box_white(&self) -> bool {
        self.box_white.unwrap_or(true)
    }
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
    /// The rule set `[print] rule_profile` and `rule_overlays` name (ADR 0031), loaded once by
    /// [`load`]. Course symbol sizes, the description cell, placement weights and the north-line
    /// spacing on sheets come from here; the scale table behind `scale.rs` stays the base profile's.
    pub rules: RuleSet,
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
    /// The operating system's error is the `source`, not part of the message, so that a chain of
    /// messages names it once.
    #[error("cannot read project file {file}")]
    Io {
        file: String,
        source: std::io::Error,
    },
    #[error("cannot write project file {file}")]
    Write {
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
    let mut project: Project = toml::from_str(&text).map_err(|e| parse_error(&file, &text, &e))?;
    for n in &mut project.numbers {
        n.direction = normalised_degrees(n.direction);
        if let Some(b) = &mut n.base_seen {
            b.direction = normalised_degrees(b.direction);
        }
    }

    let abs = std::path::absolute(path).map_err(|source| ProjectError::Io {
        file: file.clone(),
        source,
    })?;
    let dir = abs.parent().map(Path::to_path_buf).unwrap_or_default();
    let rules = rule_set_of(&file, &project.print).map_err(|e| with_line(e, &text))?;
    let loaded = LoadedProject {
        path: path.to_path_buf(),
        dir,
        project,
        rules,
    };
    validate(&file, &loaded).map_err(|e| with_line(e, &text))?;
    Ok(loaded)
}

/// `degrees` in [0, 360); a value that is not finite stays as it is for the validation to refuse.
fn normalised_degrees(degrees: f64) -> f64 {
    if !degrees.is_finite() {
        return degrees;
    }
    let d = degrees.rem_euclid(360.0);
    // `rem_euclid` of a tiny negative value rounds up to 360.
    if d >= 360.0 { 0.0 } else { d }
}

/// `err` with the line and column of its field when the TOML text has the field, or of the
/// nearest table above it that is written (a missing `file` points at its element). Errors
/// without a field (`NoLayout`) and files that do not parse again stay as they are.
fn with_line(err: ProjectError, text: &str) -> ProjectError {
    let located = |file: String, field: &str| match field_offset(text, field) {
        Some(offset) => {
            let (line, column) = line_col(text, offset);
            format!("{file}:{line}:{column}")
        }
        None => file,
    };
    match err {
        ProjectError::Invalid {
            file,
            field,
            message,
        } => ProjectError::Invalid {
            file: located(file, &field),
            field,
            message,
        },
        ProjectError::DuplicateId { file, field, id } => ProjectError::DuplicateId {
            file: located(file, &field),
            field,
            id,
        },
        ProjectError::MissingMap { file, map } => ProjectError::MissingMap {
            file: located(file, "sources.map"),
            map,
        },
        other => other,
    }
}

/// The byte offset in `text` of the value at `field` (`layouts[0].map_frame.margin_mm`), or of
/// the deepest table or array on the way that the text has.
fn field_offset(text: &str, field: &str) -> Option<usize> {
    use toml::de::{DeTable, DeValue};
    let doc = DeTable::parse(text).ok()?;
    let mut table = doc.get_ref();
    let mut best = None;
    for part in field.split('.') {
        let (key, indices) = match part.split_once('[') {
            Some((key, rest)) => (key, Some(rest)),
            None => (part, None),
        };
        let Some((_, value)) = table.iter().find(|(k, _)| k.get_ref().as_ref() == key) else {
            return best;
        };
        let mut value = value;
        best = Some(value.span().start);
        for index in indices.into_iter().flat_map(|r| r.split('[')) {
            let i: usize = index.trim_end_matches(']').parse().ok()?;
            let DeValue::Array(items) = value.get_ref() else {
                return best;
            };
            let Some(item) = items.get(i) else {
                return best;
            };
            value = item;
            best = Some(value.span().start);
        }
        match value.get_ref() {
            DeValue::Table(t) => table = t,
            _ => return best,
        }
    }
    best
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
    if let Some(c) = p.map.contour_interval_m {
        positive(file, "map.contour_interval_m".to_string(), c)?;
        at_most(
            file,
            "map.contour_interval_m".to_string(),
            c,
            MAX_CONTOUR_INTERVAL_M,
            "m",
        )?;
    }
    let mut layout_ids = HashSet::new();
    // Lower-case id -> the id as written: the PDFs `<id>.pdf` of `A4` and `a4` are one file on
    // macOS and Windows.
    let mut folded: std::collections::HashMap<String, &str> = std::collections::HashMap::new();
    for (i, l) in p.layouts.iter().enumerate() {
        check_layout(file, i, l)?;
        if !layout_ids.insert(l.id.as_str()) {
            return Err(ProjectError::DuplicateId {
                file: file.to_string(),
                field: format!("layouts[{i}].id"),
                id: l.id.clone(),
            });
        }
        if let Some(other) = folded.insert(l.id.to_lowercase(), &l.id) {
            return Err(invalid(
                file,
                format!("layouts[{i}].id"),
                &format!(
                    "`{}` differs from the layout id `{other}` only in upper and lower case; \
                     their PDFs would be the same file on macOS and Windows",
                    l.id
                ),
            ));
        }
    }
    let mut shared_ids = HashSet::new();
    for (i, e) in p.shared.iter().enumerate() {
        check_element(file, &format!("shared[{i}]"), e)?;
        if !shared_ids.insert(e.id.as_str()) {
            return Err(ProjectError::DuplicateId {
                file: file.to_string(),
                field: format!("shared[{i}].id"),
                id: e.id.clone(),
            });
        }
    }
    for (i, l) in p.layouts.iter().enumerate() {
        let mut ids = shared_ids.clone();
        for (j, e) in l.elements.iter().enumerate() {
            check_element(file, &format!("layouts[{i}].elements[{j}]"), e)?;
            if !ids.insert(e.id.as_str()) {
                return Err(ProjectError::DuplicateId {
                    file: file.to_string(),
                    field: format!("layouts[{i}].elements[{j}].id"),
                    id: e.id.clone(),
                });
            }
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
    check_courses(file, p, &layout_ids)
}

/// Course assignments and number corrections: known layouts, no two entries for one course, values in
/// range. Course names and controls are checked against the course file at load time, not here.
fn check_courses(file: &str, p: &Project, layout_ids: &HashSet<&str>) -> Result<(), ProjectError> {
    if let Some(c) = &p.sources.courses
        && c.as_os_str().is_empty()
    {
        return Err(invalid(file, "sources.courses".into(), "must not be empty"));
    }
    let known = |id: &str| {
        let mut ids: Vec<&str> = p.layouts.iter().map(|l| l.id.as_str()).collect();
        ids.sort_unstable();
        format!(
            "there is no layout with the id `{id}`; available: {}",
            ids.join(", ")
        )
    };
    let mut courses = HashSet::new();
    for (i, a) in p.assignments.iter().enumerate() {
        non_empty(file, format!("assignments[{i}].course"), &a.course)?;
        if !layout_ids.contains(a.layout.as_str()) {
            return Err(invalid(
                file,
                format!("assignments[{i}].layout"),
                &known(&a.layout),
            ));
        }
        if !courses.insert(a.course.as_str()) {
            return Err(ProjectError::DuplicateId {
                file: file.to_string(),
                field: format!("assignments[{i}].course"),
                id: a.course.clone(),
            });
        }
    }
    let mut corrections = HashSet::new();
    for (i, n) in p.numbers.iter().enumerate() {
        let at = |k: &str| format!("numbers[{i}].{k}");
        non_empty(file, at("control"), &n.control)?;
        match (&n.layout, &n.course) {
            (Some(_), Some(_)) => {
                return Err(invalid(
                    file,
                    format!("numbers[{i}]"),
                    "set either `layout` or `course`, not both",
                ));
            }
            (None, None) => {
                return Err(invalid(
                    file,
                    format!("numbers[{i}]"),
                    "needs `layout` (all courses on that layout) or `course` (one course)",
                ));
            }
            (Some(l), None) if !layout_ids.contains(l.as_str()) => {
                return Err(invalid(file, at("layout"), &known(l)));
            }
            (None, Some(c)) => non_empty(file, at("course"), c)?,
            _ => {}
        }
        let scope = (n.control.as_str(), n.layout.as_deref(), n.course.as_deref());
        if !corrections.insert(scope) {
            return Err(ProjectError::DuplicateId {
                file: file.to_string(),
                field: format!("numbers[{i}].control"),
                id: n.control.clone(),
            });
        }
        if !n.direction.is_finite() {
            return Err(invalid(file, at("direction"), "must be a finite number"));
        }
        if let Some(g) = n.gap_mm {
            gap_in_range(file, at("gap_mm"), g)?;
        }
        // `base_seen` records the course file's placement as it was, so any finite gap goes: a Purple Pen
        // number that touches its circle has a negative one, a far one more than the corrections allow.
        if let Some(b) = &n.base_seen {
            for (value, field) in [(b.direction, "direction"), (b.gap_mm, "gap_mm")] {
                if !value.is_finite() {
                    return Err(invalid(
                        file,
                        at(&format!("base_seen.{field}")),
                        "must be a finite number",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn gap_in_range(file: &str, field: String, gap_mm: f64) -> Result<(), ProjectError> {
    if gap_mm.is_finite() && gap_mm > 0.0 && gap_mm <= MAX_NUMBER_GAP_MM {
        Ok(())
    } else {
        Err(invalid(
            file,
            field,
            &format!("must be more than 0 and at most {MAX_NUMBER_GAP_MM} mm"),
        ))
    }
}

/// The rule set of `[print] rule_profile` and `rule_overlays`, which must name embedded profiles
/// that load together. The error names the field of the profile that is wrong.
fn rule_set_of(file: &str, print: &PrintSettings) -> Result<RuleSet, ProjectError> {
    use crate::rules::{RuleError, load_rule_set};
    let err = match load_rule_set(&print.rule_profile, &print.rule_overlays) {
        Ok(set) => return Ok(set),
        Err(err) => err,
    };
    let overlay_field = |id: &str| {
        print
            .rule_overlays
            .iter()
            .position(|o| o == id)
            .map(|i| format!("print.rule_overlays[{i}]"))
    };
    let named = match &err {
        // The base profile loads first, so an unknown id that is the base's is the base.
        RuleError::UnknownProfile { id, .. } if id == &print.rule_profile => None,
        RuleError::UnknownProfile { id, .. } | RuleError::NotAnOverlay(id) => overlay_field(id),
        RuleError::OverlayForAnotherBase { overlay, .. }
        | RuleError::UnknownRule { overlay, .. } => overlay_field(overlay),
        RuleError::Malformed { profile, .. } | RuleError::UnknownKey { profile, .. } => {
            overlay_field(profile)
        }
        RuleError::NotABase(_) | RuleError::Incomplete { .. } => None,
    };
    let field = named.unwrap_or_else(|| {
        if print.rule_overlays.is_empty() || matches!(err, RuleError::NotABase(_)) {
            "print.rule_profile".to_string()
        } else {
            "print.rule_overlays".to_string()
        }
    });
    Err(invalid(file, field, &err.to_string()))
}

fn positive(file: &str, field: String, v: f64) -> Result<(), ProjectError> {
    if v.is_finite() && v > 0.0 {
        Ok(())
    } else {
        Err(invalid(file, field, "must be a number larger than 0"))
    }
}

/// `v` must not exceed `max` (in `unit`).
fn at_most(file: &str, field: String, v: f64, max: f64, unit: &str) -> Result<(), ProjectError> {
    if v <= max {
        Ok(())
    } else {
        Err(invalid(
            file,
            field,
            &format!("must be at most {max} {unit}"),
        ))
    }
}

fn non_empty(file: &str, field: String, v: &str) -> Result<(), ProjectError> {
    if v.trim().is_empty() {
        Err(invalid(file, field, "must not be empty"))
    } else {
        Ok(())
    }
}

fn check_element(file: &str, at: &str, e: &ElementSpec) -> Result<(), ProjectError> {
    non_empty(file, format!("{at}.id"), &e.id)?;
    if e.id == crate::sheet::FOOTER_ID {
        return Err(invalid(
            file,
            format!("{at}.id"),
            "`footer` is the name of the default footer in notes; choose another id",
        ));
    }
    for (i, v) in e.anchor.offset_mm.iter().enumerate() {
        let field = format!("{at}.anchor.offset_mm[{i}]");
        if !v.is_finite() {
            return Err(invalid(file, field, "must be a finite number"));
        }
        if v.abs() > MAX_LENGTH_MM {
            return Err(invalid(
                file,
                field,
                "must lie between -2000 and 2000 mm, which reaches past any paper",
            ));
        }
    }
    let kind = kind_name(e.kind);
    let text = matches!(e.kind, ElementKind::TitleBlock | ElementKind::Text);
    let sized = text || matches!(e.kind, ElementKind::CreditLine | ElementKind::CopyrightLine);
    let logo = e.kind == ElementKind::Logo;
    let stray = [
        ("lines", !e.lines.is_empty(), text),
        ("size_pt", e.size_pt.is_some(), sized),
        ("box_white", e.box_white.is_some(), text),
        ("file", e.file.is_some(), logo),
        ("width_mm", e.width_mm.is_some(), logo),
        (
            "cell_mm",
            e.cell_mm.is_some(),
            e.kind == ElementKind::ControlDescriptions,
        ),
    ];
    for (name, present, allowed) in stray {
        if present && !allowed {
            return Err(invalid(
                file,
                format!("{at}.{name}"),
                &format!("not valid for an element of kind `{kind}`"),
            ));
        }
    }
    if let Some(v) = e.size_pt {
        positive(file, format!("{at}.size_pt"), v)?;
        at_most(file, format!("{at}.size_pt"), v, MAX_SIZE_PT, "pt")?;
        if e.kind == ElementKind::CreditLine && v < MIN_CREDIT_PT {
            return Err(invalid(
                file,
                format!("{at}.size_pt"),
                "must be at least 5 for the credit line, so that it stays readable",
            ));
        }
    }
    if let Some(v) = e.cell_mm
        && !(v.is_finite() && (MIN_CELL_MM..=MAX_CELL_MM).contains(&v))
    {
        return Err(invalid(
            file,
            format!("{at}.cell_mm"),
            &format!("must lie between {MIN_CELL_MM} and {MAX_CELL_MM} mm"),
        ));
    }
    if logo {
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
            Some(w) => {
                positive(file, format!("{at}.width_mm"), w)?;
                at_most(file, format!("{at}.width_mm"), w, MAX_LENGTH_MM, "mm")?;
            }
        }
    }
    Ok(())
}

/// The name of an element kind as the project file writes it.
fn kind_name(k: ElementKind) -> &'static str {
    match k {
        ElementKind::TitleBlock => "title_block",
        ElementKind::Text => "text",
        ElementKind::Logo => "logo",
        ElementKind::ScaleCheckBar => "scale_check_bar",
        ElementKind::CreditLine => "credit_line",
        ElementKind::CopyrightLine => "copyright_line",
        ElementKind::ControlDescriptions => "control_descriptions",
    }
}

/// Why `id` cannot be the name of a PDF file `<id>.pdf` on Linux, macOS and Windows; `None` when
/// it can.
fn file_name_problem(id: &str) -> Option<&'static str> {
    const RESERVED: [&str; 22] = [
        "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
        "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
    ];
    if id
        .chars()
        .any(|c| c.is_control() || "/\\:*?\"<>|".contains(c))
    {
        Some(
            "must not contain / \\ : * ? \" < > | or control characters, because it names the PDF file",
        )
    } else if id.starts_with('.') || id.ends_with('.') || id.trim() != id {
        Some(
            "must not start with a dot, end with a dot or start or end with a space, because it names the PDF file",
        )
    } else if id.chars().any(|c| ('\u{300}'..='\u{36f}').contains(&c)) {
        // Without NFC normalisation, `é` as e plus a combining accent would not clash with the
        // one-character `é` in the case check, though macOS treats both names as one file.
        Some(
            "contains a combining accent; write the accented letter as one character, because it names the PDF file",
        )
    } else if RESERVED.contains(&id.to_lowercase().as_str()) {
        Some("is a device name on Windows and cannot name the PDF file")
    } else if id.len() > 200 {
        Some("is longer than 200 bytes, too long for the name of the PDF file")
    } else {
        None
    }
}

fn check_layout(file: &str, i: usize, l: &LayoutSpec) -> Result<(), ProjectError> {
    non_empty(file, format!("layouts[{i}].id"), &l.id)?;
    if let Some(problem) = file_name_problem(&l.id) {
        return Err(invalid(file, format!("layouts[{i}].id"), problem));
    }
    let f = &l.map_frame;
    let margin = format!("layouts[{i}].map_frame.margin_mm");
    positive(file, margin.clone(), f.margin_mm)?;
    // Half the shorter side or more leaves no map frame.
    let (w, h) = l.paper.size_mm();
    let half = w.min(h) / 2.0;
    if f.margin_mm >= half {
        let paper = match l.paper {
            Paper::A4 => "A4",
            Paper::A3 => "A3",
        };
        return Err(invalid(
            file,
            margin,
            &format!(
                "must be less than {half} mm, half the shorter side of {paper}; \
                 otherwise no map frame is left"
            ),
        ));
    }
    let border = format!("layouts[{i}].map_frame.border_mm");
    if !(f.border_mm.is_finite() && f.border_mm >= 0.0) {
        return Err(invalid(file, border, "must be a number of 0 or more"));
    }
    at_most(file, border, f.border_mm, MAX_BORDER_MM, "mm")?;
    if let Some(c) = f.centre_map_mm {
        for (k, v) in c.iter().enumerate() {
            let field = format!("layouts[{i}].map_frame.centre_map_mm[{k}]");
            if !v.is_finite() {
                return Err(invalid(file, field, "must be a finite number"));
            }
            if v.abs() > MAX_CENTRE_MM {
                return Err(invalid(
                    file,
                    field,
                    "must lie between -2 000 000 and 2 000 000 mm, the reach of an OOM map",
                ));
            }
        }
    }
    Ok(())
}

/// A comment for the file [`save_new_with_comments`] writes: `# text` lines in the table `table`
/// (`map`, or `shared` for the first `[[shared]]`), above the line of `key` when the table has
/// it, else at the end of the table, or above the table header when `key` is `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyComment<'a> {
    pub table: &'a str,
    pub key: Option<&'a str>,
    pub text: &'a str,
}

/// Writes a new project file. Refuses to replace an existing file.
pub fn save_new(path: &Path, project: &Project) -> Result<(), ProjectError> {
    save_new_with_comments(path, project, &[])
}

/// Longest comment line [`comment_lines`] writes, `# ` included, unless one word is longer.
const COMMENT_WIDTH: usize = 100;

/// `text` as TOML comment lines: each line of the text wrapped at word boundaries to at most
/// [`COMMENT_WIDTH`] characters, each output line starting with `# `; an empty line becomes `#`.
fn comment_lines(text: &str) -> String {
    let mut out = String::new();
    for l in text.lines() {
        let mut line = String::new();
        for word in l.split_whitespace() {
            if !line.is_empty()
                && 2 + line.chars().count() + 1 + word.chars().count() > COMMENT_WIDTH
            {
                let _ = writeln!(out, "# {line}");
                line.clear();
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        if line.is_empty() {
            out.push_str("#\n");
        } else {
            let _ = writeln!(out, "# {line}");
        }
    }
    out
}

/// The table a header line opens: `map` for `[map]`, `shared` for `[[shared]]`, `shared.anchor`
/// for `[shared.anchor]`; `None` for any other line.
fn header_name(line: &str) -> Option<&str> {
    let t = line.trim();
    let inner = t
        .strip_prefix("[[")
        .and_then(|r| r.strip_suffix("]]"))
        .or_else(|| t.strip_prefix('[').and_then(|r| r.strip_suffix(']')))?;
    Some(inner.trim())
}

/// `body` (TOML from the serializer: one key per line, tables in order) with `comments` added.
fn add_comments(body: &str, comments: &[KeyComment<'_>]) -> String {
    let mut placed = vec![false; comments.len()];
    let mut out = String::new();
    let mut table = "";
    let flush_table_end = |table: &str, placed: &mut Vec<bool>, out: &mut String| {
        for (c, done) in comments.iter().zip(placed.iter_mut()) {
            if !*done && c.table == table && c.key.is_some() {
                out.push_str(&comment_lines(c.text));
                *done = true;
            }
        }
    };
    for line in body.lines() {
        if let Some(name) = header_name(line) {
            // A blank line separates tables; the end-of-table comment goes before it.
            let trailing_blank = out.ends_with("\n\n");
            if trailing_blank {
                out.pop();
            }
            flush_table_end(table, &mut placed, &mut out);
            if trailing_blank {
                out.push('\n');
            }
            table = name;
            for (c, done) in comments.iter().zip(placed.iter_mut()) {
                if !*done && c.table == name && c.key.is_none() {
                    out.push_str(&comment_lines(c.text));
                    *done = true;
                }
            }
        } else if let Some((key, _)) = line.split_once('=') {
            for (c, done) in comments.iter().zip(placed.iter_mut()) {
                if !*done && c.table == table && c.key == Some(key.trim()) {
                    out.push_str(&comment_lines(c.text));
                    *done = true;
                }
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    flush_table_end(table, &mut placed, &mut out);
    out
}

/// [`save_new`] with comments for the reader of the file, for example a value to confirm.
pub fn save_new_with_comments(
    path: &Path,
    project: &Project,
    comments: &[KeyComment<'_>],
) -> Result<(), ProjectError> {
    let file = path.display().to_string();
    let body =
        toml::to_string_pretty(project).map_err(|e| ProjectError::Serialize(e.to_string()))?;
    let mut text = String::new();
    let _ = writeln!(
        text,
        "# freemapper-print event project file, format {FORMAT}"
    );
    let _ = writeln!(text, "# Paths are relative to this file.\n");
    // The comment scanner reads the serializer's output line by line. A multi-line string (a
    // title with a line break) could fool it into putting a comment inside the string, so the
    // result must read back as the same project; otherwise the file goes without comments.
    let commented = add_comments(&body, comments);
    let same = toml::from_str::<Project>(&commented).is_ok_and(|back| &back == project);
    text.push_str(if same { &commented } else { &body });
    use std::io::Write as _;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| {
            if source.kind() == std::io::ErrorKind::AlreadyExists {
                ProjectError::Exists { file: file.clone() }
            } else {
                ProjectError::Write {
                    file: file.clone(),
                    source,
                }
            }
        })?;
    f.write_all(text.as_bytes())
        .map_err(|source| ProjectError::Write { file, source })
}

/// Writes `bytes` to `path` through a temporary file next to it and a rename, so that a failed write leaves the
/// old file whole. The temporary file is removed again on failure.
fn replace_file(path: &Path, bytes: &[u8]) -> Result<(), ProjectError> {
    let file = path.display().to_string();
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".part");
    let tmp = PathBuf::from(tmp);
    let written = std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, path));
    written.map_err(|source| {
        let _ = std::fs::remove_file(&tmp);
        ProjectError::Write { file, source }
    })
}

/// Appends `correction` as a `[[numbers]]` table to the project file at `path` and loads the result (ADR 0025,
/// addendum). Everything before the new table stays byte for byte as it was, comments included. The file is
/// replaced through a temporary file. When the result does not load (for example because `numbers` is written
/// as an inline array), the old content is put back and the load error is returned.
pub fn append_correction(
    path: &Path,
    correction: &NumberCorrection,
) -> Result<LoadedProject, ProjectError> {
    #[derive(Serialize)]
    struct Wrapper<'a> {
        numbers: [&'a NumberCorrection; 1],
    }
    let file = path.display().to_string();
    let old = std::fs::read(path).map_err(|source| ProjectError::Io {
        file: file.clone(),
        source,
    })?;
    let table = toml::to_string(&Wrapper {
        numbers: [correction],
    })
    .map_err(|e| ProjectError::Serialize(e.to_string()))?;
    let mut new = old.clone();
    if !new.ends_with(b"\n") {
        new.push(b'\n');
    }
    new.push(b'\n');
    new.extend_from_slice(table.as_bytes());
    replace_file(path, &new)?;
    load(path).inspect_err(|_| {
        // Best effort: the load error is the one to report.
        let _ = replace_file(path, &old);
    })
}

/// `to` relative to the directory `from`, both absolute: `../maps/m.omap`, with `/` between the
/// parts on every system, so that the project file moves between Windows, macOS and Linux. `to`
/// itself when the two share no root (another drive on Windows).
pub fn relative_path(from: &Path, to: &Path) -> PathBuf {
    let from: Vec<_> = from.components().collect();
    let to_parts: Vec<_> = to.components().collect();
    let common = from
        .iter()
        .zip(&to_parts)
        .take_while(|(a, b)| a == b)
        .count();
    if common == 0 {
        return to.to_path_buf();
    }
    let parts: Vec<String> = std::iter::repeat_n("..".to_string(), from.len() - common)
        .chain(
            to_parts[common..]
                .iter()
                .map(|p| p.as_os_str().to_string_lossy().into_owned()),
        )
        .collect();
    PathBuf::from(parts.join("/"))
}

/// The layout id for `paper`: `a4`, `a3`.
pub fn starter_layout_id(paper: Paper) -> &'static str {
    match paper {
        Paper::A4 => "a4",
        Paper::A3 => "a3",
    }
}

/// The project `init` starts from: `title`, the map at `map` (relative to the project file) of
/// `kind`, the contour interval [`MapKind::proposed_contour_interval_m`] proposes, and one A4
/// layout at the map scale `map_scale` with two shared elements; [`fit_starter`] then picks paper
/// and passe-partout. The title block, with a white box in the
/// top-left corner of the map frame, shows the event and `1:{print_scale} · {contour_interval}`.
/// The scale check bar sits in the bottom-right corner of the frame. The copyright and credit
/// lines are left to the default footer, which moves inside the frame when the passe-partout is
/// too narrow for them; an element hung on the page could only stay put and cover the map.
pub fn starter(
    title: &str,
    map: PathBuf,
    kind: MapKind,
    map_scale: u32,
    lang: SheetLang,
) -> Project {
    let element = |id: &str, kind, corner| ElementSpec {
        id: id.into(),
        kind,
        anchor: Anchor {
            to: AnchorTarget::MapFrame,
            corner,
            offset_mm: [2.0, 2.0],
        },
        lines: vec![],
        size_pt: None,
        box_white: None,
        file: None,
        width_mm: None,
        cell_mm: None,
    };
    let mut title_block = element("title", ElementKind::TitleBlock, Corner::TopLeft);
    title_block.lines = vec![
        "{event}".into(),
        "1:{print_scale} \u{b7} {contour_interval}".into(),
    ];
    Project {
        format: FORMAT,
        event: Event {
            title: title.into(),
            date: None,
            organiser: None,
            lang,
        },
        sources: Sources::new(map),
        map: MapSettings {
            kind,
            contour_interval_m: kind.proposed_contour_interval_m(),
        },
        print: PrintSettings::default(),
        shared: vec![
            title_block,
            element(
                "scale_check_bar",
                ElementKind::ScaleCheckBar,
                Corner::BottomRight,
            ),
        ],
        layouts: vec![LayoutSpec {
            id: starter_layout_id(Paper::A4).into(),
            paper: Paper::A4,
            orientation: Orientation::Auto,
            print_scale: Some(map_scale),
            map_frame: MapFrameSpec::default(),
            north_lines: NorthLines::Auto,
            hide_parts: vec![],
            elements: vec![],
            hide_shared: vec![],
        }],
        assignments: Vec::new(),
        numbers: Vec::new(),
    }
}

/// Widest passe-partout (mm) [`fit_starter`] tries to make room for the footer.
const MAX_STARTER_MARGIN_MM: u32 = 24;
/// Steps (mm) in which [`fit_starter`] widens the passe-partout.
const STARTER_MARGIN_STEP_MM: usize = 2;

/// Whether the starter sheet in `report` is ready: no note but added north lines, and the missing
/// contour interval value when the starter proposes none (schoolyard).
fn starter_is_clean(project: &Project, report: &crate::sheet::SheetReport) -> bool {
    use crate::sheet::SheetNote;
    !report.print.has_notes()
        && report.notes.iter().all(|n| match n {
            SheetNote::NorthLinesAdded { .. } => true,
            SheetNote::MissingScaleOrContour { missing, .. } => {
                project.map.contour_interval_m.is_none()
                    && missing
                        .iter()
                        .all(|m| *m == crate::sheet::MissingItem::ContourValue)
            }
            _ => false,
        })
}

/// Whether every note that keeps `report` from being clean is about the default footer, so a
/// wider passe-partout could help.
fn only_footer_in_the_way(project: &Project, report: &crate::sheet::SheetReport) -> bool {
    use crate::sheet::SheetNote;
    let mut rest = report.clone();
    rest.notes.retain(|n| {
        !matches!(
            n,
            SheetNote::FooterOverlapsMap { .. }
                | SheetNote::FooterOutsidePage { .. }
                | SheetNote::FooterCoversElement { .. }
        )
    });
    rest.notes.len() < report.notes.len() && starter_is_clean(project, &rest)
}

/// The starter on the first sheet that shows `map` without notes (only added north lines and,
/// for schoolyard maps, the contour interval left to the user may remain): A4 with the default
/// 8 mm passe-partout, A4 with a passe-partout wide enough for the footer below the map frame,
/// then A3 the same way; always with auto orientation and through the same drawing as `check`.
/// `dir` is the project folder. The second value is false when no candidate fits: the project
/// then keeps A4 with 8 mm, and `check` will list its notes.
///
/// When only the footer is in the way at 8 mm, the widest passe-partout (24 mm) is tried next:
/// if the footer is still in the way there, no width helps and the next paper follows. Otherwise
/// the passe-partout grows from 10 mm in 2 mm steps to the first that fits, at most ten sheets
/// per paper.
pub fn fit_starter(
    starter: Project,
    dir: &Path,
    map: &crate::omap::MapDocument,
) -> (Project, bool) {
    let logos = crate::sheet::LogoCache::new();
    fit_with(starter, dir, |candidate| {
        let layout = candidate.project.layouts[0].clone();
        crate::export::sheet_pdf(candidate, map, &layout, &logos)
            .ok()
            .map(|(_, report)| report)
    })
}

/// [`fit_starter`] with the sheet check `check` (`None`: the sheet cannot be drawn).
fn fit_with(
    starter: Project,
    dir: &Path,
    mut check: impl FnMut(&LoadedProject) -> Option<crate::sheet::SheetReport>,
) -> (Project, bool) {
    let mut candidate = LoadedProject {
        path: dir.join(FILE_NAME),
        dir: dir.to_path_buf(),
        project: starter,
        // A starter names the default profile.
        rules: crate::rules::base().clone(),
    };
    // Clean, only the footer in the way, or something else.
    enum Outcome {
        Clean,
        Footer,
        Other,
    }
    let mut try_sheet = |candidate: &mut LoadedProject, paper: Paper, margin: u32| {
        let layout = &mut candidate.project.layouts[0];
        layout.id = starter_layout_id(paper).into();
        layout.paper = paper;
        layout.orientation = Orientation::Auto;
        layout.map_frame.margin_mm = f64::from(margin);
        match check(candidate) {
            Some(r) if starter_is_clean(&candidate.project, &r) => Outcome::Clean,
            // A wider passe-partout only shrinks the frame, so it helps the footer alone.
            Some(r) if only_footer_in_the_way(&candidate.project, &r) => Outcome::Footer,
            _ => Outcome::Other,
        }
    };
    let first = MapFrameSpec::default().margin_mm as u32;
    for paper in [Paper::A4, Paper::A3] {
        match try_sheet(&mut candidate, paper, first) {
            Outcome::Clean => return (candidate.project, true),
            Outcome::Other => continue,
            Outcome::Footer => {}
        }
        if let Outcome::Footer = try_sheet(&mut candidate, paper, MAX_STARTER_MARGIN_MM) {
            continue;
        }
        let wider = (first + 2..=MAX_STARTER_MARGIN_MM).step_by(STARTER_MARGIN_STEP_MM);
        for margin in wider {
            match try_sheet(&mut candidate, paper, margin) {
                Outcome::Clean => return (candidate.project, true),
                Outcome::Footer => {}
                Outcome::Other => break,
            }
        }
    }
    let layout = &mut candidate.project.layouts[0];
    layout.id = starter_layout_id(Paper::A4).into();
    layout.paper = Paper::A4;
    layout.map_frame.margin_mm = MapFrameSpec::default().margin_mm;
    (candidate.project, false)
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
        assert_eq!(lp.project.print, PrintSettings::default());
        assert_eq!(l.north_lines, NorthLines::Auto);
        assert_eq!(l.paper, Paper::A4);
        assert_eq!(l.orientation, Orientation::Auto);
    }

    #[test]
    fn rule_profile_defaults_to_iof_print() {
        let lp = load(&minimal_path()).unwrap();
        assert_eq!(lp.project.print.rule_profile, "iof-print");
        assert!(lp.project.print.rule_overlays.is_empty());
        assert!(lp.project.print.is_empty());
        // `load` loads the named set once; the default is the base profile alone.
        assert_eq!(&lp.rules, crate::rules::base());
        // Written out as the default, both are named; the file loads the same.
        let (_d, path) = temp_project(&format!(
            "{HEAD}[print]\nrule_profile = \"iof-print\"\nrule_overlays = []\n[[layouts]]\nid = \"a\"\n"
        ));
        let print = load(&path).unwrap().project.print;
        assert_eq!(print, PrintSettings::default());
        // Only values other than the default are written, so the committed example writes as before.
        assert_eq!(toml::to_string(&PrintSettings::default()).unwrap(), "");
        let example = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/luebeck-competition-layouts")
            .join(FILE_NAME);
        let written = toml::to_string(&load(&example).unwrap().project).unwrap();
        assert!(!written.contains("rule_"), "{written}");
        let other = PrintSettings {
            rule_profile: "x-print".into(),
            rule_overlays: vec!["y".into()],
            ..PrintSettings::default()
        };
        assert_eq!(
            toml::to_string(&other).unwrap(),
            "rule_profile = \"x-print\"\nrule_overlays = [\"y\"]\n"
        );
    }

    #[test]
    fn unknown_rule_profile_names_its_line() {
        let (_d, path) = temp_project(&format!(
            "{HEAD}[print]\nrule_profile = \"iof-2099\"\n[[layouts]]\nid = \"a\"\n"
        ));
        let msg = load(&path).unwrap_err().to_string();
        assert!(
            msg.contains(&format!("{FILE_NAME}:9:16: field `print.rule_profile`"))
                && msg.contains("iof-2099")
                && msg.contains("available: iof-print"),
            "{msg}"
        );
        let (_d, path) = temp_project(&format!(
            "{HEAD}[print]\nrule_overlays = [\"iof-print\", \"nor-bb\"]\n[[layouts]]\nid = \"a\"\n"
        ));
        let msg = load(&path).unwrap_err().to_string();
        // The first overlay is the base profile itself, not an overlay.
        assert!(
            msg.contains(":9:18: field `print.rule_overlays[0]`") && msg.contains("not an overlay"),
            "{msg}"
        );
        let (_d, path) = temp_project(&format!(
            "{HEAD}[print]\nrule_overlays = [\n  \"nor-bb\",\n]\n[[layouts]]\nid = \"a\"\n"
        ));
        let msg = load(&path).unwrap_err().to_string();
        assert!(
            msg.contains(":10:3: field `print.rule_overlays[0]`") && msg.contains("nor-bb"),
            "{msg}"
        );
    }

    #[test]
    fn credit_line_switch_is_gone() {
        let (_d, path) = temp_project(&format!(
            "{HEAD}[print]\ncredit_line = false\n[[layouts]]\nid = \"a\"\n"
        ));
        let msg = load(&path).unwrap_err().to_string();
        assert!(msg.contains(":9:1") && msg.contains("credit_line"), "{msg}");
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

    /// Loads a project with one shared element given as TOML lines and one layout; returns the error text.
    fn err_for(extra_head: &str, shared: &str, layout: &str) -> String {
        let head = HEAD.replace(
            "kind = \"forest\"\n",
            &format!("kind = \"forest\"\n{extra_head}"),
        );
        let (_d, path) = temp_project(&format!(
            "{head}\n[[layouts]]\nid = \"a\"\n{layout}\n{shared}"
        ));
        load(&path).unwrap_err().to_string()
    }

    fn shared(kind: &str, fields: &str) -> String {
        format!(
            "[[shared]]\nid=\"x\"\nkind=\"{kind}\"\n{fields}\n[shared.anchor]\nto=\"page\"\ncorner=\"top\"\n"
        )
    }

    #[test]
    fn fields_must_fit_the_kind() {
        for (kind, field, line) in [
            ("text", "width_mm", "width_mm = 5.0"),
            ("text", "file", "file = \"a.png\""),
            ("logo", "lines", "lines = [\"a\"]"),
            ("logo", "size_pt", "size_pt = 9.0"),
            ("logo", "box_white", "box_white = false"),
            ("scale_check_bar", "size_pt", "size_pt = 9.0"),
            ("scale_check_bar", "lines", "lines = [\"a\"]"),
            ("credit_line", "lines", "lines = [\"a\"]"),
            ("copyright_line", "box_white", "box_white = true"),
            ("title_block", "width_mm", "width_mm = 5.0"),
            ("text", "cell_mm", "cell_mm = 6.0"),
            ("control_descriptions", "size_pt", "size_pt = 9.0"),
            ("control_descriptions", "lines", "lines = [\"a\"]"),
        ] {
            let msg = err_for("", &shared(kind, line), "");
            assert!(
                msg.contains(&format!("shared[0].{field}"))
                    && msg.contains(kind)
                    && msg.contains(FILE_NAME),
                "{kind} {field}: {msg}"
            );
        }
    }

    #[test]
    fn description_cell_size_is_checked() {
        for bad in ["3.9", "10.5", "nan", "-6.0"] {
            let msg = err_for(
                "",
                &shared("control_descriptions", &format!("cell_mm = {bad}")),
                "",
            );
            assert!(
                msg.contains("shared[0].cell_mm")
                    && msg.contains("between 4 and 10 mm")
                    && msg.contains(&format!("{FILE_NAME}:")),
                "{bad}: {msg}"
            );
        }
        for good in ["4.0", "6.0", "10.0"] {
            let (_d, path) = temp_project(&format!(
                "{HEAD}\n[[layouts]]\nid = \"a\"\n{}",
                shared("control_descriptions", &format!("cell_mm = {good}"))
            ));
            let p = load(&path).unwrap();
            assert_eq!(p.project.shared[0].kind, ElementKind::ControlDescriptions);
            assert_eq!(p.project.shared[0].cell_mm, Some(good.parse().unwrap()));
        }
        // Without cell_mm the rule profile decides.
        let (_d, path) = temp_project(&format!(
            "{HEAD}\n[[layouts]]\nid = \"a\"\n{}",
            shared("control_descriptions", "")
        ));
        assert_eq!(load(&path).unwrap().project.shared[0].cell_mm, None);
    }

    #[test]
    fn numbers_are_checked() {
        let cases: [(&str, String, &str); 9] = [
            ("", shared("text", "size_pt = 0.0"), "shared[0].size_pt"),
            ("", shared("text", "size_pt = nan"), "shared[0].size_pt"),
            (
                "",
                shared("logo", "file=\"a.png\"\nwidth_mm = -1.0"),
                "shared[0].width_mm",
            ),
            (
                "",
                shared("text", "")
                    .replace("corner=\"top\"", "corner=\"top\"\noffset_mm=[1.0, inf]"),
                "shared[0].anchor.offset_mm[1]",
            ),
            (
                "contour_interval_m = 0.0\n",
                shared("scale_check_bar", ""),
                "map.contour_interval_m",
            ),
            (
                "",
                shared("text", "").replace("id=\"x\"", "id=\"\""),
                "shared[0].id",
            ),
            ("", String::new(), "layouts[0].map_frame.margin_mm"),
            ("", String::new(), "layouts[0].map_frame.border_mm"),
            ("", String::new(), "layouts[0].id"),
        ];
        for (i, (head, sh, field)) in cases.iter().enumerate() {
            let layout = match i {
                6 => "[layouts.map_frame]\nmargin_mm = 0.0",
                7 => "[layouts.map_frame]\nborder_mm = -0.1",
                _ => "",
            };
            let msg = if i == 8 {
                let (_d, path) = temp_project(&format!("{HEAD}\n[[layouts]]\nid = \"\"\n"));
                load(&path).unwrap_err().to_string()
            } else {
                err_for(head, sh, layout)
            };
            assert!(
                msg.contains(field) && msg.contains(FILE_NAME),
                "{field}: {msg}"
            );
        }
    }

    #[test]
    fn print_scale_zero_is_refused() {
        let msg = err_for("", String::new().as_str(), "print_scale = 0");
        assert!(
            msg.contains("layouts[0].print_scale") && msg.contains("larger than 0"),
            "{msg}"
        );
    }

    #[test]
    fn checked_values_name_their_line() {
        // HEAD has 7 lines, then a blank one and the layout.
        let msg = err_for(
            "",
            &shared("text", "size_pt = 0.0"),
            "[layouts.map_frame]\nborder_mm = -1.0",
        );
        // The layout comes first: its border is the first error, on line 12, column 13.
        assert!(
            msg.contains(&format!(
                "{FILE_NAME}:12:13: field `layouts[0].map_frame.border_mm`"
            )),
            "{msg}"
        );
        let msg = err_for("", &shared("text", "size_pt = 0.0"), "");
        assert!(
            msg.contains(&format!("{FILE_NAME}:15:11: field `shared[0].size_pt`")),
            "{msg}"
        );
        // An inline value in an array: the item itself.
        let msg = err_for(
            "",
            &shared("text", "")
                .replace("corner=\"top\"", "corner=\"top\"\noffset_mm=[1.0, 3000.0]"),
            "",
        );
        assert!(
            msg.contains(":19:17: field `shared[0].anchor.offset_mm[1]`"),
            "{msg}"
        );
        // A missing field points at the table that lacks it.
        let msg = err_for("", &shared("logo", "width_mm = 5.0"), "");
        assert!(msg.contains(":12:1: field `shared[0].file`"), "{msg}");
    }

    #[test]
    fn upper_bounds_are_checked() {
        let offset = |v: &str| {
            shared("text", "").replace(
                "corner=\"top\"",
                &format!("corner=\"top\"\noffset_mm=[{v}, 0.0]"),
            )
        };
        // (head, shared element, layout lines, field, words of the message)
        let cases: Vec<(&str, String, &str, &str, &str)> = vec![
            (
                "",
                shared("text", "size_pt = 200.5"),
                "",
                "shared[0].size_pt",
                "200",
            ),
            (
                "",
                shared("logo", "file=\"a.png\"\nwidth_mm = 2000.5"),
                "",
                "shared[0].width_mm",
                "2000",
            ),
            (
                "",
                offset("-2000.5"),
                "",
                "shared[0].anchor.offset_mm[0]",
                "2000",
            ),
            (
                "",
                String::new(),
                "[layouts.map_frame]\nborder_mm = 20.5",
                "layouts[0].map_frame.border_mm",
                "20",
            ),
            (
                "",
                String::new(),
                "[layouts.map_frame]\nmargin_mm = 105.0",
                "layouts[0].map_frame.margin_mm",
                "105",
            ),
            (
                "",
                String::new(),
                "paper = \"A3\"\n[layouts.map_frame]\nmargin_mm = 148.5",
                "layouts[0].map_frame.margin_mm",
                "148.5",
            ),
            (
                "contour_interval_m = 100.5\n",
                String::new(),
                "",
                "map.contour_interval_m",
                "100",
            ),
            (
                "",
                String::new(),
                "[layouts.map_frame]\ncentre_map_mm = [0.0, -2000000.5]",
                "layouts[0].map_frame.centre_map_mm[1]",
                "2 000 000",
            ),
        ];
        for (head, sh, layout, field, words) in cases {
            let msg = err_for(head, &sh, layout);
            assert!(
                msg.contains(field) && msg.contains(words) && msg.contains(FILE_NAME),
                "{field}: {msg}"
            );
        }
        // The bounds themselves are fine.
        let ok = format!(
            "{HEAD}contour_interval_m = 100.0\n\n[[layouts]]\nid = \"a\"\n\
             [layouts.map_frame]\nmargin_mm = 104.9\nborder_mm = 20.0\n\
             centre_map_mm = [2000000.0, -2000000.0]\n{}",
            shared("text", "size_pt = 200.0").replace(
                "corner=\"top\"",
                "corner=\"top\"\noffset_mm=[2000.0, -2000.0]"
            )
        );
        let (_d, path) = temp_project(&ok);
        load(&path).unwrap();
    }

    #[test]
    fn credit_line_at_least_five_pt() {
        let msg = err_for("", &shared("credit_line", "size_pt = 4.9"), "");
        assert!(
            msg.contains("shared[0].size_pt") && msg.contains("at least 5"),
            "{msg}"
        );
        assert!(msg.contains(FILE_NAME), "{msg}");
        let (_d, path) = temp_project(&format!(
            "{HEAD}\n[[layouts]]\nid = \"a\"\n{}",
            shared("credit_line", "size_pt = 5.0")
        ));
        assert!(load(&path).is_ok());
    }

    #[test]
    fn border_zero_and_empty_layout_elements_are_fine() {
        let (_d, path) = temp_project(&format!(
            "{HEAD}\n[[layouts]]\nid = \"a\"\n[layouts.map_frame]\nborder_mm = 0.0\n"
        ));
        assert_eq!(
            load(&path).unwrap().project.layouts[0].map_frame.border_mm,
            0.0
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
            box_white: None,
            file: None,
            width_mm: None,
            cell_mm: None,
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
            sources: Sources::new("m.omap".into()),
            map: MapSettings {
                kind: MapKind::Forest,
                contour_interval_m: Some(5.0),
            },
            print: PrintSettings {
                copyright: Some("(c) OC".into()),
                ..PrintSettings::default()
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
                        crop: true,
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
            assignments: vec![Assignment {
                course: "Men".into(),
                layout: "a4".into(),
            }],
            numbers: vec![NumberCorrection {
                control: "31".into(),
                layout: None,
                course: Some("Men".into()),
                direction: 45.0,
                gap_mm: Some(1.5),
                base_seen: Some(NumberPlacement {
                    direction: 90.0,
                    gap_mm: 1.0,
                }),
            }],
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
    fn layout_id_must_name_a_file() {
        for id in [
            "a/b", "a\\\\b", "e\u{301}", "..", ".hidden", "end.", " a", "a:b", "x*", "CON", "lpt1",
            "a\\u0001",
        ] {
            let (_d, path) = temp_project(&format!("{HEAD}\n[[layouts]]\nid = \"{id}\"\n"));
            let msg = load(&path).unwrap_err().to_string();
            assert!(msg.contains("layouts[0].id"), "{id}: {msg}");
        }
        let long = "x".repeat(201);
        let (_d, path) = temp_project(&format!("{HEAD}\n[[layouts]]\nid = \"{long}\"\n"));
        assert!(load(&path).is_err());
        for id in ["A4 Sprint", "kurz-1", "Läufer_2", "con1"] {
            let (_d, path) = temp_project(&format!("{HEAD}\n[[layouts]]\nid = \"{id}\"\n"));
            assert!(load(&path).is_ok(), "{id}");
        }
    }

    #[test]
    fn comments_that_would_break_the_file_are_dropped() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("m.omap"), "x").unwrap();
        let path = dir.path().join(FILE_NAME);
        // A title whose lines look like a table header and a key fools the line scanner.
        let p = starter(
            "Lauf\n[map]\ncontour_interval_m = 9",
            "m.omap".into(),
            MapKind::Sprint,
            4000,
            SheetLang::De,
        );
        let comments = [KeyComment {
            table: "map",
            key: Some("contour_interval_m"),
            text: "confirm",
        }];
        save_new_with_comments(&path, &p, &comments).unwrap();
        assert_eq!(load(&path).unwrap().project, p);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("# confirm"), "{text}");
        // A plain title keeps the comment.
        std::fs::remove_file(&path).unwrap();
        let p = starter(
            "Lauf",
            "m.omap".into(),
            MapKind::Sprint,
            4000,
            SheetLang::De,
        );
        save_new_with_comments(&path, &p, &comments).unwrap();
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("# confirm")
        );
    }

    #[test]
    fn starter_fits_luebeck_on_a3() {
        let map_path = fixture_dir().join("../../maps/luebeck.omap");
        let map = crate::omap::read::read_map_file(&map_path).unwrap();
        let p = starter("T", map_path.clone(), MapKind::Sprint, 4000, SheetLang::De);
        let (fitted, fits) = fit_starter(p, &fixture_dir(), &map);
        assert!(fits);
        let l = &fitted.layouts[0];
        assert_eq!((l.id.as_str(), l.paper), ("a3", Paper::A3));
        assert!(l.map_frame.margin_mm > 8.0, "{}", l.map_frame.margin_mm);
        // The forest sample fits A4 as it is.
        let map_path = fixture_dir().join("../../maps/forest-sample.omap");
        let map = crate::omap::read::read_map_file(&map_path).unwrap();
        let p = starter("T", map_path, MapKind::Forest, 10000, SheetLang::De);
        let (fitted, fits) = fit_starter(p, &fixture_dir(), &map);
        assert!(fits);
        let l = &fitted.layouts[0];
        assert_eq!((l.paper, l.map_frame.margin_mm), (Paper::A4, 8.0));
    }

    /// A sheet report with the given notes and nothing else to say.
    fn report_with(notes: Vec<crate::sheet::SheetNote>) -> crate::sheet::SheetReport {
        crate::sheet::SheetReport {
            print: crate::print::PrintReport {
                page_mm: (210.0, 297.0),
                print_scale: 4000,
                geometry_factor: 1.0,
                symbol_factor: 1.0,
                non_standard_scale: false,
                overflow_mm: None,
                skipped_objects: 0,
                unknown_type_objects: 0,
                truncated_objects: 0,
                objects_not_drawn_page_limit: 0,
                dropped_pattern_objects: 0,
                substituted_fonts: vec![],
                missing_glyphs: vec![],
            },
            notes,
        }
    }

    /// The sheets [`fit_with`] tries for a map whose footer needs `needs` mm of passe-partout on
    /// each paper (`None`: no margin helps), as (paper, margin) in order, and its result.
    fn fit_trace(needs: [Option<u32>; 2]) -> (Vec<(Paper, u32)>, Project, bool) {
        use crate::sheet::{FooterContent, SheetNote};
        let p = starter(
            "T",
            PathBuf::from("m.omap"),
            MapKind::Sprint,
            4000,
            SheetLang::De,
        );
        let mut tried = Vec::new();
        let (project, fits) = fit_with(p, Path::new("/nowhere"), |lp| {
            let l = &lp.project.layouts[0];
            let margin = l.map_frame.margin_mm as u32;
            tried.push((l.paper, margin));
            let need = needs[usize::from(l.paper == Paper::A3)];
            let footer = SheetNote::FooterOverlapsMap {
                content: FooterContent::CopyrightAndCredit,
            };
            Some(report_with(match need {
                Some(n) if margin >= n => vec![],
                _ => vec![footer],
            }))
        });
        (tried, project, fits)
    }

    #[test]
    fn fit_search_steps_2_mm_and_stops_early() {
        // The footer fits A4 from 13 mm on: 8, then the widest (24) to see that some margin
        // helps, then up from 10 in 2 mm steps to the first that fits.
        let (tried, project, fits) = fit_trace([Some(13), Some(9)]);
        let a4 = |m| (Paper::A4, m);
        assert_eq!(tried, [a4(8), a4(24), a4(10), a4(12), a4(14)]);
        assert!(fits);
        assert_eq!(project.layouts[0].map_frame.margin_mm, 14.0);
        // No margin helps on A4: after 8 and 24 the search moves on to A3.
        let (tried, project, fits) = fit_trace([None, Some(9)]);
        let a3 = |m| (Paper::A3, m);
        assert_eq!(tried, [a4(8), a4(24), a3(8), a3(24), a3(10)]);
        assert!(fits);
        assert_eq!(
            (
                project.layouts[0].paper,
                project.layouts[0].map_frame.margin_mm
            ),
            (Paper::A3, 10.0)
        );
        // Nothing fits: four sheets, then A4 with 8 mm.
        let (tried, project, fits) = fit_trace([None, None]);
        assert_eq!(tried, [a4(8), a4(24), a3(8), a3(24)]);
        assert!(!fits);
        assert_eq!(
            (
                project.layouts[0].paper,
                project.layouts[0].map_frame.margin_mm
            ),
            (Paper::A4, 8.0)
        );
    }

    #[test]
    fn starter_without_fit_keeps_a4() {
        let map_path = fixture_dir().join("../../maps/luebeck.omap");
        let map = crate::omap::read::read_map_file(&map_path).unwrap();
        // 1:5000 is no standard sprint scale: a note on every paper.
        let p = starter("T", map_path, MapKind::Sprint, 5000, SheetLang::De);
        let (fitted, fits) = fit_starter(p, &fixture_dir(), &map);
        assert!(!fits);
        let l = &fitted.layouts[0];
        assert_eq!(
            (l.id.as_str(), l.paper, l.map_frame.margin_mm),
            ("a4", Paper::A4, 8.0)
        );
    }

    #[test]
    fn layout_ids_differ_in_more_than_case() {
        let (_d, path) = temp_project(&format!(
            "{HEAD}\n[[layouts]]\nid = \"a4\"\n[[layouts]]\nid = \"A4\"\n"
        ));
        let msg = load(&path).unwrap_err().to_string();
        assert!(
            msg.contains("layouts[1].id") && msg.contains("`a4`") && msg.contains("case"),
            "{msg}"
        );
    }

    #[test]
    fn footer_is_no_element_id() {
        let msg = err_for(
            "",
            &shared("text", "").replace("id=\"x\"", "id=\"footer\""),
            "",
        );
        assert!(
            msg.contains("shared[0].id") && msg.contains("footer"),
            "{msg}"
        );
    }

    #[test]
    fn relative_paths() {
        let rel = |a: &str, b: &str| relative_path(Path::new(a), Path::new(b));
        assert_eq!(
            rel("/x/event", "/x/maps/m.omap"),
            Path::new("../maps/m.omap")
        );
        assert_eq!(rel("/x/event", "/x/event/m.omap"), Path::new("m.omap"));
        assert_eq!(rel("/x/a/b", "/y/m.omap"), Path::new("../../../y/m.omap"));
        // `/` on every system, also where the native separator is `\`.
        let text = rel("/x/event", "/x/maps/sub/m.omap");
        assert_eq!(text.to_str(), Some("../maps/sub/m.omap"));
    }

    #[test]
    fn long_comments_wrap_at_100_characters() {
        let text = "word ".repeat(60);
        let out = comment_lines(&format!("{text}\n\nshort"));
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines.len() > 3, "{out}");
        assert!(lines.iter().all(|l| l.chars().count() <= 100), "{out}");
        assert!(lines.iter().all(|l| l.starts_with('#')), "{out}");
        // The words survive in order, the blank line too.
        let words: Vec<&str> = lines
            .iter()
            .flat_map(|l| l.trim_start_matches('#').split_whitespace())
            .collect();
        assert_eq!(words.len(), 61);
        assert!(lines.contains(&"#") && lines.last() == Some(&"# short"));
        // A word longer than a line stays whole.
        let long = "x".repeat(150);
        assert_eq!(comment_lines(&long), format!("# {long}\n"));
    }

    #[test]
    fn comments_go_above_their_key_or_table() {
        let body = "a = 1\n\n[map]\nkind = \"x\"\nv = 2\n\n[[shared]]\nid = \"t\"\n";
        let c = |table, key, text| KeyComment { table, key, text };
        let out = add_comments(
            body,
            &[
                c("map", Some("v"), "about v\nsecond line"),
                c("shared", None, "about shared"),
                c("map", Some("missing"), "at the end of map"),
            ],
        );
        assert_eq!(
            out,
            "a = 1\n\n[map]\nkind = \"x\"\n# about v\n# second line\nv = 2\n\
             # at the end of map\n\n# about shared\n[[shared]]\nid = \"t\"\n"
        );
        let toml_ok: toml::Table = toml::from_str(&out).unwrap();
        assert!(toml_ok.contains_key("map"));
    }

    #[test]
    fn starter_project_loads_and_omits_empty_tables() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("m.omap"), "x").unwrap();
        let path = dir.path().join(FILE_NAME);
        let p = starter(
            "Event",
            "m.omap".into(),
            MapKind::Schoolyard,
            1000,
            SheetLang::En,
        );
        assert_eq!(p.map.contour_interval_m, None);
        save_new_with_comments(
            &path,
            &p,
            &[KeyComment {
                table: "map",
                key: Some("contour_interval_m"),
                text: "set it",
            }],
        )
        .unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("[print]"), "{text}");
        assert!(text.contains("kind = \"schoolyard\"\n# set it\n"), "{text}");
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.project, p);
        assert_eq!(loaded.project.layouts[0].id, "a4");
        assert_eq!(loaded.project.layouts[0].print_scale, Some(1000));
    }

    #[test]
    fn save_new_refuses_overwrite() {
        let (_d, path) = temp_project("keep");
        let err = save_new(&path, &full_project()).unwrap_err();
        assert!(matches!(err, ProjectError::Exists { .. }));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "keep");
    }
}
