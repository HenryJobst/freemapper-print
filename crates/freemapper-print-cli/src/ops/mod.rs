//! The work of the commands without terminal output. Each op takes a plain argument struct (no clap types) and
//! returns a result struct that serde serialises; `main.rs` words the results for the terminal, the MCP server
//! returns them as JSON. No program text is made here: notes are typed values ([`Notice`]) that
//! `Notice::describe` turns into text, and errors are [`OpError`] values that the caller words. Where an op
//! writes text of the user's language into the project file (a comment), the caller passes a function that
//! makes it.

pub mod check;
pub mod error;
pub mod export;
pub mod masks;
pub mod note;
pub mod numbers;

#[cfg(test)]
pub(crate) mod testing {
    use std::path::PathBuf;

    /// The folder of the Viktoriapark example project (a course file with one course, one layout).
    pub fn viktoriapark() -> PathBuf {
        PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/berlin-viktoriapark-training"
        ))
    }

    /// A copy of the Viktoriapark example in a temporary folder, for ops that write: the project file, the map,
    /// its `freemapper.toml` and the course file.
    pub fn viktoriapark_copy() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "freemapper-print.toml",
            "berlin-viktoriapark.omap",
            "freemapper.toml",
            "training.ppen",
        ] {
            std::fs::copy(viktoriapark().join(name), dir.path().join(name)).unwrap();
        }
        dir
    }
}

use std::path::{Path, PathBuf};

use freemapper_print::courses::{LoadedCourses, load_courses};
use freemapper_print::omap::MapDocument;
use freemapper_print::omap::read::read_map_file;
use freemapper_print::print::{Orientation, Paper, PrintReport};
use freemapper_print::project::{self, LayoutSpec, LoadedProject};
use freemapper_print::scale::MapKind;
use freemapper_print::sheet::SheetReport;
use serde::Serialize;

pub use error::OpError;
pub use note::{NoteValue, Notice};

/// The project file named by `arg`: the file itself, or `freemapper-print.toml` in a folder.
pub fn project_file(arg: &Path) -> PathBuf {
    if arg.is_dir() {
        arg.join(project::FILE_NAME)
    } else {
        arg.to_path_buf()
    }
}

/// The project file named by `arg` (a file or its folder), read and checked.
pub fn load_project(arg: &Path) -> Result<LoadedProject, OpError> {
    let path = project_file(arg);
    project::load(&path).map_err(|err| match err {
        project::ProjectError::BasePartHidden { file, field } => {
            OpError::BasePartHidden { path, file, field }
        }
        other => OpError::LoadProject {
            path,
            source: anyhow::Error::new(other),
        },
    })
}

/// The map file at `path`.
pub fn read_map(path: &Path) -> Result<MapDocument, OpError> {
    read_map_file(path).map_err(|err| OpError::ReadMap {
        path: path.to_path_buf(),
        source: anyhow::Error::new(err),
    })
}

/// The course file of the project, read and fitted to `map`; `None` when the project names none.
pub fn read_courses(
    loaded: &LoadedProject,
    map: &MapDocument,
) -> Result<Option<LoadedCourses>, OpError> {
    load_courses(loaded, map).map_err(|err| OpError::Courses(anyhow::Error::new(err)))
}

/// The page size, scales and factors of a printed page: what the first line about a sheet names.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct PrintFacts {
    /// Width and height of the page (mm), in the orientation printed.
    pub page_mm: (f64, f64),
    pub print_scale: u32,
    pub geometry_factor: f64,
    pub symbol_factor: f64,
}

impl From<&PrintReport> for PrintFacts {
    fn from(report: &PrintReport) -> Self {
        PrintFacts {
            page_mm: report.page_mm,
            print_scale: report.print_scale,
            geometry_factor: report.geometry_factor,
            symbol_factor: report.symbol_factor,
        }
    }
}

/// One drawn sheet: a layout, with or without a course, and its notes in the order the CLI prints them (the
/// print report's, then the sheet's).
#[derive(Debug, Serialize)]
pub struct Sheet {
    /// The course on the sheet; `None` for a layout sheet without a course.
    pub course: Option<String>,
    /// The id of the layout.
    pub layout: String,
    pub paper: Paper,
    pub print: PrintFacts,
    /// The scale of the map file.
    pub map_scale: u32,
    pub notes: Vec<Notice>,
    /// A note asks the user to look (`check` fails on it): any print note, or a sheet note that does more than
    /// inform.
    pub needs_attention: bool,
}

impl Sheet {
    /// The sheet of `layout` (with `course`, if any) that `report` describes. `kind` is the project's map kind,
    /// `map_scale` the scale of the map file.
    pub fn new(
        course: Option<&str>,
        layout: &LayoutSpec,
        report: SheetReport,
        kind: MapKind,
        map_scale: u32,
    ) -> Self {
        let needs_attention = report.needs_attention();
        let explicit_orientation = layout.orientation != Orientation::Auto;
        let mut notes: Vec<Notice> = note::print_notes(
            &report.print,
            layout.paper,
            explicit_orientation,
            kind,
            true,
        )
        .into_iter()
        .map(|n| Notice::print(course, n))
        .collect();
        notes.extend(report.notes.into_iter().map(|n| Notice::sheet(course, n)));
        Sheet {
            course: course.map(str::to_string),
            layout: layout.id.clone(),
            paper: layout.paper,
            print: PrintFacts::from(&report.print),
            map_scale,
            notes,
            needs_attention,
        }
    }
}
