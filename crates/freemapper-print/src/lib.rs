//! freemapper-print turns a finished orienteering map and the courses from a course-setting program into
//! print PDFs, one sheet per course.
//!
//! All domain logic lives in this library. The binary crate `freemapper-print-cli` holds the CLI, the MCP
//! server and the layout editor (ADR 0001). Terms follow `docs/glossary.md`.

pub mod export;
pub mod omap;
pub mod output;
pub mod print;
pub mod project;
pub mod render;
pub mod scale;
pub mod sheet;

/// The program and where to find it, as named in the PDF metadata.
pub const CREDIT: &str = concat!("freemapper-print – ", env!("CARGO_PKG_REPOSITORY"));

/// The credit line every German sheet carries at the bottom (it cannot be switched off). The dash
/// is U+2013.
pub const CREDIT_LINE_DE: &str = concat!(
    "Erstellt mit freemapper-print – ",
    env!("CARGO_PKG_REPOSITORY")
);

/// The credit line every English sheet carries at the bottom.
pub const CREDIT_LINE_EN: &str = concat!(
    "Created with freemapper-print – ",
    env!("CARGO_PKG_REPOSITORY")
);

/// The credit line for a sheet in `lang`.
pub const fn credit_line(lang: project::SheetLang) -> &'static str {
    match lang {
        project::SheetLang::De => CREDIT_LINE_DE,
        project::SheetLang::En => CREDIT_LINE_EN,
    }
}
