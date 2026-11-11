//! Why an op stopped, as values. The caller words them (`op_error` in `main.rs`); the variants carry what the
//! message names. Where the cause is an error of the library or the operating system, it stays the `source`, so
//! that a chain of messages names it once.

use std::path::PathBuf;

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
}
