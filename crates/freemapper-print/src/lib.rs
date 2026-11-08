//! freemapper-print turns a finished orienteering map and the courses from a course-setting program into
//! print PDFs, one sheet per course.
//!
//! All domain logic lives in this library. The binary crate `freemapper-print-cli` holds the CLI, the MCP
//! server and the layout editor (ADR 0001). Terms follow `docs/glossary.md`.
