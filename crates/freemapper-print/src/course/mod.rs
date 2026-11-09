//! Courses from a course-setting program: the model that does not depend on the file format, and the
//! readers. Terms follow `docs/glossary.md`.

pub mod fit;
pub mod iof_xml;
pub mod load;
pub mod model;
pub mod ppen;
mod tm;
mod xml;

pub use fit::{Fit, FitError, FitNote, SkipReason, fit};
pub use iof_xml::read_iof_xml;
pub use load::{
    CorrectionScope, CourseLoadError, LayoutNote, LoadedCourses, Stale, base_changed, layout_for,
    load_courses,
};
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
