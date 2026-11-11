//! `freemapper-print mcp`: the MCP server on stdio, so an AI agent can read, check, change and export a
//! project. The tools are thin wrappers over [`crate::ops`]; results are compact JSON, notes and errors carry
//! the same texts the CLI prints (ADR 0004). Nothing but JSON-RPC goes to stdout. The tools that write hold one
//! lock of the server, so that each sees what the one before it wrote.

use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use fluent_bundle::FluentArgs;
use rmcp::handler::server::{router::tool::ToolRouter, wrapper::Parameters};
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::service::RequestContext;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt, tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};

use crate::ops;
use crate::ops::note::Note;
use crate::text::{Lang, Texts, detect_lang};
use crate::{op_error, printable};
use freemapper_print::print::{Orientation, Paper};
use freemapper_print::project::{self, AnchorTarget, Corner, ElementKind, NorthLines};
use freemapper_print::scale::MapKind;

/// The most notes `check_project` and `export_project` return; `more` counts the rest.
const MAX_NOTES: usize = 200;

/// The most files `export_project` lists; `more_files` counts the rest.
const MAX_FILES: usize = 200;

/// The public home of the program, named in the server info and the instructions.
const WEBSITE: &str = "https://github.com/HenryJobst/freemapper-print";

const INSTRUCTIONS: &str = "freemapper-print prints orienteering maps from OpenOrienteering Mapper \
files as PDFs at an exact scale, one sheet per course or per layout. A project is a folder with a \
freemapper-print.toml that names the map, the optional course file (Purple Pen or IOF XML) and the \
layouts. Start with project_info to see what a project holds, then check_project to see the notes \
of every sheet. init_project starts a project; masks_window, masks_corridor, numbers_accept, \
set_layout, set_element and assign_mask change the project file and keep its comments. Run \
check_project after a change to see its effect. export_project writes the PDFs. Paths must be \
absolute because the server does not know your working folder. The messages in the results are German or English; pass lang to choose. \
Manual and source: https://github.com/HenryJobst/freemapper-print";

/// The arguments of every read tool.
#[derive(Deserialize, rmcp::schemars::JsonSchema)]
struct ProjectArgs {
    /// Absolute path of freemapper-print.toml or of the folder that holds it.
    project: String,
    /// Language of the messages: "de" or "en". Default: the language the server started with.
    lang: Option<String>,
}

/// The arguments of `init_project`, as the CLI's `init`.
#[derive(Deserialize, rmcp::schemars::JsonSchema)]
struct InitIn {
    /// Absolute path of the project folder. It is created when missing and must not hold a
    /// freemapper-print.toml yet.
    folder: String,
    /// Absolute path of the map file (.omap or .xmap).
    map: String,
    /// The event title. Default: the title in the map's freemapper.toml, else the map's file name.
    title: Option<String>,
    /// The map kind. Default: map_kind in the map's freemapper.toml, else inferred from the symbol set and
    /// scale.
    kind: Option<KindIn>,
    /// Absolute path of a course file (Purple Pen .ppen or IOF XML): one layout per print scale and an
    /// assignment per course.
    courses: Option<String>,
    /// Language of the messages, and of the texts the new file puts on the sheet: "de" or "en".
    lang: Option<String>,
}

/// The arguments of `masks_window`, as the CLI's `masks window`.
#[derive(Deserialize, rmcp::schemars::JsonSchema)]
struct WindowIn {
    /// Absolute path of freemapper-print.toml or of the folder that holds it.
    project: String,
    /// The course whose controls get a window.
    course: String,
    /// The controls with a window, by code ("31,32") or by place in the course ("2-5"). Default: all.
    controls: Option<String>,
    /// Radius of each window in mm on paper. Default: 10.
    radius: Option<f64>,
    /// Windows with a radius of their own, each "CODE=MM" such as "35=20".
    #[serde(default)]
    radius_for: Vec<String>,
    /// No window around the start.
    #[serde(default)]
    no_start: bool,
    /// No window around the finish.
    #[serde(default)]
    no_finish: bool,
    /// The mask id. Default: one made from the course name.
    id: Option<String>,
    /// Print the course with the new mask.
    #[serde(default)]
    assign: bool,
    /// Language of the messages: "de" or "en". Default: the language the server started with.
    lang: Option<String>,
}

/// The arguments of `masks_corridor`, as the CLI's `masks corridor`.
#[derive(Deserialize, rmcp::schemars::JsonSchema)]
struct CorridorIn {
    /// Absolute path of freemapper-print.toml or of the folder that holds it.
    project: String,
    /// The line of the corridor in map mm, "x,y;x,y;...", at least two points.
    points: String,
    /// Width of the corridor in mm on paper. Default: 8.
    width: Option<f64>,
    /// The mask id. Default: corridor-<n>.
    id: Option<String>,
    /// Print the course named in course with the new mask; needs course.
    #[serde(default)]
    assign: bool,
    /// The course whose layout gives the print scale for the width. Without it, all layouts must print at
    /// one scale.
    course: Option<String>,
    /// Language of the messages: "de" or "en". Default: the language the server started with.
    lang: Option<String>,
}

/// Where a number correction applies.
#[derive(Deserialize, rmcp::schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
enum ScopeIn {
    /// The sheet of the course alone.
    Sheet,
    /// Every course on the course's layout.
    LayoutWide,
}

/// The arguments of `numbers_accept`, as the CLI's `numbers accept`.
#[derive(Deserialize, rmcp::schemars::JsonSchema)]
struct AcceptIn {
    /// Absolute path of freemapper-print.toml or of the folder that holds it.
    project: String,
    /// The course.
    course: String,
    /// The control code.
    control: String,
    /// sheet: this course's sheet; layout_wide: every course on its layout.
    scope: ScopeIn,
    /// Direction of the number in degrees (0 is east, counter-clockwise). Goes with gap.
    direction: Option<f64>,
    /// Gap between number and circle in mm on paper. Goes with direction.
    gap: Option<f64>,
    /// Language of the messages: "de" or "en". Default: the language the server started with.
    lang: Option<String>,
}

/// The map kind.
#[derive(Deserialize, rmcp::schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
enum KindIn {
    Sprint,
    Schoolyard,
    Forest,
}

impl From<KindIn> for MapKind {
    fn from(k: KindIn) -> Self {
        match k {
            KindIn::Sprint => MapKind::Sprint,
            KindIn::Schoolyard => MapKind::Schoolyard,
            KindIn::Forest => MapKind::Forest,
        }
    }
}

#[derive(Deserialize, rmcp::schemars::JsonSchema)]
enum PaperIn {
    A4,
    A3,
}

impl From<PaperIn> for Paper {
    fn from(p: PaperIn) -> Self {
        match p {
            PaperIn::A4 => Paper::A4,
            PaperIn::A3 => Paper::A3,
        }
    }
}

#[derive(Deserialize, rmcp::schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
enum OrientationIn {
    Auto,
    Portrait,
    Landscape,
}

impl From<OrientationIn> for Orientation {
    fn from(o: OrientationIn) -> Self {
        match o {
            OrientationIn::Auto => Orientation::Auto,
            OrientationIn::Portrait => Orientation::Portrait,
            OrientationIn::Landscape => Orientation::Landscape,
        }
    }
}

#[derive(Deserialize, rmcp::schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
enum NorthLinesIn {
    Auto,
    Never,
}

impl From<NorthLinesIn> for NorthLines {
    fn from(n: NorthLinesIn) -> Self {
        match n {
            NorthLinesIn::Auto => NorthLines::Auto,
            NorthLinesIn::Never => NorthLines::Never,
        }
    }
}

/// A field that can be left out (keep the value) or null (clear it).
fn nullable<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

/// The arguments of `set_layout`: the keys of a `[[layouts]]` table. A key left out keeps its value, or takes the
/// default for a new layout.
#[derive(Deserialize, rmcp::schemars::JsonSchema)]
struct LayoutIn {
    /// Absolute path of freemapper-print.toml or of the folder that holds it.
    project: String,
    /// The layout id; it names the PDF of the layout.
    id: String,
    /// Default for a new layout: A4.
    paper: Option<PaperIn>,
    /// auto takes the orientation in which more of the map fits. Default for a new layout: auto.
    orientation: Option<OrientationIn>,
    /// The print scale denominator, such as 4000; null prints at the map scale.
    #[serde(default, deserialize_with = "nullable")]
    print_scale: Option<Option<u32>>,
    /// The map point (mm on the map sheet) in the centre of the map frame; null centres the printed area.
    #[serde(default, deserialize_with = "nullable")]
    centre_map_mm: Option<Option<[f64; 2]>>,
    /// Width of the passe-partout around the map frame in mm. Default for a new layout: 8.
    margin_mm: Option<f64>,
    /// Width of the frame line in mm; 0 draws none. Default for a new layout: 0.25.
    border_mm: Option<f64>,
    /// The frame cuts the map on purpose, so check reports it as information only.
    crop: Option<bool>,
    /// auto adds north lines when the map has none; never leaves them out.
    north_lines: Option<NorthLinesIn>,
    /// Map parts the layout does not print, by name.
    hide_parts: Option<Vec<String>>,
    /// Ids of shared elements the layout does not show.
    hide_shared: Option<Vec<String>>,
    /// Language of the messages: "de" or "en". Default: the language the server started with.
    lang: Option<String>,
}

#[derive(Deserialize, rmcp::schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
enum ElementKindIn {
    TitleBlock,
    Text,
    Logo,
    ScaleCheckBar,
    CreditLine,
    CopyrightLine,
    ControlDescriptions,
}

impl From<ElementKindIn> for ElementKind {
    fn from(k: ElementKindIn) -> Self {
        match k {
            ElementKindIn::TitleBlock => ElementKind::TitleBlock,
            ElementKindIn::Text => ElementKind::Text,
            ElementKindIn::Logo => ElementKind::Logo,
            ElementKindIn::ScaleCheckBar => ElementKind::ScaleCheckBar,
            ElementKindIn::CreditLine => ElementKind::CreditLine,
            ElementKindIn::CopyrightLine => ElementKind::CopyrightLine,
            ElementKindIn::ControlDescriptions => ElementKind::ControlDescriptions,
        }
    }
}

#[derive(Deserialize, rmcp::schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
enum AnchorTargetIn {
    Page,
    MapFrame,
}

