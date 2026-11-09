//! The `freemapper-print` binary. Commands come with the milestones in the design
//! (`docs/superpowers/specs/2026-11-08-freemapper-print-design.md`).

mod text;

use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow, bail};
use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
use fluent_bundle::FluentArgs;
use freemapper_print::export::{ExportError, export_layouts, select_layouts, sheet_pdf};
use freemapper_print::omap::MapDocument;
use freemapper_print::omap::read::read_map_file;
use freemapper_print::output::OutputError;
use freemapper_print::print::{Orientation, Paper, PrintOptions, PrintReport, print_map};
use freemapper_print::project::{self, KeyComment, LayoutSpec, LoadedProject, SheetLang};
use freemapper_print::scale::{MapKind, infer_kind, is_standard, read_sidecar};
use freemapper_print::sheet::{
    FOOTER_ID, LogoCache, LogoSimplification, LogoSkipReason, MissingItem, SheetNote, SheetReport,
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
}

#[derive(Args)]
struct CheckArgs {
    project: PathBuf,
}

#[derive(Args)]
struct ExportArgs {
    project: PathBuf,
    #[arg(long = "layout", value_name = "ID")]
    layout: Vec<String>,
    #[arg(long, value_name = "DIR")]
    out_dir: Option<PathBuf>,
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
    let kind = detect_kind(texts, &map, &args.map, args.kind)?;
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
        SheetNote::ElementOverlapsMap { id } if id == FOOTER_ID => {
            texts.plain("note-render-footer-on-map")
        }
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
    project::load(&path).with_context(|| {
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

/// The map kind: `arg`, else `map_kind` in `freemapper.toml` next to the map, else inferred from
/// the symbol set and scale (ADR 0021). A `freemapper.toml` that cannot be read or parsed gets a
/// note that says so; without any kind the error asks for `--kind`.
fn detect_kind(
    texts: &Texts,
    map: &MapDocument,
    path: &Path,
    arg: Option<KindArg>,
) -> anyhow::Result<MapKind> {
    if let Some(kind) = arg {
        return Ok(kind.into());
    }
    if let Err(err) = read_sidecar(path) {
        eprintln!(
            "{}",
            texts.get(
                "note-sidecar",
                // The whole chain: the operating system's error is the source.
                Some(&args!("error" => printable(&format!("{:#}", anyhow::Error::new(err)))))
            )
        );
    }
    infer_kind(map, path).context(texts.plain("error-kind-unknown"))
}

fn init(texts: &Texts, args: &InitArgs) -> anyhow::Result<u8> {
    if let Some(title) = &args.title
        && title.chars().any(|c| c.is_control() || is_format(c))
    {
        return Err(UsageError(texts.plain("error-title-control")).into());
    }
    let map = read_map(texts, &args.map)?;
    let kind = detect_kind(texts, &map, &args.map, args.kind)?;
    let shown = args.folder.join(project::FILE_NAME);
    if shown.exists() {
        bail!(texts.get(
            "error-project-exists",
            Some(&args!("path" => shown.display().to_string()))
        ));
    }
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
    let starter = project::starter(
        &title,
        project::relative_path(&dir, &map_path),
        kind,
        map_scale,
        sheet_lang,
    );
    let (new, fits) = project::fit_starter(starter, &dir, &map);
    let kind_word = texts.plain(kind_word_id(kind));
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
        };
        let (lines, _) = check_lines(texts, &loaded, &map, layout, &LogoCache::new());
        for line in lines {
            eprintln!("{line}");
        }
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
    eprintln!("{}", texts.get("init-next", Some(&args!("path" => path))));
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
    match sheet_pdf(loaded, map, layout, logos) {
        Ok((_, report)) => {
            let head = summary_line(
                texts,
                "check-summary",
                ("layout", printable(&layout.id)),
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
    let out_dir = match &args.out_dir {
        Some(dir) => {
            if !dir.is_dir() {
                bail!(texts.get(
                    "error-out-dir-missing",
                    Some(&args!("path" => dir.display().to_string()))
                ));
            }
            dir.clone()
        }
        None => {
            let dir = loaded.dir.join("out");
            std::fs::create_dir_all(&dir).with_context(|| {
                texts.get(
                    "error-create-folder",
                    Some(&args!("path" => dir.display().to_string())),
                )
            })?;
            dir
        }
    };
    let map_scale = map.georef.scale_denominator;
    let kind = loaded.project.map.kind;
    export_layouts(&loaded, &map, &layouts, &out_dir, |done| {
        let head = summary_line(
            texts,
            "report-summary",
            ("out", done.path.display().to_string()),
            done.layout.paper,
            &done.report.print,
            map_scale,
        );
        for line in layout_lines(texts, head, done.layout, &done.report, kind).0 {
            eprintln!("{line}");
        }
    })
    .map_err(|err| export_error(texts, err))?;
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

/// A write error with a translated lead-in; a full disk is translated whole.
fn output_error(texts: &Texts, err: OutputError) -> anyhow::Error {
    match err {
        OutputError::DiskFull {
            needed,
            available,
            path,
        } => anyhow!(disk_full_message(texts, needed, available, &path)),
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

/// The first line about a printed page: message `id` with `key` (the PDF path or the layout id)
/// and the page size, scales and factors.
fn summary_line(
    texts: &Texts,
    id: &str,
    key: (&'static str, String),
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
    a.set(key.0, key.1);
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
        ("out", out.display().to_string()),
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
        SheetNote::ElementOutsidePage { id } if id == FOOTER_ID => {
            texts.plain("note-footer-outside-page")
        }
        SheetNote::ElementOutsidePage { id } => texts.get(
            "note-element-outside-page",
            Some(&args!("id" => printable(id))),
        ),
        SheetNote::ElementOverlapsMap { id } if id == FOOTER_ID => {
            texts.plain("note-footer-overlaps-map")
        }
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
        SheetNote::FooterCoversElement { id } => texts.get(
            "note-footer-covers-element",
            Some(&args!("id" => printable(id))),
        ),
        SheetNote::MissingScaleCheckBar { layout } => texts.get(
            "note-missing-scale-check-bar",
            Some(&args!("layout" => printable(layout))),
        ),
        SheetNote::CreditLineMoved { id } => texts.get(
            "note-credit-line-moved",
            Some(&args!("id" => printable(id))),
        ),
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

/// The notes of a print report, without the summary and the reminder. `explicit_orientation`:
/// the orientation was chosen, so trying the other one is worth a hint.
fn print_notes(
    texts: &Texts,
    report: &PrintReport,
    paper: Paper,
    explicit_orientation: bool,
    kind: MapKind,
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
        let mut cut = Vec::new();
        if x > 0.0 {
            cut.push(texts.get("overflow-too-wide", Some(&args!("mm" => mm(texts, x)))));
        }
        if y > 0.0 {
            cut.push(texts.get("overflow-too-tall", Some(&args!("mm" => mm(texts, y)))));
        }
        let cut = if cut.len() == 2 {
            texts.get(
                "overflow-and",
                Some(&args!("first" => cut[0].clone(), "second" => cut[1].clone())),
            )
        } else {
            cut.remove(0)
        };
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
        lines.push(if hints.is_empty() {
            texts.get(
                "warning-overflow-single",
                Some(&args!("cut" => cut, "last" => last)),
            )
        } else {
            texts.get(
                "warning-overflow",
                Some(&args!("cut" => cut, "hints" => hints.join(", "), "last" => last)),
            )
        });
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
                .mut_arg("out_dir", |a| a.help(texts.plain("help-out-dir")))
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
        let notes: Vec<(SheetNote, Vec<&str>)> = vec![
            (
                SheetNote::ElementOutsidePage { id: s("logo1") },
                vec!["logo1"],
            ),
            (SheetNote::ElementOutsidePage { id: s(FOOTER_ID) }, vec![]),
            (
                SheetNote::ElementOverlapsMap { id: s("logo1") },
                vec!["logo1"],
            ),
            (
                SheetNote::ElementOverlapsMap { id: s(FOOTER_ID) },
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
                SheetNote::FooterCoversElement { id: s("bar1") },
                vec!["bar1"],
            ),
            (
                SheetNote::MissingScaleCheckBar { layout: s("L1") },
                vec!["L1", "scale_check_bar"],
            ),
        ];
        for lang in text::available() {
            let texts = Texts::new(&lang);
            let mut seen = std::collections::HashSet::new();
            for (note, fields) in &notes {
                let line = sheet_note_line(&texts, note);
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
