//! Notes as typed values. [`Notice`] holds the note of the library as it came, with the course it is about;
//! `Notice::describe` (in `main.rs`, next to the formatting functions) turns it into a [`Note`] with the message
//! the CLI prints. Serialised on its own, a notice gives its kind, severity and course.

use freemapper_print::course::{CourseNote, FitNote};
use freemapper_print::courses::{CorrectionScope, MaskNote, Stale};
use freemapper_print::export::ExportError;
use freemapper_print::print::{Paper, PrintReport};
use freemapper_print::scale::MapKind;
use freemapper_print::sheet::SheetNote;
use serde::Serialize;
use serde::ser::SerializeStruct;

/// How much a note asks of the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Informs only; does not count toward `check`'s exit code.
    Info,
    /// Asks the user to look; `check` fails on it.
    Warning,
    /// The sheet could not be drawn.
    Error,
}

/// A note in words: what the MCP server returns and what the CLI prints (`message`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Note {
    /// A stable snake_case name of the kind of note, such as `number_conflict`.
    pub kind: &'static str,
    pub severity: Severity,
    /// The course the note is about, if any.
    pub course: Option<String>,
    /// The message in the program language, as the CLI prints it.
    pub message: String,
}

/// A note of the library with the course it is about.
#[derive(Debug)]
pub struct Notice {
    /// The course of the sheet, else the course the note names, else `None`.
    pub course: Option<String>,
    pub value: NoteValue,
}

/// The kinds of notes a sheet gets.
#[derive(Debug)]
pub enum NoteValue {
    /// A note about a sheet or the course file.
    Sheet(SheetNote),
    /// A note of the print report: the map on the page.
    Print(PrintNote),
    /// The sheet could not be drawn.
    SheetFailed(ExportError),
}

impl Notice {
    /// A note of the print report of the sheet of `course`.
    pub fn print(course: Option<&str>, note: PrintNote) -> Self {
        Notice {
            course: course.map(str::to_string),
            value: NoteValue::Print(note),
        }
    }

    /// A sheet note; on a sheet without a course, the course the note itself names.
    pub fn sheet(course: Option<&str>, note: SheetNote) -> Self {
        let course = course
            .or_else(|| sheet_note_kind(&note).1)
            .map(str::to_string);
        Notice {
            course,
            value: NoteValue::Sheet(note),
        }
    }

    /// The sheet of `course` (or a layout sheet) could not be drawn.
    pub fn failed(course: Option<&str>, error: ExportError) -> Self {
        Notice {
            course: course.map(str::to_string),
            value: NoteValue::SheetFailed(error),
        }
    }

    /// A stable snake_case name of the kind of note.
    pub fn kind(&self) -> &'static str {
        match &self.value {
            NoteValue::Sheet(note) => sheet_note_kind(note).0,
            NoteValue::Print(note) => note.kind(),
            NoteValue::SheetFailed(_) => "sheet_failed",
        }
    }

    pub fn severity(&self) -> Severity {
        match &self.value {
            NoteValue::Sheet(note) if note.is_info() => Severity::Info,
            NoteValue::Sheet(_) | NoteValue::Print(_) => Severity::Warning,
            NoteValue::SheetFailed(_) => Severity::Error,
        }
    }
}

impl Serialize for Notice {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("Notice", 3)?;
        s.serialize_field("kind", self.kind())?;
        s.serialize_field("severity", &self.severity())?;
        s.serialize_field("course", &self.course)?;
        s.end()
    }
}

/// A note of the print report, with what its text needs to know about the page.
#[derive(Debug, Clone, PartialEq)]
pub enum PrintNote {
    /// The standard does not foresee the print scale for the map kind.
    NonStandardScale { print_scale: u32, kind: MapKind },
    /// The map reaches `x_mm` and `y_mm` past the frame. The rest says which hints the text gives.
    Overflow {
        x_mm: f64,
        y_mm: f64,
        paper: Paper,
        /// The orientation was chosen, so trying the other one is worth a hint.
        explicit_orientation: bool,
        kind: MapKind,
        print_scale: u32,
        /// The page is a layout's sheet, so the text also points at `centre_map_mm` and `crop`.
        in_layout: bool,
    },
    /// Objects not drawn because their symbol is unknown or hidden.
    Skipped(usize),
    /// Objects of a type the reader does not know.
    UnknownType(usize),
    /// Objects whose drawing a limit cut short.
    Truncated(usize),
    /// Objects not drawn because the page budget was spent.
    PageLimit(usize),
    /// Objects whose symbol lost an area pattern when read.
    DroppedPattern(usize),
    /// Font families that Arimo stands in for.
    SubstitutedFonts(Vec<String>),
    /// Characters the embedded font has no glyph for.
    MissingGlyphs(Vec<char>),
}

