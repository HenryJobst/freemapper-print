//! The `freemapper-print` binary. Commands come with the milestones in the design
//! (`docs/superpowers/specs/2026-11-08-freemapper-print-design.md`).

mod text;

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
use fluent_bundle::FluentArgs;
use freemapper_print::omap::read::read_map_file;
use freemapper_print::print::{Orientation, Paper, PrintOptions, PrintReport, print_map};
use freemapper_print::scale::{MapKind, infer_kind, is_standard, read_sidecar};

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

/// Message arguments: `args!("name" => value, ...)`.
macro_rules! args {
    ($($name:literal => $value:expr),* $(,)?) => {{
        let mut a = FluentArgs::new();
        $(a.set($name, $value);)*
        a
    }};
}

fn render(texts: &Texts, args: &RenderArgs) -> anyhow::Result<()> {
    let map = read_map_file(&args.map).with_context(|| {
        texts.get(
            "error-read-map",
            Some(&args!("path" => args.map.display().to_string())),
        )
    })?;
    let kind = match args.kind {
        Some(kind) => kind.into(),
        None => {
            if let Err(err) = read_sidecar(&args.map) {
                eprintln!(
                    "{}",
                    texts.get(
                        "note-sidecar",
                        Some(&args!("error" => printable(&err.to_string())))
                    )
                );
            }
            infer_kind(&map, &args.map).context(texts.plain("error-kind-unknown"))?
        }
    };
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
    };
    let title = args
        .map
        .file_stem()
        .map_or_else(|| "map".into(), |s| s.to_string_lossy().into_owned());
    // Library errors keep their English text for now (ADR 0024); the lead-in is translated.
    let (pdf, report) =
        print_map(&map, &options, &title).with_context(|| texts.plain("error-print"))?;
    freemapper_print::output::write_pdf_file(&args.out, &pdf)
        .with_context(|| texts.plain("error-write"))?;
    for line in report_lines(
        texts,
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
fn mm(texts: &Texts, value: f64) -> String {
    texts.decimal(&format!("{:.1}", value.max(0.1)))
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
    let paper_name = paper
        .to_possible_value()
        .map_or_else(String::new, |v| v.get_name().to_uppercase());
    let (w, h) = report.page_mm;
    let orientation = texts.plain(if w > h {
        "word-landscape"
    } else {
        "word-portrait"
    });
    let mut lines = vec![texts.get(
        "report-summary",
        Some(&args!(
            "out" => out.display().to_string(),
            "paper" => paper_name,
            "orientation" => orientation,
            "print_scale" => report.print_scale.to_string(),
            "map_scale" => map_scale.to_string(),
            "geometry" => texts.decimal(&format!("{:.3}", report.geometry_factor)),
            "symbol" => texts.decimal(&format!("{:.3}", report.symbol_factor)),
        )),
    )];
    if report.non_standard_scale {
        let kind_word = match kind {
            MapKind::Sprint => "word-kind-sprint",
            MapKind::Schoolyard => "word-kind-schoolyard",
            MapKind::Forest => "word-kind-forest",
        };
        lines.push(texts.get(
            "warning-non-standard-scale",
            Some(&args!(
                "print_scale" => report.print_scale.to_string(),
                "kind" => texts.plain(kind_word),
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
        if matches!(paper, PaperArg::A4) {
            hints.push(texts.plain("hint-paper-a3"));
        }
        // Auto already took the orientation that cuts off less.
        if orientation_arg != OrientationArg::Auto {
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
    lines.push(texts.plain("reminder-actual-size"));
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
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            // Errors can quote map content, so they get the same escaping as the notes.
            let message = printable(&format!("{err:#}"));
            eprintln!(
                "{}",
                texts.get("error-line", Some(&args!("message" => message)))
            );
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
