//! The `freemapper-print` binary. Commands come with the milestones in the design
//! (`docs/superpowers/specs/2026-11-08-freemapper-print-design.md`).

use clap::Parser;

#[derive(Parser)]
#[command(
    name = "freemapper-print",
    version,
    about = "Print layouts for orienteering maps"
)]
struct Cli {}

fn main() {
    Cli::parse();
}