impl PrintNote {
    fn kind(&self) -> &'static str {
        match self {
            PrintNote::NonStandardScale { .. } => "non_standard_scale",
            PrintNote::Overflow { .. } => "overflow",
            PrintNote::Skipped(_) => "skipped_objects",
            PrintNote::UnknownType(_) => "unknown_type_objects",
            PrintNote::Truncated(_) => "truncated_objects",
            PrintNote::PageLimit(_) => "page_limit",
            PrintNote::DroppedPattern(_) => "dropped_pattern_objects",
            PrintNote::SubstitutedFonts(_) => "substituted_fonts",
            PrintNote::MissingGlyphs(_) => "missing_glyphs",
        }
    }
}

/// The notes of a print report, in the order the CLI prints them. `explicit_orientation`: the orientation was
/// chosen. `in_layout`: the page is a layout's sheet.
pub fn print_notes(
    report: &PrintReport,
    paper: Paper,
    explicit_orientation: bool,
    kind: MapKind,
    in_layout: bool,
) -> Vec<PrintNote> {
    let mut notes = Vec::new();
    if report.non_standard_scale {
        notes.push(PrintNote::NonStandardScale {
            print_scale: report.print_scale,
            kind,
        });
    }
    if let Some((x, y)) = report.overflow_mm
        && (x > 0.0 || y > 0.0)
    {
        notes.push(PrintNote::Overflow {
            x_mm: x,
            y_mm: y,
            paper,
            explicit_orientation,
            kind,
            print_scale: report.print_scale,
            in_layout,
        });
    }
    let counted = [
        (
            PrintNote::Skipped as fn(usize) -> PrintNote,
            report.skipped_objects,
        ),
        (PrintNote::UnknownType, report.unknown_type_objects),
        (PrintNote::Truncated, report.truncated_objects),
        (PrintNote::PageLimit, report.objects_not_drawn_page_limit),
        (PrintNote::DroppedPattern, report.dropped_pattern_objects),
    ];
    for (note, count) in counted {
        if count > 0 {
            notes.push(note(count));
        }
    }
    if !report.substituted_fonts.is_empty() {
        notes.push(PrintNote::SubstitutedFonts(
            report.substituted_fonts.clone(),
        ));
    }
    if !report.missing_glyphs.is_empty() {
        notes.push(PrintNote::MissingGlyphs(report.missing_glyphs.clone()));
    }
    notes
}

