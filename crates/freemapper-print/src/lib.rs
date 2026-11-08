//! freemapper-print turns a finished orienteering map and the courses from a course-setting program into
//! print PDFs, one sheet per course.
//!
//! All domain logic lives in this library. The binary crate `freemapper-print-cli` holds the CLI, the MCP
//! server and the layout editor (ADR 0001). Terms follow `docs/glossary.md`.

pub mod omap;
pub mod print;
pub mod render;
pub mod scale;

/// The program and where to find it, as named in the PDF metadata.
pub const CREDIT: &str = concat!("freemapper-print – ", env!("CARGO_PKG_REPOSITORY"));
