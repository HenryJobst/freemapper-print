//! The `freemapper-print` binary. Commands come with the milestones in the design
//! (`docs/superpowers/specs/2026-11-08-freemapper-print-design.md`).

mod text;

use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow, bail};
use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
use fluent_bundle::FluentArgs;
use freemapper_print::course::SourceFormat;
use freemapper_print::course::fit::{SkipReason, YAxis};
use freemapper_print::course::{CourseNote, FitNote};
use freemapper_print::courses::{CorrectionScope, LoadedCourses, MaskNote, Stale, load_courses};
use freemapper_print::export::{
    ExportError, course_sheet_report, export_courses, export_courses_single_file, export_layouts,
    select_courses, select_layouts, sheet_report,
};
use freemapper_print::mask_build::{self, BuildError, Selection, WindowOptions};
use freemapper_print::numbers::{Conflict, NumberConflict};
use freemapper_print::omap::MapDocument;
use freemapper_print::omap::read::read_map_file;
use freemapper_print::output::{self, OutputError};
use freemapper_print::print::{Orientation, Paper, PrintOptions, PrintReport, print_map};
use freemapper_print::project::{self, KeyComment, LayoutSpec, LoadedProject, SheetLang};
use freemapper_print::scale::{MapKind, infer_kind, is_standard, read_sidecar};
use freemapper_print::sheet::{
    FooterContent, LogoCache, LogoSimplification, LogoSkipReason, MaskedObject, MissingItem,
    NoBoxReason, SheetNote, SheetReport, course_event_notes,
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
    include_start: bool,
    #[arg(long)]
    include_finish: bool,
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

fn render(texts: &Texts, args: &RenderArgs) -> anyhow::Result<u8> {
    let map = read_map(texts, &args.map)?;
    let (kind, _) = detect_kind(texts, &map, &args.map, args.kind)?;
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
        .map_err(|e| output_error(texts, e))?;
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
    read_map_file(path).with_context(|| {
        texts.get(
            "error-read-map",
            Some(&args!("path" => path.display().to_string())),
        )
    })
}

/// The project file named by `arg`: the file itself, or `freemapper-print.toml` in a folder.
fn project_file(arg: &Path) -> PathBuf {
    if arg.is_dir() {
        arg.join(project::FILE_NAME)
    } else {
        arg.to_path_buf()
    }
}

fn load_project(texts: &Texts, arg: &Path) -> anyhow::Result<LoadedProject> {
    let path = project_file(arg);
    project::load(&path)
        .map_err(|err| match err {
            // A rule of this program rather than a typo, so it is said in the user's language (ADR 0044).
            project::ProjectError::BasePartHidden { file, field } => anyhow!(texts.get(
                "error-base-part-hidden",
                Some(&args!("path" => file, "field" => field)),
            )),
            other => anyhow::Error::new(other),
        })
        .with_context(|| {
            texts.get(
                "error-load-project",
                Some(&args!("path" => path.display().to_string())),
            )
        })
}

/// Metres with the decimal separator of the language and no needless `.0`: `2.5`, `5`.
fn metres(texts: &Texts, value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        texts.decimal(&format!("{value}"))
    }
}

/// Where the map kind came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KindSource {
    Flag,
    Sidecar,
    Inferred,
}

/// The map kind: `arg`, else `map_kind` in `freemapper.toml` next to the map, else inferred from
/// the symbol set and scale (ADR 0021), and where it came from. A `freemapper.toml` that cannot
/// be read or parsed gets a note that says so; without any kind the error asks for `--kind`.
fn detect_kind(
    texts: &Texts,
    map: &MapDocument,
    path: &Path,
    arg: Option<KindArg>,
) -> anyhow::Result<(MapKind, KindSource)> {
    if let Some(kind) = arg {
        return Ok((kind.into(), KindSource::Flag));
    }
    match read_sidecar(path) {
        Ok(Some(kind)) => return Ok((kind, KindSource::Sidecar)),
        Ok(None) => {}
        Err(err) => eprintln!(
            "{}",
            texts.get(
                "note-sidecar",
                // The whole chain: the operating system's error is the source.
                Some(&args!("error" => printable(&format!("{:#}", anyhow::Error::new(err)))))
            )
        ),
    }
    let kind = infer_kind(map, path).context(texts.plain("error-kind-unknown"))?;
    Ok((kind, KindSource::Inferred))
}

