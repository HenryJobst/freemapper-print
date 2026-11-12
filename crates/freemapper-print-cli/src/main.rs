//! The `freemapper-print` binary. Commands come with the milestones in the design
//! (`docs/superpowers/specs/2026-11-08-freemapper-print-design.md`).

mod ops;
mod text;

use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow, bail};
use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
use fluent_bundle::FluentArgs;
use freemapper_print::course::fit::{SkipReason, YAxis};
use freemapper_print::course::{CourseNote, FitNote};
use freemapper_print::courses::{CorrectionScope, MaskNote, Stale, StopName};
use freemapper_print::export::ExportError;
use freemapper_print::mask_build::{self, BuildError};
use freemapper_print::numbers::{Conflict, NumberConflict};
use freemapper_print::omap::MapDocument;
use freemapper_print::output::{self, OutputError};
use freemapper_print::print::{Orientation, Paper, PrintOptions, PrintReport, print_map};
use freemapper_print::project::{self, SheetLang};
use freemapper_print::scale::{MapKind, is_standard};
use freemapper_print::sheet::{
    FooterContent, LogoSimplification, LogoSkipReason, MaskedObject, MissingItem, NoBoxReason,
    SheetNote,
};

use text::{Lang, Texts, detect_lang};

#[derive(Parser)]
// The help texts come from the catalogs (see `localised_command`).
#[command(name = "freemapper-print", version, arg_required_else_help = true)]
struct Cli {
    #[arg(long = "lang", global = true, value_name = "LANG")]
    _lang: Option<String>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Render(RenderArgs),
    Init(InitArgs),
    Check(CheckArgs),
    Export(ExportArgs),
    Numbers(NumbersArgs),
    Masks(MasksArgs),
    Edit(editor::EditArgs),
    Mcp,
    Update(UpdateArgs),
}

impl Commands {
    /// Whether the daily update check runs after this command (ADR 0060, P3): the commands a
    /// person looks at the result of. Mask and number edits come in series and `check` follows
    /// them; `mcp` and `edit` tell at their start instead; `update` is the check itself.
    fn update_check(&self) -> bool {
        matches!(
            self,
            Commands::Init(_) | Commands::Check(_) | Commands::Export(_) | Commands::Render(_)
        )
    }
}

#[derive(Args)]
struct UpdateArgs {
    #[arg(long, conflicts_with_all = ["yes", "rollback"])]
    check: bool,
    #[arg(long)]
    yes: bool,
    #[arg(long, value_name = "X.Y.Z")]
    version: Option<freemapper_update::Version>,
    #[arg(long, conflicts_with = "version")]
    rollback: bool,
    #[arg(long, value_enum, conflicts_with_all = ["check", "yes", "version", "rollback"])]
    mode: Option<UpdateModeArg>,
}

#[derive(Clone, Copy, ValueEnum)]
enum UpdateModeArg {
    Notify,
    Install,
    Off,
}

impl From<UpdateModeArg> for freemapper_update::Mode {
    fn from(mode: UpdateModeArg) -> Self {
        match mode {
            UpdateModeArg::Notify => Self::Notify,
            UpdateModeArg::Install => Self::Install,
            UpdateModeArg::Off => Self::Off,
        }
    }
}

#[derive(Args)]
struct RenderArgs {
    map: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
    scale: Option<u32>,
    #[arg(long, value_enum, default_value_t = PaperArg::A4)]
    paper: PaperArg,
    #[arg(long, value_enum, default_value_t = OrientationArg::Auto)]
    orientation: OrientationArg,
    #[arg(long, value_enum)]
    kind: Option<KindArg>,
    #[arg(long)]
    show_sheet_objects: bool,
}

#[derive(Args)]
struct InitArgs {
    folder: PathBuf,
    #[arg(long)]
    map: PathBuf,
    #[arg(long)]
    title: Option<String>,
    #[arg(long, value_enum)]
    kind: Option<KindArg>,
    #[arg(long, value_name = "FILE")]
    courses: Option<PathBuf>,
}

#[derive(Args)]
struct CheckArgs {
    project: PathBuf,
}

#[derive(Args)]
struct ExportArgs {
    project: PathBuf,
    #[arg(long = "layout", value_name = "ID", conflicts_with_all = ["course", "single_file"])]
    layout: Vec<String>,
    #[arg(long = "course", value_name = "NAME")]
    course: Vec<String>,
    #[arg(long)]
    single_file: bool,
    #[arg(long, value_name = "DIR")]
    out_dir: Option<PathBuf>,
}

#[derive(Args)]
struct NumbersArgs {
    #[command(subcommand)]
    command: NumbersCommand,
}

#[derive(Subcommand)]
enum NumbersCommand {
    Accept(AcceptArgs),
}

#[derive(Args)]
#[command(group(clap::ArgGroup::new("scope").required(true).args(["layout_wide", "sheet"])))]
struct AcceptArgs {
    project: PathBuf,
    #[arg(long, value_name = "NAME")]
    course: String,
    #[arg(long, value_name = "CODE")]
    control: String,
    #[arg(long)]
    layout_wide: bool,
    #[arg(long)]
    sheet: bool,
    #[arg(long, requires = "gap", allow_negative_numbers = true)]
    direction: Option<f64>,
    #[arg(long, requires = "direction", allow_negative_numbers = true)]
    gap: Option<f64>,
}

#[derive(Args)]
struct MasksArgs {
    #[command(subcommand)]
    command: MasksCommand,
}

#[derive(Subcommand)]
enum MasksCommand {
    Window(WindowArgs),
    Corridor(CorridorArgs),
}

#[derive(Args)]
struct WindowArgs {
    project: PathBuf,
    #[arg(long, value_name = "NAME")]
    course: String,
    #[arg(long, value_name = "LIST|RANGE")]
    controls: Option<String>,
    #[arg(long, value_name = "MM", allow_negative_numbers = true)]
    radius: Option<f64>,
    #[arg(long = "radius-for", value_name = "CODE=MM")]
    radius_for: Vec<String>,
    #[arg(long)]
    no_start: bool,
    #[arg(long)]
    no_finish: bool,
    #[arg(long, value_name = "ID")]
    id: Option<String>,
    #[arg(long)]
    assign: bool,
}

#[derive(Args)]
struct CorridorArgs {
    project: PathBuf,
    #[arg(long, value_name = "X,Y;X,Y;…", allow_hyphen_values = true)]
    points: String,
    #[arg(long, value_name = "MM", allow_negative_numbers = true)]
    width: Option<f64>,
    #[arg(long, value_name = "ID")]
    id: Option<String>,
    #[arg(long, requires = "course")]
    assign: bool,
    #[arg(long, value_name = "NAME")]
    course: Option<String>,
}

#[derive(Clone, Copy, ValueEnum)]
enum PaperArg {
    A4,
    A3,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OrientationArg {
    Auto,
    Portrait,
    Landscape,
}

#[derive(Clone, Copy, ValueEnum)]
enum KindArg {
    Sprint,
    Schoolyard,
    Forest,
}

impl From<PaperArg> for Paper {
    fn from(p: PaperArg) -> Self {
        match p {
            PaperArg::A4 => Paper::A4,
            PaperArg::A3 => Paper::A3,
        }
    }
}

impl From<OrientationArg> for Orientation {
    fn from(o: OrientationArg) -> Self {
        match o {
            OrientationArg::Auto => Orientation::Auto,
            OrientationArg::Portrait => Orientation::Portrait,
            OrientationArg::Landscape => Orientation::Landscape,
        }
    }
}

impl From<KindArg> for MapKind {
    fn from(k: KindArg) -> Self {
        match k {
            KindArg::Sprint => MapKind::Sprint,
            KindArg::Schoolyard => MapKind::Schoolyard,
            KindArg::Forest => MapKind::Forest,
        }
    }
}

/// A wrong command line found after clap parsed it, such as an unknown layout id: exit code 2.
#[derive(Debug)]
struct UsageError(String);

impl std::fmt::Display for UsageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for UsageError {}

/// Message arguments: `args!("name" => value, ...)`.
macro_rules! args {
    ($($name:literal => $value:expr),* $(,)?) => {{
        let mut a = FluentArgs::new();
        $(a.set($name, $value);)*
        a
    }};
}

// After `args!`, which it uses.
mod editor;
mod mcp;
// The self-update (ADR 0060); after `args!`, which its command uses.
mod update;

fn render(texts: &Texts, args: &RenderArgs) -> anyhow::Result<u8> {
    let map = read_map(texts, &args.map)?;
    let (kind, _) = ops::init::detect_kind(
        &map,
        &args.map,
        args.kind.map(MapKind::from),
        &mut |error| sidecar_note(texts, error),
    )
    .map_err(|e| op_error(texts, e))?;
    let parent = match args.out.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    if !parent.is_dir() {
        bail!(texts.get(
            "error-out-folder-missing",
            Some(&args!("path" => parent.display().to_string()))
        ));
    }
    if same_file(&args.map, &args.out) {
        bail!(texts.get(
            "error-out-is-map",
            Some(&args!("path" => args.map.display().to_string()))
        ));
    }
    let options = PrintOptions {
        print_scale: args.scale,
        kind,
        paper: args.paper.into(),
        page_mm: None,
        orientation: args.orientation.into(),
        centre: None,
        show_sheet_objects: args.show_sheet_objects,
        frame_margin_mm: None,
    };
    let title = args
        .map
        .file_stem()
        .map_or_else(|| "map".into(), |s| s.to_string_lossy().into_owned());
    // Library errors keep their English text for now (ADR 0024); the lead-in is translated.
    let (pdf, report, footer_notes) = print_map(&map, &options, &title, sheet_lang(texts))
        .with_context(|| texts.plain("error-print"))?;
    freemapper_print::output::write_pdf_file(&args.out, &pdf)
        .map_err(|e| output_error(texts, &e))?;
    let mut lines = report_lines(
        texts,
        &report,
        args.paper,
        args.orientation,
        kind,
        map.georef.scale_denominator,
        &args.out,
    );
    // The footer notes go before the closing reminder.
    let reminder = lines.pop();
    lines.extend(footer_notes.iter().map(|n| render_note_line(texts, n)));
    lines.extend(reminder);
    for line in lines {
        eprintln!("{line}");
    }
    Ok(0)
}

/// The sheet language that goes with the program language: German for `de`, else English.
fn sheet_lang(texts: &Texts) -> SheetLang {
    if texts.lang().code() == "de" {
        SheetLang::De
    } else {
        SheetLang::En
    }
}

/// The message for a note about `render`'s footer. `render` has no layout and no passe-partout
/// to widen, so two notes get their own text.
fn render_note_line(texts: &Texts, note: &SheetNote) -> String {
    match note {
        SheetNote::FooterOverlapsMap { content } => texts.get(
            "note-render-footer-on-map",
            Some(&args!("content" => footer_content(*content))),
        ),
        SheetNote::CopyrightTruncated { total, shown, .. } => texts.get(
            "note-render-copyright-truncated",
            Some(&args!("total" => *total, "shown" => *shown)),
        ),
        other => sheet_note_line(texts, other),
    }
}

fn read_map(texts: &Texts, path: &Path) -> anyhow::Result<MapDocument> {
    ops::read_map(path).map_err(|e| op_error(texts, e))
}

/// The message for an op that stopped. A wrong command line, such as an unknown course, is a [`UsageError`]
/// (exit code 2).
fn op_error(texts: &Texts, err: ops::OpError) -> anyhow::Error {
    use ops::OpError as E;
    match err {
        E::LoadProject { path, source } => source.context(texts.get(
            "error-load-project",
            Some(&args!("path" => path.display().to_string())),
        )),
        // A rule of this program rather than a typo, so it is said in the user's language (ADR 0044).
        E::BasePartHidden { path, file, field } => anyhow!(texts.get(
            "error-base-part-hidden",
            Some(&args!("path" => file, "field" => field)),
        ))
        .context(texts.get(
            "error-load-project",
            Some(&args!("path" => path.display().to_string())),
        )),
        E::ReadMap { path, source } => source.context(texts.get(
            "error-read-map",
            Some(&args!("path" => path.display().to_string())),
        )),
        E::Courses(source) => source.context(texts.plain("error-courses")),
        E::UnknownLayout { id, known } => {
            let known: Vec<String> = known.iter().map(|id| printable(id)).collect();
            UsageError(texts.get(
                "error-unknown-layout",
                Some(&args!("id" => printable(&id), "known" => known.join(", "))),
            ))
            .into()
        }
        E::UnknownCourse { name, known } => UsageError(texts.get(
            "error-unknown-course",
            Some(&args!("name" => printable(&name), "known" => quoted_list(texts, &known))),
        ))
        .into(),
        E::ExportNeedsCourseFile => UsageError(texts.plain("error-no-course-file")).into(),
        E::OutDirMissing { path } => anyhow!(texts.get(
            "error-out-dir-missing",
            Some(&args!("path" => path.display().to_string()))
        )),
        E::CreateFolder { path, source } => source.context(texts.get(
            "error-create-folder",
            Some(&args!("path" => path.display().to_string())),
        )),
        E::Output(err) => output_error(texts, &err),
        E::Export(err) => export_error(texts, &err),
        E::ExportStopped { written } => anyhow!(texts.get(
            "error-export-stopped",
            Some(&args!("count" => written.len())),
        )),
        E::MasksNeedCourseFile => UsageError(texts.plain("error-masks-no-course-file")).into(),
        E::Build { error, course } => build_error(texts, error, &course),
        E::RadiusFor { text } => UsageError(texts.get(
            "error-masks-radius-for",
            Some(&args!("text" => printable(&text))),
        ))
        .into(),
        E::MasksMapScale => anyhow!(texts.plain("error-masks-map-scale")),
        E::MaskId => UsageError(texts.get(
            "error-masks-id",
            Some(&args!("max" => freemapper_print::mask::MAX_ID_CHARS.to_string())),
        ))
        .into(),
        E::MaskIdTaken { id } => {
            UsageError(texts.get("error-masks-id-taken", Some(&args!("id" => printable(&id)))))
                .into()
        }
        E::CourseHasMask { course, mask } => UsageError(texts.get(
            "error-masks-course-has-mask",
            Some(&args!("course" => printable(&course), "mask" => printable(&mask))),
        ))
        .into(),
        E::AssignmentInline { course, mask, new } => UsageError(texts.get(
            if new {
                "error-masks-assignments-inline"
            } else {
                "error-masks-assignment-inline"
            },
            Some(&args!("course" => printable(&course), "id" => printable(&mask))),
        ))
        .into(),
        E::Serve { source } => source.context(texts.plain("error-serve")),
        E::WriteFile { path, source } => source.context(texts.get(
            "error-write-file",
            Some(&args!("path" => path.display().to_string())),
        )),
        E::LoadChangedMask { path, source } => source.context(texts.get(
            "error-load-changed-mask",
            Some(&args!("path" => path.display().to_string())),
        )),
        E::MasksScales { scales } => {
            let scales: Vec<String> = scales.iter().map(|s| format!("1:{s}")).collect();
            UsageError(texts.get(
                "error-masks-scales",
                Some(&args!("scales" => join_and(texts, scales))),
            ))
            .into()
        }
        E::NumbersNeedCourseFile => UsageError(texts.plain("error-numbers-no-course-file")).into(),
        E::UnknownControl {
            course,
            control,
            known,
        } => {
            let known: Vec<String> = known.iter().map(|c| printable(c)).collect();
            UsageError(texts.get(
                "error-unknown-control",
                Some(&args!(
                    "course" => printable(&course),
                    "control" => printable(&control),
                    "known" => known.join(", "),
                )),
            ))
            .into()
        }
        E::NumbersExists { control, scope } => UsageError(texts.get(
            "error-numbers-exists",
            Some(&args!(
                "control" => printable(&control),
                "scope" => scope_text(texts, &scope),
            )),
        ))
        .into(),
        E::NumbersValue => UsageError(texts.get(
            "error-numbers-value",
            Some(&args!("max" => project::MAX_NUMBER_GAP_MM.to_string())),
        ))
        .into(),
        E::NumbersNoProposal { control, course } => UsageError(texts.get(
            "error-numbers-no-proposal",
            Some(&args!("control" => printable(&control), "course" => printable(&course))),
        ))
        .into(),
        E::NumbersLayoutConflict {
            control,
            layout,
            courses,
            course,
            direction,
            gap_mm,
        } => UsageError(texts.get(
            "error-numbers-layout-conflict",
            Some(&args!(
                "control" => printable(&control),
                "layout" => printable(&layout),
                "courses" => quoted_list(texts, &courses),
                "course" => printable(&course),
                "direction" => short_decimal(texts, direction, 1),
                "gap" => short_decimal(texts, gap_mm, 2),
            )),
        ))
        .into(),
        E::TitleControl => UsageError(texts.plain("error-title-control")).into(),
        E::KindUnknown => anyhow!(texts.plain("error-kind-unknown")),
        E::ProjectExists { path } => anyhow!(texts.get(
            "error-project-exists",
            Some(&args!("path" => path.display().to_string()))
        )),
        E::ReadCourses { path, source } => source.context(texts.get(
            "error-read-courses",
            Some(&args!("path" => path.display().to_string())),
        )),
        E::AssignNeedsCourseFile => UsageError(texts.plain("error-assign-no-course-file")).into(),
        E::UnknownMask { id, known } => UsageError(if known.is_empty() {
            texts.get(
                "error-unknown-mask-none",
                Some(&args!("id" => printable(&id))),
            )
        } else {
            texts.get(
                "error-unknown-mask",
                Some(&args!("id" => printable(&id), "known" => quoted_list(texts, &known))),
            )
        })
        .into(),
        E::NotEditable { path, target } => {
            use project::EditTarget as T;
            let (id, name) = match &target {
                T::Layout(id) => ("error-not-editable-layout", id.as_str()),
                T::LayoutElements(id) => ("error-not-editable-elements", id.as_str()),
                T::SharedElements => ("error-not-editable-shared", ""),
                T::Assignment(course) => ("error-not-editable-assignment", course.as_str()),
            };
            anyhow!(texts.get(
                id,
                Some(&args!("path" => path.display().to_string(), "name" => printable(name))),
            ))
        }
        E::EditProject { path, source } => anyhow::Error::new(source).context(texts.get(
            "error-edit-project",
            Some(&args!("path" => path.display().to_string())),
        )),
        E::AppendCorrection { path, source } => {
            // A file that no longer loads has been put back as it was.
            let lead_in = match source {
                project::ProjectError::Write { .. } => "error-write-file",
                _ => "error-load-changed-project",
            };
            anyhow::Error::new(source)
                .context(texts.get(lead_in, Some(&args!("path" => path.display().to_string()))))
        }
    }
}

/// Metres with the decimal separator of the language and no needless `.0`: `2.5`, `5`.
fn metres(texts: &Texts, value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        texts.decimal(&format!("{value}"))
    }
}