/// The kind name of a sheet note and the course it names, if any. No wildcard arm: a new kind of note does not
/// compile until it has a name.
fn sheet_note_kind(note: &SheetNote) -> (&'static str, Option<&str>) {
    use SheetNote as N;
    match note {
        N::ElementOutsidePage { .. } => ("element_outside_page", None),
        N::ElementOverlapsMap { .. } => ("element_overlaps_map", None),
        N::LargerThanA3 => ("larger_than_a3", None),
        N::MissingScaleOrContour { .. } => ("missing_scale_or_contour", None),
        N::FooterOverlapsMap { .. } => ("footer_overlaps_map", None),
        N::FooterOutsidePage { .. } => ("footer_outside_page", None),
        N::FooterCoversElement { .. } => ("footer_covers_element", None),
        N::MissingScaleCheckBar { .. } => ("missing_scale_check_bar", None),
        N::MapCropped { .. } => ("map_cropped", None),
        N::MapCroppedToCourse { .. } => ("map_cropped_to_course", None),
        N::NorthLinesAdded { .. } => ("north_lines_added", None),
        N::NorthLinesWithoutSymbol { .. } => ("north_lines_without_symbol", None),
        N::CopyrightTruncated { .. } => ("copyright_truncated", None),
        N::CopyrightLineCut { .. } => ("copyright_line_cut", None),
        N::LogoSkipped { .. } => ("logo_skipped", None),
        N::LogoSimplified { .. } => ("logo_simplified", None),
        N::UnknownPlaceholder { .. } => ("unknown_placeholder", None),
        N::UnknownPart { .. } => ("unknown_part", None),
        N::CreditLineMoved { .. } => ("credit_line_moved", None),
        N::CourseOutsideFrame { course, .. } => ("course_outside_frame", Some(course)),
        N::DefaultLayout { course, .. } => ("default_layout", Some(course)),
        N::ElementCoversCourse { .. } => ("element_covers_course", None),
        N::NoDescriptionBox { course, .. } => ("no_description_box", Some(course)),
        N::UnknownDescriptionSymbol { course, .. } => ("unknown_description_symbol", Some(course)),
        N::OverprintCut { course } => ("overprint_cut", Some(course)),
        N::MasksCapped { course, .. } => ("masks_capped", Some(course)),
        N::OverprintInMaskedArea { course, .. } => ("overprint_in_masked_area", Some(course)),
        N::ControlInMaskedArea { course, .. } => ("control_in_masked_area", Some(course)),
        N::MaskCoversNothing { course, .. } => ("mask_covers_nothing", Some(course)),
        N::WhiteOutsCoverNothing { course, .. } => ("white_outs_cover_nothing", Some(course)),
        N::ElementCoversWindow { course, .. } => ("element_covers_window", Some(course)),
        N::MaskLeavesNoWindow { course, .. } => ("mask_leaves_no_window", Some(course)),
        N::MaskCheckLimited { course } => ("mask_check_limited", Some(course)),
        N::CourseValueUnknown { course, .. } => ("course_value_unknown", Some(course)),
        N::CoursePlaceholderNotFilled { .. } => ("course_placeholder_not_filled", None),
        N::NumberConflict(c) => ("number_conflict", Some(&c.course)),
        N::NumberGapOutOfRange { course, .. } => ("number_gap_out_of_range", Some(course)),
        N::NumberSearchLimited { course } => ("number_search_limited", Some(course)),
        N::Course(note) => course_note_kind(note),
        N::Fit(note) => (fit_note_kind(note), None),
        N::Stale(stale) => stale_kind(stale),
        N::Mask(note) => mask_note_kind(note),
    }
}

fn course_note_kind(note: &CourseNote) -> (&'static str, Option<&str>) {
    match note {
        CourseNote::UnknownElement(_) => ("course_unknown_element", None),
        CourseNote::Forked { .. } => ("course_forked", None),
        CourseNote::AlternativeControls(course) => ("course_alternative_controls", Some(course)),
        CourseNote::NotesTruncated => ("course_notes_truncated", None),
        CourseNote::WhiteOutLaterInM4 => ("course_white_out", None),
        CourseNote::NoDescriptions => ("course_no_descriptions", None),
        CourseNote::MapIssueAsStart(_) => ("course_map_issue_as_start", None),
        CourseNote::Variations(course) => ("course_variations", Some(course)),
    }
}

fn fit_note_kind(note: &FitNote) -> &'static str {
    match note {
        FitNote::YUpDetected => "fit_y_up_detected",
        FitNote::OrientationAssumed => "fit_orientation_assumed",
        FitNote::AxisAgainstFormat { .. } => "fit_axis_against_format",
        FitNote::ScaleUnchecked => "fit_scale_unchecked",
        FitNote::GeorefChecked { .. } => "fit_georef_checked",
        FitNote::GeorefSkipped(_) => "fit_georef_skipped",
        FitNote::GeorefCheckOff => "fit_georef_check_off",
        FitNote::PlacedByGeoref(_) => "fit_placed_by_georef",
    }
}

fn stale_kind(stale: &Stale) -> (&'static str, Option<&str>) {
    match stale {
        Stale::Assignment { course } => ("stale_assignment", Some(course)),
        Stale::Correction { scope, .. } => (
            "stale_correction",
            match scope {
                CorrectionScope::Course(course) => Some(course),
                CorrectionScope::Layout(_) => None,
            },
        ),
        Stale::CorrectionOverridesChangedBase { .. } => ("stale_changed_base", None),
        Stale::Mask { course, .. } => ("stale_mask", Some(course)),
    }
}

fn mask_note_kind(note: &MaskNote) -> (&'static str, Option<&str>) {
    match note {
        MaskNote::MaskUnused { .. } => ("mask_unused", None),
        MaskNote::WhiteOutByNumber { course, .. } => ("white_out_by_number", Some(course)),
    }
}
