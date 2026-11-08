//! The `freemapper-print` binary. Commands come with the milestones in the design
//! (`docs/superpowers/specs/2026-11-08-freemapper-print-design.md`).

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use clap::{Parser, Subcommand, ValueEnum};
use freemapper_print::omap::read::read_map_file;
use freemapper_print::print::{Orientation, Paper, PrintOptions, PrintReport, print_map};
use freemapper_print::scale::{MapKind, infer_kind};

#[derive(Parser)]
#[command(
    name = "freemapper-print",
    version,
    about = "Prints finished orienteering maps as exact-scale PDFs",
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Render a map to a PDF in which 1 mm on paper is the exact map distance.
    Render {
        /// The OOM map file (.omap).
        map: PathBuf,
        /// PDF file to write. An existing file is overwritten; its folder must exist.
        #[arg(long)]
        out: PathBuf,
        /// Print scale denominator, e.g. 4000 for 1:4000. The default is the scale of the map.
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
        scale: Option<u32>,
        /// Paper size.
        #[arg(long, value_enum, default_value_t = PaperArg::A4)]
        paper: PaperArg,
        /// Page orientation. Auto picks the one that cuts off less of the map.
        #[arg(long, value_enum, default_value_t = OrientationArg::Auto)]
        orientation: OrientationArg,
        /// Map kind. Needed only when the program cannot tell it from freemapper.toml or the map file.
        #[arg(long, value_enum)]
        kind: Option<KindArg>,
        /// Also draw the north lines and the information block of the map. They scale with the
        /// map at another print scale, so a printed scale label in them can be wrong.
        #[arg(long)]
        show_sheet_objects: bool,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum PaperArg {
    A4,
    A3,
}

#[derive(Clone, Copy, ValueEnum)]
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

struct RenderArgs {
    map: PathBuf,
    out: PathBuf,
    scale: Option<u32>,
    paper: PaperArg,
    orientation: OrientationArg,
    kind: Option<KindArg>,
    show_sheet_objects: bool,
}

fn render(args: RenderArgs) -> anyhow::Result<()> {
    let map = read_map_file(&args.map)
        .with_context(|| format!("could not read the map {}", args.map.display()))?;
    let kind = match args.kind {
        Some(kind) => kind.into(),
        None => infer_kind(&map, &args.map).context(
            "cannot tell the map kind from the file; pass --kind sprint, --kind schoolyard or --kind forest",
        )?,
    };
    let parent = match args.out.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    if !parent.is_dir() {
        bail!(
            "the folder {} for the PDF does not exist; create it first",
            parent.display()
        );
    }
    let options = PrintOptions {
        print_scale: args.scale,
        kind,
        paper: args.paper.into(),
        orientation: args.orientation.into(),
        centre: None,
        show_sheet_objects: args.show_sheet_objects,
    };
    let title = args
        .map
        .file_stem()
        .map_or_else(|| "map".into(), |s| s.to_string_lossy().into_owned());
    let (pdf, report) = print_map(&map, &options, &title)?;
    std::fs::write(&args.out, pdf)
        .with_context(|| format!("could not write {}", args.out.display()))?;
    for line in report_lines(
        &report,
        args.paper,
        map.georef.scale_denominator,
        args.out.as_path(),
    ) {
        eprintln!("{line}");
    }
    Ok(())
}

/// The messages for a finished print, in the order they appear on stderr.
fn report_lines(report: &PrintReport, paper: PaperArg, map_scale: u32, out: &Path) -> Vec<String> {
    let paper_name = match paper {
        PaperArg::A4 => "A4",
        PaperArg::A3 => "A3",
    };
    let (w, h) = report.page_mm;
    let orientation = if w > h { "landscape" } else { "portrait" };
    let mut lines = vec![format!(
        "Wrote {}: {paper_name} {orientation}, print scale 1:{}, map scale 1:{map_scale}, \
         geometry factor {:.3}, symbol factor {:.3}.",
        out.display(),
        report.print_scale,
        report.geometry_factor,
        report.symbol_factor,
    )];
    if let Some((x, y)) = report.overflow_mm {
        lines.push(format!(
            "Warning: the map does not fit on the page. {x:.1} mm across and {y:.1} mm down are cut off. \
             Try A3, the other orientation, or a smaller print scale."
        ));
    }
    if report.skipped_objects > 0 {
        lines.push(format!(
            "Note: {} object(s) were not drawn because their symbol is unknown or hidden.",
            report.skipped_objects
        ));
    }
    if report.truncated_objects > 0 {
        lines.push(format!(
            "Note: {} object(s) were simplified or cut short because the file asks for more detail than the renderer draws.",
            report.truncated_objects
        ));
    }
    if !report.substituted_fonts.is_empty() {
        lines.push(format!(
            "Note: the font Arimo replaces {}.",
            report.substituted_fonts.join(", ")
        ));
    }
    if !report.missing_glyphs.is_empty() {
        let chars: String = report.missing_glyphs.iter().collect();
        lines.push(format!(
            "Note: the embedded font has no glyph for these characters, so they are missing in the PDF: {chars}"
        ));
    }
    lines.push(
        "Print at actual size: 100 %, with \"fit to page\" switched off. Any other size breaks the scale."
            .into(),
    );
    lines
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Render {
            map,
            out,
            scale,
            paper,
            orientation,
            kind,
            show_sheet_objects,
        } => render(RenderArgs {
            map,
            out,
            scale,
            paper,
            orientation,
            kind,
            show_sheet_objects,
        })?,
    }
    Ok(())
}