/// The note about a map's `freemapper.toml` that could not be read: the kind is inferred instead.
fn sidecar_note(texts: &Texts, error: &str) {
    eprintln!("{}", sidecar_note_line(texts, error));
}

fn sidecar_note_line(texts: &Texts, error: &str) -> String {
    texts.get("note-sidecar", Some(&args!("error" => printable(error))))
}

/// The comments `init` writes into the new project file for the map kind and the contour interval the starter
/// proposes.
fn init_comments(texts: &Texts, kind: MapKind, contour: Option<f64>) -> ops::init::FileComments {
    ops::init::FileComments {
        contour: match contour {
            Some(_) => texts.get(
                "init-comment-contour",
                Some(&args!("kind" => texts.plain(kind_word_id(kind)))),
            ),
            None => texts.plain("init-comment-no-contour"),
        },
        footer: texts.plain("init-comment-footer"),
    }
}

/// The line `init` prints for a step while it works.
fn init_progress_line(texts: &Texts, progress: &ops::init::InitProgress<'_>) -> String {
    use ops::init::{InitProgress, KindSource};
    match progress {
        InitProgress::SidecarUnusable { error } => sidecar_note_line(texts, error),
        InitProgress::Kind { kind, source } => {
            let source_id = match source {
                KindSource::Given => "init-kind-flag",
                KindSource::Sidecar => "init-kind-sidecar",
                KindSource::Inferred => "init-kind-inferred",
            };
            texts.get(
                source_id,
                Some(&args!("kind" => texts.plain(kind_word_id(*kind)))),
            )
        }
        InitProgress::CoursesRead { path, count } => texts.get(
            "init-courses-read",
            // As the project file names it: relative to the project folder.
            Some(&args!(
                "path" => path.display().to_string(),
                "count" => *count,
            )),
        ),
        InitProgress::Fitting { scale } => {
            texts.get("init-fitting", Some(&args!("scale" => scale.to_string())))
        }
    }
}

/// `init`: writes a starter project file and says what it holds: the layouts (per print scale with a course
/// file), whether the map fits, where the kind and the contour interval came from, and what to do next.
fn init(texts: &Texts, args: &InitArgs) -> anyhow::Result<u8> {
    let result = ops::init::init(
        &ops::init::InitArgs {
            folder: args.folder.clone(),
            map: args.map.clone(),
            title: args.title.clone(),
            kind: args.kind.map(MapKind::from),
            courses: args.courses.clone(),
            sheet_lang: sheet_lang(texts),
        },
        &|kind, contour| init_comments(texts, kind, contour),
        &mut |progress| eprintln!("{}", init_progress_line(texts, &progress)),
    )
    .map_err(|e| op_error(texts, e))?;
    for line in init_written_lines(texts, &result) {
        eprintln!("{line}");
    }
    Ok(0)
}

/// The lines `init` prints after it wrote the file.
fn init_written_lines(texts: &Texts, result: &ops::init::InitResult) -> Vec<String> {
    let mut lines = Vec::new();
    let path = result.path.display().to_string();
    let kind_word = texts.plain(kind_word_id(result.kind));
    let map_scale = result.map_scale;
    match &result.course_file {
        None => {
            let layout = &result.layouts[0];
            lines.push(texts.get(
                "init-written",
                Some(&args!(
                    "path" => path.clone(),
                    "layout" => layout.id.clone(),
                    "paper" => paper_name(layout.paper),
                    "margin" => metres(texts, layout.margin_mm),
                    "scale" => map_scale.to_string(),
                    "kind" => kind_word.clone(),
                )),
            ));
            if !result.fits() {
                lines.push(texts.get(
                    "init-no-fit",
                    Some(&args!("scale" => map_scale.to_string())),
                ));
                lines.extend(result.check.iter().flat_map(|c| check_lines(texts, c)));
            }
        }
        Some(courses) => lines.extend(course_init_lines(texts, result, courses, &path, &kind_word)),
    }
    lines.push(match result.contour_interval_m {
        Some(value) => texts.get(
            "init-contour-hint",
            Some(&args!("value" => metres(texts, value), "kind" => kind_word)),
        ),
        None => texts.plain("init-no-contour-hint"),
    });
    let next = if result.fits() {
        "init-next"
    } else {
        "init-next-with-notes"
    };
    lines.push(texts.get(next, Some(&args!("path" => path))));
    lines
}

/// The lines `init --courses` prints after it wrote the file: what was written, each layout with its
/// courses, where the scales and the description box came from.
fn course_init_lines(
    texts: &Texts,
    result: &ops::init::InitResult,
    courses: &ops::init::InitCourses,
    path: &str,
    kind_word: &str,
) -> Vec<String> {
    let mut lines = vec![texts.get(
        "init-written-courses",
        Some(&args!(
            "path" => path.to_string(),
            "layouts" => result.layouts.len(),
            "assignments" => result.assignments,
            "kind" => kind_word.to_string(),
        )),
    )];
    for layout in &result.layouts {
        lines.push(texts.get(
            "init-courses-layout",
            Some(&args!(
                "layout" => printable(&layout.id),
                "scale" => layout.print_scale.to_string(),
                "paper" => paper_name(layout.paper),
                "margin" => metres(texts, layout.margin_mm),
                "courses" => quoted_list(texts, &layout.courses),
            )),
        ));
        if !layout.fits {
            lines.push(texts.get(
                "init-courses-no-fit",
                Some(&args!(
                    "scale" => layout.print_scale.to_string(),
                    "layout" => printable(&layout.id),
                )),
            ));
        }
    }
    let scales_id = if courses.iof {
        "init-courses-scales-iof"
    } else {
        "init-courses-scales-ppen"
    };
    lines.push(texts.get(
        scales_id,
        Some(&args!("scale" => result.map_scale.to_string())),
    ));
    if !courses.iof {
        lines.push(texts.plain("init-courses-description-box"));
    }
    lines.push(texts.plain("init-courses-crop"));
    lines
}

/// `numbers accept`: appends a number correction to the project file (ADR 0025, addendum).
fn numbers_accept(texts: &Texts, args: &AcceptArgs) -> anyhow::Result<u8> {
    use ops::numbers::AcceptScope;
    let result = ops::numbers::accept(&ops::numbers::AcceptArgs {
        project: args.project.clone(),
        course: args.course.clone(),
        control: args.control.clone(),
        // clap takes exactly one of `--sheet` and `--layout-wide`.
        scope: if args.sheet {
            AcceptScope::Sheet
        } else {
            AcceptScope::Layout
        },
        // clap takes `--direction` and `--gap` only together.
        position: args.direction.zip(args.gap),
    })
    .map_err(|e| op_error(texts, e))?;
    eprintln!("{}", numbers_accepted_line(texts, &result));
    Ok(0)
}

/// What `numbers accept` says after it wrote the correction.
fn numbers_accepted_line(texts: &Texts, result: &ops::numbers::AcceptResult) -> String {
    texts.get(
        "numbers-accepted",
        Some(&args!(
            "control" => printable(&result.control),
            "scope" => scope_text(texts, &result.scope),
            "path" => result.path.display().to_string(),
            "direction" => short_decimal(texts, result.direction, 1),
            "gap" => short_decimal(texts, result.gap_mm, 2),
        )),
    )
}

/// "for course "A"", "for layout a4": the scope of a number correction.
fn scope_text(texts: &Texts, scope: &ops::numbers::Scope) -> String {
    match scope {
        ops::numbers::Scope::Course(name) => texts.get(
            "stale-scope-course",
            Some(&args!("name" => printable(name))),
        ),
        ops::numbers::Scope::Layout(name) => texts.get(
            "stale-scope-layout",
            Some(&args!("name" => printable(name))),
        ),
    }
}

/// The message for a mask that could not be built: a usage error (exit 2).
fn build_error(texts: &Texts, err: BuildError, course: &str) -> anyhow::Error {
    let point = |p: [f64; 2]| {
        args!(
            "x" => short_decimal(texts, p[0], 3),
            "y" => short_decimal(texts, p[1], 3),
        )
    };
    let (id, a) = match err {
        BuildError::SizeOutOfRange { value } => (
            "error-masks-size",
            args!(
                "value" => if value.is_finite() { short_decimal(texts, value, 3) } else { value.to_string() },
                "min" => short_decimal(texts, mask_build::MIN_SIZE_MM, 1),
                "max" => short_decimal(texts, mask_build::MAX_SIZE_MM, 1),
            ),
        ),
        BuildError::BadSelection { text } => {
            ("error-masks-selection", args!("text" => printable(&text)))
        }
        BuildError::UnknownControl { code, known } => (
            "error-unknown-control",
            args!(
                "course" => printable(course),
                "control" => printable(&code),
                "known" => stops_with_codes(texts, &known),
            ),
        ),
        BuildError::RangeOutOfCourse { from, to, count } => (
            "error-masks-range",
            args!(
                "course" => printable(course),
                "from" => from.to_string(),
                "to" => to.to_string(),
                "count" => count,
            ),
        ),
        BuildError::RadiusForUnselected { code } => (
            "error-masks-radius-unselected",
            args!("control" => printable(&code)),
        ),
        BuildError::NoControls => (
            "error-masks-no-controls",
            args!("course" => printable(course)),
        ),
        BuildError::NoPosition { code } => (
            "error-masks-no-position",
            args!("control" => printable(&code)),
        ),
        BuildError::TooManyPoints => (
            "error-masks-too-many-points",
            args!("max" => freemapper_print::mask::MAX_TOTAL_POINTS.to_string()),
        ),
        BuildError::BadPoints { text } => ("error-masks-points", args!("text" => printable(&text))),
        BuildError::TooFewPoints => ("error-masks-too-few-points", FluentArgs::new()),
        BuildError::TooManyCorridorPoints => (
            "error-masks-too-many-corridor-points",
            args!("max" => mask_build::MAX_CORRIDOR_POINTS.to_string()),
        ),
        BuildError::PointOutOfReach { at } => ("error-masks-point-out-of-reach", point(at)),
        BuildError::SharpTurn { at } => ("error-masks-sharp-turn", point(at)),
        BuildError::SelfIntersecting { near } => ("error-masks-self-intersecting", point(near)),
        BuildError::Invalid(problem) => (
            "error-masks-invalid",
            args!("problem" => problem.to_string()),
        ),
    };
    UsageError(texts.get(id, Some(&a))).into()
}