#[derive(Deserialize, rmcp::schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
enum CornerIn {
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

impl From<CornerIn> for Corner {
    fn from(c: CornerIn) -> Self {
        match c {
            CornerIn::TopLeft => Corner::TopLeft,
            CornerIn::Top => Corner::Top,
            CornerIn::TopRight => Corner::TopRight,
            CornerIn::Left => Corner::Left,
            CornerIn::Centre => Corner::Centre,
            CornerIn::Right => Corner::Right,
            CornerIn::BottomLeft => Corner::BottomLeft,
            CornerIn::Bottom => Corner::Bottom,
            CornerIn::BottomRight => Corner::BottomRight,
        }
    }
}

/// Where an element hangs.
#[derive(Deserialize, rmcp::schemars::JsonSchema)]
struct AnchorIn {
    /// The page, or the map frame inside the passe-partout.
    to: AnchorTargetIn,
    /// The corner or edge of it the element hangs from.
    corner: CornerIn,
    /// Offset in mm inward from that corner, x to the right and y down. Default: [0, 0].
    #[serde(default)]
    offset_mm: [f64; 2],
}

/// The arguments of `set_element`: a sheet element as the project file writes it.
#[derive(Deserialize, rmcp::schemars::JsonSchema)]
struct ElementIn {
    /// Absolute path of freemapper-print.toml or of the folder that holds it.
    project: String,
    /// The layout id. Left out: the element is shared by every layout.
    layout: Option<String>,
    /// The element id; an element with this id there is replaced.
    id: String,
    kind: ElementKindIn,
    anchor: AnchorIn,
    /// title_block and text: the lines, with placeholders such as {event} or {course}.
    #[serde(default)]
    lines: Vec<String>,
    /// title_block, text, credit_line and copyright_line: the text size in pt.
    size_pt: Option<f64>,
    /// title_block and text: a white box behind the text. Default: true.
    box_white: Option<bool>,
    /// logo: absolute path of the image (PNG, JPEG or SVG). The file stores it relative to the project file.
    file: Option<String>,
    /// logo: printed width in mm.
    width_mm: Option<f64>,
    /// control_descriptions: printed side of one cell in mm, 4 to 10. Default: the rule profile's size.
    cell_mm: Option<f64>,
    /// Language of the messages: "de" or "en". Default: the language the server started with.
    lang: Option<String>,
}

/// The arguments of `assign_mask`.
#[derive(Deserialize, rmcp::schemars::JsonSchema)]
struct AssignIn {
    /// Absolute path of freemapper-print.toml or of the folder that holds it.
    project: String,
    /// The course of the course file.
    course: String,
    /// The mask variant: a [[masks]] id or whiteout-<n>. Left out or null: the course prints without one.
    mask: Option<String>,
    /// The layout the course prints on. Left out: the layout it prints on now.
    layout: Option<String>,
    /// Language of the messages: "de" or "en". Default: the language the server started with.
    lang: Option<String>,
}

/// The arguments of `export_project`, as the CLI's `export`.
#[derive(Deserialize, rmcp::schemars::JsonSchema)]
struct ExportIn {
    /// Absolute path of freemapper-print.toml or of the folder that holds it.
    project: String,
    /// Absolute path of the folder for the PDFs. It must exist.
    out_dir: String,
    /// Only these courses, by name. Default: every course of the course file.
    #[serde(default)]
    courses: Vec<String>,
    /// These layouts by id, printed without courses. Does not go with courses or single_file.
    #[serde(default)]
    layouts: Vec<String>,
    /// All courses as the pages of one PDF.
    #[serde(default)]
    single_file: bool,
    /// Language of the messages: "de" or "en". Default: the language the server started with.
    lang: Option<String>,
}

/// One PDF that `export_project` wrote.
#[derive(Serialize)]
struct FileOut {
    path: PathBuf,
    /// The size of the file; `None` when it could not be read back.
    bytes: Option<u64>,
    /// The course on the sheet; left out for a layout without a course and for the single file.
    #[serde(skip_serializing_if = "Option::is_none")]
    course: Option<String>,
    /// The layout of the sheet; left out for the single file, whose pages may use several.
    #[serde(skip_serializing_if = "Option::is_none")]
    layout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    paper: Option<Paper>,
    #[serde(skip_serializing_if = "Option::is_none")]
    print: Option<ops::PrintFacts>,
    /// The number of pages of the single file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pages: Option<usize>,
}

/// The result of `export_project`.
#[derive(Serialize)]
struct ExportOut {
    out_dir: PathBuf,
    /// The first [`MAX_FILES`] files written, in the order written.
    files: Vec<FileOut>,
    /// How many files were written past [`MAX_FILES`].
    #[serde(skip_serializing_if = "is_zero")]
    more_files: usize,
    /// The notes of the course file, then those of each sheet in the order drawn.
    notes: Vec<Note>,
    /// How many notes were left out past [`MAX_NOTES`].
    #[serde(skip_serializing_if = "is_zero")]
    more: usize,
    /// The reminder to print at actual size; left out when nothing was written.
    #[serde(skip_serializing_if = "Option::is_none")]
    reminder: Option<String>,
    /// Why nothing was written, when the course file has no course to export.
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

fn export_out(texts: &Texts, result: &ops::export::ExportResult, max_files: usize) -> ExportOut {
    use ops::export::ExportMode;
    let bytes = |path: &Path| {
        result
            .files
            .iter()
            .find(|f| f.path == path)
            .and_then(|f| f.bytes)
    };
    let files = match result.mode {
        ExportMode::SingleFile => result
            .files
            .iter()
            .map(|f| FileOut {
                path: f.path.clone(),
                bytes: f.bytes,
                course: None,
                layout: None,
                paper: None,
                print: None,
                pages: Some(result.pages.iter().filter(|p| p.path == f.path).count()),
            })
            .collect(),
        ExportMode::Courses | ExportMode::Layouts => result
            .pages
            .iter()
            .map(|p| FileOut {
                path: p.path.clone(),
                bytes: bytes(&p.path),
                course: p.sheet.course.clone(),
                layout: Some(p.sheet.layout.clone()),
                paper: Some(p.sheet.paper),
                print: Some(p.sheet.print),
                pages: None,
            })
            .collect(),
    };
    let mut file_budget = NoteBudget::new(max_files);
    let files = file_budget.take(files);
    let mut budget = NoteBudget::new(MAX_NOTES);
    let mut notes = result
        .course_file
        .as_ref()
        .map(|n| budget.describe(texts, n))
        .unwrap_or_default();
    for page in &result.pages {
        notes.extend(budget.describe(texts, &page.sheet.notes));
    }
    let written = !result.files.is_empty();
    ExportOut {
        out_dir: result.out_dir.clone(),
        files,
        more_files: file_budget.cut,
        notes,
        more: budget.cut,
        reminder: written.then(|| texts.plain("reminder-actual-size")),
        message: (!written && result.mode != ExportMode::Layouts)
            .then(|| texts.plain("note-no-courses-to-export")),
    }
}

/// A result of a write tool, with what the CLI says after the same command.
#[derive(Serialize)]
struct Written<'a, T: Serialize> {
    #[serde(flatten)]
    result: &'a T,
    messages: Vec<String>,
}

/// The result of `init_project`.
#[derive(Serialize)]
struct InitOut<'a> {
    path: &'a Path,
    kind: MapKind,
    kind_source: ops::init::KindSource,
    map_scale: u32,
    layouts: &'a [ops::init::InitLayout],
    assignments: usize,
    course_file: &'a Option<ops::init::InitCourses>,
    contour_interval_m: Option<f64>,
    /// Without a course file, when the layout does not fit: what check_project says about its sheet.
    #[serde(skip_serializing_if = "Option::is_none")]
    check: Option<SheetOut>,
    messages: Vec<String>,
}

/// One sheet of `check_project`.
#[derive(Serialize)]
struct SheetOut {
    /// `drawn`, or `failed` when the sheet could not be drawn (its note says why).
    outcome: &'static str,
    course: Option<String>,
    layout: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    paper: Option<freemapper_print::print::Paper>,
    #[serde(skip_serializing_if = "Option::is_none")]
    print: Option<ops::PrintFacts>,
    needs_attention: bool,
    notes: Vec<Note>,
}

/// The result of `check_project`.
#[derive(Serialize)]
struct CheckOut {
    map_scale: u32,
    /// A note other than info, or a sheet that could not be drawn: the CLI's `check` exits with 1.
    needs_attention: bool,
    /// The notes of the course file as a whole; `None` without a course file.
    course_file: Option<Vec<Note>>,
    courses: Vec<SheetOut>,
    layouts: Vec<SheetOut>,
    /// How many notes were left out past [`MAX_NOTES`].
    #[serde(skip_serializing_if = "is_zero")]
    more: usize,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// The result of `list_courses`.
#[derive(Serialize)]
struct CoursesOut {
    courses_file: Option<PathBuf>,
    courses: Vec<ops::project::CourseInfo>,
}

/// Keeps at most a fixed number of notes (or files) over several lists and counts the rest.
struct NoteBudget {
    left: usize,
    cut: usize,
}

impl NoteBudget {
    fn new(max: usize) -> Self {
        NoteBudget { left: max, cut: 0 }
    }

    /// The first items of `items` that still fit.
    fn take<T>(&mut self, mut items: Vec<T>) -> Vec<T> {
        if items.len() > self.left {
            self.cut += items.len() - self.left;
            items.truncate(self.left);
        }
        self.left -= items.len();
        items
    }

    /// The notes in words, as many as still fit.
    fn describe(&mut self, texts: &Texts, notices: &[ops::Notice]) -> Vec<Note> {
        let kept = self.take(notices.iter().collect());
        kept.into_iter().map(|n| n.describe(texts)).collect()
    }
}

fn sheet_out(texts: &Texts, budget: &mut NoteBudget, sheet: &ops::check::SheetCheck) -> SheetOut {
    let needs_attention = sheet.needs_attention();
    match sheet {
        ops::check::SheetCheck::Drawn(s) => SheetOut {
            outcome: "drawn",
            course: s.course.clone(),
            layout: s.layout.clone(),
            paper: Some(s.paper),
            print: Some(s.print),
            needs_attention,
            notes: budget.describe(texts, &s.notes),
        },
        ops::check::SheetCheck::Failed {
            course,
            layout,
            error,
        } => SheetOut {
            outcome: "failed",
            course: course.clone(),
            layout: layout.clone(),
            paper: None,
            print: None,
            needs_attention,
            notes: budget.describe(texts, std::slice::from_ref(error)),
        },
    }
}

fn check_out(texts: &Texts, result: &ops::check::CheckResult) -> CheckOut {
    let mut budget = NoteBudget::new(MAX_NOTES);
    let course_file = result
        .course_file
        .as_ref()
        .map(|notes| budget.describe(texts, notes));
    let courses: Vec<SheetOut> = result
        .courses
        .iter()
        .map(|s| sheet_out(texts, &mut budget, s))
        .collect();
    let layouts: Vec<SheetOut> = result
        .layouts
        .iter()
        .map(|s| sheet_out(texts, &mut budget, s))
        .collect();
    CheckOut {
        map_scale: result.map_scale,
        needs_attention: result.course_file_needs_attention()
            || courses.iter().chain(&layouts).any(|s| s.needs_attention),
        course_file,
        courses,
        layouts,
        more: budget.cut,
    }
}

/// A path from the agent: absolute, not empty, without NUL. Relative paths are refused because the server
/// does not know the agent's working folder (ADR 0004).
fn absolute_path(texts: &Texts, path: &str) -> Result<PathBuf, McpError> {
    if path.is_empty() {
        return Err(McpError::invalid_params(
            texts.plain("mcp-error-empty-path"),
            None,
        ));
    }
    if path.contains('\0') {
        return Err(McpError::invalid_params(
            texts.get(
                "mcp-error-nul-path",
                Some(&args!("path" => printable(path))),
            ),
            None,
        ));
    }
    let resolved = PathBuf::from(path);
    if !resolved.is_absolute() {
        return Err(McpError::invalid_params(
            texts.get(
                "mcp-error-relative-path",
                Some(&args!("path" => printable(path))),
            ),
            None,
        ));
    }
    Ok(resolved)
}

/// A tool result with one text block.
fn text(text: impl Into<String>) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
}

