//! Copied from freemapper 743ce4b (ADR 0014).
//! The training map: OOM symbols drawn the way OOM draws them, written as PDF.
//!
//! The public part is [`page`] (what every backend draws), [`pdf`] (the PDF backend) and
//! [`fonts`] (text metrics for layouts); symbol parsing and drawing stay inside the crate.

pub(crate) mod bounds;
mod dash;
pub(crate) mod draw;
pub mod fonts;
pub(crate) mod geometry;
pub mod image;
mod line;
pub mod page;
mod pattern;
pub mod pdf;
pub(crate) mod symbols;
