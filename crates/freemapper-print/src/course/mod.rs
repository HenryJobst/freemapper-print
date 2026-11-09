//! Courses from a course-setting program: the model that does not depend on the file format, the readers and
//! the fit to the map. Terms follow `docs/glossary.md`. The module uses only the map model (`omap`); loading the
//! course file a project names happens outside it (ADR 0039).

pub mod fit;
pub mod iof_xml;
pub mod model;
pub mod ppen;
mod tm;
pub(crate) mod xml;

pub use fit::{Fit, FitError, FitNote, SkipReason, fit};
pub use iof_xml::read_iof_xml;
pub use model::{
    Control, ControlKind, Course, CourseEvent, CourseFilter, CourseKind, CourseNote, CourseSource,
    Description, FittedEvent, Leg, NumberPlacement, RawEvent, RawPoint, SourceFormat, Special,
    Stop,
};
pub use ppen::read_ppen;

/// Why a course file could not be read.
#[derive(Debug, thiserror::Error)]
pub enum CourseError {
    #[error("invalid course file: {0}")]
    Xml(String),
    #[error("course file too large or too complex: {0}")]
    Limit(String),
    #[error("{0}")]
    Unsupported(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