/// A tool error the agent can relay to the user.
fn failure(message: impl Into<String>) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::error(vec![ContentBlock::text(message)]))
}

/// An op error in words, with its causes, as the CLI prints it after "Error:".
fn op_message(texts: &Texts, err: ops::OpError) -> String {
    printable(&format!("{:#}", op_error(texts, err)))
}

/// Compact JSON of a result.
fn json(texts: &Texts, value: &impl Serialize) -> Result<String, String> {
    serde_json::to_string(value)
        .map_err(|e| texts.get("mcp-error-internal", Some(&args!("error" => e.to_string()))))
}

#[derive(Clone)]
pub struct PrintServer {
    tool_router: ToolRouter<Self>,
    /// The language of the messages when a call names none.
    lang: Lang,
    /// Held by every tool that writes, from before it reads the project until it has written.
    writes: Arc<tokio::sync::Mutex<()>>,
}

impl PrintServer {
    /// The texts for a call: its `lang`, else the server's.
    fn texts(&self, lang: Option<&str>) -> Result<Texts, String> {
        let Some(given) = lang else {
            return Ok(Texts::for_agent(&self.lang));
        };
        detect_lang(Some(given), &|_| None)
            .map(|lang| Texts::for_agent(&lang))
            .map_err(|unknown| {
                let available: Vec<String> = unknown.available.iter().map(Lang::code).collect();
                Texts::for_agent(&self.lang).get(
                    "error-unknown-lang",
                    Some(&args!(
                        "lang" => printable(&unknown.given),
                        "available" => available.join(", "),
                    )),
                )
            })
    }

    /// The texts of a call and its project path, checked; a tool error when either is unusable.
    fn prepare(
        &self,
        lang: Option<&str>,
        project: &str,
    ) -> Result<(Texts, PathBuf), CallToolResult> {
        let texts = self
            .texts(lang)
            .map_err(|message| CallToolResult::error(vec![ContentBlock::text(message)]))?;
        let path = absolute_path(&texts, project)
            .map_err(|e| CallToolResult::error(vec![ContentBlock::text(e.message)]))?;
        Ok((texts, path))
    }

    /// Checks the arguments, then runs `op` off the async runtime: the ops read files and draw sheets.
    async fn run(
        &self,
        args: ProjectArgs,
        op: impl FnOnce(&Texts, &Path) -> Result<String, String> + Send + 'static,
    ) -> Result<CallToolResult, McpError> {
        let (texts, path) = match self.prepare(args.lang.as_deref(), &args.project) {
            Ok(prepared) => prepared,
            Err(refused) => return Ok(refused),
        };
        blocking(texts, move |texts| op(texts, &path)).await
    }

    /// [`Self::run`] for a tool that changes the project file: under the write lock, and refused when the project
    /// file is a link.
    async fn write(
        &self,
        lang: Option<&str>,
        project: &str,
        op: impl FnOnce(&Texts, &Path) -> Result<String, String> + Send + 'static,
    ) -> Result<CallToolResult, McpError> {
        let (texts, path) = match self.prepare(lang, project) {
            Ok(prepared) => prepared,
            Err(refused) => return Ok(refused),
        };
        self.with_project_lock(texts, move |texts| {
            refuse_link(texts, &path)?;
            op(texts, &path)
        })
        .await
    }

    /// Runs `op` off the async runtime while it holds the write lock. The lock goes with the work, so a call the
    /// client gives up on still keeps the next write waiting until its own write is done.
    async fn with_project_lock(
        &self,
        texts: Texts,
        op: impl FnOnce(&Texts) -> Result<String, String> + Send + 'static,
    ) -> Result<CallToolResult, McpError> {
        let guard = self.writes.clone().lock_owned().await;
        blocking(texts, move |texts| {
            let _guard = guard;
            op(texts)
        })
        .await
    }
}

/// Runs `op` on a thread for blocking work. A panic in `op` comes back as a tool error, and the server keeps
/// serving.
async fn blocking(
    texts: Texts,
    op: impl FnOnce(&Texts) -> Result<String, String> + Send + 'static,
) -> Result<CallToolResult, McpError> {
    let lang = texts.lang().clone();
    match tokio::task::spawn_blocking(move || op(&texts)).await {
        Ok(Ok(json)) => text(json),
        Ok(Err(message)) => failure(message),
        Err(e) => failure(Texts::for_agent(&lang).get(
            "mcp-error-internal",
            Some(&args!("error" => printable(&panic_text(e)))),
        )),
    }
}

/// Refuses a project file that is a link: the CLI replaces such a link with a regular file (ADR 0041), which an
/// agent would not notice, so the server changes nothing. A folder that is a link is followed.
fn refuse_link(texts: &Texts, path: &Path) -> Result<(), String> {
    let file = ops::project_file(path);
    if std::fs::symlink_metadata(&file).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(texts.get(
            "mcp-error-project-link",
            Some(&args!("path" => file.display().to_string())),
        ));
    }
    Ok(())
}

/// An agent's path as one checked by [`absolute_path`], with the error in words.
fn agent_path(texts: &Texts, path: &str) -> Result<PathBuf, String> {
    absolute_path(texts, path).map_err(|e| e.message.to_string())
}

/// What a failed blocking task says: the panic message if there is one.
fn panic_text(err: tokio::task::JoinError) -> String {
    if !err.is_panic() {
        return err.to_string();
    }
    let panic = err.into_panic();
    panic
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_default()
}

#[tool_router]
impl PrintServer {
    pub fn new(lang: Lang) -> Self {
        Self {
            tool_router: Self::tool_router(),
            lang,
            writes: Arc::default(),
        }
    }

