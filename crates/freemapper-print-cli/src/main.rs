//! The `freemapper-print` binary. Commands come with the milestones in the design
//! (`docs/superpowers/specs/2026-11-08-freemapper-print-design.md`).

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use freemapper_print::omap::read::read_map_file;
use freemapper_print::print::{Orientation, Paper, PrintOptions, PrintReport, print_map};
use freemapper_print::scale::{MapKind, infer_kind, is_standard};

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
    /// Also draw freemapper's information block: title, scale, scale bar and credits. It scales
    /// with the map at another print scale, so its scale text can be wrong then.
    #[arg(long)]
    show_sheet_objects: bool,
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
    for line in report_lines(
        &report,
        args.paper,
        args.orientation,
        kind,
        map.georef.scale_denominator,
        &args.out,
    ) {
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
fn report_lines(
    report: &PrintReport,
    paper: PaperArg,
    orientation_arg: OrientationArg,
    kind: MapKind,
    map_scale: u32,
    out: &Path,
) -> Vec<String> {
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
    if report.non_standard_scale {
        lines.push(format!(
            "Warning: 1:{} is not a standard scale for {} maps. The map is reduced and the symbols \
             keep their 1:{} size. Use this print for training only.",
            report.print_scale,
            kind.name(),
            kind.base_scale(),
        ));
    }
    if let Some((x, y)) = report.overflow_mm
        && (x > 0.0 || y > 0.0)
    {
        let mut cut = Vec::new();
        if x > 0.0 {
            cut.push(format!("{} mm too wide", mm(x)));
        }
        if y > 0.0 {
            cut.push(format!("{} mm too tall", mm(y)));
        }
        let mut hints = Vec::new();
        if matches!(paper, PaperArg::A4) {
            hints.push("A3 paper".to_string());
        }
        // Auto already took the orientation that cuts off less.
        if orientation_arg != OrientationArg::Auto {
            hints.push("the other orientation".into());
        }
        // The next larger allowed number shrinks the map on the page.
        match kind
            .allowed_scales()
            .into_iter()
            .find(|&s| s > report.print_scale)
        {
            Some(next) if is_standard(kind, next) => hints.push(format!(
                "a print scale with a larger number, for example 1:{next}"
            )),
            Some(next) => hints.push(format!(
                "a print scale with a larger number, for example 1:{next} (not standard, for \
                 training only)"
            )),
            None => hints.push("a print scale with a larger number".into()),
        }
        let last = hints.pop().unwrap_or_default();
        lines.push(format!(
            "Warning: the map does not fit on the page. It is {} for the printable area. Try {}, or {last}.",
            cut.join(" and "),
            hints.join(", ")
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

fn main() -> std::process::ExitCode {
    let result = match Cli::parse().command {
        Commands::Render(args) => render(&args),
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            // Errors can quote map content, so they get the same escaping as the notes.
            eprintln!("Error: {}", printable(&format!("{err:#}")));
            std::process::ExitCode::FAILURE
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
            truncated_objects: 0,
            substituted_fonts: vec![],
            missing_glyphs: vec![],
        }
    }

    fn lines(r: &PrintReport, paper: PaperArg) -> Vec<String> {
        report_lines(
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
