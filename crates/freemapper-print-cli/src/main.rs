//! The `freemapper-print` binary. Commands come with the milestones in the design
//! (`docs/superpowers/specs/2026-11-08-freemapper-print-design.md`).

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
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
    /// Render a map to a PDF at an exact print scale.
    #[command(after_long_help = "\
The PDF has 1 mm on paper for the exact map distance at the print scale. Print it at actual size.

Exit codes: 0 on success, 1 when the map cannot be read or printed, 2 when the command line is wrong.")]
    Render(RenderArgs),
}

#[derive(Args)]
struct RenderArgs {
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

fn render(args: &RenderArgs) -> anyhow::Result<()> {
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
    if same_file(&args.map, &args.out) {
        bail!(
            "--out points at the map file {}; choose a PDF path so the map stays safe",
            args.map.display()
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
    for line in report_lines(&report, args.paper, map.georef.scale_denominator, &args.out) {
        eprintln!("{line}");
    }
    Ok(())
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

/// Map text for the terminal: control characters (such as ESC) become visible escapes, so a map
/// cannot drive the terminal.
fn printable(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_control() {
            out.extend(c.escape_debug());
        } else {
            out.push(c);
        }
    }
    out
}

/// A millimetre value with one decimal, never below 0.1 for a positive value.
fn mm(value: f64) -> String {
    format!("{:.1}", value.max(0.1))
}

/// "1 object is" or "3 objects are" (with the verbs the caller gives).
fn objects(count: usize, is: &str, are: &str) -> String {
    if count == 1 {
        format!("1 object {is}")
    } else {
        format!("{count} objects {are}")
    }
}

/// The messages for a finished print, in the order they appear on stderr.
fn report_lines(report: &PrintReport, paper: PaperArg, map_scale: u32, out: &Path) -> Vec<String> {
    let paper_name = paper
        .to_possible_value()
        .map_or_else(String::new, |v| v.get_name().to_uppercase());
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
        let mut cut = Vec::new();
        if x > 0.0 {
            cut.push(format!("{} mm too wide", mm(x)));
        }
        if y > 0.0 {
            cut.push(format!("{} mm too tall", mm(y)));
        }
        let a3 = if matches!(paper, PaperArg::A4) {
            "A3 paper, "
        } else {
            ""
        };
        lines.push(format!(
            "Warning: the map does not fit on the page. It is {} for the printable area. \
             Try {a3}the other orientation, or a print scale with a larger number, for example 1:5000.",
            cut.join(" and ")
        ));
    }
    if report.skipped_objects > 0 {
        lines.push(format!(
            "Note: {} not drawn because the symbol is unknown or hidden.",
            objects(report.skipped_objects, "is", "are")
        ));
    }
    if report.truncated_objects > 0 {
        lines.push(format!(
            "Note: {} simplified or cut short because the file asks for more detail than the renderer draws.",
            objects(report.truncated_objects, "was", "were")
        ));
    }
    if !report.substituted_fonts.is_empty() {
        let names: Vec<String> = report
            .substituted_fonts
            .iter()
            .map(|f| printable(f))
            .collect();
        lines.push(format!(
            "Note: the font Arimo replaces {}.",
            names.join(", ")
        ));
    }
    if !report.missing_glyphs.is_empty() {
        let chars: String = report
            .missing_glyphs
            .iter()
            .map(|c| printable(&c.to_string()))
            .collect::<Vec<_>>()
            .join(" ");
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
    match Cli::parse().command {
        Commands::Render(args) => render(&args),
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
            overflow_mm: None,
            skipped_objects: 0,
            truncated_objects: 0,
            substituted_fonts: vec![],
            missing_glyphs: vec![],
        }
    }

    fn lines(r: &PrintReport, paper: PaperArg) -> Vec<String> {
        report_lines(r, paper, 4000, Path::new("o.pdf"))
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
            missing_glyphs: vec!['\u{1b}', '\u{7}'],
            ..report()
        };
        let all = lines(&r, PaperArg::A4).join("\n");
        assert!(!all.contains('\u{1b}') && !all.contains('\u{7}'), "{all:?}");
        assert!(all.contains("Evil\\u{1b}[2J"), "{all:?}");
    }
}