    #[tool(
        description = "What a project holds: title, map kind and scale, the map and course file it \
                       reads, the layouts with their elements, the courses with the layout and masks \
                       each prints with, the mask variants and the assignments."
    )]
    async fn project_info(
        &self,
        Parameters(args): Parameters<ProjectArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.run(args, |texts, path| {
            let info = ops::project::info(&ops::project::InfoArgs {
                project: path.to_path_buf(),
            })
            .map_err(|e| op_message(texts, e))?;
            json(texts, &info)
        })
        .await
    }

    #[tool(
        description = "Draw every sheet of a project in memory, as the check command does, and \
                       return the notes of each sheet. With a course file there is one sheet per \
                       course, then one per layout without a course. Each note has kind, severity \
                       (info, warning or error), course and message. needs_attention is true when a \
                       note is more than info or a sheet could not be drawn. At most 200 notes come \
                       back; more counts the rest."
    )]
    async fn check_project(
        &self,
        Parameters(args): Parameters<ProjectArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.run(args, |texts, path| {
            let result = ops::check::check(&ops::check::CheckArgs {
                project: path.to_path_buf(),
            })
            .map_err(|e| op_message(texts, e))?;
            json(texts, &check_out(texts, &result))
        })
        .await
    }

    #[tool(
        description = "The courses of the project's course file in file order, each with its number \
                       of controls, the layout it prints on and the masks its sheet applies. The list \
                       is empty when the project has no course file."
    )]
    async fn list_courses(
        &self,
        Parameters(args): Parameters<ProjectArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.run(args, |texts, path| {
            let info = ops::project::info(&ops::project::InfoArgs {
                project: path.to_path_buf(),
            })
            .map_err(|e| op_message(texts, e))?;
            json(
                texts,
                &CoursesOut {
                    courses_file: info.courses_file,
                    courses: info.courses,
                },
            )
        })
        .await
    }

    #[tool(
        description = "Write a new project file freemapper-print.toml for a map into a folder, as the \
                       init command does. With a course file there is one layout per print scale and \
                       an assignment per course. The tool refuses a folder that holds a project \
                       file already. Returns the layouts written, where the map kind came from, and the \
                       messages of init."
    )]
    async fn init_project(
        &self,
        Parameters(args): Parameters<InitIn>,
    ) -> Result<CallToolResult, McpError> {
        let (texts, folder) = match self.prepare(args.lang.as_deref(), &args.folder) {
            Ok(prepared) => prepared,
            Err(refused) => return Ok(refused),
        };
        let map = match agent_path(&texts, &args.map) {
            Ok(map) => map,
            Err(message) => return failure(message),
        };
        let courses = match args.courses.as_deref().map(|c| agent_path(&texts, c)) {
            Some(Err(message)) => return failure(message),
            Some(Ok(path)) => Some(path),
            None => None,
        };
        let (title, kind) = (args.title, args.kind.map(MapKind::from));
        self.with_project_lock(texts, move |texts| {
            let mut messages = Vec::new();
            let result = ops::init::init(
                &ops::init::InitArgs {
                    folder,
                    map,
                    title,
                    kind,
                    courses,
                    sheet_lang: crate::sheet_lang(texts),
                },
                &|kind, contour| crate::init_comments(texts, kind, contour),
                &mut |progress| {
                    // Progress while the layouts are fitted is for a terminal.
                    if !matches!(progress, ops::init::InitProgress::Fitting { .. }) {
                        messages.push(crate::init_progress_line(texts, &progress));
                    }
                },
            )
            .map_err(|e| op_message(texts, e))?;
            messages.extend(crate::init_written_lines(texts, &result));
            let mut budget = NoteBudget::new(MAX_NOTES);
            json(
                texts,
                &InitOut {
                    path: &result.path,
                    kind: result.kind,
                    kind_source: result.kind_source,
                    map_scale: result.map_scale,
                    layouts: &result.layouts,
                    assignments: result.assignments,
                    course_file: &result.course_file,
                    contour_interval_m: result.contour_interval_m,
                    check: result
                        .check
                        .as_ref()
                        .map(|c| sheet_out(texts, &mut budget, c)),
                    messages,
                },
            )
        })
        .await
    }

    #[tool(
        description = "Add a mask variant with round windows around the controls of a course, as \
                       masks window does: on the sheet the map shows only inside the windows. With \
                       assign the course prints with the new mask. Returns the mask id, the stops \
                       with a window and the messages of the command."
    )]
    async fn masks_window(
        &self,
        Parameters(args): Parameters<WindowIn>,
    ) -> Result<CallToolResult, McpError> {
        let WindowIn {
            project,
            course,
            controls,
            radius,
            radius_for,
            no_start,
            no_finish,
            id,
            assign,
            lang,
        } = args;
        self.write(lang.as_deref(), &project, move |texts, path| {
            let result = ops::masks::window(
                &ops::masks::WindowArgs {
                    project: path.to_path_buf(),
                    course,
                    controls,
                    radius_mm: radius,
                    radius_for,
                    no_start,
                    no_finish,
                    id,
                    assign,
                },
                &|r| crate::window_comment(texts, r),
            )
            .map_err(|e| op_message(texts, e))?;
            let messages = crate::window_written_lines(texts, &result);
            json(
                texts,
                &Written {
                    result: &result,
                    messages,
                },
            )
        })
        .await
    }

    #[tool(
        description = "Add a mask variant with a corridor along a line, as masks corridor does: on the \
                       sheet the map shows only inside the corridor. The width is mm on paper, so it \
                       needs a print scale: course names the course whose layout gives it. With \
                       assign that course prints with the new mask. Returns the mask id and the \
                       messages of the command."
    )]
    async fn masks_corridor(
        &self,
        Parameters(args): Parameters<CorridorIn>,
    ) -> Result<CallToolResult, McpError> {
        let CorridorIn {
            project,
            points,
            width,
            id,
            assign,
            course,
            lang,
        } = args;
        self.write(lang.as_deref(), &project, move |texts, path| {
            if assign && course.is_none() {
                return Err(texts.plain("mcp-error-corridor-assign"));
            }
            let result = ops::masks::corridor(
                &ops::masks::CorridorArgs {
                    project: path.to_path_buf(),
                    points,
                    width_mm: width,
                    id,
                    assign,
                    course,
                },
                &|r| crate::corridor_comment(texts, r),
            )
            .map_err(|e| op_message(texts, e))?;
            let messages = crate::corridor_written_lines(texts, &result);
            json(
                texts,
                &Written {
                    result: &result,
                    messages,
                },
            )
        })
        .await
    }

    #[tool(
        description = "Fix where the number of a control is printed, as numbers accept does. Without \
                       direction and gap it takes the free position that check_project proposes for \
                       the number. scope sheet changes the sheet of the course, layout_wide every course \
                       on its layout. Returns the correction written and the message of the command."
    )]
    async fn numbers_accept(
        &self,
        Parameters(args): Parameters<AcceptIn>,
    ) -> Result<CallToolResult, McpError> {
        let AcceptIn {
            project,
            course,
            control,
            scope,
            direction,
            gap,
            lang,
        } = args;
        self.write(lang.as_deref(), &project, move |texts, path| {
            let position = match (direction, gap) {
                (Some(d), Some(g)) => Some((d, g)),
                (None, None) => None,
                _ => return Err(texts.plain("mcp-error-numbers-position")),
            };
            let result = ops::numbers::accept(&ops::numbers::AcceptArgs {
                project: path.to_path_buf(),
                course,
                control,
                scope: match scope {
                    ScopeIn::Sheet => ops::numbers::AcceptScope::Sheet,
                    ScopeIn::LayoutWide => ops::numbers::AcceptScope::Layout,
                },
                position,
            })
            .map_err(|e| op_message(texts, e))?;
            let messages = vec![crate::numbers_accepted_line(texts, &result)];
            json(
                texts,
                &Written {
                    result: &result,
                    messages,
                },
            )
        })
        .await
    }

    #[tool(
        description = "Add a layout to the project file, or change the layout with this id. A key left \
                       out keeps its value, or takes the default for a new layout. The elements of the \
                       layout stay as they are; set_element changes them. Returns the layout as the \
                       file now reads, and change: added, changed or unchanged."
    )]
    async fn set_layout(
        &self,
        Parameters(args): Parameters<LayoutIn>,
    ) -> Result<CallToolResult, McpError> {
        let LayoutIn {
            project,
            id,
            paper,
            orientation,
            print_scale,
            centre_map_mm,
            margin_mm,
            border_mm,
            crop,
            north_lines,
            hide_parts,
            hide_shared,
            lang,
        } = args;
        self.write(lang.as_deref(), &project, move |texts, path| {
            let result = ops::edit::set_layout(&ops::edit::LayoutArgs {
                project: path.to_path_buf(),
                id,
                paper: paper.map(Paper::from),
                orientation: orientation.map(Orientation::from),
                print_scale,
                centre_map_mm,
                margin_mm,
                border_mm,
                crop,
                north_lines: north_lines.map(NorthLines::from),
                hide_parts,
                hide_shared,
            })
            .map_err(|e| op_message(texts, e))?;
            json(texts, &result)
        })
        .await
    }

    #[tool(
        description = "Add a sheet element to a layout, or to the shared elements when layout is left \
                       out, or replace the element with the same id there. Which fields apply depends \
                       on kind: lines for title_block and text, size_pt for those and for credit_line \
                       and copyright_line, file and width_mm for logo, cell_mm for \
                       control_descriptions. Returns the element and change: added, changed or \
                       unchanged."
    )]
    async fn set_element(
        &self,
        Parameters(args): Parameters<ElementIn>,
    ) -> Result<CallToolResult, McpError> {
        let ElementIn {
            project,
            layout,
            id,
            kind,
            anchor,
            lines,
            size_pt,
            box_white,
            file,
            width_mm,
            cell_mm,
            lang,
        } = args;
        self.write(lang.as_deref(), &project, move |texts, path| {
            // The file names the logo relative to itself, as it names the map.
            let file = match file {
                Some(logo) => {
                    let logo = agent_path(texts, &logo)?;
                    let project_file = ops::project_file(path);
                    let dir = project_file.parent().unwrap_or(path);
                    let canonical = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
                    Some(project::relative_path(&canonical(dir), &canonical(&logo)))
                }
                None => None,
            };
            let result = ops::edit::set_element(&ops::edit::ElementArgs {
                project: path.to_path_buf(),
                layout,
                element: project::ElementSpec {
                    id,
                    kind: kind.into(),
                    anchor: project::Anchor {
                        to: match anchor.to {
                            AnchorTargetIn::Page => AnchorTarget::Page,
                            AnchorTargetIn::MapFrame => AnchorTarget::MapFrame,
                        },
                        corner: anchor.corner.into(),
                        offset_mm: anchor.offset_mm,
                    },
                    lines,
                    size_pt,
                    box_white,
                    file,
                    width_mm,
                    cell_mm,
                },
            })
            .map_err(|e| op_message(texts, e))?;
            json(texts, &result)
        })
        .await
    }

    #[tool(
        description = "Set the mask variant a course prints with: a [[masks]] entry of the project file \
                       or a white-out area of the course file (whiteout-<n>); project_info lists them. \
                       Without mask the course prints without one. layout moves the course to another \
                       layout as well. Returns the assignment and change: added, changed or unchanged."
    )]
    async fn assign_mask(
        &self,
        Parameters(args): Parameters<AssignIn>,
    ) -> Result<CallToolResult, McpError> {
        let AssignIn {
            project,
            course,
            mask,
            layout,
            lang,
        } = args;
        self.write(lang.as_deref(), &project, move |texts, path| {
            let result = ops::edit::assign_mask(&ops::edit::AssignArgs {
                project: path.to_path_buf(),
                course,
                mask,
                layout,
            })
            .map_err(|e| op_message(texts, e))?;
            json(texts, &result)
        })
        .await
    }

    #[tool(
        description = "Write the PDFs of a project into out_dir, as the export command does. Without \
                       a course file it writes one PDF per layout, with one a PDF per course, named \
                       after the course. single_file puts the courses on the pages of one PDF \
                       instead. layouts prints the named layouts without courses and does not go \
                       with courses or single_file. The tool replaces a PDF of the same name in \
                       out_dir. Returns the files with their size, the notes of the sheets and the \
                       reminder to print at actual size. At most 200 files and 200 notes come \
                       back; more_files and more count the rest. When the client cancels the call, the export stops after the sheet \
                       it is drawing."
    )]
    async fn export_project(
        &self,
        Parameters(args): Parameters<ExportIn>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let ExportIn {
            project,
            out_dir,
            courses,
            layouts,
            single_file,
            lang,
        } = args;
        let (texts, path) = match self.prepare(lang.as_deref(), &project) {
            Ok(prepared) => prepared,
            Err(refused) => return Ok(refused),
        };
        // The CLI's argument rule: layouts are exported as such, so they go alone.
        if !layouts.is_empty() && (!courses.is_empty() || single_file) {
            return failure(texts.plain("mcp-error-export-layouts"));
        }
        let out_dir = match agent_path(&texts, &out_dir) {
            Ok(dir) => dir,
            Err(message) => return failure(message),
        };
        if !out_dir.is_dir() {
            return failure(op_message(
                &texts,
                ops::OpError::OutDirMissing { path: out_dir },
            ));
        }
        // Set when the client cancels this call or the connection ends; the export stops after the sheet in hand.
        let cancelled = context.ct.clone();
        // The export reads the project, so it waits for a write in progress and keeps the next one waiting.
        self.with_project_lock(texts, move |texts| {
            let stop = || {
                if cancelled.is_cancelled() {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            };
            if stop().is_break() {
                return Err(op_message(
                    texts,
                    ops::OpError::ExportStopped {
                        written: Vec::new(),
                    },
                ));
            }
            let result = ops::export::export(
                &ops::export::ExportArgs {
                    project: path,
                    layouts,
                    courses,
                    single_file,
                    out_dir: Some(out_dir),
                },
                &mut |_| stop(),
            )
            .map_err(|e| op_message(texts, e))?;
            json(texts, &export_out(texts, &result, MAX_FILES))
        })
        .await
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for PrintServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new("freemapper-print", env!("CARGO_PKG_VERSION"))
                    .with_website_url(WEBSITE),
            )
            .with_instructions(INSTRUCTIONS)
    }
}

/// Serves MCP on `transport` until the client disconnects.
async fn serve<T, E, A>(lang: Lang, transport: T) -> anyhow::Result<()>
where
    T: rmcp::transport::IntoTransport<rmcp::RoleServer, E, A>,
    E: std::error::Error + Send + Sync + 'static,
{
    let service = PrintServer::new(lang).serve(transport).await?;
    service.waiting().await?;
    Ok(())
}