/// The closing lines after a mask is written: who prints with it, and `check`.
fn mask_written_lines(
    texts: &Texts,
    id: &str,
    assigned: Option<&ops::masks::Assigned>,
) -> [String; 2] {
    [
        match assigned {
            Some(a) => texts.get(
                "masks-assigned",
                Some(&args!("course" => printable(&a.course), "layout" => printable(&a.layout))),
            ),
            None => texts.get("masks-not-assigned", Some(&args!("id" => printable(id)))),
        },
        texts.plain("masks-run-check"),
    ]
}

/// `masks window`: appends a mask with circular windows around the controls of a course (ADR 0047).
fn masks_window(texts: &Texts, args: &WindowArgs) -> anyhow::Result<u8> {
    let result = ops::masks::window(
        &ops::masks::WindowArgs {
            project: args.project.clone(),
            course: args.course.clone(),
            controls: args.controls.clone(),
            radius_mm: args.radius,
            radius_for: args.radius_for.clone(),
            no_start: args.no_start,
            no_finish: args.no_finish,
            id: args.id.clone(),
            assign: args.assign,
        },
        &|r| window_comment(texts, r),
    )
    .map_err(|e| op_error(texts, e))?;
    for line in window_written_lines(texts, &result) {
        eprintln!("{line}");
    }
    Ok(0)
}

/// The comment written above a window mask.
fn window_comment(texts: &Texts, r: &ops::masks::WindowResult) -> String {
    let mut comment = texts.get("masks-window-comment", Some(&window_summary(texts, r)));
    for line in own_radius_lines(texts, r) {
        comment.push('\n');
        comment.push_str(&line);
    }
    comment
}

/// What `masks window` says after it wrote the mask.
fn window_written_lines(texts: &Texts, result: &ops::masks::WindowResult) -> Vec<String> {
    let mut summary = window_summary(texts, result);
    summary.set("path", result.path.display().to_string());
    let mut lines = vec![texts.get("masks-window-written", Some(&summary))];
    lines.extend(own_radius_lines(texts, result));
    if let Some(which) = result.without_window {
        let which = match which {
            ops::masks::StartFinish::Both => "both",
            ops::masks::StartFinish::Start => "start",
            ops::masks::StartFinish::Finish => "finish",
        };
        lines.push(texts.get("masks-no-start-finish", Some(&args!("which" => which))));
    }
    lines.extend(mask_written_lines(
        texts,
        &result.id,
        result.assigned.as_ref(),
    ));
    lines
}

/// The arguments of the messages about a window mask: id, number of windows, the stops, course, radius and
/// print scale.
fn window_summary<'a>(texts: &Texts, r: &ops::masks::WindowResult) -> FluentArgs<'a> {
    args!(
        "id" => printable(&r.id),
        "count" => r.windows.len(),
        "controls" => stops_phrase(texts, &r.windows, true),
        "course" => printable(&r.course),
        "radius" => short_decimal(texts, r.radius_mm, 2),
        "scale" => r.print_scale.to_string(),
    )
}

/// One line per window whose radius differs from the default.
fn own_radius_lines(texts: &Texts, r: &ops::masks::WindowResult) -> Vec<String> {
    r.own_radius
        .iter()
        .map(|w| {
            texts.get(
                "masks-window-radius-for",
                Some(&args!(
                    "kind" => stop_kind(&w.stop),
                    "control" => printable(w.stop.code()),
                    "radius" => short_decimal(texts, w.radius_mm, 2),
                )),
            )
        })
        .collect()
}

/// `start`, `finish` or `control`: a selector for the messages that name a stop by kind.
fn stop_kind(stop: &StopName) -> &'static str {
    match stop {
        StopName::Start(_) => "start",
        StopName::Control(_) => "control",
        StopName::Finish(_) => "finish",
    }
}

/// `stops` (course order) as a phrase in the user's language (ruling R13): the start and the finish by kind, the
/// controls between them by code, those next to each other in one group ("controls 31, 32 and 33"). `accusative`
/// picks the German article for an object ("um den Start").
fn stops_phrase(texts: &Texts, stops: &[StopName], accusative: bool) -> String {
    let mut items: Vec<String> = Vec::new();
    let mut group: Vec<String> = Vec::new();
    let flush = |group: &mut Vec<String>, items: &mut Vec<String>| {
        if !group.is_empty() {
            let count = group.len();
            let codes = join_and(texts, std::mem::take(group));
            items.push(texts.get(
                "stop-controls",
                Some(&args!("codes" => codes, "count" => count)),
            ));
        }
    };
    for stop in stops {
        match stop {
            StopName::Control(code) => group.push(printable(code)),
            other => {
                flush(&mut group, &mut items);
                let id = match (other, accusative) {
                    (StopName::Start(_), false) => "stop-start",
                    (StopName::Start(_), true) => "stop-start-acc",
                    _ => "stop-finish",
                };
                items.push(texts.plain(id));
            }
        }
    }
    flush(&mut group, &mut items);
    join_and(texts, items)
}

/// `stops` as a list of the codes the user can type, the start and the finish with their kind: "31, 32, start
/// (STA1) and finish (FIN1)".
fn stops_with_codes(texts: &Texts, stops: &[StopName]) -> String {
    let items = stops
        .iter()
        .map(|stop| match stop {
            StopName::Control(code) => printable(code),
            StopName::Start(code) => {
                texts.get("stop-start-code", Some(&args!("code" => printable(code))))
            }
            StopName::Finish(code) => {
                texts.get("stop-finish-code", Some(&args!("code" => printable(code))))
            }
        })
        .collect();
    join_and(texts, items)
}

/// `masks corridor`: appends a mask with a corridor along the polyline the user gives (ADR 0047).
fn masks_corridor(texts: &Texts, args: &CorridorArgs) -> anyhow::Result<u8> {
    let result = ops::masks::corridor(
        &ops::masks::CorridorArgs {
            project: args.project.clone(),
            points: args.points.clone(),
            width_mm: args.width,
            id: args.id.clone(),
            assign: args.assign,
            course: args.course.clone(),
        },
        &|r| corridor_comment(texts, r),
    )
    .map_err(|e| op_error(texts, e))?;
    for line in corridor_written_lines(texts, &result) {
        eprintln!("{line}");
    }
    Ok(0)
}

/// The comment written above a corridor mask.
fn corridor_comment(texts: &Texts, r: &ops::masks::CorridorResult) -> String {
    // The points as read, so that the command can be run again from the comment.
    let written: Vec<String> = r.points.iter().map(|[x, y]| format!("{x},{y}")).collect();
    format!(
        "{}\n--points \"{}\"",
        texts.get("masks-corridor-comment", Some(&corridor_summary(texts, r))),
        written.join(";")
    )
}

/// What `masks corridor` says after it wrote the mask.
fn corridor_written_lines(texts: &Texts, result: &ops::masks::CorridorResult) -> Vec<String> {
    let mut summary = corridor_summary(texts, result);
    summary.set("path", result.path.display().to_string());
    let mut lines = vec![texts.get("masks-corridor-written", Some(&summary))];
    lines.extend(mask_written_lines(
        texts,
        &result.id,
        result.assigned.as_ref(),
    ));
    lines
}

/// The arguments of the messages about a corridor mask: id, number of points, width and print scale.
fn corridor_summary<'a>(texts: &Texts, r: &ops::masks::CorridorResult) -> FluentArgs<'a> {
    args!(
        "id" => printable(&r.id),
        "count" => r.points.len(),
        "width" => short_decimal(texts, r.width_mm, 2),
        "scale" => r.print_scale.to_string(),
    )
}

/// The lines about one sheet: the first line (message `id` with `keys`, the page size, scales and factors), then
/// its notes.
fn sheet_lines(
    texts: &Texts,
    id: &str,
    keys: &[(&'static str, String)],
    sheet: &ops::Sheet,
) -> Vec<String> {
    let mut lines = vec![summary_line(
        texts,
        id,
        keys,
        sheet.paper,
        &sheet.print,
        sheet.map_scale,
    )];
    lines.extend(sheet.notes.iter().map(|n| n.describe(texts).message));
    lines
}

/// What `check` prints for one sheet. A sheet that cannot be drawn gives its error line.
fn check_lines(texts: &Texts, sheet: &ops::check::SheetCheck) -> Vec<String> {
    match sheet {
        ops::check::SheetCheck::Drawn(s) => match &s.course {
            Some(course) => sheet_lines(
                texts,
                "check-course-summary",
                &[
                    ("course", printable(course)),
                    ("layout", printable(&s.layout)),
                ],
                s,
            ),
            None => sheet_lines(
                texts,
                "check-summary",
                &[("layout", printable(&s.layout))],
                s,
            ),
        },
        ops::check::SheetCheck::Failed { error, .. } => vec![error.describe(texts).message],
    }
}

/// Prints the lines of `sheets` and returns how many of them need a look.
fn print_checks(texts: &Texts, sheets: &[ops::check::SheetCheck]) -> usize {
    let mut with_notes = 0;
    for sheet in sheets {
        for line in check_lines(texts, sheet) {
            eprintln!("{line}");
        }
        with_notes += usize::from(sheet.needs_attention());
    }
    with_notes
}

/// `check`: without a course file every layout; with one, the notes of the file once, then every course on its
/// layout, then every layout sheet without a course, as `export --layout` writes it (ruling R40). Exit code 1
/// when the file, a course or a layout sheet has notes that count.
fn check(texts: &Texts, args: &CheckArgs) -> anyhow::Result<u8> {
    let result = ops::check::check(&ops::check::CheckArgs {
        project: args.project.clone(),
    })
    .map_err(|e| op_error(texts, e))?;
    let Some(file_notes) = &result.course_file else {
        let total = result.layouts.len();
        let with_notes = print_checks(texts, &result.layouts);
        if with_notes == 0 {
            eprintln!(
                "{}",
                texts.get("check-clean", Some(&args!("count" => total)))
            );
            return Ok(0);
        }
        eprintln!(
            "{}",
            texts.get(
                "check-with-notes",
                Some(&args!("count" => with_notes, "total" => total))
            )
        );
        return Ok(1);
    };
    for note in file_notes {
        eprintln!("{}", note.describe(texts).message);
    }
    let file_notes = result.course_file_needs_attention();
    let total = result.courses.len();
    let with_notes = print_checks(texts, &result.courses);
    let layouts = result.layouts.len();
    let layouts_with_notes = print_checks(texts, &result.layouts);
    if file_notes {
        eprintln!("{}", texts.plain("check-course-file-notes"));
    }
    if with_notes == 0 && layouts_with_notes == 0 && !file_notes {
        eprintln!(
            "{}",
            texts.get(
                "check-courses-clean",
                Some(&args!("count" => total, "layouts" => layouts))
            )
        );
        Ok(0)
    } else {
        eprintln!(
            "{}",
            texts.get(
                "check-courses-with-notes",
                Some(&args!(
                    "count" => with_notes,
                    "total" => total,
                    "layouts" => layouts,
                    "layouts_with_notes" => layouts_with_notes,
                ))
            )
        );
        Ok(1)
    }
}

/// The lines about a page `export` drew.
fn export_page_lines(
    texts: &Texts,
    mode: ops::export::ExportMode,
    page: &ops::export::ExportedPage,
) -> Vec<String> {
    use ops::export::ExportMode;
    let sheet = &page.sheet;
    let course_keys = |out: String| {
        [
            ("out", out),
            (
                "course",
                printable(sheet.course.as_deref().unwrap_or_default()),
            ),
            ("layout", printable(&sheet.layout)),
            ("page", page.page.to_string()),
        ]
    };
    match mode {
        ExportMode::Layouts => sheet_lines(
            texts,
            "report-summary",
            &[("out", page.path.display().to_string())],
            sheet,
        ),
        ExportMode::Courses => sheet_lines(
            texts,
            "report-course-summary",
            &course_keys(page.path.display().to_string()),
            sheet,
        ),
        ExportMode::SingleFile => sheet_lines(
            texts,
            "report-course-page",
            &course_keys(String::new()),
            sheet,
        ),
    }
}

/// `export`: each page's lines as soon as it is written, then the single file and the reminder to print at
/// actual size.
fn export(texts: &Texts, args: &ExportArgs) -> anyhow::Result<u8> {
    use ops::export::ExportProgress;
    let result = ops::export::export(
        &ops::export::ExportArgs {
            project: args.project.clone(),
            layouts: args.layout.clone(),
            courses: args.course.clone(),
            single_file: args.single_file,
            out_dir: args.out_dir.clone(),
        },
        &mut |progress| {
            match progress {
                ExportProgress::CourseFileNotes(notes) => {
                    for note in notes {
                        eprintln!("{}", note.describe(texts).message);
                    }
                }
                ExportProgress::Page { mode, page } => {
                    for line in export_page_lines(texts, mode, page) {
                        eprintln!("{line}");
                    }
                }
            }
            std::ops::ControlFlow::Continue(())
        },
    )
    .map_err(|e| op_error(texts, e))?;
    if result.mode != ops::export::ExportMode::Layouts && result.pages.is_empty() {
        // Every course was left out by the reader: nothing to write, not even an empty PDF.
        eprintln!("{}", texts.plain("note-no-courses-to-export"));
        return Ok(0);
    }
    if result.mode == ops::export::ExportMode::SingleFile
        && let Some(file) = result.files.first()
    {
        eprintln!(
            "{}",
            texts.get(
                "report-single-file",
                Some(&args!(
                    "out" => file.path.display().to_string(),
                    "count" => result.pages.len(),
                ))
            )
        );
    }
    eprintln!("{}", texts.plain("reminder-actual-size"));
    Ok(0)
}

/// The message for a full disk, translated whole. `available` is `None` when the volume did not
/// say how much is free.
fn disk_full_message(texts: &Texts, needed: u64, available: Option<u64>, path: &Path) -> String {
    const MB: u64 = 1024 * 1024;
    let mut a = args!(
        "path" => path.display().to_string(),
        "needed" => needed.div_ceil(MB).to_string(),
    );
    match available {
        Some(free) => {
            a.set("available", (free / MB).to_string());
            texts.get("error-disk-full", Some(&a))
        }
        None => texts.get("error-disk-full-unknown", Some(&a)),
    }
}

/// A write error with a translated lead-in; a full disk and a write refused because a link leads
/// out of the project folder are translated whole.
fn output_error(texts: &Texts, err: &OutputError) -> anyhow::Error {
    match err {
        OutputError::DiskFull {
            needed,
            available,
            path,
        } => anyhow!(disk_full_message(texts, *needed, *available, path)),
        OutputError::Io { path, source } if output::is_outside_project(source) => {
            anyhow!(texts.get(
                "error-outside-project",
                Some(&args!("path" => path.display().to_string()))
            ))
        }
        OutputError::Io { path, source } => chain(source).context(texts.get(
            "error-write-file",
            Some(&args!("path" => path.display().to_string())),
        )),
    }
}

/// An export error with a translated lead-in; a full disk is translated whole.
fn export_error(texts: &Texts, err: &ExportError) -> anyhow::Error {
    let sheet =
        |layout: &str| texts.get("error-sheet", Some(&args!("layout" => printable(layout))));
    match err {
        ExportError::Sheet { layout, error } => chain(error).context(sheet(layout)),
        ExportError::Pdf { layout, error } => chain(error).context(sheet(layout)),
        ExportError::Write { error, .. } => output_error(texts, error),
        ExportError::CourseSheet { course, error } => chain(error).context(texts.get(
            "error-course-sheet",
            Some(&args!("course" => printable(course))),
        )),
        ExportError::CoursePdf { course, error } => chain(error).context(texts.get(
            "error-course-sheet",
            Some(&args!("course" => printable(course))),
        )),
        ExportError::CourseWrite { error, .. } => output_error(texts, error),
    }
}

/// True when `out` names the same file as `map`, even through links or `..`.
fn same_file(map: &Path, out: &Path) -> bool {
    let Ok(map) = map.canonicalize() else {
        return false;
    };
    if out.canonicalize().is_ok_and(|out| out == map) {
        return true;
    }
    let parent = match out.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    match (parent.canonicalize(), out.file_name()) {
        (Ok(parent), Some(name)) => parent.join(name) == map,
        _ => false,
    }
}

/// Map text for the terminal. Control characters get Rust's readable escapes (`\n`, `\t`,
/// `\u{1b}`) and format characters such as bidi overrides get `\u{..}`, so a map cannot drive
/// the terminal.
fn printable(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_control() {
            out.extend(c.escape_debug());
        } else if ops::is_format(c) {
            out.extend(c.escape_unicode());
        } else {
            out.push(c);
        }
    }
    out
}

/// A millimetre value with one decimal, never below 0.1 for a positive value.
fn mm(texts: &Texts, value: f64) -> String {
    texts.decimal(&format!("{:.1}", value.max(0.1)))
}

fn paper_name(paper: Paper) -> &'static str {
    match paper {
        Paper::A4 => "A4",
        Paper::A3 => "A3",
    }
}