fn init(texts: &Texts, args: &InitArgs) -> anyhow::Result<u8> {
    if let Some(title) = &args.title
        && title.chars().any(|c| c.is_control() || is_format(c))
    {
        return Err(UsageError(texts.plain("error-title-control")).into());
    }
    // `export` writes into `out` later; a link there that leads out of the project stops `init`
    // already (ADR 0041). A folder that does not exist yet holds no link.
    if args.folder.is_dir() {
        output::check_out_folder(&args.folder, &args.folder.join("out"))
            .map_err(|e| output_error(texts, e))?;
    }
    let map = read_map(texts, &args.map)?;
    let (kind, source) = detect_kind(texts, &map, &args.map, args.kind)?;
    let shown = args.folder.join(project::FILE_NAME);
    if shown.exists() {
        bail!(texts.get(
            "error-project-exists",
            Some(&args!("path" => shown.display().to_string()))
        ));
    }
    // The course file is read and fitted before anything is created, so that an error writes nothing.
    let plan = match &args.courses {
        Some(file) => Some(plan_courses(texts, args, &map, kind, file)?),
        None => None,
    };
    let folder_error = || {
        texts.get(
            "error-create-folder",
            Some(&args!("path" => args.folder.display().to_string())),
        )
    };
    std::fs::create_dir_all(&args.folder).with_context(folder_error)?;
    // Both canonical, so that links and `..` do not spoil the relative path.
    let dir = args.folder.canonicalize().with_context(folder_error)?;
    let map_path = args.map.canonicalize().with_context(|| {
        texts.get(
            "error-read-map",
            Some(&args!("path" => args.map.display().to_string())),
        )
    })?;
    let title = args.title.clone().unwrap_or_else(|| {
        args.map
            .file_stem()
            .map_or_else(|| "map".into(), |s| s.to_string_lossy().into_owned())
    });
    let sheet_lang = sheet_lang(texts);
    let map_scale = map.georef.scale_denominator;
    let map_rel = project::relative_path(&dir, &map_path);
    let kind_word = texts.plain(kind_word_id(kind));
    let source_id = match source {
        KindSource::Flag => "init-kind-flag",
        KindSource::Sidecar => "init-kind-sidecar",
        KindSource::Inferred => "init-kind-inferred",
    };
    eprintln!(
        "{}",
        texts.get(source_id, Some(&args!("kind" => kind_word.clone())))
    );
    if let Some(plan) = &plan {
        eprintln!(
            "{}",
            texts.get(
                "init-courses-read",
                // As the project file names it: relative to the project folder.
                Some(&args!(
                    "path" => project::relative_path(&dir, &plan.file).display().to_string(),
                    "count" => plan.courses.len(),
                ))
            )
        );
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
        let mut starter = project::starter(&title, map_rel.clone(), kind, scale, sheet_lang);
        // Course sheets cut the map to the course by design.
        starter.layouts[0].map_frame.crop = plan.is_some();
        eprintln!(
            "{}",
            texts.get("init-fitting", Some(&args!("scale" => scale.to_string())))
        );
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
    let fits = fitted.iter().all(|(_, f, _)| *f);
    let contour = new.map.contour_interval_m;
    let contour_comment = match contour {
        Some(_) => texts.get(
            "init-comment-contour",
            Some(&args!("kind" => kind_word.clone())),
        ),
        None => texts.plain("init-comment-no-contour"),
    };
    let footer_comment = texts.plain("init-comment-footer");
    let comments = [
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
    match project::save_new_with_comments(&file, &new, &comments) {
        Ok(()) => {}
        Err(project::ProjectError::Exists { .. }) => bail!(texts.get(
            "error-project-exists",
            Some(&args!("path" => shown.display().to_string()))
        )),
        Err(err) => {
            return Err(anyhow::Error::new(err).context(texts.get(
                "error-write-file",
                Some(&args!("path" => shown.display().to_string())),
            )));
        }
    }
    let path = shown.display().to_string();
    match &plan {
        None => {
            let layout = &new.layouts[0];
            eprintln!(
                "{}",
                texts.get(
                    "init-written",
                    Some(&args!(
                        "path" => path.clone(),
                        "layout" => layout.id.clone(),
                        "paper" => paper_name(layout.paper),
                        "margin" => metres(texts, layout.map_frame.margin_mm),
                        "scale" => map_scale.to_string(),
                        "kind" => kind_word.clone(),
                    ))
                )
            );
            if !fits {
                eprintln!(
                    "{}",
                    texts.get(
                        "init-no-fit",
                        Some(&args!("scale" => map_scale.to_string()))
                    )
                );
                let loaded = LoadedProject {
                    path: file.clone(),
                    dir: dir.clone(),
                    project: new.clone(),
                    // `init` writes the default profile.
                    rules: freemapper_print::rules::base().clone(),
                };
                let (lines, _) = check_lines(texts, &loaded, &map, layout, &LogoCache::new());
                for line in lines {
                    eprintln!("{line}");
                }
            }
        }
        Some(plan) => print_course_init(texts, plan, &new, &fitted, &path, &kind_word, map_scale),
    }
    eprintln!(
        "{}",
        match contour {
            Some(value) => texts.get(
                "init-contour-hint",
                Some(&args!("value" => metres(texts, value), "kind" => kind_word)),
            ),
            None => texts.plain("init-no-contour-hint"),
        }
    );
    let next = if fits {
        "init-next"
    } else {
        "init-next-with-notes"
    };
    eprintln!("{}", texts.get(next, Some(&args!("path" => path))));
    Ok(0)
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

/// Reads and fits the course file of `init --courses` with the real loader, so that a course file that does
/// not fit the map stops `init` before it writes anything. A `.ppen` gives each course its print scale (the
/// map scale where the file has none); IOF XML has none, so every course takes the map scale.
fn plan_courses(
    texts: &Texts,
    args: &InitArgs,
    map: &MapDocument,
    kind: MapKind,
    file: &Path,
) -> anyhow::Result<CoursePlan> {
    let file = file.canonicalize().with_context(|| {
        texts.get(
            "error-read-courses",
            Some(&args!("path" => file.display().to_string())),
        )
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
    let courses =
        read_courses(texts, &loaded, map)?.expect("the probe project names a course file");
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

/// The lines `init --courses` prints after it wrote the file: what was written, each layout with its
/// courses, where the scales and the description box came from.
fn print_course_init(
    texts: &Texts,
    plan: &CoursePlan,
    new: &project::Project,
    fitted: &[(LayoutSpec, bool, u32)],
    path: &str,
    kind_word: &str,
    map_scale: u32,
) {
    eprintln!(
        "{}",
        texts.get(
            "init-written-courses",
            Some(&args!(
                "path" => path.to_string(),
                "layouts" => new.layouts.len(),
                "assignments" => new.assignments.len(),
                "kind" => kind_word.to_string(),
            ))
        )
    );
    for (layout, fits, scale) in fitted {
        let names: Vec<String> = plan
            .courses
            .iter()
            .filter(|(_, s)| s == scale)
            .map(|(n, _)| n.clone())
            .collect();
        eprintln!(
            "{}",
            texts.get(
                "init-courses-layout",
                Some(&args!(
                    "layout" => printable(&layout.id),
                    "scale" => scale.to_string(),
                    "paper" => paper_name(layout.paper),
                    "margin" => metres(texts, layout.map_frame.margin_mm),
                    "courses" => quoted_list(texts, &names),
                ))
            )
        );
        if !fits {
            eprintln!(
                "{}",
                texts.get(
                    "init-courses-no-fit",
                    Some(&args!("scale" => scale.to_string(), "layout" => printable(&layout.id)))
                )
            );
        }
    }
    let scales_id = if plan.iof {
        "init-courses-scales-iof"
    } else {
        "init-courses-scales-ppen"
    };
    eprintln!(
        "{}",
        texts.get(scales_id, Some(&args!("scale" => map_scale.to_string())))
    );
    if !plan.iof {
        eprintln!("{}", texts.plain("init-courses-description-box"));
    }
    eprintln!("{}", texts.plain("init-courses-crop"));
}

/// `numbers accept`: appends a number correction to the project file (ADR 0025, addendum).
fn numbers_accept(texts: &Texts, args: &AcceptArgs) -> anyhow::Result<u8> {
    let usage = |id: &str, a: FluentArgs| UsageError(texts.get(id, Some(&a)));
    let loaded = load_project(texts, &args.project)?;
    let map = read_map(texts, &loaded.map_path())?;
    let Some(courses) = read_courses(texts, &loaded, &map)? else {
        return Err(UsageError(texts.plain("error-numbers-no-course-file")).into());
    };
    let course = courses
        .event
        .courses
        .iter()
        .find(|c| c.name == args.course)
        .ok_or_else(|| {
            let known: Vec<String> = courses
                .event
                .courses
                .iter()
                .map(|c| c.name.clone())
                .collect();
            usage(
                "error-unknown-course",
                args!("name" => printable(&args.course), "known" => quoted_list(texts, &known)),
            )
        })?;
    let mut known: Vec<String> = Vec::new();
    for stop in &course.stops {
        if !known.contains(&stop.control) {
            known.push(stop.control.clone());
        }
    }
    if !known.contains(&args.control) {
        let known: Vec<String> = known.iter().map(|c| printable(c)).collect();
        return Err(usage(
            "error-unknown-control",
            args!(
                "course" => printable(&course.name),
                "control" => printable(&args.control),
                "known" => known.join(", "),
            ),
        )
        .into());
    }
    let layout = freemapper_print::courses::layout_for(&loaded.project, &course.name).0;
    // The scope: the sheet (the course) or every course on the course's layout.
    let (scope_layout, scope_course, scope_text) = if args.sheet {
        (
            None,
            Some(course.name.clone()),
            texts.get(
                "stale-scope-course",
                Some(&args!("name" => printable(&course.name))),
            ),
        )
    } else {
        (
            Some(layout.id.clone()),
            None,
            texts.get(
                "stale-scope-layout",
                Some(&args!("name" => printable(&layout.id))),
            ),
        )
    };
    let exists = loaded.project.numbers.iter().any(|n| {
        n.control == args.control
            && match (&scope_course, &scope_layout) {
                (Some(c), _) => n.course.as_ref() == Some(c),
                (None, Some(l)) => n.course.is_none() && n.layout.as_ref() == Some(l),
                (None, None) => false,
            }
    });
    if exists {
        return Err(usage(
            "error-numbers-exists",
            args!("control" => printable(&args.control), "scope" => scope_text),
        )
        .into());
    }
    let (direction, gap) = match (args.direction, args.gap) {
        (Some(d), Some(g)) => {
            if !d.is_finite() || !g.is_finite() || g <= 0.0 || g > project::MAX_NUMBER_GAP_MM {
                return Err(usage(
                    "error-numbers-value",
                    args!("max" => project::MAX_NUMBER_GAP_MM.to_string()),
                )
                .into());
            }
            (d, g)
        }
        _ => {
            // The proposal of `check`: the same sheet, drawn the same way.
            let (_, report) =
                course_sheet_report(&loaded, &map, &courses, course, &LogoCache::new())
                    .map_err(|err| export_error(texts, err))?;
            let proposal = report.notes.iter().find_map(|n| match n {
                SheetNote::NumberConflict(c)
                    if c.control == args.control && c.course == course.name && c.searched =>
                {
                    Some(c.proposal)
                }
                _ => None,
            });
            match proposal {
                Some(p) => (p.direction, p.gap_mm),
                None => {
                    return Err(usage(
                        "error-numbers-no-proposal",
                        args!("control" => printable(&args.control), "course" => printable(&course.name)),
                    )
                    .into());
                }
            }
        }
    };
    // 0.1 degrees and 0.01 mm, as the placement rounds.
    let direction = (direction * 10.0).round() / 10.0;
    let gap = (gap * 100.0).round() / 100.0;
    // The course file's own position (level 2), so that `check` notices when the file moves the number.
    let base_seen = freemapper_print::numbers::resolve(course, &courses.event, &loaded, &layout.id)
        .into_iter()
        .find(|n| n.control == args.control)
        .and_then(|n| n.base)
        .map(freemapper_print::course::NumberPlacement::from);
    let correction = project::NumberCorrection {
        control: args.control.clone(),
        layout: scope_layout,
        course: scope_course,
        direction,
        gap_mm: Some(gap),
        base_seen,
    };
    // A proposal for the whole layout must suit every course there that prints this number with it (ruling
    // R39): it was searched on one sheet only.
    if args.layout_wide && args.direction.is_none() {
        let blocked = layout_conflicts(texts, &loaded, &map, &courses, &layout.id, &correction)?;
        if !blocked.is_empty() {
            return Err(usage(
                "error-numbers-layout-conflict",
                args!(
                    "control" => printable(&args.control),
                    "layout" => printable(&layout.id),
                    "courses" => quoted_list(texts, &blocked),
                    "course" => printable(&course.name),
                    "direction" => short_decimal(texts, direction, 1),
                    "gap" => short_decimal(texts, gap, 2),
                ),
            )
            .into());
        }
    }
    project::append_correction(&loaded.path, &correction).map_err(|err| {
        // A file that no longer loads has been put back as it was.
        let lead_in = match err {
            project::ProjectError::Write { .. } => "error-write-file",
            _ => "error-load-changed-project",
        };
        anyhow::Error::new(err).context(texts.get(
            lead_in,
            Some(&args!("path" => loaded.path.display().to_string())),
        ))
    })?;
    eprintln!(
        "{}",
        texts.get(
            "numbers-accepted",
            Some(&args!(
                "control" => printable(&args.control),
                "scope" => scope_text,
                "path" => loaded.path.display().to_string(),
                "direction" => short_decimal(texts, direction, 1),
                "gap" => short_decimal(texts, gap, 2),
            ))
        )
    );
    Ok(0)
}

/// The courses printed with layout `layout_id` whose sheet, with the layout-wide `correction` added, has a
/// conflict at its control. Courses with a correction of their own for the control are left out: theirs wins.
fn layout_conflicts(
    texts: &Texts,
    loaded: &LoadedProject,
    map: &MapDocument,
    courses: &LoadedCourses,
    layout_id: &str,
    correction: &project::NumberCorrection,
) -> anyhow::Result<Vec<String>> {
    let mut trial = loaded.clone();
    trial.project.numbers.push(correction.clone());
    let logos = LogoCache::new();
    let mut blocked = Vec::new();
    for c in &courses.event.courses {
        let on_layout = freemapper_print::courses::layout_for(&loaded.project, &c.name)
            .0
            .id
            == layout_id;
        let visits = c.stops.iter().any(|s| s.control == correction.control);
        let own = loaded.project.numbers.iter().any(|n| {
            n.control == correction.control && n.course.as_deref() == Some(c.name.as_str())
        });
        if !on_layout || !visits || own {
            continue;
        }
        let (_, report) = course_sheet_report(&trial, map, courses, c, &logos)
            .map_err(|err| export_error(texts, err))?;
        let conflict = report
            .notes
            .iter()
            .any(|n| matches!(n, SheetNote::NumberConflict(k) if k.control == correction.control));
        if conflict {
            blocked.push(c.name.clone());
        }
    }
    Ok(blocked)
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
        BuildError::UnknownControl { code, known } => {
            let known: Vec<String> = known.iter().map(|c| printable(c)).collect();
            (
                "error-unknown-control",
                args!(
                    "course" => printable(course),
                    "control" => printable(&code),
                    "known" => known.join(", "),
                ),
            )
        }
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

/// Map mm per print mm when the map is printed at 1:`print_scale`.
fn map_mm_per_print_mm(texts: &Texts, map: &MapDocument, print_scale: u32) -> anyhow::Result<f64> {
    let map_scale = map.georef.scale_denominator;
    if map_scale == 0 {
        bail!(texts.plain("error-masks-map-scale"));
    }
    Ok(f64::from(print_scale) / f64::from(map_scale))
}

/// The id a new mask gets: `given` after the checks of the project file, else `default` made unique. A taken id is
/// a usage error.
fn new_mask_id(
    texts: &Texts,
    loaded: &LoadedProject,
    given: Option<&str>,
    default: impl FnOnce(&[&str]) -> String,
) -> anyhow::Result<String> {
    let taken: Vec<&str> = loaded.project.masks.iter().map(|m| m.id.as_str()).collect();
    let Some(id) = given else {
        return Ok(default(&taken));
    };
    let count = id.chars().count();
    if count == 0
        || count > freemapper_print::mask::MAX_ID_CHARS
        || id.chars().any(char::is_control)
    {
        return Err(UsageError(texts.get(
            "error-masks-id",
            Some(&args!("max" => freemapper_print::mask::MAX_ID_CHARS.to_string())),
        ))
        .into());
    }
    if taken.contains(&id) {
        return Err(UsageError(
            texts.get("error-masks-id-taken", Some(&args!("id" => printable(id)))),
        )
        .into());
    }
    Ok(id.to_string())
}

/// Refuses `--assign` for a course whose assignment names another mask (exit 2).
fn check_assignable(texts: &Texts, loaded: &LoadedProject, course: &str) -> anyhow::Result<()> {
    let other = loaded
        .project
        .assignments
        .iter()
        .find(|a| a.course == course)
        .and_then(|a| a.mask.as_deref());
    match other {
        Some(mask) => Err(UsageError(texts.get(
            "error-masks-course-has-mask",
            Some(&args!("course" => printable(course), "mask" => printable(mask))),
        ))
        .into()),
        None => Ok(()),
    }
}

/// Writes `mask` with [`project::append_mask`] and says what went wrong in the user's language.
fn write_mask(
    texts: &Texts,
    loaded: &LoadedProject,
    mask: &freemapper_print::mask::Mask,
    assign: Option<(&str, &str)>,
    comment: &str,
) -> anyhow::Result<()> {
    project::append_mask(&loaded.path, mask, assign, comment).map_err(|err| {
        let path = loaded.path.display().to_string();
        match err {
            project::ProjectError::DuplicateId { id, .. } => {
                UsageError(texts.get("error-masks-id-taken", Some(&args!("id" => printable(&id)))))
                    .into()
            }
            project::ProjectError::CourseHasMask { course, mask, .. } => UsageError(texts.get(
                "error-masks-course-has-mask",
                Some(&args!("course" => printable(&course), "mask" => printable(&mask))),
            ))
            .into(),
            project::ProjectError::AssignmentNotEditable {
                course, mask, new, ..
            } => UsageError(texts.get(
                if new {
                    "error-masks-assignments-inline"
                } else {
                    "error-masks-assignment-inline"
                },
                Some(&args!("course" => printable(&course), "id" => printable(&mask))),
            ))
            .into(),
            err @ project::ProjectError::Write { .. } => anyhow::Error::new(err)
                .context(texts.get("error-write-file", Some(&args!("path" => path)))),
            // A file that no longer loads has been put back as it was.
            err => anyhow::Error::new(err)
                .context(texts.get("error-load-changed-mask", Some(&args!("path" => path)))),
        }
    })?;
    Ok(())
}

/// The closing lines after a mask is written: who prints with it, and `check`.
fn mask_written_lines(texts: &Texts, id: &str, assigned: Option<(&str, &str)>) {
    match assigned {
        Some((course, layout)) => eprintln!(
            "{}",
            texts.get(
                "masks-assigned",
                Some(&args!("course" => printable(course), "layout" => printable(layout))),
            )
        ),
        None => eprintln!(
            "{}",
            texts.get("masks-not-assigned", Some(&args!("id" => printable(id))))
        ),
    }
    eprintln!("{}", texts.plain("masks-run-check"));
}

/// `masks window`: appends a mask with circular windows around the controls of a course (ADR 0047).
fn masks_window(texts: &Texts, args: &WindowArgs) -> anyhow::Result<u8> {
    let usage = |id: &str, a: FluentArgs| UsageError(texts.get(id, Some(&a)));
    let loaded = load_project(texts, &args.project)?;
    let map = read_map(texts, &loaded.map_path())?;
    let Some(courses) = read_courses(texts, &loaded, &map)? else {
        return Err(UsageError(texts.plain("error-masks-no-course-file")).into());
    };
    let course = find_course(texts, &courses, &args.course)?;
    let selection = match &args.controls {
        Some(text) => Selection::parse(text).map_err(|e| build_error(texts, e, &course.name))?,
        None => Selection::All,
    };
    let mut per_control = std::collections::BTreeMap::new();
    for item in &args.radius_for {
        let parsed = item.rsplit_once('=').and_then(|(code, mm)| {
            let mm: f64 = mm.trim().parse().ok()?;
            let code = code.trim();
            (!code.is_empty()).then(|| (code.to_string(), mm))
        });
        let Some((code, mm)) = parsed else {
            return Err(usage("error-masks-radius-for", args!("text" => printable(item))).into());
        };
        per_control.insert(code, mm);
    }
    let layout = freemapper_print::courses::layout_for(&loaded.project, &course.name).0;
    let print_scale = layout.print_scale.unwrap_or(map.georef.scale_denominator);
    let options = WindowOptions {
        selection,
        include_start: args.include_start,
        include_finish: args.include_finish,
        radius_mm: args.radius.unwrap_or(mask_build::DEFAULT_RADIUS_MM),
        per_control,
        map_mm_per_print_mm: map_mm_per_print_mm(texts, &map, print_scale)?,
    };
    let id = new_mask_id(texts, &loaded, args.id.as_deref(), |taken| {
        mask_build::window_id(&course.name, taken)
    })?;
    if args.assign {
        check_assignable(texts, &loaded, &course.name)?;
    }
    let (mask, windows) = mask_build::windows(
        &id,
        course,
        &courses.event,
        &options,
        mask_build::map_extent(&map),
    )
    .map_err(|e| build_error(texts, e, &course.name))?;
    let codes: Vec<String> = windows.iter().map(|w| printable(&w.code)).collect();
    let radius = short_decimal(texts, options.radius_mm, 2);
    let summary = args!(
        "id" => printable(&id),
        "count" => windows.len(),
        "controls" => join_and(texts, codes),
        "course" => printable(&course.name),
        "radius" => radius,
        "scale" => print_scale.to_string(),
    );
    // The windows whose radius differs from the default, one line each.
    let own_radius: Vec<String> = windows
        .iter()
        .filter(|w| w.radius_mm != options.radius_mm)
        .map(|w| {
            texts.get(
                "masks-window-radius-for",
                Some(&args!(
                    "control" => printable(&w.code),
                    "radius" => short_decimal(texts, w.radius_mm, 2),
                )),
            )
        })
        .collect();
    let mut comment = texts.get("masks-window-comment", Some(&summary));
    for line in &own_radius {
        comment.push('\n');
        comment.push_str(line);
    }
    let assign = args
        .assign
        .then_some((course.name.as_str(), layout.id.as_str()));
    write_mask(texts, &loaded, &mask, assign, &comment)?;
    let mut summary = summary;
    summary.set("path", loaded.path.display().to_string());
    eprintln!("{}", texts.get("masks-window-written", Some(&summary)));
    for line in &own_radius {
        eprintln!("{line}");
    }
    use freemapper_print::course::model::ControlKind;
    // A start or finish of the course whose code got no window: check names it.
    let without_window = |kind: ControlKind| {
        course.stops.iter().any(|s| {
            let of_kind = courses
                .event
                .controls
                .iter()
                .find(|c| c.code == s.control)
                .is_some_and(|c| s.kind.unwrap_or(c.kind) == kind);
            of_kind && !windows.iter().any(|w| w.code == s.control)
        })
    };
    let (start, finish) = (
        without_window(ControlKind::Start),
        without_window(ControlKind::Finish),
    );
    let which = match (start, finish) {
        (true, true) => Some("both"),
        (true, false) => Some("start"),
        (false, true) => Some("finish"),
        (false, false) => None,
    };
    if let Some(which) = which {
        eprintln!(
            "{}",
            texts.get("masks-no-start-finish", Some(&args!("which" => which)))
        );
    }
    mask_written_lines(texts, &id, assign);
    Ok(0)
}

/// The course `name` of the course file, or a usage error that lists the courses.
fn find_course<'a>(
    texts: &Texts,
    courses: &'a LoadedCourses,
    name: &str,
) -> anyhow::Result<&'a freemapper_print::course::model::Course<freemapper_print::omap::MapCoord>> {
    courses
        .event
        .courses
        .iter()
        .find(|c| c.name == name)
        .ok_or_else(|| {
            let known: Vec<String> = courses
                .event
                .courses
                .iter()
                .map(|c| c.name.clone())
                .collect();
            UsageError(texts.get(
                "error-unknown-course",
                Some(&args!("name" => printable(name), "known" => quoted_list(texts, &known))),
            ))
            .into()
        })
}

/// `masks corridor`: appends a mask with a corridor along the polyline the user gives (ADR 0047).
fn masks_corridor(texts: &Texts, args: &CorridorArgs) -> anyhow::Result<u8> {
    let loaded = load_project(texts, &args.project)?;
    let map = read_map(texts, &loaded.map_path())?;
    let points = mask_build::parse_points(&args.points).map_err(|e| build_error(texts, e, ""))?;
    let map_scale = map.georef.scale_denominator;
    let scale_of = |l: &LayoutSpec| l.print_scale.unwrap_or(map_scale);
    // The width is print mm, so it needs the print scale: the course's layout, or the one all layouts share.
    let (course, layout) = match &args.course {
        Some(name) => {
            let Some(courses) = read_courses(texts, &loaded, &map)? else {
                return Err(UsageError(texts.plain("error-masks-no-course-file")).into());
            };
            let course = find_course(texts, &courses, name)?.name.clone();
            let layout = freemapper_print::courses::layout_for(&loaded.project, &course).0;
            (Some(course), layout)
        }
        None => {
            let mut scales: Vec<u32> = loaded.project.layouts.iter().map(scale_of).collect();
            scales.sort_unstable();
            scales.dedup();
            if scales.len() > 1 {
                let scales: Vec<String> = scales.iter().map(|s| format!("1:{s}")).collect();
                return Err(UsageError(texts.get(
                    "error-masks-scales",
                    Some(&args!("scales" => join_and(texts, scales))),
                ))
                .into());
            }
            (None, &loaded.project.layouts[0])
        }
    };
    let print_scale = scale_of(layout);
    let width = args.width.unwrap_or(mask_build::DEFAULT_WIDTH_MM);
    let k = map_mm_per_print_mm(texts, &map, print_scale)?;
    let id = new_mask_id(texts, &loaded, args.id.as_deref(), mask_build::corridor_id)?;
    if args.assign
        && let Some(course) = &course
    {
        check_assignable(texts, &loaded, course)?;
    }
    let polygon = mask_build::corridor(&points, width, k, mask_build::map_extent(&map))
        .map_err(|e| build_error(texts, e, course.as_deref().unwrap_or("")))?;
    let mask = freemapper_print::mask::Mask {
        id: id.clone(),
        polygons: vec![polygon],
    };
    let summary = args!(
        "id" => printable(&id),
        "count" => points.len(),
        "width" => short_decimal(texts, width, 2),
        "scale" => print_scale.to_string(),
    );
    // The points as read, so that the command can be run again from the comment.
    let written: Vec<String> = points.iter().map(|[x, y]| format!("{x},{y}")).collect();
    let comment = format!(
        "{}\n--points \"{}\"",
        texts.get("masks-corridor-comment", Some(&summary)),
        written.join(";")
    );
    let assigned = match (&course, args.assign) {
        (Some(c), true) => Some((c.as_str(), layout.id.as_str())),
        _ => None,
    };
    write_mask(texts, &loaded, &mask, assigned, &comment)?;
    let mut summary = summary;
    summary.set("path", loaded.path.display().to_string());
    eprintln!("{}", texts.get("masks-corridor-written", Some(&summary)));
    mask_written_lines(texts, &id, assigned);
    Ok(0)
}

/// What `check` prints for one layout, and whether the layout needs a look. A layout that cannot
/// be drawn gives its error line and counts as needing a look.
fn check_lines(
    texts: &Texts,
    loaded: &LoadedProject,
    map: &MapDocument,
    layout: &LayoutSpec,
    logos: &LogoCache,
) -> (Vec<String>, bool) {
    // The PDF is made in memory only: the font check needs it.
    match sheet_report(loaded, map, layout, logos) {
        Ok(report) => {
            let head = summary_line(
                texts,
                "check-summary",
                &[("layout", printable(&layout.id))],
                layout.paper,
                &report.print,
                map.georef.scale_denominator,
            );
            layout_lines(texts, head, layout, &report, loaded.project.map.kind)
        }
        Err(err) => {
            let message = printable(&format!("{:#}", export_error(texts, err)));
            (
                vec![texts.get("error-line", Some(&args!("message" => message)))],
                true,
            )
        }
    }
}

fn check(texts: &Texts, args: &CheckArgs) -> anyhow::Result<u8> {
    let loaded = load_project(texts, &args.project)?;
    let map = read_map(texts, &loaded.map_path())?;
    if let Some(courses) = read_courses(texts, &loaded, &map)? {
        return Ok(check_courses(texts, &loaded, &map, &courses));
    }
    let total = loaded.project.layouts.len();
    let mut with_notes = 0;
    // One logo cache for all layouts: each logo file is decoded once.
    let logos = LogoCache::new();
    // Every layout is checked, also after one that cannot be drawn.
    for layout in &loaded.project.layouts {
        let (lines, has_notes) = check_lines(texts, &loaded, &map, layout, &logos);
        for line in lines {
            eprintln!("{line}");
        }
        with_notes += usize::from(has_notes);
    }
    if with_notes == 0 {
        eprintln!(
            "{}",
            texts.get("check-clean", Some(&args!("count" => total)))
        );
        Ok(0)
    } else {
        eprintln!(
            "{}",
            texts.get(
                "check-with-notes",
                Some(&args!("count" => with_notes, "total" => total))
            )
        );
        Ok(1)
    }
}

/// The course file of the project, read and fitted to `map`; `None` when the project names none.
fn read_courses(
    texts: &Texts,
    loaded: &LoadedProject,
    map: &MapDocument,
) -> anyhow::Result<Option<LoadedCourses>> {
    load_courses(loaded, map).context(texts.plain("error-courses"))
}

/// Prints the notes of the course file as a whole and returns whether one of them is a warning.
fn print_course_file_notes(texts: &Texts, courses: &LoadedCourses) -> bool {
    let notes = course_event_notes(courses);
    for note in &notes {
        eprintln!("{}", sheet_note_line(texts, note));
    }
    notes.iter().any(|n| !n.is_info())
}

/// `check` with a course file: the notes of the file once, then every course on its layout, then every layout
/// sheet without a course, as `export --layout` writes it. Returns the exit code: 1 when the file, a course or
/// a layout sheet has notes that count.
fn check_courses(
    texts: &Texts,
    loaded: &LoadedProject,
    map: &MapDocument,
    courses: &LoadedCourses,
) -> u8 {
    let file_notes = print_course_file_notes(texts, courses);
    let logos = LogoCache::new();
    let total = courses.event.courses.len();
    let mut with_notes = 0;
    // Every course is checked, also after one that cannot be drawn.
    for course in &courses.event.courses {
        let (lines, has_notes) = match course_sheet_report(loaded, map, courses, course, &logos) {
            Ok((layout, report)) => {
                let head = summary_line(
                    texts,
                    "check-course-summary",
                    &[
                        ("course", printable(&course.name)),
                        ("layout", printable(&layout.id)),
                    ],
                    layout.paper,
                    &report.print,
                    map.georef.scale_denominator,
                );
                layout_lines(texts, head, layout, &report, loaded.project.map.kind)
            }
            Err(err) => {
                let message = printable(&format!("{:#}", export_error(texts, err)));
                (
                    vec![texts.get("error-line", Some(&args!("message" => message)))],
                    true,
                )
            }
        };
        for line in lines {
            eprintln!("{line}");
        }
        with_notes += usize::from(has_notes);
    }
    // The layout sheets without a course, as `export --layout` writes them (ruling R40).
    let layouts = loaded.project.layouts.len();
    let mut layouts_with_notes = 0;
    for layout in &loaded.project.layouts {
        let (lines, has_notes) = check_lines(texts, loaded, map, layout, &logos);
        for line in lines {
            eprintln!("{line}");
        }
        layouts_with_notes += usize::from(has_notes);
    }
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
        0
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
        1
    }
}

fn export(texts: &Texts, args: &ExportArgs) -> anyhow::Result<u8> {
    let loaded = load_project(texts, &args.project)?;
    let layouts = select_layouts(&loaded, &args.layout).map_err(|unknown| {
        let known: Vec<String> = unknown.known.iter().map(|id| printable(id)).collect();
        UsageError(texts.get(
            "error-unknown-layout",
            Some(&args!("id" => printable(&unknown.id), "known" => known.join(", "))),
        ))
    })?;
    let map = read_map(texts, &loaded.map_path())?;
    // `--layout` exports the layouts as such, without courses, also when the project has a course file.
    let courses = if args.layout.is_empty() {
        read_courses(texts, &loaded, &map)?
    } else {
        None
    };
    if courses.is_none() && (!args.course.is_empty() || args.single_file) {
        return Err(UsageError(texts.plain("error-no-course-file")).into());
    }
    let selected = match &courses {
        Some(courses) => Some(select_courses(courses, &args.course).map_err(|unknown| {
            UsageError(texts.get(
                "error-unknown-course",
                Some(&args!(
                    "name" => printable(&unknown.name),
                    "known" => quoted_list(texts, &unknown.known),
                )),
            ))
        })?),
        None => None,
    };
    let out_dir = out_dir(texts, args, &loaded)?;
    let map_scale = map.georef.scale_denominator;
    let kind = loaded.project.map.kind;
    match (&courses, selected) {
        (Some(courses), Some(selected)) => {
            print_course_file_notes(texts, courses);
            if selected.is_empty() {
                // Every course was left out by the reader: nothing to write, not even an empty PDF.
                eprintln!("{}", texts.plain("note-no-courses-to-export"));
                return Ok(0);
            }
            let print = |id: &str, out: String, done: &freemapper_print::export::ExportedCourse| {
                let head = summary_line(
                    texts,
                    id,
                    &[
                        ("out", out),
                        ("course", printable(&done.course.name)),
                        ("layout", printable(&done.layout.id)),
                        ("page", done.page.to_string()),
                    ],
                    done.layout.paper,
                    &done.report.print,
                    map_scale,
                );
                for line in layout_lines(texts, head, done.layout, &done.report, kind).0 {
                    eprintln!("{line}");
                }
            };
            if args.single_file {
                let mut count = 0;
                let path = export_courses_single_file(
                    &loaded,
                    &map,
                    courses,
                    &selected,
                    &out_dir,
                    |done| {
                        count += 1;
                        print("report-course-page", String::new(), &done);
                    },
                )
                .map_err(|err| export_error(texts, err))?;
                if let Some(path) = path {
                    eprintln!(
                        "{}",
                        texts.get(
                            "report-single-file",
                            Some(&args!("out" => path.display().to_string(), "count" => count))
                        )
                    );
                }
            } else {
                export_courses(&loaded, &map, courses, &selected, &out_dir, |done| {
                    print(
                        "report-course-summary",
                        done.path.display().to_string(),
                        &done,
                    );
                })
                .map_err(|err| export_error(texts, err))?;
            }
        }
        _ => {
            export_layouts(&loaded, &map, &layouts, &out_dir, |done| {
                let head = summary_line(
                    texts,
                    "report-summary",
                    &[("out", done.path.display().to_string())],
                    done.layout.paper,
                    &done.report.print,
                    map_scale,
                );
                for line in layout_lines(texts, head, done.layout, &done.report, kind).0 {
                    eprintln!("{line}");
                }
            })
            .map_err(|err| export_error(texts, err))?;
        }
    }
    eprintln!("{}", texts.plain("reminder-actual-size"));
    Ok(0)
}

/// The output folder: `--out-dir`, which must exist, else `out` next to the project file, created if missing.
/// A folder inside the project by name that a link leads out of the project stops the command before anything
/// is written (ADR 0041). Partial files older than an hour that interrupted runs left there are removed.
fn out_dir(texts: &Texts, args: &ExportArgs, loaded: &LoadedProject) -> anyhow::Result<PathBuf> {
    let dir = match &args.out_dir {
        Some(dir) => {
            if !dir.is_dir() {
                bail!(texts.get(
                    "error-out-dir-missing",
                    Some(&args!("path" => dir.display().to_string()))
                ));
            }
            output::check_out_folder(&loaded.dir, dir).map_err(|e| output_error(texts, e))?;
            dir.clone()
        }
        None => {
            let dir = loaded.dir.join("out");
            output::check_out_folder(&loaded.dir, &dir).map_err(|e| output_error(texts, e))?;
            // The library's writes create no folder (ADR 0041), so `out` is created here, once checked.
            std::fs::create_dir_all(&dir).with_context(|| {
                texts.get(
                    "error-create-folder",
                    Some(&args!("path" => dir.display().to_string())),
                )
            })?;
            dir
        }
    };
    output::remove_stale_parts(&loaded.dir, &dir);
    Ok(dir)
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
fn output_error(texts: &Texts, err: OutputError) -> anyhow::Error {
    match err {
        OutputError::DiskFull {
            needed,
            available,
            path,
        } => anyhow!(disk_full_message(texts, needed, available, &path)),
        OutputError::Io { path, source } if output::is_outside_project(&source) => {
            anyhow!(texts.get(
                "error-outside-project",
                Some(&args!("path" => path.display().to_string()))
            ))
        }
        OutputError::Io { path, source } => anyhow::Error::new(source).context(texts.get(
            "error-write-file",
            Some(&args!("path" => path.display().to_string())),
        )),
    }
}

/// An export error with a translated lead-in; a full disk is translated whole.
fn export_error(texts: &Texts, err: ExportError) -> anyhow::Error {
    let sheet =
        |layout: &str| texts.get("error-sheet", Some(&args!("layout" => printable(layout))));
    match err {
        ExportError::Sheet { layout, error } => anyhow::Error::new(error).context(sheet(&layout)),
        ExportError::Pdf { layout, error } => anyhow::Error::new(error).context(sheet(&layout)),
        ExportError::Write { error, .. } => output_error(texts, error),
        ExportError::CourseSheet { course, error } => anyhow::Error::new(error).context(texts.get(
            "error-course-sheet",
            Some(&args!("course" => printable(&course))),
        )),
        ExportError::CoursePdf { course, error } => anyhow::Error::new(error).context(texts.get(
            "error-course-sheet",
            Some(&args!("course" => printable(&course))),
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

/// Unicode format characters (category Cf): bidi controls, zero-width characters, joiners, tags.
///
/// A hand-written table of Unicode 16.0's category Cf, kept on purpose: `std` has no lookup of
/// general categories ([`char::is_control`] covers Cc only), and a dependency for this one table
/// is not worth it (ADR 0006). Re-check it when a Unicode version adds format characters.
fn is_format(c: char) -> bool {
    matches!(c as u32,
        0x00AD | 0x0600..=0x0605 | 0x061C | 0x06DD | 0x070F | 0x0890..=0x0891 | 0x08E2 | 0x180E
        | 0x200B..=0x200F | 0x202A..=0x202E | 0x2060..=0x2064 | 0x2066..=0x206F | 0xFEFF
        | 0xFFF9..=0xFFFB | 0x110BD | 0x110CD | 0x13430..=0x1343F | 0x1BCA0..=0x1BCA3
        | 0x1D173..=0x1D17A | 0xE0001 | 0xE0020..=0xE007F)
}

/// Map text for the terminal. Control characters get Rust's readable escapes (`\n`, `\t`,
/// `\u{1b}`) and format characters such as bidi overrides get `\u{..}`, so a map cannot drive
/// the terminal.
fn printable(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_control() {
            out.extend(c.escape_debug());
        } else if is_format(c) {
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
    report: &PrintReport,
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
        report,
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

/// The lines about one sheet: `head`, the notes of its print report, then its sheet notes. Also
/// whether any of them asks the user to look ([`SheetReport::needs_attention`]).
fn layout_lines(
    texts: &Texts,
    head: String,
    layout: &LayoutSpec,
    report: &SheetReport,
    kind: MapKind,
) -> (Vec<String>, bool) {
    let print = print_notes(
        texts,
        &report.print,
        layout.paper,
        layout.orientation != Orientation::Auto,
        kind,
        true,
    );
    let has_notes = report.needs_attention();
    let mut lines = vec![head];
    lines.extend(print);
    lines.extend(report.notes.iter().map(|n| sheet_note_line(texts, n)));
    (lines, has_notes)
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
                "controls" => printable(&controls.join(", ")),
                "count" => controls.len(),
            )),
        ),
        SheetNote::MaskCoversNothing { course, mask } => texts.get(
            "note-mask-covers-nothing",
            Some(&args!("course" => printable(course), "mask" => printable(mask))),
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
    let mut lines = Vec::new();
    if report.non_standard_scale {
        lines.push(texts.get(
            "warning-non-standard-scale",
            Some(&args!(
                "print_scale" => report.print_scale.to_string(),
                "kind" => texts.plain(kind_word_id(kind)),
                "base_scale" => kind.base_scale().to_string(),
            )),
        ));
    }
    if let Some((x, y)) = report.overflow_mm
        && (x > 0.0 || y > 0.0)
    {
        let cut = cut_text(texts, x, y);
        let mut hints = Vec::new();
        if paper == Paper::A4 {
            hints.push(texts.plain("hint-paper-a3"));
        }
        // Auto already took the orientation that cuts off less.
        if explicit_orientation {
            hints.push(texts.plain("hint-other-orientation"));
        }
        // The next larger allowed number shrinks the map on the page.
        match kind
            .allowed_scales()
            .into_iter()
            .find(|&s| s > report.print_scale)
        {
            Some(next) => {
                let id = if is_standard(kind, next) {
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
        if in_layout {
            warning.push(' ');
            warning.push_str(&texts.plain("hint-crop"));
        }
        lines.push(warning);
    }
    let counted = [
        ("note-skipped", report.skipped_objects),
        ("note-unknown-type", report.unknown_type_objects),
        ("note-truncated", report.truncated_objects),
        ("note-page-limit", report.objects_not_drawn_page_limit),
        ("note-dropped-pattern", report.dropped_pattern_objects),
    ];
    for (id, count) in counted {
        if count > 0 {
            lines.push(texts.get(id, Some(&args!("count" => count))));
        }
    }
    if !report.substituted_fonts.is_empty() {
        let names: Vec<String> = report
            .substituted_fonts
            .iter()
            .map(|f| printable(f))
            .collect();
        lines.push(texts.get("note-fonts", Some(&args!("names" => names.join(", ")))));
    }
    if !report.missing_glyphs.is_empty() {
        let chars: String = report
            .missing_glyphs
            .iter()
            .map(|c| printable(&c.to_string()))
            .collect::<Vec<_>>()
            .join(" ");
        lines.push(texts.get("note-missing-glyphs", Some(&args!("chars" => chars))));
    }
    lines
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
                        .mut_arg("include_start", |a| {
                            a.help(texts.plain("help-masks-include-start"))
                        })
                        .mut_arg("include_finish", |a| {
                            a.help(texts.plain("help-masks-include-finish"))
                        })
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
}

fn main() -> std::process::ExitCode {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let env = |name: &str| std::env::var(name).ok();
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
    };
    match result {
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
    }
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
                    controls: vec![s("S1"), s("31")],
                },
                vec!["Kurz", "S1, 31"],
            ),
            (
                SheetNote::ControlInMaskedArea {
                    course: s("Kurz"),
                    controls: vec![s("32")],
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
