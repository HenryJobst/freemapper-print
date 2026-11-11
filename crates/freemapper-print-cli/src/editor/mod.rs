//! The editor: `freemapper-print edit`, a local server and a browser page (ADR 0005, ADR 0059).
//!
//! A third adapter beside the CLI and the MCP server. It follows the suite page rules of freemapper-core
//! ADR 0030. Everything that changes the project file goes through the `ops` functions; no logic of the
//! editor lives in `main.rs`.

mod api;
mod edits;
mod preview;
mod server;
mod state;

#[cfg(test)]
mod page_tests;

use std::io::Write;
use std::path::PathBuf;

use clap::Args;
use fluent_bundle::FluentArgs;

use crate::ops::{self, OpError};
use crate::text::Texts;

/// The arguments of `freemapper-print edit`.
#[derive(Args, Debug, Clone)]
pub struct EditArgs {
    /// `freemapper-print.toml` or the folder that holds it.
    pub project: PathBuf,
    /// Port on 127.0.0.1; 0 lets the system choose a free one.
    #[arg(long, default_value_t = 0)]
    pub port: u16,
    /// Print the address and open no browser.
    #[arg(long)]
    pub no_browser: bool,
    /// The language the person set (`--lang`, `FREEMAPPER_LANG`); `main` fills it, see [`fixed_language`].
    #[arg(skip)]
    pub lang: Option<String>,
}

/// The language the person set for the page: `--lang`, else `FREEMAPPER_LANG`. Without either the page follows
/// the browser's `Accept-Language`, then English (core ADR 0030). `LANG` and `LC_*` do not count here.
pub fn fixed_language(arg: Option<&str>, env: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    arg.map(str::to_owned)
        .or_else(|| env("FREEMAPPER_LANG"))
        .filter(|code| !code.trim().is_empty())
}

/// `freemapper-print edit`: serves the editor until the page sends `done` or the person presses Ctrl+C.
pub fn run(args: &EditArgs, texts: &Texts) -> Result<(), OpError> {
    run_with(
        args,
        texts,
        &mut std::io::stdout(),
        &freemapper_print::web::open_browser,
    )
}

/// [`run`] with the output and the browser opener given, so tests can watch both.
pub(crate) fn run_with(
    args: &EditArgs,
    texts: &Texts,
    out: &mut dyn Write,
    open: &dyn Fn(&str) -> std::io::Result<()>,
) -> Result<(), OpError> {
    let path = ops::project_file(&args.project);
    if let Err(source) = std::fs::metadata(&path) {
        return Err(OpError::LoadProject {
            path,
            source: anyhow::Error::new(source),
        });
    }
    server::serve_until_done(args.port, path, args.lang.clone(), |url| {
        let line = texts.get("edit-listening", Some(&args!("url" => url.to_string())));
        // A closed stdout must not stop the editor.
        let _ = writeln!(out, "{line}");
        let _ = out.flush();
        if !args.no_browser {
            // Without a browser the person still has the printed address.
            let _ = open(url);
        }
    })
}