/// `freemapper-print mcp`: serves on stdin and stdout until stdin closes.
pub fn run(texts: &Texts) -> anyhow::Result<u8> {
    let lang = texts.lang().clone();
    tokio::runtime::Runtime::new()?.block_on(serve(lang, rmcp::transport::stdio()))?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::testing::{viktoriapark, viktoriapark_copy};
    use serde_json::{Value, json};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream, ReadHalf, WriteHalf};

    /// An MCP client in the same process, over a pipe: the messages an agent sends, line by line.
    struct Client {
        lines: tokio::io::Lines<BufReader<ReadHalf<DuplexStream>>>,
        writer: WriteHalf<DuplexStream>,
        next_id: u64,
        /// The answer to `initialize`.
        info: Value,
    }

    impl Client {
        async fn start() -> Client {
            let (ours, theirs) = tokio::io::duplex(1 << 16);
            tokio::spawn(serve(Lang::english(), theirs));
            let (reader, writer) = tokio::io::split(ours);
            let mut client = Client {
                lines: BufReader::new(reader).lines(),
                writer,
                next_id: 1,
                info: Value::Null,
            };
            let info = client
                .request(
                    "initialize",
                    json!({
                        "protocolVersion": "2025-06-18",
                        "capabilities": {},
                        "clientInfo": { "name": "test", "version": "1" }
                    }),
                )
                .await;
            assert_eq!(
                info["result"]["serverInfo"]["name"], "freemapper-print",
                "{info}"
            );
            client.info = info;
            client
                .send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
                .await;
            client
        }

        async fn send(&mut self, message: Value) {
            let line = format!("{message}\n");
            self.writer.write_all(line.as_bytes()).await.unwrap();
            self.writer.flush().await.unwrap();
        }

        /// The whole answer to a request.
        async fn request(&mut self, method: &str, params: Value) -> Value {
            let id = self.next_id;
            self.next_id += 1;
            self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))
                .await;
            loop {
                let line = self
                    .lines
                    .next_line()
                    .await
                    .unwrap()
                    .expect("server closed");
                let message: Value = serde_json::from_str(&line).unwrap();
                if message["id"] == id {
                    return message;
                }
            }
        }

        /// Calls a tool; returns whether it failed and the text of its first content block.
        async fn call(&mut self, tool: &str, arguments: Value) -> (bool, String) {
            let answer = self
                .request(
                    "tools/call",
                    json!({ "name": tool, "arguments": arguments }),
                )
                .await;
            outcome(&answer)
        }

        /// Calls a tool and returns its result as JSON; panics when the call fails.
        async fn ok(&mut self, tool: &str, arguments: Value) -> Value {
            let (error, text) = self.call(tool, arguments).await;
            assert!(!error, "{tool}: {text}");
            serde_json::from_str(&text).unwrap()
        }

        /// Calls a tool that must fail; returns its message.
        async fn refused(&mut self, tool: &str, arguments: Value) -> String {
            let (error, text) = self.call(tool, arguments).await;
            assert!(error, "{tool} should fail: {text}");
            text
        }

        /// Sends all `calls` before it reads an answer; returns the outcomes in the order of `calls`.
        async fn call_together(&mut self, calls: &[(&str, Value)]) -> Vec<(bool, String)> {
            let first = self.next_id;
            for (tool, arguments) in calls {
                let id = self.next_id;
                self.next_id += 1;
                self.send(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "method": "tools/call",
                    "params": { "name": tool, "arguments": arguments },
                }))
                .await;
            }
            let mut answers: Vec<Option<(bool, String)>> = vec![None; calls.len()];
            while answers.iter().any(Option::is_none) {
                let line = self
                    .lines
                    .next_line()
                    .await
                    .unwrap()
                    .expect("server closed");
                let message: Value = serde_json::from_str(&line).unwrap();
                if let Some(id) = message["id"].as_u64()
                    && id >= first
                    && ((id - first) as usize) < calls.len()
                {
                    answers[(id - first) as usize] = Some(outcome(&message));
                }
            }
            answers.into_iter().map(Option::unwrap).collect()
        }
    }

    /// Whether a tool call failed, and the text of its first content block.
    fn outcome(answer: &Value) -> (bool, String) {
        let result = &answer["result"];
        assert!(result.is_object(), "{answer}");
        let text = result["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        (result["isError"] == true, text)
    }

    /// The project file of a copied project, as text.
    fn file_text(dir: &tempfile::TempDir) -> String {
        std::fs::read_to_string(dir.path().join(project::FILE_NAME)).unwrap()
    }

    fn folder(dir: &tempfile::TempDir) -> String {
        dir.path().to_str().unwrap().to_string()
    }

    /// A copy of the Lübeck example with three courses on two layouts, without its number correction.
    fn luebeck_copy() -> tempfile::TempDir {
        let from = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/luebeck-competition-courses"
        ));
        let dir = tempfile::tempdir().unwrap();
        for name in ["luebeck.omap", "freemapper.toml", "courses.ppen"] {
            std::fs::copy(from.join(name), dir.path().join(name)).unwrap();
        }
        let text = std::fs::read_to_string(from.join(project::FILE_NAME)).unwrap();
        let cut = text.find("[[numbers]]").unwrap();
        std::fs::write(dir.path().join(project::FILE_NAME), &text[..cut]).unwrap();
        dir
    }

    fn project() -> String {
        viktoriapark().to_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn lists_the_read_and_write_tools() {
        let mut client = Client::start().await;
        let answer = client.request("tools/list", json!({})).await;
        let tools = answer["result"]["tools"].as_array().unwrap();
        let mut names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        names.sort();
        assert_eq!(
            names,
            [
                "assign_mask",
                "check_project",
                "export_project",
                "init_project",
                "list_courses",
                "masks_corridor",
                "masks_window",
                "numbers_accept",
                "project_info",
                "set_element",
                "set_layout",
            ]
        );
        let required = |name: &str| match name {
            "init_project" => json!(["folder", "map"]),
            "masks_window" | "assign_mask" => json!(["project", "course"]),
            "masks_corridor" => json!(["project", "points"]),
            "numbers_accept" => json!(["project", "course", "control", "scope"]),
            "set_layout" => json!(["project", "id"]),
            "set_element" => json!(["project", "id", "kind", "anchor"]),
            "export_project" => json!(["project", "out_dir"]),
            _ => json!(["project"]),
        };
        for tool in tools {
            assert!(tool["description"].as_str().unwrap().len() > 40, "{tool}");
            let schema = &tool["inputSchema"];
            assert!(schema["properties"]["lang"].is_object(), "{tool}");
            let mut wanted: Vec<Value> = required(tool["name"].as_str().unwrap())
                .as_array()
                .unwrap()
                .clone();
            let mut found = schema["required"].as_array().unwrap().clone();
            wanted.sort_by_key(|v| v.to_string());
            found.sort_by_key(|v| v.to_string());
            assert_eq!(found, wanted, "{tool}");
        }
        // The public home of the program goes with the server info.
        let info = &client.info["result"];
        assert_eq!(info["serverInfo"]["websiteUrl"], WEBSITE, "{info}");
        assert!(
            info["instructions"].as_str().unwrap().contains(WEBSITE),
            "{info}"
        );
    }

    #[tokio::test]
    async fn project_info_describes_viktoriapark() {
        let mut client = Client::start().await;
        let (error, text) = client
            .call("project_info", json!({ "project": project() }))
            .await;
        assert!(!error, "{text}");
        assert!(!text.contains('\n'), "compact JSON: {text}");
        let info: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(info["title"], "Training Viktoriapark");
        assert_eq!(info["map_scale"], 4000);
        assert_eq!(info["courses"][0]["name"], "Training");
        assert_eq!(info["courses"][0]["controls"], 11);
        assert_eq!(info["courses"][0]["layout"], "a4-4000");
        assert_eq!(info["layouts"][0]["id"], "a4-4000");
        // The project file itself works as well as its folder.
        let file = viktoriapark().join("freemapper-print.toml");
        let (error, text) = client
            .call("project_info", json!({ "project": file }))
            .await;
        assert!(!error, "{text}");
    }

    #[tokio::test]
    async fn list_courses_names_the_course() {
        let mut client = Client::start().await;
        let (error, text) = client
            .call("list_courses", json!({ "project": project() }))
            .await;
        assert!(!error, "{text}");
        let list: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(list["courses_file"], "training.ppen");
        assert_eq!(
            list["courses"],
            json!([{ "name": "Training", "controls": 11, "layout": "a4-4000", "masks": [] }])
        );
    }

    #[tokio::test]
    async fn check_project_returns_notes_with_messages() {
        let mut client = Client::start().await;
        let (error, text) = client
            .call("check_project", json!({ "project": project() }))
            .await;
        assert!(!error, "{text}");
        let check: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(check["map_scale"], 4000);
        assert_eq!(check["needs_attention"], false);
        assert!(check.get("more").is_none(), "{check}");
        let course = &check["courses"][0];
        assert_eq!(course["outcome"], "drawn");
        assert_eq!(course["course"], "Training");
        assert_eq!(course["layout"], "a4-4000");
        let note = &course["notes"][0];
        let mut keys: Vec<&str> = note
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort();
        assert_eq!(keys, ["course", "kind", "message", "severity"]);
        assert_eq!(note["kind"], "map_cropped");
        assert_eq!(note["severity"], "info");
        assert_eq!(note["course"], "Training");
        assert!(!note["message"].as_str().unwrap().is_empty(), "{note}");
        assert_eq!(check["course_file"][0]["kind"], "fit_georef_skipped");
        assert_eq!(check["layouts"][0]["notes"][0]["course"], Value::Null);
        // The messages follow lang.
        let (_, en) = client
            .call(
                "check_project",
                json!({ "project": project(), "lang": "en" }),
            )
            .await;
        let (_, de) = client
            .call(
                "check_project",
                json!({ "project": project(), "lang": "de" }),
            )
            .await;
        let message = |text: &str| {
            serde_json::from_str::<Value>(text).unwrap()["courses"][0]["notes"][0]["message"]
                .as_str()
                .unwrap()
                .to_string()
        };
        assert_ne!(message(&en), message(&de));
    }

    #[tokio::test]
    async fn relative_and_unusable_paths_are_refused() {
        let mut client = Client::start().await;
        for tool in ["project_info", "check_project", "list_courses"] {
            let (error, text) = client
                .call(
                    tool,
                    json!({ "project": "examples/berlin-viktoriapark-training" }),
                )
                .await;
            assert!(error, "{tool}");
            assert!(
                text.contains("relative")
                    && text.contains("absolute")
                    && text.contains("working folder"),
                "{text}"
            );
        }
        let (error, text) = client.call("project_info", json!({ "project": "" })).await;
        assert!(error && text.contains("empty"), "{text}");
        let (error, text) = client
            .call("project_info", json!({ "project": "/tmp/a\u{0}b" }))
            .await;
        assert!(error && text.contains("NUL"), "{text}");
        let (error, text) = client
            .call(
                "project_info",
                json!({ "project": "relativ", "lang": "de" }),
            )
            .await;
        assert!(error && text.contains("relativer Pfad"), "{text}");
        let (error, text) = client
            .call(
                "project_info",
                json!({ "project": project(), "lang": "fr" }),
            )
            .await;
        assert!(
            error && text.contains("\"fr\"") && text.contains("de"),
            "{text}"
        );
    }

    #[tokio::test]
    async fn missing_or_broken_project_gives_a_clear_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing");
        let mut client = Client::start().await;
        let (error, text) = client
            .call("check_project", json!({ "project": missing }))
            .await;
        assert!(error, "{text}");
        assert!(
            text.contains("could not load the project") && text.contains("missing"),
            "{text}"
        );
        let broken = dir.path().join("broken");
        std::fs::create_dir(&broken).unwrap();
        std::fs::write(broken.join("freemapper-print.toml"), "this is [no toml").unwrap();
        let (error, text) = client
            .call("project_info", json!({ "project": broken }))
            .await;
        assert!(error, "{text}");
        assert!(text.contains("could not load the project"), "{text}");
        // The server still answers.
        let (error, text) = client
            .call("list_courses", json!({ "project": project() }))
            .await;
        assert!(!error, "{text}");
    }

    #[tokio::test]
    async fn a_window_mask_is_written_and_the_next_write_sees_it() {
        let dir = viktoriapark_copy();
        let project = folder(&dir);
        let before = file_text(&dir);
        let mut client = Client::start().await;
        let window = client
            .ok(
                "masks_window",
                json!({ "project": project, "course": "Training", "controls": "1-3", "assign": true }),
            )
            .await;
        assert_eq!(window["id"], "window-Training", "{window}");
        assert_eq!(window["assigned"]["layout"], "a4-4000");
        assert_eq!(window["windows"][1]["code"], "34");
        let messages = window["messages"].as_array().unwrap();
        assert!(
            messages[0].as_str().unwrap().contains("window-Training"),
            "{window}"
        );
        let text = file_text(&dir);
        // Everything before stays, comments included: the assignment gets its mask line, the mask follows.
        let without = text.replacen("mask = \"window-Training\"\n", "", 1);
        assert!(without.starts_with(before.trim_end()), "{text}");
        assert!(
            text.contains("[[masks]]\nid = \"window-Training\""),
            "{text}"
        );
        assert!(
            text.contains("[[assignments]]\nmask = \"window-Training\"\ncourse = \"Training\""),
            "{text}"
        );
        // The second write sees the first: the id is taken and the course has its mask.
        let taken = client
            .refused(
                "masks_window",
                json!({ "project": project, "course": "Training", "id": "window-Training" }),
            )
            .await;
        assert!(taken.contains("window-Training"), "{taken}");
        let has_mask = client
            .refused(
                "masks_corridor",
                json!({ "project": project, "points": "10,10;40,20", "course": "Training", "assign": true }),
            )
            .await;
        assert!(has_mask.contains("window-Training"), "{has_mask}");
        let accepted = client
            .ok(
                "numbers_accept",
                json!({
                    "project": project, "course": "Training", "control": "34", "scope": "sheet",
                    "direction": 45.0, "gap": 0.74
                }),
            )
            .await;
        assert_eq!(
            accepted["scope"],
            json!({ "kind": "course", "name": "Training" })
        );
        let after = file_text(&dir);
        assert!(after.starts_with(&text), "{after}");
        assert!(after.contains("[[numbers]]"), "{after}");
        let info = client
            .ok("project_info", json!({ "project": project }))
            .await;
        assert_eq!(info["courses"][0]["masks"], json!(["window-Training"]));
        let (error, check) = client
            .call("check_project", json!({ "project": project }))
            .await;
        assert!(!error, "{check}");
    }

    #[tokio::test]
    async fn bad_corridor_and_number_arguments_are_refused_and_nothing_is_written() {
        let dir = viktoriapark_copy();
        let project = folder(&dir);
        let before = file_text(&dir);
        let mut client = Client::start().await;
        let width = client
            .refused(
                "masks_corridor",
                json!({ "project": project, "points": "10,10;40,20", "width": 0.01 }),
            )
            .await;
        assert!(width.contains("0.01"), "{width}");
        let no_course = client
            .refused(
                "masks_corridor",
                json!({ "project": project, "points": "10,10;40,20", "assign": true }),
            )
            .await;
        assert!(no_course.contains("assign needs a course"), "{no_course}");
        let half = client
            .refused(
                "numbers_accept",
                json!({
                    "project": project, "course": "Training", "control": "34", "scope": "sheet",
                    "direction": 45.0
                }),
            )
            .await;
        assert!(half.contains("direction and gap go together"), "{half}");
        let scope = client
            .refused(
                "numbers_accept",
                json!({ "project": project, "course": "Training", "control": "34", "scope": "both" }),
            )
            .await;
        assert!(scope.contains("layout_wide"), "{scope}");
        assert_eq!(file_text(&dir), before);
    }

    #[tokio::test]
    async fn numbers_accept_takes_the_proposal_of_check_on_luebeck() {
        let dir = luebeck_copy();
        let project = folder(&dir);
        let before = file_text(&dir);
        let mut client = Client::start().await;
        let accepted = client
            .ok(
                "numbers_accept",
                json!({ "project": project, "course": "A", "control": "49", "scope": "sheet", "lang": "de" }),
            )
            .await;
        assert_eq!(accepted["direction"], 161.0, "{accepted}");
        assert_eq!(accepted["gap_mm"], 0.5, "{accepted}");
        assert_eq!(accepted["proposed"], true);
        assert!(
            accepted["messages"][0].as_str().unwrap().contains("49"),
            "{accepted}"
        );
        let text = file_text(&dir);
        assert!(text.starts_with(before.trim_end()), "{text}");
        assert!(
            text.contains("[[numbers]]\ncontrol = \"49\"\ncourse = \"A\""),
            "{text}"
        );
        let again = client
            .refused(
                "numbers_accept",
                json!({ "project": project, "course": "A", "control": "49", "scope": "sheet" }),
            )
            .await;
        assert!(again.contains("49"), "{again}");
        assert_eq!(file_text(&dir), text);
    }

    #[tokio::test]
    async fn layouts_and_elements_are_added_and_changed_and_the_project_still_checks() {
        let dir = viktoriapark_copy();
        let project = folder(&dir);
        let mut client = Client::start().await;
        let added = client
            .ok(
                "set_layout",
                json!({ "project": project, "id": "a3", "paper": "A3", "print_scale": 3000 }),
            )
            .await;
        assert_eq!(added["change"], "added", "{added}");
        assert_eq!(added["layout"]["paper"], "A3");
        assert_eq!(added["layout"]["map_frame"]["margin_mm"], 8.0);
        let changed = client
            .ok(
                "set_layout",
                json!({ "project": project, "id": "a3", "margin_mm": 12.0, "orientation": "landscape" }),
            )
            .await;
        assert_eq!(changed["change"], "changed", "{changed}");
        // The keys left out keep their value.
        assert_eq!(changed["layout"]["print_scale"], 3000);
        assert_eq!(changed["layout"]["map_frame"]["margin_mm"], 12.0);
        let same = client
            .ok(
                "set_layout",
                json!({ "project": project, "id": "a3", "margin_mm": 12.0 }),
            )
            .await;
        assert_eq!(same["change"], "unchanged");
        // null prints the course layout at the map scale again.
        let scale = client
            .ok(
                "set_layout",
                json!({ "project": project, "id": "a4-4000", "print_scale": null }),
            )
            .await;
        assert_eq!(scale["layout"]["print_scale"], Value::Null, "{scale}");
        assert_eq!(scale["layout"]["elements"][0]["id"], "descriptions");
        let note = json!({
            "project": project, "layout": "a3", "id": "note", "kind": "text",
            "anchor": { "to": "page", "corner": "bottom_left", "offset_mm": [5.0, 5.0] },
            "lines": ["Start 10:00"]
        });
        let element = client.ok("set_element", note.clone()).await;
        assert_eq!(element["change"], "added", "{element}");
        let mut again = note.clone();
        again["lines"] = json!(["Start 10:30", "{event}"]);
        let element = client.ok("set_element", again).await;
        assert_eq!(element["change"], "changed", "{element}");
        let shared = client
            .ok(
                "set_element",
                json!({
                    "project": project, "id": "credit", "kind": "credit_line", "size_pt": 6.0,
                    "anchor": { "to": "page", "corner": "bottom_right" }
                }),
            )
            .await;
        assert_eq!(shared["layout"], Value::Null, "{shared}");
        // A value the project file refuses is not written.
        let before = file_text(&dir);
        let refused = client
            .refused(
                "set_element",
                json!({
                    "project": project, "layout": "a3", "id": "bad", "kind": "scale_check_bar",
                    "lines": ["x"], "anchor": { "to": "page", "corner": "top" }
                }),
            )
            .await;
        assert!(
            refused.contains("was not changed") && refused.contains("lines"),
            "{refused}"
        );
        assert_eq!(file_text(&dir), before);
        let text = file_text(&dir);
        assert!(text.contains("# Vorschlag für Sprintkarten."), "{text}");
        assert!(
            text.contains("lines = [\n    \"Start 10:30\",\n    \"{event}\",\n]"),
            "{text}"
        );
        let check = client
            .ok("check_project", json!({ "project": project }))
            .await;
        let layouts: Vec<&str> = check["layouts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["layout"].as_str().unwrap())
            .collect();
        assert!(layouts.contains(&"a3"), "{check}");
        let info = client
            .ok("project_info", json!({ "project": project }))
            .await;
        assert_eq!(info["layouts"][1]["elements"][0]["id"], "note", "{info}");
        assert_eq!(info["shared_elements"][2]["id"], "credit", "{info}");
    }

    #[tokio::test]
    async fn assign_mask_takes_known_masks_only() {
        let dir = viktoriapark_copy();
        let project = folder(&dir);
        let mut client = Client::start().await;
        let before = file_text(&dir);
        let unknown = client
            .refused(
                "assign_mask",
                json!({ "project": project, "course": "Training", "mask": "nope" }),
            )
            .await;
        assert!(unknown.contains("no mask variant \"nope\""), "{unknown}");
        assert_eq!(file_text(&dir), before);
        let corridor = client
            .ok(
                "masks_corridor",
                json!({ "project": project, "points": "10,10;40,20;70,15" }),
            )
            .await;
        let id = corridor["id"].as_str().unwrap().to_string();
        let unknown = client
            .refused(
                "assign_mask",
                json!({ "project": project, "course": "Training", "mask": "nope" }),
            )
            .await;
        assert!(unknown.contains(&format!("\"{id}\"")), "{unknown}");
        let assigned = client
            .ok(
                "assign_mask",
                json!({ "project": project, "course": "Training", "mask": id }),
            )
            .await;
        assert_eq!(assigned["change"], "changed", "{assigned}");
        assert_eq!(assigned["assignment"]["mask"], json!(id));
        assert_eq!(assigned["assignment"]["layout"], "a4-4000");
        let info = client
            .ok("project_info", json!({ "project": project }))
            .await;
        assert_eq!(info["courses"][0]["masks"], json!([id]));
        let removed = client
            .ok(
                "assign_mask",
                json!({ "project": project, "course": "Training" }),
            )
            .await;
        assert_eq!(removed["assignment"]["mask"], Value::Null, "{removed}");
        let course = client
            .refused(
                "assign_mask",
                json!({ "project": project, "course": "Lang" }),
            )
            .await;
        assert!(course.contains("Lang"), "{course}");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_project_file_that_is_a_link_is_not_written() {
        let dir = viktoriapark_copy();
        let real = dir.path().join("real.toml");
        let link = dir.path().join(project::FILE_NAME);
        std::fs::rename(&link, &real).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let before = std::fs::read_to_string(&real).unwrap();
        let project = folder(&dir);
        let mut client = Client::start().await;
        for (tool, arguments) in [
            (
                "masks_window",
                json!({ "project": project, "course": "Training", "assign": true }),
            ),
            (
                "numbers_accept",
                json!({
                    "project": project, "course": "Training", "control": "34", "scope": "sheet",
                    "direction": 45.0, "gap": 0.7
                }),
            ),
            ("set_layout", json!({ "project": link, "id": "a3" })),
            (
                "assign_mask",
                json!({ "project": project, "course": "Training" }),
            ),
        ] {
            let message = client.refused(tool, arguments).await;
            assert!(message.contains("is a link"), "{tool}: {message}");
        }
        assert_eq!(std::fs::read_to_string(&real).unwrap(), before);
        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        // Reading through the link still works.
        let (error, text) = client
            .call("project_info", json!({ "project": project }))
            .await;
        assert!(!error, "{text}");
    }

    #[tokio::test]
    async fn the_write_tools_refuse_relative_paths() {
        let mut client = Client::start().await;
        let anchor = json!({ "to": "page", "corner": "top" });
        for (tool, arguments) in [
            ("masks_window", json!({ "project": "p", "course": "A" })),
            (
                "masks_corridor",
                json!({ "project": "p", "points": "0,0;1,1" }),
            ),
            (
                "numbers_accept",
                json!({ "project": "p", "course": "A", "control": "31", "scope": "sheet" }),
            ),
            ("set_layout", json!({ "project": "p", "id": "a4" })),
            (
                "set_element",
                json!({ "project": "p", "id": "t", "kind": "text", "anchor": anchor }),
            ),
            ("assign_mask", json!({ "project": "p", "course": "A" })),
            ("init_project", json!({ "folder": "p", "map": "/m.omap" })),
            ("init_project", json!({ "folder": "/p", "map": "m.omap" })),
            (
                "init_project",
                json!({ "folder": "/p", "map": "/m.omap", "courses": "c.ppen" }),
            ),
        ] {
            let message = client.refused(tool, arguments.clone()).await;
            assert!(message.contains("relative path"), "{arguments}: {message}");
        }
        // A logo path is the agent's too.
        let dir = viktoriapark_copy();
        let message = client
            .refused(
                "set_element",
                json!({
                    "project": folder(&dir), "id": "logo", "kind": "logo", "file": "logo.png",
                    "width_mm": 20.0, "anchor": anchor
                }),
            )
            .await;
        assert!(message.contains("relative path"), "{message}");
    }

    #[tokio::test]
    async fn two_writes_sent_together_both_land() {
        let dir = viktoriapark_copy();
        let project = folder(&dir);
        let mut client = Client::start().await;
        let answers = client
            .call_together(&[
                (
                    "set_layout",
                    json!({ "project": project, "id": "a3", "paper": "A3" }),
                ),
                (
                    "set_layout",
                    json!({ "project": project, "id": "a5-like", "margin_mm": 20.0 }),
                ),
                (
                    "masks_corridor",
                    json!({ "project": project, "points": "10,10;40,20" }),
                ),
            ])
            .await;
        for (error, text) in &answers {
            assert!(!error, "{text}");
        }
        let loaded = project::load(&dir.path().join(project::FILE_NAME)).unwrap();
        let ids: Vec<&str> = loaded
            .project
            .layouts
            .iter()
            .map(|l| l.id.as_str())
            .collect();
        assert_eq!(ids.len(), 3, "{ids:?}");
        assert!(ids.contains(&"a3") && ids.contains(&"a5-like"), "{ids:?}");
        assert_eq!(loaded.project.masks.len(), 1);
    }

    #[tokio::test]
    async fn init_project_writes_a_project_once() {
        let maps = viktoriapark_copy();
        let target = tempfile::tempdir().unwrap();
        let folder = target.path().join("event");
        let mut client = Client::start().await;
        let arguments = json!({
            "folder": folder,
            "map": maps.path().join("berlin-viktoriapark.omap"),
            "courses": maps.path().join("training.ppen"),
            "lang": "en"
        });
        let init = client.ok("init_project", arguments.clone()).await;
        assert_eq!(init["kind"], "sprint", "{init}");
        assert_eq!(init["kind_source"], "sidecar", "{init}");
        assert_eq!(init["layouts"][0]["courses"], json!(["Training"]), "{init}");
        assert_eq!(init["assignments"], 1);
        let messages: Vec<&str> = init["messages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m.as_str().unwrap())
            .collect();
        assert!(
            messages.iter().any(|m| m.contains("Training")),
            "{messages:?}"
        );
        assert!(folder.join(project::FILE_NAME).is_file());
        let exists = client.refused("init_project", arguments).await;
        assert!(exists.contains("exists"), "{exists}");
        let (error, text) = client
            .call("check_project", json!({ "project": folder }))
            .await;
        assert!(!error, "{text}");
    }

    /// The number of pages of a PDF, from its page tree.
    fn page_count(pdf: &[u8]) -> usize {
        let text = String::from_utf8_lossy(pdf);
        let i = text.find("/Count ").unwrap() + 7;
        text[i..]
            .split(|c: char| !c.is_ascii_digit())
            .next()
            .unwrap()
            .parse()
            .unwrap()
    }

    /// The names in `dir`, sorted.
    fn names_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// Every file in `dir` is a whole PDF: no partial file, nothing cut short.
    fn only_whole_pdfs(dir: &Path) -> usize {
        let names = names_in(dir);
        for name in &names {
            assert!(name.ends_with(".pdf"), "{name} in {names:?}");
            let bytes = std::fs::read(dir.join(name)).unwrap();
            assert!(bytes.starts_with(b"%PDF"), "{name}");
            assert!(bytes.trim_ascii_end().ends_with(b"%%EOF"), "{name}");
        }
        names.len()
    }

    /// The hostile case of ADR 0040 (ruling R41): a course file of 1000 courses with 200 controls each, on a copy
    /// of the Lübeck map, every course on one A3 layout. Exporting it all takes minutes.
    fn hostile_copy() -> tempfile::TempDir {
        let from = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/luebeck-competition-courses"
        ));
        let dir = tempfile::tempdir().unwrap();
        for name in ["luebeck.omap", "freemapper.toml"] {
            std::fs::copy(from.join(name), dir.path().join(name)).unwrap();
        }
        std::fs::write(dir.path().join("hostile.xml"), hostile_courses(1000, 200)).unwrap();
        std::fs::write(
            dir.path().join(project::FILE_NAME),
            "format = 1\n\n[event]\ntitle = \"Hostile\"\nlang = \"en\"\n\n[sources]\n\
             map = \"luebeck.omap\"\ncourses = \"hostile.xml\"\n\n[map]\nkind = \"sprint\"\n\n\
             [[layouts]]\nid = \"a3\"\npaper = \"A3\"\nmap_frame = { crop = true }\n",
        )
        .unwrap();
        dir
    }

    /// IOF XML with `courses` courses, each visiting `controls` controls on a grid over the Lübeck map in its own
    /// shuffled order, so that legs cross and numbers collide.
    fn hostile_courses(courses: usize, controls: usize) -> String {
        let mut xml = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<CourseData \
             xmlns=\"http://www.orienteering.org/datastandard/3.0\" iofVersion=\"3.0\">\n\
             <RaceCourseData><Map><Scale>4000</Scale></Map>\n\
             <Control type=\"Start\"><Id>S1</Id><MapPosition x=\"-100\" y=\"0\" unit=\"mm\"/></Control>\n",
        );
        for i in 0..controls {
            let (x, y) = (-95 + (i % 20) as i32 * 10, -70 + (i / 20) as i32 * 15);
            xml += &format!(
                "<Control type=\"Control\"><Id>{}</Id><MapPosition x=\"{x}\" y=\"{y}\" unit=\"mm\"/></Control>\n",
                100 + i
            );
        }
        xml += "<Control type=\"Finish\"><Id>F1</Id><MapPosition x=\"100\" y=\"0\" unit=\"mm\"/></Control>\n";
        for c in 0..courses {
            xml += &format!(
                "<Course><Name>K{}</Name><CourseControl type=\"Start\"><Control>S1</Control></CourseControl>",
                c + 1
            );
            let mut order: Vec<usize> = (0..controls).collect();
            let mut seed = (c * 7919 + 1) as u64;
            for i in (1..controls).rev() {
                seed = (seed * 1_103_515_245 + 12_345) % (1 << 31);
                order.swap(i, seed as usize % (i + 1));
            }
            for k in order {
                xml += &format!(
                    "<CourseControl type=\"Control\"><Control>{}</Control></CourseControl>",
                    100 + k
                );
            }
            xml +=
                "<CourseControl type=\"Finish\"><Control>F1</Control></CourseControl></Course>\n";
        }
        xml + "</RaceCourseData></CourseData>\n"
    }

    #[tokio::test]
    async fn export_project_writes_the_pdf_the_cli_writes() {
        let dir = viktoriapark_copy();
        let out = tempfile::tempdir().unwrap();
        let mut client = Client::start().await;
        let export = client
            .ok(
                "export_project",
                json!({ "project": folder(&dir), "out_dir": out.path() }),
            )
            .await;
        let pdf = out.path().join("Training.pdf");
        // Byte for byte the PDF in the example, which the CLI wrote.
        let committed = std::fs::read(viktoriapark().join("out/Training.pdf")).unwrap();
        assert!(std::fs::read(&pdf).unwrap() == committed, "{export}");
        assert_eq!(export["out_dir"], json!(out.path()));
        assert_eq!(
            export["files"],
            json!([{
                "path": pdf, "bytes": committed.len(), "course": "Training", "layout": "a4-4000",
                "paper": "A4",
                "print": export["files"][0]["print"].clone(),
            }])
        );
        assert_eq!(export["files"][0]["print"]["print_scale"], 4000);
        let kinds: Vec<&str> = export["notes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["kind"].as_str().unwrap())
            .collect();
        assert_eq!(kinds, ["fit_georef_skipped", "map_cropped"], "{export}");
        assert!(export.get("more").is_none(), "{export}");
        let reminder = Texts::new(&Lang::english()).plain("reminder-actual-size");
        assert_eq!(export["reminder"], json!(reminder));
        assert!(reminder.contains("100 %"), "{reminder}");
        assert_eq!(names_in(out.path()), ["Training.pdf"]);
        // The map's own folder can take the PDFs: a PDF never has the map's name, and the map stays as it was.
        let map = std::fs::read(dir.path().join("berlin-viktoriapark.omap")).unwrap();
        let export = client
            .ok(
                "export_project",
                json!({ "project": folder(&dir), "out_dir": dir.path(), "lang": "de" }),
            )
            .await;
        assert_eq!(
            std::fs::read(dir.path().join("berlin-viktoriapark.omap")).unwrap(),
            map
        );
        assert!(std::fs::read(dir.path().join("Training.pdf")).unwrap() == committed);
        assert!(
            export["reminder"]
                .as_str()
                .unwrap()
                .contains("Originalgröße"),
            "{export}"
        );
    }

    #[tokio::test]
    async fn single_file_puts_the_luebeck_courses_on_three_pages() {
        let dir = luebeck_copy();
        let out = tempfile::tempdir().unwrap();
        let mut client = Client::start().await;
        let export = client
            .ok(
                "export_project",
                json!({ "project": folder(&dir), "out_dir": out.path(), "single_file": true }),
            )
            .await;
        let files = export["files"].as_array().unwrap();
        assert_eq!(files.len(), 1, "{export}");
        assert_eq!(files[0]["pages"], 3, "{export}");
        assert!(files[0].get("course").is_none(), "{export}");
        let path = PathBuf::from(files[0]["path"].as_str().unwrap());
        assert_eq!(
            path,
            out.path().join("Stadtsprint Lübeck - alle Bahnen.pdf")
        );
        let pdf = std::fs::read(&path).unwrap();
        assert_eq!(files[0]["bytes"], pdf.len());
        assert_eq!(page_count(&pdf), 3);
        assert_eq!(only_whole_pdfs(out.path()), 1);
        let courses: Vec<&str> = export["notes"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|n| n["course"].as_str())
            .collect();
        for course in ["A", "B", "C"] {
            assert!(courses.contains(&course), "{export}");
        }
        // Courses by name, and layouts as such.
        let some = client
            .ok(
                "export_project",
                json!({ "project": folder(&dir), "out_dir": out.path(), "courses": ["C", "A"] }),
            )
            .await;
        let picked: Vec<(&str, &str)> = some["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| (f["course"].as_str().unwrap(), f["layout"].as_str().unwrap()))
            .collect();
        assert_eq!(picked, [("C", "a3-3000"), ("A", "a4-4000")], "{some}");
        let layouts = client
            .ok(
                "export_project",
                json!({ "project": folder(&dir), "out_dir": out.path(), "layouts": ["a3-3000"] }),
            )
            .await;
        assert_eq!(layouts["files"][0]["layout"], "a3-3000", "{layouts}");
        assert!(layouts["files"][0].get("course").is_none(), "{layouts}");
        assert_eq!(only_whole_pdfs(out.path()), 4);
    }

    #[tokio::test]
    async fn export_project_refuses_what_the_cli_refuses() {
        let dir = viktoriapark_copy();
        let out = tempfile::tempdir().unwrap();
        let project = folder(&dir);
        let mut client = Client::start().await;
        for arguments in [
            json!({ "project": project, "out_dir": out.path(), "layouts": ["a4-4000"], "courses": ["Training"] }),
            json!({ "project": project, "out_dir": out.path(), "layouts": ["a4-4000"], "single_file": true }),
        ] {
            let message = client.refused("export_project", arguments).await;
            assert!(
                message.contains("does not go with courses or single_file"),
                "{message}"
            );
        }
        let unknown = client
            .refused(
                "export_project",
                json!({ "project": project, "out_dir": out.path(), "courses": ["Training", "Lang"] }),
            )
            .await;
        assert!(
            unknown.contains("\"Lang\"") && unknown.contains("\"Training\""),
            "{unknown}"
        );
        let layout = client
            .refused(
                "export_project",
                json!({ "project": project, "out_dir": out.path(), "layouts": ["a5"] }),
            )
            .await;
        assert!(
            layout.contains("a5") && layout.contains("a4-4000"),
            "{layout}"
        );
        let relative = client
            .refused(
                "export_project",
                json!({ "project": project, "out_dir": "out" }),
            )
            .await;
        assert!(relative.contains("relative path"), "{relative}");
        let missing = out.path().join("missing");
        let message = client
            .refused(
                "export_project",
                json!({ "project": project, "out_dir": missing }),
            )
            .await;
        assert!(
            message.contains("does not exist") && message.contains("missing"),
            "{message}"
        );
        // A file is no folder.
        let file = dir.path().join("training.ppen");
        let message = client
            .refused(
                "export_project",
                json!({ "project": project, "out_dir": file }),
            )
            .await;
        assert!(message.contains("does not exist"), "{message}");
        let relative = client
            .refused(
                "export_project",
                json!({ "project": "p", "out_dir": out.path() }),
            )
            .await;
        assert!(relative.contains("relative path"), "{relative}");
        assert!(names_in(out.path()).is_empty());
        assert!(!dir.path().join("out").exists());
    }

    #[tokio::test]
    async fn a_hostile_export_comes_back_with_its_notes_capped() {
        let dir = hostile_copy();
        let out = tempfile::tempdir().unwrap();
        let mut client = Client::start().await;
        let started = std::time::Instant::now();
        let export = client
            .ok(
                "export_project",
                json!({ "project": folder(&dir), "out_dir": out.path(), "courses": ["K1", "K2"] }),
            )
            .await;
        let took = started.elapsed();
        eprintln!("hostile export of two courses: {took:?}");
        assert!(took < std::time::Duration::from_secs(120), "{took:?}");
        assert_eq!(export["files"].as_array().unwrap().len(), 2);
        assert_eq!(export["notes"].as_array().unwrap().len(), MAX_NOTES);
        assert!(export["more"].as_u64().unwrap() > 0, "{}", export["more"]);
        assert_eq!(only_whole_pdfs(out.path()), 2);
    }

    #[tokio::test]
    async fn cancelling_a_hostile_export_stops_it_after_the_sheet_in_hand() {
        let dir = hostile_copy();
        let out = tempfile::tempdir().unwrap();
        let mut client = Client::start().await;
        let id = client.next_id;
        client.next_id += 1;
        client
            .send(json!({
                "jsonrpc": "2.0", "id": id, "method": "tools/call",
                "params": {
                    "name": "export_project",
                    "arguments": { "project": folder(&dir), "out_dir": out.path() },
                },
            }))
            .await;
        // Wait for the first course PDF, then cancel the call.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
        while !names_in(out.path()).iter().any(|n| n.ends_with(".pdf")) {
            assert!(std::time::Instant::now() < deadline, "no PDF after 300 s");
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        client
            .send(json!({
                "jsonrpc": "2.0", "method": "notifications/cancelled",
                "params": { "requestId": id, "reason": "test" },
            }))
            .await;
        let cancelled = std::time::Instant::now();
        // The next export waits for the lock, so it answers once the cancelled one has stopped.
        let other = viktoriapark_copy();
        let other_out = tempfile::tempdir().unwrap();
        client
            .ok(
                "export_project",
                json!({ "project": folder(&other), "out_dir": other_out.path() }),
            )
            .await;
        let took = cancelled.elapsed();
        eprintln!("cancelled hostile export stopped within {took:?}");
        assert!(took < std::time::Duration::from_secs(60), "{took:?}");
        let written = only_whole_pdfs(out.path());
        assert!((1..1000).contains(&written), "{written}");
    }

    #[tokio::test]
    async fn messages_name_the_tools_and_fields_not_the_cli_flags() {
        let dir = viktoriapark_copy();
        let project = folder(&dir);
        let mut client = Client::start().await;
        for lang in ["en", "de"] {
            let id = format!("window-{lang}");
            let window = client
                .ok(
                    "masks_window",
                    json!({ "project": project, "course": "Training", "id": id, "lang": lang }),
                )
                .await;
            let messages: Vec<&str> = window["messages"]
                .as_array()
                .unwrap()
                .iter()
                .map(|m| m.as_str().unwrap())
                .collect();
            assert!(
                messages.iter().any(|m| m.contains("assign_mask")),
                "{messages:?}"
            );
            assert!(
                messages.iter().any(|m| m.contains("check_project")),
                "{messages:?}"
            );
            let taken = client
                .refused(
                    "masks_window",
                    json!({ "project": project, "course": "Training", "id": id, "lang": lang }),
                )
                .await;
            let radius_for = client
                .refused(
                    "masks_window",
                    json!({ "project": project, "course": "Training", "radius_for": ["35:20"], "lang": lang }),
                )
                .await;
            assert!(radius_for.contains("radius_for"), "{radius_for}");
            let no_proposal = client
                .refused(
                    "numbers_accept",
                    json!({ "project": project, "course": "Training", "control": "34", "scope": "sheet", "lang": lang }),
                )
                .await;
            assert!(no_proposal.contains("direction"), "{no_proposal}");
            for text in messages.iter().copied().chain([
                taken.as_str(),
                radius_for.as_str(),
                no_proposal.as_str(),
            ]) {
                assert!(!text.contains("--"), "{text}");
            }
        }
        // A project without a course file: init's messages and the export refusal name the tools and fields.
        let target = tempfile::tempdir().unwrap();
        let event = target.path().join("event");
        let init = client
            .ok(
                "init_project",
                json!({ "folder": event, "map": dir.path().join("berlin-viktoriapark.omap") }),
            )
            .await;
        let messages = init["messages"].to_string();
        assert!(messages.contains("check_project"), "{messages}");
        assert!(!messages.contains("--"), "{messages}");
        let out = tempfile::tempdir().unwrap();
        let refused = client
            .refused(
                "export_project",
                json!({ "project": event, "out_dir": out.path(), "single_file": true }),
            )
            .await;
        assert!(refused.contains("single_file"), "{refused}");
        assert!(!refused.contains("--"), "{refused}");
    }

    #[test]
    fn export_files_past_the_cap_are_counted() {
        let dir = luebeck_copy();
        let out = tempfile::tempdir().unwrap();
        let result = ops::export::export(
            &ops::export::ExportArgs {
                project: dir.path().to_path_buf(),
                layouts: Vec::new(),
                courses: Vec::new(),
                single_file: false,
                out_dir: Some(out.path().to_path_buf()),
            },
            &mut |_| ControlFlow::Continue(()),
        )
        .unwrap();
        assert_eq!(result.files.len(), 3);
        let texts = Texts::for_agent(&Lang::english());
        let capped = serde_json::to_value(export_out(&texts, &result, 2)).unwrap();
        let courses: Vec<&str> = capped["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["course"].as_str().unwrap())
            .collect();
        assert_eq!(courses, ["A", "B"], "{capped}");
        assert_eq!(capped["more_files"], 1, "{capped}");
        // Under the cap the field is left out.
        let all = serde_json::to_value(export_out(&texts, &result, MAX_FILES)).unwrap();
        assert_eq!(all["files"].as_array().unwrap().len(), 3);
        assert!(all.get("more_files").is_none(), "{all}");
    }

    #[test]
    fn note_budget_keeps_the_first_and_counts_the_rest() {
        let mut budget = NoteBudget::new(5);
        assert_eq!(budget.take(vec![1, 2, 3]), [1, 2, 3]);
        assert_eq!(budget.take(vec![4, 5, 6, 7]), [4, 5]);
        assert_eq!(budget.take(vec![8]), Vec::<i32>::new());
        assert_eq!(budget.cut, 3);
    }

    #[test]
    fn absolute_path_refuses_relative_empty_and_nul() {
        let texts = Texts::new(&Lang::english());
        let absolute = std::env::temp_dir().join("project");
        assert_eq!(
            absolute_path(&texts, absolute.to_str().unwrap()).unwrap(),
            absolute
        );
        for (path, expected) in [
            ("project", "relative"),
            ("./project", "relative"),
            ("", "empty"),
            ("/a\u{0}b", "NUL"),
        ] {
            let err = absolute_path(&texts, path).unwrap_err();
            assert!(err.message.contains(expected), "{path:?}: {}", err.message);
        }
    }
}
