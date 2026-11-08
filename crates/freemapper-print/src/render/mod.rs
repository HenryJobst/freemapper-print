//! Copied from freemapper 743ce4b (ADR 0014).
//! The training map: OOM symbols drawn the way OOM draws them, written as PDF.

mod dash;
pub mod draw;
pub mod geometry;
mod line;
pub(crate) mod metrics;
mod pattern;
pub mod pdf;
pub mod symbols;
