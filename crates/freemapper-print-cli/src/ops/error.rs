//! Why an op stopped, as values. The caller words them (`op_error` in `main.rs`); the variants carry what the
//! message names. Where the cause is an error of the library or the operating system, it stays the `source`, so
//! that a chain of messages names it once.

use std::path::PathBuf;

use freemapper_print::export::ExportError;
use freemapper_print::output::OutputError;

/// Why an op stopped. Nothing was written unless the variant says otherwise.
#[derive(Debug)]
pub enum OpError {
    /// The project file could not be read or checked.
    LoadProject {
        path: PathBuf,
        source: anyhow::Error,
    },
    /// A layout hides the base part of a sprint map (ADR 0044).
    BasePartHidden {
        path: PathBuf,
        file: String,
        field: String,
    },
    /// The map file could not be read.
    ReadMap {
        path: PathBuf,
        source: anyhow::Error,
    },
    /// The course file could not be read or fitted to the map.
    Courses(anyhow::Error),
    /// A layout id the project does not have (usage).
    UnknownLayout { id: String, known: Vec<String> },
    /// A course name the course file does not have (usage).
    UnknownCourse { name: String, known: Vec<String> },
    /// `export` was asked for courses or the single file, but the project names no course file (usage).
    ExportNeedsCourseFile,
    /// The output folder given does not exist.
    OutDirMissing { path: PathBuf },
    /// A folder could not be created.
    CreateFolder {
        path: PathBuf,
        source: anyhow::Error,
    },
    /// A write was refused or failed: a full disk, a link out of the project folder (ADR 0041), an error of the
    /// operating system.
    Output(OutputError),
    /// A sheet could not be drawn or written. Sheets before it are written already.
    Export(ExportError),
    /// `masks` needs a course file for this, and the project names none (usage).
    MasksNeedCourseFile,
    /// The mask could not be built from what was given; `course` is empty for a corridor without one (usage).
    Build {
        error: freemapper_print::mask_build::BuildError,
        course: String,
    },
    /// A `--radius-for` item that is not `CODE=MM` (usage).
    RadiusFor { text: String },
    /// The map file has no scale, so print mm cannot become map mm.
    MasksMapScale,
    /// The mask id given is empty, too long or has control characters (usage).
    MaskId,
    /// The mask id is taken (usage).
    MaskIdTaken { id: String },
    /// The course's assignment names another mask already (usage).
    CourseHasMask { course: String, mask: String },
    /// The assignment of the course is written inline and cannot be changed by the program (usage). `new`: the
    /// course has no assignment yet, but the others are inline.
    AssignmentInline {
        course: String,
        mask: String,
        new: bool,
    },
    /// A file could not be written.
    WriteFile {
        path: PathBuf,
        source: anyhow::Error,
    },
    /// The project file with the new mask no longer loads; it has been put back as it was.
    LoadChangedMask {
        path: PathBuf,
        source: anyhow::Error,
    },
    /// The layouts print at different scales, so a corridor without a course has no one width (usage).
    MasksScales { scales: Vec<u32> },
}