/// The message id of the plural kind word: "sprint" in "sprint maps", "Sprintkarten".
fn kind_word_id(kind: MapKind) -> &'static str {
    match kind {
        MapKind::Sprint => "word-kind-sprint",
        MapKind::Schoolyard => "word-kind-schoolyard",
        MapKind::Forest => "word-kind-forest",
    }
}

/// `items` as "a, b and c" in the language of `texts`.
fn join_and(texts: &Texts, mut items: Vec<String>) -> String {
    match items.len() {
        0 | 1 => items.pop().unwrap_or_default(),
        _ => {
            let last = items.pop().unwrap_or_default();
            texts.get(
                "list-and",
                Some(&args!("first" => items.join(", "), "last" => last)),
            )
        }
    }
}

/// The first line about a printed page: message `id` with `keys` (the PDF path, the layout id, the course)
/// and the page size, scales and factors.
fn summary_line(
    texts: &Texts,
    id: &str,
    keys: &[(&'static str, String)],
    paper: Paper,
    report: &ops::PrintFacts,
    map_scale: u32,
) -> String {
    let (w, h) = report.page_mm;
    let orientation = texts.plain(if w > h {
        "word-landscape"
    } else {
        "word-portrait"
    });
    let mut a = args!(
        "paper" => paper_name(paper),
        "orientation" => orientation,
        "print_scale" => report.print_scale.to_string(),
        "map_scale" => map_scale.to_string(),
        "geometry" => texts.decimal(&format!("{:.3}", report.geometry_factor)),
        "symbol" => texts.decimal(&format!("{:.3}", report.symbol_factor)),
    );
    for (key, value) in keys {
        a.set(*key, value.clone());
    }
    texts.get(id, Some(&a))
}

/// The messages for a finished print, in the order they appear on stderr.
fn report_lines(
    texts: &Texts,
    report: &PrintReport,
    paper: PaperArg,
    orientation_arg: OrientationArg,
    kind: MapKind,
    map_scale: u32,
    out: &Path,
) -> Vec<String> {
    let mut lines = vec![summary_line(
        texts,
        "report-summary",
        &[("out", out.display().to_string())],
        paper.into(),
        &ops::PrintFacts::from(report),
        map_scale,
    )];
    lines.extend(print_notes(
        texts,
        report,
        paper.into(),
        orientation_arg != OrientationArg::Auto,
        kind,
        false,
    ));
    lines.push(texts.plain("reminder-actual-size"));
    lines
}

/// The message for one sheet note. No wildcard arm: a new kind of note does not compile until it
/// has a text.
fn sheet_note_line(texts: &Texts, note: &SheetNote) -> String {
    match note {
        SheetNote::ElementOutsidePage { id } => texts.get(
            "note-element-outside-page",
            Some(&args!("id" => printable(id))),
        ),
        SheetNote::ElementOverlapsMap { id } => texts.get(
            "note-element-overlaps-map",
            Some(&args!("id" => printable(id))),
        ),
        SheetNote::LargerThanA3 => texts.plain("note-larger-than-a3"),
        SheetNote::MissingScaleOrContour { layout, missing } => {
            let items: Vec<String> = missing
                .iter()
                .map(|item| {
                    let id = match item {
                        MissingItem::PrintScaleText => "missing-print-scale-text",
                        MissingItem::ContourText => "missing-contour-text",
                        MissingItem::ContourValue => "missing-contour-value",
                    };
                    texts.get(id, Some(&args!("name" => item.name())))
                })
                .collect();
            let count = items.len();
            texts.get(
                "note-missing-scale-or-contour",
                Some(&args!(
                    "layout" => printable(layout),
                    "missing" => join_and(texts, items),
                    "count" => count,
                )),
            )
        }
        SheetNote::MapCropped {
            overflow_mm: (x, y),
        } => texts.get(
            "note-map-cropped",
            Some(&args!("cut" => cut_text(texts, *x, *y))),
        ),
        SheetNote::MapCroppedToCourse {
            overflow_mm: (x, y),
        } => texts.get(
            "note-map-cropped-to-course",
            Some(&args!("cut" => cut_text(texts, *x, *y))),
        ),
        SheetNote::NorthLinesAdded { count } => {
            texts.get("note-north-lines-added", Some(&args!("count" => *count)))
        }
        SheetNote::NorthLinesWithoutSymbol { count } => texts.get(
            "note-north-lines-without-symbol",
            Some(&args!("count" => *count)),
        ),
        SheetNote::CopyrightTruncated {
            layout,
            total,
            shown,
        } => texts.get(
            "note-copyright-truncated",
            Some(&args!(
                "layout" => printable(layout),
                "total" => *total,
                "shown" => *shown,
            )),
        ),
        SheetNote::LogoSkipped { id, file, reason } => texts.get(
            "note-logo-skipped",
            Some(&args!(
                "id" => printable(id),
                "reason" => logo_skip_reason(texts, file.as_deref(), reason),
            )),
        ),
        SheetNote::LogoSimplified { id, reason } => texts.get(
            "note-logo-simplified",
            Some(&args!("id" => printable(id), "reason" => logo_simplification(texts, *reason))),
        ),
        SheetNote::UnknownPlaceholder { id, name } => texts.get(
            "note-unknown-placeholder",
            Some(&args!("id" => printable(id), "name" => printable(&format!("{{{name}}}")))),
        ),
        SheetNote::UnknownPart { layout, name } => texts.get(
            "note-unknown-part",
            Some(&args!("layout" => printable(layout), "name" => printable(name))),
        ),
        SheetNote::CopyrightLineCut { line } => texts.get(
            "note-copyright-line-cut",
            Some(&args!("line" => printable(line))),
        ),
        SheetNote::FooterOverlapsMap { content } => texts.get(
            "note-footer-overlaps-map",
            Some(&args!("content" => footer_content(*content))),
        ),
        SheetNote::FooterOutsidePage { content } => texts.get(
            "note-footer-outside-page",
            Some(&args!("content" => footer_content(*content))),
        ),
        SheetNote::FooterCoversElement { id, content } => texts.get(
            "note-footer-covers-element",
            Some(&args!("id" => printable(id), "content" => footer_content(*content))),
        ),
        SheetNote::MissingScaleCheckBar { layout } => texts.get(
            "note-missing-scale-check-bar",
            Some(&args!("layout" => printable(layout))),
        ),
        SheetNote::CreditLineMoved { id } => texts.get(
            "note-credit-line-moved",
            Some(&args!("id" => printable(id))),
        ),
        SheetNote::CourseOutsideFrame { course, controls } => texts.get(
            "note-course-outside-frame",
            Some(&args!(
                "course" => printable(course),
                "controls" => printable(&controls.join(", ")),
                "count" => controls.len(),
            )),
        ),
        SheetNote::DefaultLayout { course, layout } => texts.get(
            "note-default-layout",
            Some(&args!("course" => printable(course), "layout" => printable(layout))),
        ),
        SheetNote::NoDescriptionBox { course, reason } => texts.get(
            match reason {
                NoBoxReason::IofXml => "note-no-description-box-iof-xml",
                NoBoxReason::NoElement => "note-no-description-box-no-element",
                NoBoxReason::OutsidePage => "note-no-description-box-outside-page",
                NoBoxReason::NoDescriptions => "note-no-description-box-no-descriptions",
            },
            Some(&args!("course" => printable(course))),
        ),
        SheetNote::ElementCoversCourse { element, controls } => texts.get(
            "note-element-covers-course",
            Some(&args!(
                "id" => printable(element),
                "controls" => printable(&controls.join(", ")),
                "count" => controls.len(),
            )),
        ),
        SheetNote::UnknownDescriptionSymbol { course, id } => texts.get(
            "note-unknown-description-symbol",
            Some(&args!("course" => printable(course), "id" => printable(id))),
        ),
        SheetNote::OverprintCut { course } => texts.get(
            "note-overprint-cut",
            Some(&args!("course" => printable(course))),
        ),
        SheetNote::MasksCapped {
            course,
            applied,
            total,
        } => texts.get(
            "note-masks-capped",
            Some(&args!(
                "course" => printable(course),
                "applied" => *applied,
                "total" => *total,
            )),
        ),
        SheetNote::OverprintInMaskedArea {
            course,
            kind,
            count,
        } => {
            let id = match kind {
                MaskedObject::MarkedRoute => "masked-marked-route",
                MaskedObject::CrossingPoint => "masked-crossing-point",
                MaskedObject::ForbiddenRoute => "masked-forbidden-route",
                MaskedObject::Boundary => "masked-boundary",
                MaskedObject::OutOfBoundsArea => "masked-out-of-bounds-area",
                MaskedObject::DangerousArea => "masked-dangerous-area",
                MaskedObject::Text => "masked-text",
            };
            texts.get(
                "note-overprint-in-masked-area",
                Some(&args!(
                    "course" => printable(course),
                    "objects" => texts.get(id, Some(&args!("count" => *count))),
                    "count" => *count,
                )),
            )
        }
        SheetNote::ControlInMaskedArea { course, controls } => texts.get(
            "note-control-in-masked-area",
            Some(&args!(
                "course" => printable(course),
                "controls" => stops_phrase(texts, controls, false),
                "count" => controls.len(),
            )),
        ),
        SheetNote::MaskCoversNothing { course, mask } => texts.get(
            "note-mask-covers-nothing",
            Some(&args!("course" => printable(course), "mask" => printable(mask))),
        ),
        SheetNote::WhiteOutsCoverNothing { course, ids } => {
            // A course file can hold thousands: the first few by id, the rest counted.
            let mut shown: Vec<String> = ids
                .iter()
                .take(MAX_LISTED_WHITE_OUTS)
                .map(|id| printable(id))
                .collect();
            if ids.len() > MAX_LISTED_WHITE_OUTS {
                shown.push(texts.get(
                    "list-more",
                    Some(&args!("count" => ids.len() - MAX_LISTED_WHITE_OUTS)),
                ));
            }
            texts.get(
                "note-white-outs-cover-nothing",
                Some(&args!(
                    "course" => printable(course),
                    "ids" => join_and(texts, shown),
                    "count" => ids.len(),
                )),
            )
        }
        SheetNote::ElementCoversWindow {
            course,
            element,
            mask,
        } => texts.get(
            "note-element-covers-window",
            Some(&args!(
                "course" => printable(course),
                "id" => printable(element),
                "mask" => printable(mask),
            )),
        ),
        SheetNote::MaskLeavesNoWindow { course, mask } => texts.get(
            "note-mask-leaves-no-window",
            Some(&args!("course" => printable(course), "mask" => printable(mask))),
        ),
        SheetNote::MaskCheckLimited { course } => texts.get(
            "note-mask-check-limited",
            Some(&args!("course" => printable(course))),
        ),
        SheetNote::CourseValueUnknown { course, id, name } => texts.get(
            "note-course-value-unknown",
            Some(&args!(
                "course" => printable(course),
                "id" => printable(id),
                "name" => format!("{{{name}}}"),
            )),
        ),
        SheetNote::CoursePlaceholderNotFilled { element, name } => texts.get(
            "note-course-placeholder-not-filled",
            Some(&args!("id" => printable(element), "name" => format!("{{{name}}}"))),
        ),
        SheetNote::NumberConflict(c) => number_conflict_line(texts, c),
        SheetNote::NumberSearchLimited { course } => texts.get(
            "note-number-search-limited",
            Some(&args!("course" => printable(course))),
        ),
        SheetNote::NumberGapOutOfRange {
            course,
            control,
            gap_mm,
            min,
            max,
        } => texts.get(
            "note-number-gap-out-of-range",
            Some(&args!(
                "course" => printable(course),
                "control" => printable(control),
                "gap" => short_decimal(texts, *gap_mm, 2),
                "min" => short_decimal(texts, *min, 2),
                "max" => short_decimal(texts, *max, 2),
            )),
        ),
        SheetNote::Course(note) => course_note_line(texts, note),
        SheetNote::Fit(note) => fit_note_line(texts, note),
        SheetNote::Stale(stale) => stale_line(texts, stale),
        SheetNote::Mask(note) => mask_note_line(texts, note),
    }
}

/// Most white-out ids one line names; the rest are counted.
const MAX_LISTED_WHITE_OUTS: usize = 10;

/// The message for a remark about the mask variants. No wildcard arm.
fn mask_note_line(texts: &Texts, note: &MaskNote) -> String {
    match note {
        MaskNote::MaskUnused { id } => {
            texts.get("note-mask-unused", Some(&args!("id" => printable(id))))
        }
        MaskNote::WhiteOutByNumber {
            course,
            id,
            centre,
            stand_in,
        } => texts.get(
            if *stand_in {
                "note-white-out-stand-in"
            } else {
                "note-white-out-by-number"
            },
            Some(&args!(
                "course" => printable(course),
                "id" => printable(id),
                "x" => short_decimal(texts, centre[0], 1),
                "y" => short_decimal(texts, centre[1], 1),
            )),
        ),
    }
}

/// `value` with at most `places` decimals, trailing zeros dropped, in the decimal style of the language.
fn short_decimal(texts: &Texts, value: f64, places: usize) -> String {
    let s = format!("{value:.places$}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.')
    } else {
        &s
    };
    texts.decimal(s)
}

/// The message for a control number in conflict, with what it overlaps and the proposal. No wildcard arm.
fn number_conflict_line(texts: &Texts, c: &NumberConflict) -> String {
    let parts: Vec<String> = c
        .conflicts
        .iter()
        .map(|conflict| match conflict {
            Conflict::Symbol(control) => texts.get(
                "number-conflict-symbol",
                Some(&args!("control" => printable(control))),
            ),
            Conflict::Leg(from, to) => texts.get(
                "number-conflict-leg",
                Some(&args!("from" => printable(from), "to" => printable(to))),
            ),
            Conflict::Number(control) => texts.get(
                "number-conflict-number",
                Some(&args!("control" => printable(control))),
            ),
            Conflict::Element(id) => texts.get(
                "number-conflict-element",
                Some(&args!("id" => printable(id))),
            ),
            Conflict::OutsideFrame => texts.plain("number-conflict-outside"),
            Conflict::MapSymbol { line, code, count } => texts.get(
                if *line {
                    "number-conflict-line-symbol"
                } else {
                    "number-conflict-point-symbol"
                },
                Some(&args!("code" => printable(code), "count" => *count)),
            ),
        })
        .collect();
    if !c.searched {
        return texts.get(
            "note-number-conflict-unsearched",
            Some(&args!(
                "course" => printable(&c.course),
                "control" => printable(&c.control),
                "conflicts" => parts.join(", "),
            )),
        );
    }
    let proposal = texts.get(
        if c.free {
            "number-proposal-free"
        } else {
            "number-proposal-none"
        },
        Some(&args!(
            "direction" => short_decimal(texts, c.proposal.direction, 1),
            "gap" => short_decimal(texts, c.proposal.gap_mm, 2),
        )),
    );
    texts.get(
        "note-number-conflict",
        Some(&args!(
            "course" => printable(&c.course),
            "control" => printable(&c.control),
            "conflicts" => parts.join(", "),
            "proposal" => proposal,
        )),
    )
}

/// The message for a note of the course reader. No wildcard arm, as for sheet notes.
fn course_note_line(texts: &Texts, note: &CourseNote) -> String {
    match note {
        CourseNote::UnknownElement(name) => texts.get(
            "note-course-unknown-element",
            Some(&args!("name" => printable(name))),
        ),
        CourseNote::Forked { family, variants } => texts.get(
            "note-course-forked",
            Some(&args!(
                "family" => printable(family),
                "variants" => quoted_list(texts, variants),
            )),
        ),
        CourseNote::AlternativeControls(course) => texts.get(
            "note-course-alternative-controls",
            Some(&args!("course" => printable(course))),
        ),
        CourseNote::NotesTruncated => texts.plain("note-course-notes-truncated"),
        CourseNote::WhiteOutLaterInM4 => texts.plain("note-course-white-out"),
        CourseNote::NoDescriptions => texts.plain("note-course-no-descriptions"),
        CourseNote::MapIssueAsStart(code) => texts.get(
            "note-course-map-issue-as-start",
            Some(&args!("code" => printable(code))),
        ),
        CourseNote::Variations(course) => texts.get(
            "note-course-variations",
            Some(&args!("course" => printable(course))),
        ),
    }
}

/// Course names, each in the quotes of the language, joined by commas.
fn quoted_list(texts: &Texts, names: &[String]) -> String {
    names
        .iter()
        .map(|n| texts.get("quoted", Some(&args!("text" => printable(n)))))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The message for a note of fitting the course file to the map. No wildcard arm.
fn fit_note_line(texts: &Texts, note: &FitNote) -> String {
    match note {
        FitNote::YUpDetected => texts.plain("note-fit-y-up"),
        FitNote::OrientationAssumed => texts.plain("note-fit-orientation-assumed"),
        FitNote::AxisAgainstFormat { set, .. } => texts.get(
            "note-fit-axis-against-format",
            Some(&args!("set" => match set {
                YAxis::Up => "up",
                YAxis::Down => "down",
            })),
        ),
        FitNote::ScaleUnchecked => texts.plain("note-fit-scale-unchecked"),
        FitNote::GeorefChecked { controls, metres } => texts.get(
            "note-fit-georef-checked",
            Some(&args!(
                "count" => *controls,
                "metres" => texts.decimal(&format!("{metres:.1}")),
            )),
        ),
        FitNote::GeorefSkipped(reason) => {
            let reason = match reason {
                SkipReason::NotGeoreferenced => texts.plain("fit-skip-not-georeferenced"),
                SkipReason::UnsupportedCrs(crs) => texts.get(
                    "fit-skip-unsupported-crs",
                    Some(&args!("crs" => printable(crs))),
                ),
                SkipReason::NoLatLng => texts.plain("fit-skip-no-lat-lng"),
                SkipReason::CheckOff => texts.plain("fit-skip-check-off"),
            };
            texts.get("note-fit-georef-skipped", Some(&args!("reason" => reason)))
        }
        FitNote::GeorefCheckOff => texts.plain("note-fit-georef-check-off"),
        FitNote::PlacedByGeoref(controls) => texts.get(
            "note-fit-placed-by-georef",
            Some(&args!(
                "controls" => printable(&controls.join(", ")),
                "count" => controls.len(),
            )),
        ),
    }
}

/// The message for an entry of the project file that no longer matches the course file. No wildcard arm.
fn stale_line(texts: &Texts, stale: &Stale) -> String {
    match stale {
        Stale::Assignment { course } => texts.get(
            "note-stale-assignment",
            Some(&args!("course" => printable(course))),
        ),
        Stale::Correction { control, scope } => {
            let scope = match scope {
                CorrectionScope::Layout(l) => {
                    texts.get("stale-scope-layout", Some(&args!("name" => printable(l))))
                }
                CorrectionScope::Course(c) => {
                    texts.get("stale-scope-course", Some(&args!("name" => printable(c))))
                }
            };
            texts.get(
                "note-stale-correction",
                Some(&args!("control" => printable(control), "scope" => scope)),
            )
        }
        Stale::CorrectionOverridesChangedBase { control } => texts.get(
            "note-stale-changed-base",
            Some(&args!("control" => printable(control))),
        ),
        Stale::Mask { course, mask } => texts.get(
            "note-stale-mask",
            Some(&args!("course" => printable(course), "mask" => printable(mask))),
        ),
    }
}

/// The selector the footer messages take for what the footer holds.
fn footer_content(content: FooterContent) -> &'static str {
    match content {
        FooterContent::CopyrightAndCredit => "both",
        FooterContent::Copyright => "copyright",
        FooterContent::Credit => "credit",
    }
}

/// Why a logo is left out, in the program language. Details from the operating system, the
/// image decoder or the SVG check stay as they come (ADR 0024). No wildcard arm: a new reason
/// does not compile until it has a text.
fn logo_skip_reason(texts: &Texts, file: Option<&Path>, reason: &LogoSkipReason) -> String {
    use LogoSkipReason::*;
    let file = file.map_or_else(String::new, |f| printable(&f.display().to_string()));
    let with_file = |id: &str| texts.get(id, Some(&args!("file" => file.clone())));
    let with_detail = |id: &str, name: &str, detail: &str| {
        let mut a = args!("file" => file.clone());
        a.set(name.to_string(), printable(detail));
        texts.get(id, Some(&a))
    };
    let with_size = |id: &str, width: u32, height: u32| {
        texts.get(
            id,
            Some(&args!(
                "file" => file.clone(),
                "width" => width.to_string(),
                "height" => height.to_string(),
            )),
        )
    };
    match reason {
        NeedsFileAndWidth => texts.plain("logo-reason-needs-file-and-width"),
        NotFound => with_file("logo-reason-not-found"),
        Unreadable { error } => with_detail("logo-reason-unreadable", "error", error),
        NotAFile => with_file("logo-reason-not-a-file"),
        FileTooLarge => with_file("logo-reason-file-too-large"),
        UnknownFormat => with_file("logo-reason-unknown-format"),
        TooManyPixels { width, height } => {
            with_size("logo-reason-too-many-pixels", *width, *height)
        }
        TooWideOrHigh { width, height } => with_size("logo-reason-too-wide", *width, *height),
        Undecodable { error } => with_detail("logo-reason-undecodable", "error", error),
        DecoderLimit => with_file("logo-reason-decoder-limit"),
        SvgNotUtf8 => with_file("logo-reason-svg-not-utf8"),
        SvgUnreadable { error } => with_detail("logo-reason-svg-unreadable", "error", error),
        SvgUnsupported { what } => with_detail("logo-reason-svg-unsupported", "detail", what),
        SvgTooComplex { what } => with_detail("logo-reason-svg-too-complex", "detail", what),
        CannotSize => with_file("logo-reason-cannot-size"),
        PixelBudget => with_file("logo-reason-pixel-budget"),
        SvgBudget => with_file("logo-reason-svg-budget"),
    }
}

/// How an SVG logo is drawn simpler, in the program language.
fn logo_simplification(texts: &Texts, reason: LogoSimplification) -> String {
    texts.plain(match reason {
        LogoSimplification::Gradient => "logo-simplified-gradient",
        LogoSimplification::Pattern => "logo-simplified-pattern",
        LogoSimplification::Opacity => "logo-simplified-opacity",
        LogoSimplification::Dashes => "logo-simplified-dashes",
        LogoSimplification::Image => "logo-simplified-image",
        LogoSimplification::Text => "logo-simplified-text",
        LogoSimplification::ClipPath => "logo-simplified-clip-path",
    })
}

/// "12.3 mm too wide and 0.1 mm too tall": how far the map reaches past the frame in `x` and `y`
/// (mm), naming only the axes it does.
fn cut_text(texts: &Texts, x: f64, y: f64) -> String {
    let mut cut = Vec::new();
    if x > 0.0 {
        cut.push(texts.get("overflow-too-wide", Some(&args!("mm" => mm(texts, x)))));
    }
    if y > 0.0 {
        cut.push(texts.get("overflow-too-tall", Some(&args!("mm" => mm(texts, y)))));
    }
    if cut.len() == 2 {
        texts.get(
            "overflow-and",
            Some(&args!("first" => cut[0].clone(), "second" => cut[1].clone())),
        )
    } else {
        cut.pop().unwrap_or_default()
    }
}

/// The notes of a print report, without the summary and the reminder. `explicit_orientation`:
/// the orientation was chosen, so trying the other one is worth a hint. `in_layout`: the page is
/// a layout's sheet, so the overflow warning also points at `centre_map_mm` and `crop`.
fn print_notes(
    texts: &Texts,
    report: &PrintReport,
    paper: Paper,
    explicit_orientation: bool,
    kind: MapKind,
    in_layout: bool,
) -> Vec<String> {
    ops::note::print_notes(report, paper, explicit_orientation, kind, in_layout)
        .iter()
        .map(|n| print_note_line(texts, n))
        .collect()
}

/// The message for one note of a print report. No wildcard arm.
fn print_note_line(texts: &Texts, note: &ops::note::PrintNote) -> String {
    use ops::note::PrintNote as P;
    match note {
        P::NonStandardScale { print_scale, kind } => texts.get(
            "warning-non-standard-scale",
            Some(&args!(
                "print_scale" => print_scale.to_string(),
                "kind" => texts.plain(kind_word_id(*kind)),
                "base_scale" => kind.base_scale().to_string(),
            )),
        ),
        P::Overflow {
            x_mm,
            y_mm,
            paper,
            explicit_orientation,
            kind,
            print_scale,
            in_layout,
        } => {
            let cut = cut_text(texts, *x_mm, *y_mm);
            let mut hints = Vec::new();
            if *paper == Paper::A4 {
                hints.push(texts.plain("hint-paper-a3"));
            }
            // Auto already took the orientation that cuts off less.
            if *explicit_orientation {
                hints.push(texts.plain("hint-other-orientation"));
            }
            // The next larger allowed number shrinks the map on the page.
            match freemapper_print::scale::allowed_scales(*kind)
                .into_iter()
                .find(|&s| s > *print_scale)
            {
                Some(next) => {
                    let id = if is_standard(*kind, next) {
                        "hint-scale-example"
                    } else {
                        "hint-scale-example-non-standard"
                    };
                    hints.push(texts.get(id, Some(&args!("next" => next.to_string()))));
                }
                None => hints.push(texts.plain("hint-scale-larger")),
            }
            let last = hints.pop().unwrap_or_default();
            let mut warning = if hints.is_empty() {
                texts.get(
                    "warning-overflow-single",
                    Some(&args!("cut" => cut, "last" => last)),
                )
            } else {
                texts.get(
                    "warning-overflow",
                    Some(&args!("cut" => cut, "hints" => hints.join(", "), "last" => last)),
                )
            };
            if *in_layout {
                warning.push(' ');
                warning.push_str(&texts.plain("hint-crop"));
            }
            warning
        }
        P::Skipped(count) => texts.get("note-skipped", Some(&args!("count" => *count))),
        P::UnknownType(count) => texts.get("note-unknown-type", Some(&args!("count" => *count))),
        P::Truncated(count) => texts.get("note-truncated", Some(&args!("count" => *count))),
        P::PageLimit(count) => texts.get("note-page-limit", Some(&args!("count" => *count))),
        P::DroppedPattern(count) => {
            texts.get("note-dropped-pattern", Some(&args!("count" => *count)))
        }
        P::SubstitutedFonts(fonts) => {
            let names: Vec<String> = fonts.iter().map(|f| printable(f)).collect();
            texts.get("note-fonts", Some(&args!("names" => names.join(", "))))
        }
        P::MissingGlyphs(glyphs) => {
            let chars: String = glyphs
                .iter()
                .map(|c| printable(&c.to_string()))
                .collect::<Vec<_>>()
                .join(" ");
            texts.get("note-missing-glyphs", Some(&args!("chars" => chars)))
        }
    }
}

impl ops::Notice {
    /// The note in words, with the message the CLI prints: the one conversion the CLI and the MCP server share.
    pub(crate) fn describe(&self, texts: &Texts) -> ops::note::Note {
        let message = match &self.value {
            ops::NoteValue::Sheet(note) => sheet_note_line(texts, note),
            ops::NoteValue::Print(note) => print_note_line(texts, note),
            ops::NoteValue::SheetFailed(err) => {
                let message = printable(&format!("{:#}", export_error(texts, err)));
                texts.get("error-line", Some(&args!("message" => message)))
            }
        };
        ops::note::Note {
            kind: self.kind(),
            severity: self.severity(),
            course: self.course.clone(),
            message,
        }
    }
}

/// `err` and its sources as one message, "outer: inner: innermost", as anyhow's `{:#}` writes a chain.
fn chain(err: &dyn std::error::Error) -> anyhow::Error {
    let mut text = err.to_string();
    let mut source = err.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    anyhow::Error::msg(text)
}

/// The `--lang` value from the raw arguments, found before clap runs because the help texts
/// depend on it. The last occurrence wins, as it does for clap; `--` ends the options.
fn lang_from_args(args: &[std::ffi::OsString]) -> Option<String> {
    let mut found = None;
    let mut iter = args.iter().skip(1).map(|a| a.to_string_lossy());
    while let Some(arg) = iter.next() {
        if arg == "--" {
            break;
        } else if arg == "--lang" {
            found = iter.next().map(|v| v.into_owned());
        } else if let Some(value) = arg.strip_prefix("--lang=") {
            found = Some(value.to_owned());
        }
    }
    found
}

/// The derived command with its help texts taken from the catalog of `texts`. Clap's own
/// headings ("Usage", "Options") and its parse errors stay English.
fn localised_command(texts: &Texts) -> clap::Command {
    Cli::command()
        .about(texts.plain("cli-about"))
        .mut_arg("_lang", |a| a.help(texts.plain("help-lang")))
        .mut_subcommand("render", |c| {
            c.about(texts.plain("render-about"))
                .after_long_help(format!(
                    "{}\n\n{}",
                    texts.plain("render-after-size"),
                    texts.plain("render-after-exit-codes")
                ))
                .mut_arg("map", |a| a.help(texts.plain("help-map")))
                .mut_arg("out", |a| a.help(texts.plain("help-out")))
                .mut_arg("scale", |a| a.help(texts.plain("help-scale")))
                .mut_arg("paper", |a| a.help(texts.plain("help-paper")))
                .mut_arg("orientation", |a| a.help(texts.plain("help-orientation")))
                .mut_arg("kind", |a| a.help(texts.plain("help-kind")))
                .mut_arg("show_sheet_objects", |a| {
                    a.help(texts.plain("help-show-sheet-objects"))
                })
        })
        .mut_subcommand("init", |c| {
            c.about(texts.plain("init-about"))
                .after_long_help(format!(
                    "{}\n\n{}",
                    texts.plain("init-after-content"),
                    texts.plain("init-after-exit-codes")
                ))
                .mut_arg("folder", |a| a.help(texts.plain("help-init-folder")))
                .mut_arg("map", |a| a.help(texts.plain("help-init-map")))
                .mut_arg("title", |a| a.help(texts.plain("help-init-title")))
                .mut_arg("kind", |a| a.help(texts.plain("help-init-kind")))
                .mut_arg("courses", |a| a.help(texts.plain("help-init-courses")))
        })
        .mut_subcommand("check", |c| {
            c.about(texts.plain("check-about"))
                .after_long_help(texts.plain("check-after-exit-codes"))
                .mut_arg("project", |a| a.help(texts.plain("help-project")))
        })
        .mut_subcommand("export", |c| {
            c.about(texts.plain("export-about"))
                .after_long_help(format!(
                    "{}\n\n{}",
                    texts.plain("render-after-size"),
                    texts.plain("export-after-exit-codes")
                ))
                .mut_arg("project", |a| a.help(texts.plain("help-project")))
                .mut_arg("layout", |a| a.help(texts.plain("help-layout")))
                .mut_arg("course", |a| a.help(texts.plain("help-course")))
                .mut_arg("single_file", |a| a.help(texts.plain("help-single-file")))
                .mut_arg("out_dir", |a| a.help(texts.plain("help-out-dir")))
        })
        .mut_subcommand("numbers", |c| {
            c.about(texts.plain("numbers-about"))
                .mut_subcommand("accept", |c| {
                    c.about(texts.plain("numbers-accept-about"))
                        .after_long_help(format!(
                            "{}\n\n{}",
                            texts.plain("numbers-accept-after-content"),
                            texts.plain("numbers-accept-after-exit-codes")
                        ))
                        .mut_arg("project", |a| a.help(texts.plain("help-project")))
                        .mut_arg("course", |a| a.help(texts.plain("help-numbers-course")))
                        .mut_arg("control", |a| a.help(texts.plain("help-numbers-control")))
                        .mut_arg("layout_wide", |a| {
                            a.help(texts.plain("help-numbers-layout-wide"))
                        })
                        .mut_arg("sheet", |a| a.help(texts.plain("help-numbers-sheet")))
                        .mut_arg("direction", |a| {
                            a.help(texts.plain("help-numbers-direction"))
                        })
                        .mut_arg("gap", |a| a.help(texts.plain("help-numbers-gap")))
                })
        })
        .mut_subcommand("masks", |c| {
            c.about(texts.plain("masks-about"))
                .mut_subcommand("window", |c| {
                    c.about(texts.plain("masks-window-about"))
                        .after_long_help(format!(
                            "{}\n\n{}",
                            texts.plain("masks-window-after-content"),
                            texts.plain("masks-after-exit-codes")
                        ))
                        .mut_arg("project", |a| a.help(texts.plain("help-project")))
                        .mut_arg("course", |a| {
                            a.help(texts.plain("help-masks-window-course"))
                        })
                        .mut_arg("controls", |a| a.help(texts.plain("help-masks-controls")))
                        .mut_arg("radius", |a| a.help(texts.plain("help-masks-radius")))
                        .mut_arg("radius_for", |a| {
                            a.help(texts.plain("help-masks-radius-for"))
                        })
                        .mut_arg("no_start", |a| a.help(texts.plain("help-masks-no-start")))
                        .mut_arg("no_finish", |a| a.help(texts.plain("help-masks-no-finish")))
                        .mut_arg("id", |a| a.help(texts.plain("help-masks-window-id")))
                        .mut_arg("assign", |a| {
                            a.help(texts.plain("help-masks-window-assign"))
                        })
                })
                .mut_subcommand("corridor", |c| {
                    c.about(texts.plain("masks-corridor-about"))
                        .after_long_help(format!(
                            "{}\n\n{}",
                            texts.plain("masks-corridor-after-content"),
                            texts.plain("masks-after-exit-codes")
                        ))
                        .mut_arg("project", |a| a.help(texts.plain("help-project")))
                        .mut_arg("points", |a| a.help(texts.plain("help-masks-points")))
                        .mut_arg("width", |a| a.help(texts.plain("help-masks-width")))
                        .mut_arg("id", |a| a.help(texts.plain("help-masks-corridor-id")))
                        .mut_arg("assign", |a| {
                            a.help(texts.plain("help-masks-corridor-assign"))
                        })
                        .mut_arg("course", |a| {
                            a.help(texts.plain("help-masks-corridor-course"))
                        })
                })
        })
        .mut_subcommand("edit", |c| {
            c.about(texts.plain("edit-about"))
                .mut_arg("project", |a| a.help(texts.plain("help-project")))
                .mut_arg("port", |a| a.help(texts.plain("help-edit-port")))
                .mut_arg("no_browser", |a| a.help(texts.plain("edit-no-browser")))
        })
        .mut_subcommand("update", |c| {
            c.about(texts.plain("update-about"))
                .mut_arg("check", |a| a.help(texts.plain("help-update-check")))
                .mut_arg("yes", |a| a.help(texts.plain("help-update-yes")))
                .mut_arg("version", |a| a.help(texts.plain("help-update-version")))
                .mut_arg("rollback", |a| a.help(texts.plain("help-update-rollback")))
                .mut_arg("mode", |a| a.help(texts.plain("help-update-mode")))
        })
        .mut_subcommand("mcp", |c| {
            c.about(texts.plain("mcp-about")).after_long_help(format!(
                "{}\n\n{}",
                texts.plain("mcp-after-content"),
                texts.plain("mcp-after-exit-codes")
            ))
        })
}

fn main() -> std::process::ExitCode {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let env = |name: &str| std::env::var(name).ok();
    let fixed_lang = editor::fixed_language(lang_from_args(&args).as_deref(), &env);
    let lang = match detect_lang(lang_from_args(&args).as_deref(), &env) {
        Ok(lang) => lang,
        Err(unknown) => {
            // The language of the message itself comes from the environment alone.
            let texts = Texts::new(&detect_lang(None, &env).unwrap_or_else(|_| Lang::english()));
            let available: Vec<String> = unknown.available.iter().map(Lang::code).collect();
            let message = texts.get(
                "error-unknown-lang",
                Some(&args!(
                    "lang" => printable(&unknown.given),
                    "available" => available.join(", "),
                )),
            );
            eprintln!(
                "{}",
                texts.get("error-line", Some(&args!("message" => message)))
            );
            return std::process::ExitCode::from(2);
        }
    };
    let texts = Texts::new(&lang);
    let matches = localised_command(&texts).get_matches_from(args);
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    // The MCP server's stdout is the protocol, and a stale file may be an older server still
    // running; it is left alone.
    if !matches!(cli.command, Commands::Mcp) {
        update::background::remove_stale_files();
    }
    let daily = cli
        .command
        .update_check()
        .then(update::background::CheckContext::of_process)
        .and_then(|ctx| update::background::start_background_check(&ctx));
    let result = match cli.command {
        Commands::Render(args) => render(&texts, &args),
        Commands::Init(args) => init(&texts, &args),
        Commands::Check(args) => check(&texts, &args),
        Commands::Export(args) => export(&texts, &args),
        Commands::Numbers(args) => match args.command {
            NumbersCommand::Accept(args) => numbers_accept(&texts, &args),
        },
        Commands::Masks(args) => match args.command {
            MasksCommand::Window(args) => masks_window(&texts, &args),
            MasksCommand::Corridor(args) => masks_corridor(&texts, &args),
        },
        Commands::Edit(mut args) => {
            args.lang = fixed_lang;
            // One line on stderr at the start, no page banner (ADR 0060, P4).
            update::background::check_for_server(
                &update::background::CheckContext::of_process(),
                texts.clone(),
                |note| eprintln!("{note}"),
            );
            editor::run(&args, &texts)
                .map(|()| 0)
                .map_err(|err| op_error(&texts, err))
        }
        Commands::Mcp => mcp::run(&texts),
        Commands::Update(args) => update::command::run(
            &texts,
            update::command::Request {
                check: args.check,
                yes: args.yes,
                version: args.version,
                rollback: args.rollback,
                mode: args.mode.map(Into::into),
            },
        ),
    };
    // Only a command that ended with exit code 0 counts as a success: mode `install` waits for the
    // next clean run (ADR 0060, P5).
    let command_ok = matches!(result, Ok(0));
    let code = match result {
        Ok(code) => std::process::ExitCode::from(code),
        Err(err) => {
            // Errors can quote map content, so they get the same escaping as the notes.
            let message = printable(&format!("{err:#}"));
            eprintln!(
                "{}",
                texts.get("error-line", Some(&args!("message" => message)))
            );
            let code = if err.downcast_ref::<UsageError>().is_some() {
                2
            } else {
                1
            };
            std::process::ExitCode::from(code)
        }
    };
    // After the command's own output, its error included.
    if let Some(report) = daily.and_then(|daily| daily.finish(&texts, command_ok)) {
        report.print();
    }
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> PrintReport {
        PrintReport {
            page_mm: (297.0, 210.0),
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
            source_objects: vec![],
        }
    }

    fn lines(r: &PrintReport, paper: PaperArg) -> Vec<String> {
        report_lines(
            &Texts::new(&Lang::english()),
            r,
            paper,
            OrientationArg::Auto,
            MapKind::Sprint,
            4000,
            Path::new("o.pdf"),
        )
    }

    #[test]
    fn empty_report_has_summary_and_reminder_only() {
        let l = lines(&report(), PaperArg::A4);
        assert_eq!(l.len(), 2, "{l:?}");
        assert!(
            l[0].contains("A4 landscape") && l[0].contains("1:4000"),
            "{l:?}"
        );
        assert!(l[1].contains("actual size"), "{l:?}");
    }

    #[test]
    fn full_report_has_every_note_in_order() {
        let r = PrintReport {
            overflow_mm: Some((12.34, 0.04)),
            skipped_objects: 3,
            truncated_objects: 2,
            substituted_fonts: vec!["Comic Sans".into(), "Papyrus".into()],
            missing_glyphs: vec!['Ж', '猫'],
            source_objects: vec![],
            ..report()
        };
        let l = lines(&r, PaperArg::A4);
        assert_eq!(l.len(), 7, "{l:?}");
        assert!(
            l[1].contains("does not fit") && l[1].contains("12.3 mm too wide"),
            "{l:?}"
        );
        assert!(
            l[1].contains("0.1 mm too tall") && l[1].contains("A3"),
            "{l:?}"
        );
        assert!(l[2].contains("3 objects are not drawn"), "{l:?}");
        assert!(
            l[3].contains("2 objects were simplified or cut short"),
            "{l:?}"
        );
        assert!(l[4].contains("Comic Sans, Papyrus"), "{l:?}");
        assert!(l[5].contains("no glyph") && l[5].contains("Ж 猫"), "{l:?}");
        assert!(l[6].contains("actual size"), "{l:?}");
    }

    #[test]
    fn limit_notes_name_their_cause() {
        let r = PrintReport {
            unknown_type_objects: 2,
            objects_not_drawn_page_limit: 17,
            dropped_pattern_objects: 1,
            ..report()
        };
        let l = lines(&r, PaperArg::A4);
        assert_eq!(l.len(), 5, "{l:?}");
        assert!(l[1].contains("2 objects are not drawn") && l[1].contains("object type"));
        assert!(l[2].contains("17 objects are not drawn") && l[2].contains("drawing limit"));
        assert!(
            l[3].contains("1 object is drawn without an area pattern"),
            "{l:?}"
        );
    }

    #[test]
    fn every_sheet_note_has_a_text_with_its_fields() {
        let s = |v: &str| v.to_string();
        let mut notes: Vec<(SheetNote, Vec<&str>)> = vec![
            (
                SheetNote::ElementOutsidePage { id: s("logo1") },
                vec!["logo1"],
            ),
            (
                SheetNote::FooterOutsidePage {
                    content: FooterContent::Credit,
                },
                vec![],
            ),
            (
                SheetNote::ElementOverlapsMap { id: s("logo1") },
                vec!["logo1"],
            ),
            (
                SheetNote::FooterOverlapsMap {
                    content: FooterContent::CopyrightAndCredit,
                },
                vec!["margin_mm"],
            ),
            (SheetNote::LargerThanA3, vec!["A3"]),
            (
                SheetNote::MissingScaleOrContour {
                    layout: s("L1"),
                    missing: vec![
                        MissingItem::PrintScaleText,
                        MissingItem::ContourText,
                        MissingItem::ContourValue,
                    ],
                },
                vec![
                    "L1",
                    "{print_scale}",
                    "{contour_interval}",
                    "map.contour_interval_m",
                ],
            ),
            (SheetNote::NorthLinesAdded { count: 7 }, vec!["7", "OOM"]),
            (
                SheetNote::MapCropped {
                    overflow_mm: (0.0, 14.0),
                },
                vec!["14", "map_frame.crop"],
            ),
            (
                SheetNote::NorthLinesWithoutSymbol { count: 3 },
                vec!["3", "601"],
            ),
            (
                SheetNote::CopyrightTruncated {
                    layout: s("L1"),
                    total: 9,
                    shown: 5,
                },
                vec!["L1", "9", "5"],
            ),
            (
                SheetNote::LogoSkipped {
                    id: s("logo1"),
                    file: Some(PathBuf::from("logos/x.png")),
                    reason: LogoSkipReason::Undecodable {
                        error: s("broken file"),
                    },
                },
                vec!["logo1", "logos/x.png", "broken file"],
            ),
            (
                SheetNote::LogoSimplified {
                    id: s("logo1"),
                    reason: LogoSimplification::ClipPath,
                },
                vec!["logo1"],
            ),
            (
                SheetNote::UnknownPlaceholder {
                    id: s("t1"),
                    name: s("curse"),
                },
                vec!["t1", "{curse}"],
            ),
            (
                SheetNote::UnknownPart {
                    layout: s("L1"),
                    name: s("courses"),
                },
                vec!["L1", "courses"],
            ),
            (
                SheetNote::CreditLineMoved { id: s("credit") },
                vec!["credit"],
            ),
            (
                SheetNote::CopyrightLineCut {
                    line: s("© Someone, a very long line"),
                },
                vec!["© Someone, a very long line"],
            ),
            (
                SheetNote::FooterCoversElement {
                    id: s("bar1"),
                    content: FooterContent::Copyright,
                },
                vec!["bar1"],
            ),
            (
                SheetNote::MissingScaleCheckBar { layout: s("L1") },
                vec!["L1", "scale_check_bar"],
            ),
            (
                SheetNote::CourseOutsideFrame {
                    course: s("D/H 12"),
                    controls: vec![s("31"), s("47")],
                },
                vec!["D/H 12", "31, 47", "map_frame.centre_map_mm"],
            ),
            (
                SheetNote::DefaultLayout {
                    course: s("Kurz"),
                    layout: s("a4"),
                },
                vec!["Kurz", "a4"],
            ),
            (
                SheetNote::NoDescriptionBox {
                    course: s("Kurz"),
                    reason: NoBoxReason::IofXml,
                },
                vec!["Kurz", "IOF"],
            ),
            (
                SheetNote::NoDescriptionBox {
                    course: s("Kurz"),
                    reason: NoBoxReason::NoElement,
                },
                vec!["Kurz", "control_descriptions"],
            ),
            (
                SheetNote::NoDescriptionBox {
                    course: s("Kurz"),
                    reason: NoBoxReason::OutsidePage,
                },
                vec!["Kurz", "cell_mm"],
            ),
            (
                SheetNote::NoDescriptionBox {
                    course: s("Kurz"),
                    reason: NoBoxReason::NoDescriptions,
                },
                vec!["Kurz", "Purple Pen"],
            ),
            (
                SheetNote::ElementCoversCourse {
                    element: s("descriptions"),
                    controls: vec![s("S1"), s("31")],
                },
                vec!["descriptions", "S1, 31"],
            ),
            (
                SheetNote::UnknownDescriptionSymbol {
                    course: s("Kurz"),
                    id: s("99.9"),
                },
                vec!["Kurz", "99.9"],
            ),
            (SheetNote::OverprintCut { course: s("Kurz") }, vec!["Kurz"]),
            (
                SheetNote::MasksCapped {
                    course: s("Kurz"),
                    applied: 1000,
                    total: 2345,
                },
                vec!["Kurz", "1000", "2345"],
            ),
            (
                SheetNote::ControlInMaskedArea {
                    course: s("Kurz"),
                    controls: vec![
                        StopName::Start(s("S1")),
                        StopName::Control(s("31")),
                        StopName::Control(s("32")),
                        StopName::Finish(s("F1")),
                    ],
                },
                vec!["Kurz", "31", "32"],
            ),
            (
                SheetNote::ControlInMaskedArea {
                    course: s("Kurz"),
                    controls: vec![StopName::Control(s("32"))],
                },
                vec!["Kurz", "32"],
            ),
            (
                SheetNote::MaskCoversNothing {
                    course: s("Kurz"),
                    mask: s("fenster"),
                },
                vec!["Kurz", "fenster"],
            ),
            (
                SheetNote::WhiteOutsCoverNothing {
                    course: s("Kurz"),
                    ids: vec![s("whiteout-1"), s("whiteout-3")],
                },
                vec!["Kurz", "whiteout-1", "whiteout-3"],
            ),
            (
                SheetNote::WhiteOutsCoverNothing {
                    course: s("Kurz"),
                    ids: (1..=12).map(|i| format!("whiteout-{i}")).collect(),
                },
                vec!["Kurz", "whiteout-10", "12", "2"],
            ),
            (
                SheetNote::ElementCoversWindow {
                    course: s("Kurz"),
                    element: s("descriptions"),
                    mask: s("fenster"),
                },
                vec!["Kurz", "descriptions", "fenster"],
            ),
            (
                SheetNote::MaskLeavesNoWindow {
                    course: s("Kurz"),
                    mask: s("fenster"),
                },
                vec!["Kurz", "fenster"],
            ),
            (
                SheetNote::MaskCheckLimited { course: s("Kurz") },
                vec!["Kurz"],
            ),
            (
                SheetNote::Mask(MaskNote::MaskUnused { id: s("spare") }),
                vec!["spare", "[[masks]]"],
            ),
            (
                SheetNote::Mask(MaskNote::WhiteOutByNumber {
                    course: s("Kurz"),
                    id: s("whiteout-2"),
                    centre: [-12.25, 40.0],
                    stand_in: false,
                }),
                vec!["Kurz", "whiteout-2", "-12", "40", "Purple Pen"],
            ),
            (
                SheetNote::Mask(MaskNote::WhiteOutByNumber {
                    course: s("Kurz"),
                    id: s("whiteout-2"),
                    centre: [-12.25, 40.0],
                    stand_in: true,
                }),
                vec!["Kurz", "whiteout-2", "[[masks]]", "-12", "40", "Purple Pen"],
            ),
            (
                SheetNote::MapCroppedToCourse {
                    overflow_mm: (12.0, 0.0),
                },
                vec!["12"],
            ),
            (
                SheetNote::CoursePlaceholderNotFilled {
                    element: s("t2"),
                    name: s("length_km"),
                },
                vec!["t2", "{length_km}"],
            ),
            (
                SheetNote::CourseValueUnknown {
                    course: s("Kurz"),
                    id: s("t1"),
                    name: s("climb_m"),
                },
                vec!["Kurz", "t1", "{climb_m}"],
            ),
            (
                SheetNote::Course(CourseNote::UnknownElement(s("Gizmo"))),
                vec!["Gizmo"],
            ),
            (
                SheetNote::Course(CourseNote::Forked {
                    family: s("Staffel"),
                    variants: vec![s("A1"), s("A2")],
                }),
                vec!["Staffel", "A1", "A2"],
            ),
            (
                SheetNote::Course(CourseNote::AlternativeControls(s("Lang"))),
                vec!["Lang"],
            ),
            (SheetNote::Course(CourseNote::NotesTruncated), vec![]),
            (SheetNote::Course(CourseNote::WhiteOutLaterInM4), vec![]),
            (SheetNote::Course(CourseNote::NoDescriptions), vec![]),
            (
                SheetNote::Course(CourseNote::MapIssueAsStart(s("ISSUE1"))),
                vec!["ISSUE1"],
            ),
            (
                SheetNote::Course(CourseNote::Variations(s("Schleife"))),
                vec!["Schleife"],
            ),
            (SheetNote::Fit(FitNote::YUpDetected), vec![]),
            (
                SheetNote::Fit(FitNote::OrientationAssumed),
                vec!["course_y_axis", "[sources]"],
            ),
            (
                SheetNote::Fit(FitNote::AxisAgainstFormat {
                    set: YAxis::Down,
                    format: YAxis::Up,
                }),
                vec!["course_y_axis = \"down\"", "Purple Pen"],
            ),
            (SheetNote::Fit(FitNote::ScaleUnchecked), vec![]),
            (
                SheetNote::Fit(FitNote::GeorefChecked {
                    controls: 4,
                    metres: 3.25,
                }),
                vec!["4", "3"],
            ),
            (
                SheetNote::Fit(FitNote::GeorefSkipped(SkipReason::NotGeoreferenced)),
                vec![],
            ),
            (
                SheetNote::Fit(FitNote::GeorefSkipped(SkipReason::UnsupportedCrs(s(
                    "EPSG:31467",
                )))),
                vec!["EPSG:31467", "UTM"],
            ),
            (
                SheetNote::Fit(FitNote::GeorefSkipped(SkipReason::NoLatLng)),
                vec![],
            ),
            (
                SheetNote::Fit(FitNote::GeorefSkipped(SkipReason::CheckOff)),
                vec![],
            ),
            (
                SheetNote::Fit(FitNote::GeorefCheckOff),
                vec!["course_georef_check"],
            ),
            (
                SheetNote::Fit(FitNote::PlacedByGeoref(vec![s("33")])),
                vec!["33"],
            ),
            (
                SheetNote::Stale(Stale::Assignment { course: s("Alt") }),
                vec!["Alt"],
            ),
            (
                SheetNote::Stale(Stale::Correction {
                    control: s("47"),
                    scope: CorrectionScope::Layout(s("a3")),
                }),
                vec!["47", "a3"],
            ),
            (
                SheetNote::Stale(Stale::Correction {
                    control: s("48"),
                    scope: CorrectionScope::Course(s("Kurz")),
                }),
                vec!["48", "Kurz"],
            ),
            (
                SheetNote::Stale(Stale::CorrectionOverridesChangedBase { control: s("49") }),
                vec!["49"],
            ),
            (
                SheetNote::Stale(Stale::Mask {
                    course: s("Kurz"),
                    mask: s("whiteout-9"),
                }),
                vec!["Kurz", "whiteout-9"],
            ),
            (
                SheetNote::NumberConflict(NumberConflict {
                    course: s("Kurz"),
                    control: s("31"),
                    conflicts: vec![
                        Conflict::Symbol(s("32")),
                        Conflict::Leg(s("S1"), s("31")),
                        Conflict::Number(s("33")),
                        Conflict::Element(s("title")),
                        Conflict::OutsideFrame,
                        Conflict::MapSymbol {
                            line: false,
                            code: s("204"),
                            count: 2,
                        },
                        Conflict::MapSymbol {
                            line: true,
                            code: s("506"),
                            count: 1,
                        },
                    ],
                    proposal: freemapper_print::course::NumberPlacement {
                        direction: 112.5,
                        gap_mm: 0.8,
                    },
                    free: true,
                    searched: true,
                }),
                vec![
                    "Kurz", "31", "32", "S1", "33", "title", "112", "5°", "0", "8 mm", "2 ", "204",
                    "506",
                ],
            ),
            (
                SheetNote::NumberConflict(NumberConflict {
                    course: s("Kurz"),
                    control: s("32"),
                    conflicts: vec![Conflict::MapSymbol {
                        line: true,
                        code: s("501.1"),
                        count: 3,
                    }],
                    proposal: freemapper_print::course::NumberPlacement {
                        direction: 45.0,
                        gap_mm: 0.5,
                    },
                    free: false,
                    searched: true,
                }),
                vec!["Kurz", "32", "501", "3 ", "45°"],
            ),
            (
                SheetNote::NumberConflict(NumberConflict {
                    course: s("Kurz"),
                    control: s("33"),
                    conflicts: vec![Conflict::Symbol(s("31"))],
                    proposal: freemapper_print::course::NumberPlacement {
                        direction: 45.0,
                        gap_mm: 0.5,
                    },
                    free: false,
                    searched: false,
                }),
                vec!["Kurz", "33"],
            ),
            (
                SheetNote::NumberSearchLimited { course: s("Kurz") },
                vec!["Kurz"],
            ),
            (
                SheetNote::NumberGapOutOfRange {
                    course: s("Kurz"),
                    control: s("31"),
                    gap_mm: 0.25,
                    min: 0.5,
                    max: 5.0,
                },
                vec!["Kurz", "31", "25 mm", "5 mm"],
            ),
        ];
        // Each kind of masked object, once and several times.
        for kind in [
            MaskedObject::MarkedRoute,
            MaskedObject::CrossingPoint,
            MaskedObject::ForbiddenRoute,
            MaskedObject::Boundary,
            MaskedObject::OutOfBoundsArea,
            MaskedObject::DangerousArea,
            MaskedObject::Text,
        ] {
            for count in [1, 7] {
                let fields = if count == 1 {
                    vec!["Kurz"]
                } else {
                    vec!["Kurz", "7 "]
                };
                notes.push((
                    SheetNote::OverprintInMaskedArea {
                        course: s("Kurz"),
                        kind,
                        count,
                    },
                    fields,
                ));
            }
        }
        for lang in text::available() {
            let texts = Texts::new(&lang);
            let mut seen = std::collections::HashSet::new();
            let (info, note_word) = match lang.code().as_str() {
                "de" => ("Zur Information: ", "Hinweis: "),
                _ => ("Info: ", "Note: "),
            };
            for (note, fields) in &notes {
                let line = sheet_note_line(&texts, note);
                // Notes that do not count toward the exit code say so in their prefix.
                let prefix = if note.is_info() { info } else { note_word };
                assert!(line.starts_with(prefix), "{line}");
                assert!(!line.starts_with("note-"), "{line}");
                assert!(seen.insert(line.clone()), "two notes share {line}");
                for f in fields {
                    assert!(line.contains(f), "{}: {f} missing in {line}", lang.code());
                }
            }
        }
    }

    /// Every logo reason, with details that name themselves.
    fn all_logo_reasons() -> Vec<LogoSkipReason> {
        use LogoSkipReason::*;
        let detail = |d: &str| d.to_string();
        vec![
            NeedsFileAndWidth,
            NotFound,
            Unreadable {
                error: detail("os-error-text"),
            },
            NotAFile,
            FileTooLarge,
            UnknownFormat,
            TooManyPixels {
                width: 6000,
                height: 5000,
            },
            TooWideOrHigh {
                width: 70000,
                height: 2,
            },
            Undecodable {
                error: detail("decoder-text"),
            },
            DecoderLimit,
            SvgNotUtf8,
            SvgUnreadable {
                error: detail("xml-text"),
            },
            SvgUnsupported {
                what: detail("the element <text>"),
            },
            SvgTooComplex {
                what: detail("more than 20 000 path segments"),
            },
            CannotSize,
            PixelBudget,
            SvgBudget,
        ]
    }

    #[test]
    fn logo_reasons_have_texts_in_every_language() {
        let simplifications = [
            LogoSimplification::Gradient,
            LogoSimplification::Pattern,
            LogoSimplification::Opacity,
            LogoSimplification::Dashes,
            LogoSimplification::Image,
            LogoSimplification::Text,
            LogoSimplification::ClipPath,
        ];
        for lang in text::available() {
            let texts = Texts::new(&lang);
            let mut seen = std::collections::HashSet::new();
            for reason in all_logo_reasons() {
                let note = SheetNote::LogoSkipped {
                    id: "logo1".into(),
                    file: Some(PathBuf::from("logos/x.svg")),
                    reason: reason.clone(),
                };
                let line = sheet_note_line(&texts, &note);
                assert!(!line.contains("logo-reason"), "{line}");
                assert!(seen.insert(line.clone()), "two reasons share {line}");
                if reason != LogoSkipReason::NeedsFileAndWidth {
                    assert!(line.contains("logos/x.svg"), "{line}");
                }
                // Details come through as they are.
                for d in [
                    "os-error-text",
                    "decoder-text",
                    "xml-text",
                    "<text>",
                    "20 000",
                ] {
                    if reason.to_string().contains(d) {
                        assert!(line.contains(d), "{d} missing in {line}");
                    }
                }
                if lang.code() == "de" {
                    // No English remedy reaches a German run.
                    for english in ["export the logo", "save it", "not found", "on this sheet"] {
                        assert!(!line.contains(english), "{line}");
                    }
                }
            }
            for reason in simplifications {
                let line = sheet_note_line(
                    &texts,
                    &SheetNote::LogoSimplified {
                        id: "logo1".into(),
                        reason,
                    },
                );
                assert!(!line.contains("logo-simplified"), "{line}");
                assert!(seen.insert(line.clone()), "two reasons share {line}");
                if lang.code() == "de" {
                    assert!(!line.contains(&reason.to_string()), "{line}");
                }
            }
        }
    }

    #[test]
    fn footer_notes_name_only_what_the_footer_holds() {
        let notes = |content| {
            [
                SheetNote::FooterOverlapsMap { content },
                SheetNote::FooterOutsidePage { content },
                SheetNote::FooterCoversElement {
                    id: "bar".into(),
                    content,
                },
            ]
        };
        for (lang, copyright, credit) in [
            ("en", "copyright lines", "credit line"),
            ("de", "Quellenangaben", "Namensnennung"),
        ] {
            let texts = Texts::new(
                &text::available()
                    .into_iter()
                    .find(|l| l.code() == lang)
                    .unwrap(),
            );
            for n in notes(FooterContent::Copyright) {
                let line = sheet_note_line(&texts, &n);
                assert!(line.contains(copyright) && !line.contains(credit), "{line}");
            }
            for n in notes(FooterContent::Credit) {
                let line = sheet_note_line(&texts, &n);
                assert!(line.contains(credit) && !line.contains(copyright), "{line}");
            }
            for n in notes(FooterContent::CopyrightAndCredit) {
                let line = sheet_note_line(&texts, &n);
                assert!(line.contains(credit), "{line}");
            }
            // The covers-element text holds in every position: no "inside the map frame".
            let line = sheet_note_line(&texts, &notes(FooterContent::Credit)[2]);
            assert!(
                !line.contains("inside the map frame") && !line.contains("im Kartenausschnitt"),
                "{line}"
            );
        }
    }

    #[test]
    fn notes_cannot_inject_escape_sequences() {
        let line = sheet_note_line(
            &Texts::new(&Lang::english()),
            &SheetNote::UnknownPart {
                layout: "a\u{1b}[2J".into(),
                name: "\u{202e}x".into(),
            },
        );
        assert!(
            !line.contains('\u{1b}') && !line.contains('\u{202e}'),
            "{line:?}"
        );
    }

    #[test]
    fn printable_keeps_newline_and_tab_readable() {
        assert_eq!(
            printable("a\nb\tc\u{1b}d\u{202e}"),
            "a\\nb\\tc\\u{1b}d\\u{202e}"
        );
    }

    #[test]
    fn zero_overflow_prints_no_note() {
        let r = PrintReport {
            overflow_mm: Some((0.0, 0.0)),
            ..report()
        };
        assert_eq!(lines(&r, PaperArg::A4).len(), 2);
    }

    #[test]
    fn hint_names_the_next_allowed_scale_only() {
        let r = PrintReport {
            overflow_mm: Some((1.0, 0.0)),
            ..report()
        };
        let l = lines(&r, PaperArg::A4);
        assert!(
            l[1].contains("for example 1:5000 (not standard, for training only)."),
            "{l:?}"
        );
        let r = PrintReport {
            print_scale: 3000,
            symbol_factor: 4.0 / 3.0,
            ..r
        };
        let l = lines(&r, PaperArg::A4);
        assert!(l[1].ends_with("for example 1:4000."), "{l:?}");
        let r = PrintReport {
            print_scale: 5000,
            ..r
        };
        let l = lines(&r, PaperArg::A4);
        assert!(!l[1].contains("for example"), "{l:?}");
        assert!(
            l[1].contains("a print scale with a larger number."),
            "{l:?}"
        );
    }

    #[test]
    fn non_standard_scale_warns() {
        let r = PrintReport {
            print_scale: 5000,
            non_standard_scale: true,
            ..report()
        };
        let l = lines(&r, PaperArg::A4);
        assert_eq!(l.len(), 3, "{l:?}");
        assert_eq!(
            l[1],
            "Warning: 1:5000 is not a standard scale for sprint maps. The map is reduced and the \
             symbols keep their 1:4000 size. Use this print for training only."
        );
    }

    #[test]
    fn other_orientation_hint_only_after_an_explicit_choice() {
        let r = PrintReport {
            overflow_mm: Some((1.0, 0.0)),
            ..report()
        };
        assert!(!lines(&r, PaperArg::A4)[1].contains("orientation"));
        let explicit = report_lines(
            &Texts::new(&Lang::english()),
            &r,
            PaperArg::A4,
            OrientationArg::Landscape,
            MapKind::Sprint,
            4000,
            Path::new("o.pdf"),
        );
        assert!(
            explicit[1].contains("the other orientation"),
            "{explicit:?}"
        );
    }

    #[test]
    fn single_object_uses_singular() {
        let r = PrintReport {
            skipped_objects: 1,
            truncated_objects: 1,
            ..report()
        };
        let l = lines(&r, PaperArg::A4).join("\n");
        assert!(l.contains("1 object is not drawn"), "{l}");
        assert!(l.contains("1 object was simplified"), "{l}");
    }

    #[test]
    fn single_axis_overflow_names_only_that_axis() {
        let r = PrintReport {
            overflow_mm: Some((0.0, 3.9)),
            ..report()
        };
        let l = lines(&r, PaperArg::A3);
        assert!(l[1].contains("3.9 mm too tall"), "{l:?}");
        assert!(!l[1].contains("too wide"), "{l:?}");
        assert!(!l[1].contains("A3"), "no A3 hint on A3 paper: {l:?}");
        let r = PrintReport {
            overflow_mm: Some((58.0, 0.0)),
            ..report()
        };
        let l = lines(&r, PaperArg::A4);
        assert!(
            l[1].contains("58.0 mm too wide") && !l[1].contains("too tall"),
            "{l:?}"
        );
    }

    #[test]
    fn map_text_cannot_inject_escape_sequences() {
        let r = PrintReport {
            substituted_fonts: vec!["Evil\u{1b}[2J".into()],
            missing_glyphs: vec!['\u{1b}', '\u{7}', '\u{202e}'],
            source_objects: vec![],
            ..report()
        };
        let all = lines(&r, PaperArg::A4).join("\n");
        assert!(
            !all.contains('\u{1b}') && !all.contains('\u{7}') && !all.contains('\u{202e}'),
            "{all:?}"
        );
        assert!(all.contains("Evil\\u{1b}[2J"), "{all:?}");
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    #[test]
    fn layout_overflow_points_at_centre_and_crop() {
        let texts = Texts::new(&Lang::english());
        let report = PrintReport {
            page_mm: (297.0, 210.0),
            print_scale: 4000,
            geometry_factor: 1.0,
            symbol_factor: 1.0,
            non_standard_scale: false,
            overflow_mm: Some((0.0, 14.0)),
            skipped_objects: 0,
            unknown_type_objects: 0,
            truncated_objects: 0,
            objects_not_drawn_page_limit: 0,
            dropped_pattern_objects: 0,
            substituted_fonts: vec![],
            missing_glyphs: vec![],
            source_objects: vec![],
        };
        let sheet = print_notes(&texts, &report, Paper::A4, false, MapKind::Sprint, true);
        assert!(
            sheet[0].contains("map_frame.centre_map_mm") && sheet[0].contains("map_frame.crop"),
            "{sheet:?}"
        );
        // `render` has neither.
        let render = print_notes(&texts, &report, Paper::A4, false, MapKind::Sprint, false);
        assert!(!render[0].contains("map_frame"), "{render:?}");
    }
}
