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
}
