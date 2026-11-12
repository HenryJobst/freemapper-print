//! `freemapper-print update` (spec 3.4 to 3.6): check for a newer release, install it after asking, go
//! back to the previous version, or store the mode of the daily check.
//!
//! Results and progress go to stdout, and errors back to `main`, which prints them as its usual
//! `Error:` line with exit code 1.

use std::io::{BufRead as _, IsTerminal as _, Write as _};
use std::path::Path;

use anyhow::anyhow;
use freemapper_update::{
    App, Mode, Outcome, ProcessRunner, UpdateError, Version, check_for_update, exe_path, install,
    load_mode, manifest_url, previous_path, rollback, save_mode,
};

use super::{
    HttpUpdateSource, mode_from_env, print_app, release_build, settings_dir, update_source,
};
use crate::text::Texts;

/// The exit code of `update --check` when a newer version exists.
const UPDATE_AVAILABLE: u8 = 10;

/// What `freemapper-print update` was asked to do.
pub struct Request {
    /// Only check and report.
    pub check: bool,
    /// Install without asking.
    pub yes: bool,
    /// This release instead of the latest one.
    pub version: Option<Version>,
    /// Swap the previous version back in.
    pub rollback: bool,
    /// Store this mode and do nothing else.
    pub mode: Option<Mode>,
}

/// A text with variables, each given as a name and anything printable.
macro_rules! say {
    ($texts:expr, $id:expr $(, $name:literal => $value:expr)* $(,)?) => {{
        #[allow(unused_mut)]
        let mut args = fluent_bundle::FluentArgs::new();
        $(args.set($name, $value.to_string());)*
        $texts.get($id, Some(&args))
    }};
}

/// Runs the command. Exit codes: 0 done or current, 10 `--check` found an update (also one that
/// needs a manual install); errors come back for `main` (exit 1).
pub fn run(texts: &Texts, request: Request) -> anyhow::Result<u8> {
    // Only a setting, so it works in a build from source too.
    if let Some(mode) = request.mode {
        set_mode(texts, mode)?;
        return Ok(0);
    }
    if !release_build() {
        println!("{}", texts.plain("update-source-build"));
        return Ok(0);
    }
    let app = print_app();
    if request.rollback {
        return roll_back(texts, &app, &running_exe(texts)?);
    }
    let source = update_source();
    let outcome = match check_for_update(&app, &source, request.version.as_ref()) {
        // An update exists, only not by this way (spec 3.3).
        Err(too_old @ UpdateError::TooOldToSelfUpdate { .. }) if request.check => {
            println!("{}", describe(texts, &too_old));
            return Ok(UPDATE_AVAILABLE);
        }
        result => result.map_err(|e| {
            anyhow!(check_failed(
                texts,
                &app,
                &source,
                request.version.as_ref(),
                &e
            ))
        })?,
    };
    println!("{}", checked(texts, &app, &outcome));
    let Outcome::Available { version, notes, .. } = &outcome else {
        // `Current`, and any case a later core adds: nothing to install.
        return Ok(0);
    };
    if request.check {
        return Ok(UPDATE_AVAILABLE);
    }
    if !request.yes {
        if !std::io::stdin().is_terminal() {
            return Err(anyhow!(texts.plain("update-needs-yes")));
        }
        if !ask(texts)? {
            println!("{}", texts.plain("update-not-installed"));
            return Ok(0);
        }
    }
    let exe = running_exe(texts)?;
    println!("{}", say!(texts, "update-installing", "version" => version));
    let installed = install(&app, &source, &ProcessRunner::new(app.name), &exe, &outcome)
        .map_err(|e| failed(texts, e))?;
    println!("{}", installed_text(texts, &app, &installed, notes));
    println!("{}", texts.plain("update-mcp-restart"));
    Ok(0)
}

/// What a check found: the newer version with its date, notes and size, or that this one is
/// current.
fn checked(texts: &Texts, app: &App, outcome: &Outcome) -> String {
    match outcome {
        Outcome::Available {
            version,
            released,
            notes,
            asset,
        } => format!(
            "{}\n{}",
            say!(
                texts,
                "update-available",
                "new" => version,
                "old" => app.current,
                "released" => released,
            ),
            say!(
                texts,
                "update-details",
                "notes" => notes,
                "size" => megabytes(texts, asset.size),
            )
        ),
        _ => say!(texts, "update-current", "version" => app.current),
    }
}

/// The line after an install: from which version to which, and the release notes.
fn installed_text(texts: &Texts, app: &App, installed: &Version, notes: &str) -> String {
    say!(
        texts,
        "update-done",
        "old" => app.current,
        "new" => installed,
        "notes" => notes,
    )
}

/// What a check found, for the MCP tool `check_update` (spec 3.7).
#[allow(dead_code)] // The MCP tools of Task 5 use it.
#[derive(Debug)]
pub struct CheckReport {
    /// The version of the running program.
    pub current: Version,
    /// The newer release, if there is one.
    pub available: Option<AvailableReport>,
    /// What `update --check` prints, in the language of the call.
    pub message: String,
}

/// A newer release as the MCP tool reports it.
#[allow(dead_code)] // The MCP tools of Task 5 use it.
#[derive(Debug)]
pub struct AvailableReport {
    pub version: Version,
    /// The release date; none for a release that needs a manual install.
    pub released: Option<String>,
    /// The link to the release notes.
    pub notes: String,
    /// The download size; none for a release that needs a manual install.
    pub size_bytes: Option<u64>,
    /// The release cannot be installed by the self-update (`TooOldToSelfUpdate`).
    pub manual: bool,
}

/// What the MCP tool `install_update` did.
#[allow(dead_code)] // The MCP tools of Task 5 use it.
#[derive(Debug)]
pub struct InstallReport {
    /// The version now installed; none when this one was current already.
    pub installed: Option<Version>,
    /// The link to its release notes.
    pub notes: Option<String>,
    /// The text for the person.
    pub message: String,
}

/// The MCP tool `check_update` (spec 3.7) in a release build: what `update --check` prints, or
/// the error for the person. A release that needs a manual install is a result, as for
/// `--check`.
#[allow(dead_code)] // The MCP tools of Task 5 use it.
pub fn check_for_mcp(texts: &Texts) -> Result<CheckReport, String> {
    let app = print_app();
    let source = update_source();
    match check_for_update(&app, &source, None) {
        Ok(outcome) => {
            let available = match &outcome {
                Outcome::Available {
                    version,
                    released,
                    notes,
                    asset,
                } => Some(AvailableReport {
                    version: *version,
                    released: Some(released.clone()),
                    notes: notes.clone(),
                    size_bytes: Some(asset.size),
                    manual: false,
                }),
                _ => None,
            };
            Ok(CheckReport {
                current: app.current,
                available,
                message: checked(texts, &app, &outcome),
            })
        }
        Err(
            ref too_old @ UpdateError::TooOldToSelfUpdate {
                ref version,
                ref notes,
                ..
            },
        ) => Ok(CheckReport {
            current: app.current,
            available: Some(AvailableReport {
                version: *version,
                released: None,
                notes: notes.clone(),
                size_bytes: None,
                manual: true,
            }),
            message: describe(texts, too_old),
        }),
        Err(e) => Err(check_failed(texts, &app, &source, None, &e)),
    }
}

/// The MCP tool `install_update` (spec 3.7) in a release build, once the person agreed: installs
/// the newest release over the running program, printing nothing. The text for the person, or
/// the error.
#[allow(dead_code)] // The MCP tools of Task 5 use it.
pub fn install_for_mcp(texts: &Texts) -> Result<InstallReport, String> {
    let app = print_app();
    let source = update_source();
    let outcome = check_for_update(&app, &source, None)
        .map_err(|e| check_failed(texts, &app, &source, None, &e))?;
    let Outcome::Available { notes, .. } = &outcome else {
        return Ok(InstallReport {
            installed: None,
            notes: None,
            message: say!(texts, "update-current", "version" => app.current),
        });
    };
    let exe = exe_path().map_err(|e| describe(texts, &UpdateError::Io(e)))?;
    let installed = install(&app, &source, &ProcessRunner::new(app.name), &exe, &outcome)
        .map_err(|e| describe(texts, &e))?;
    // This server is the one that still runs the old version.
    Ok(InstallReport {
        message: format!(
            "{}\n{}",
            installed_text(texts, &app, &installed, notes),
            texts.plain("update-mcp-restart-now")
        ),
        installed: Some(installed),
        notes: Some(notes.clone()),
    })
}

/// A failed check in the language of the run. A manifest that is not there (HTTP 404) means a
/// release that cannot install itself; one asked for by `version` may also not exist at all. Only
/// the manifest counts: a missing signature or asset stays a plain failure.
fn check_failed(
    texts: &Texts,
    app: &App,
    source: &HttpUpdateSource,
    version: Option<&Version>,
    error: &UpdateError,
) -> String {
    if !matches!(error, UpdateError::Download(_))
        || !source.was_not_found(&manifest_url(app, version))
    {
        return describe(texts, error);
    }
    match version {
        None => say!(
            texts,
            "update-no-manifest",
            "page" => format!("{}/releases/latest", app.repo_url),
        ),
        Some(version) => say!(
            texts,
            "update-no-manifest-version",
            "version" => version,
            "page" => format!("{}/releases", app.repo_url),
        ),
    }
}

/// Stores `mode` in `settings.toml` and says where. Mentions `FREEMAPPER_PRINT_UPDATE` when it
/// overrides the stored mode.
fn set_mode(texts: &Texts, mode: Mode) -> anyhow::Result<()> {
    let dir = settings_dir().ok_or_else(|| anyhow!(texts.plain("update-no-config-dir")))?;
    let file = dir.join("settings.toml");
    save_mode(&dir, mode).map_err(|e| {
        anyhow!(say!(
            texts,
            "update-mode-failed",
            "path" => file.display(),
            "error" => e,
        ))
    })?;
    println!(
        "{}",
        say!(
            texts,
            "update-mode-set",
            "mode" => mode,
            "path" => file.display(),
        )
    );
    if let Some(value) = mode_from_env()
        && load_mode(&dir, Some(&value)) != mode
    {
        println!("{}", say!(texts, "update-mode-env", "value" => value));
    }
    Ok(())
}

/// `update --rollback`: the previous version takes the place of the running one.
fn roll_back(texts: &Texts, app: &App, exe: &Path) -> anyhow::Result<u8> {
    let previous = previous_path(exe);
    match rollback(exe, &ProcessRunner::new(app.name)) {
        Ok(version) => {
            println!(
                "{}",
                say!(
                    texts,
                    "update-rolled-back",
                    "version" => version,
                    "path" => shown(&previous),
                )
            );
            println!("{}", texts.plain("update-mcp-restart"));
            Ok(0)
        }
        Err(e) => Err(rollback_error(texts, e, &previous)),
    }
}

/// A failed rollback. A missing file means "no previous version" only when it is the previous
/// version that is missing.
fn rollback_error(texts: &Texts, error: UpdateError, previous: &Path) -> anyhow::Error {
    match error {
        UpdateError::Io(e)
            if e.kind() == std::io::ErrorKind::NotFound
                && std::fs::symlink_metadata(previous).is_err() =>
        {
            anyhow!(say!(texts, "update-no-previous", "path" => shown(previous)))
        }
        other => failed(texts, other),
    }
}

/// The running executable with links resolved: the file an update replaces.
fn running_exe(texts: &Texts) -> anyhow::Result<std::path::PathBuf> {
    exe_path().map_err(|e| failed(texts, UpdateError::Io(e)))
}

/// Asks before installing; only `y`, `yes` (and the German `j`, `ja`) install.
fn ask(texts: &Texts) -> anyhow::Result<bool> {
    print!("{} ", texts.plain("update-ask"));
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().lock().read_line(&mut answer)?;
    Ok(is_yes(&answer))
}

fn is_yes(answer: &str) -> bool {
    matches!(
        answer.trim().to_lowercase().as_str(),
        "y" | "yes" | "j" | "ja"
    )
}

/// An update error in the language of the run. The cases the user can act on have their own
/// text; any other keeps the core's English detail inside a translated frame.
fn failed(texts: &Texts, error: UpdateError) -> anyhow::Error {
    anyhow!(describe(texts, &error))
}

pub(super) fn describe(texts: &Texts, error: &UpdateError) -> String {
    match error {
        UpdateError::NotWritable(folder) => {
            say!(texts, "update-not-writable", "path" => shown(folder))
        }
        UpdateError::TooOldToSelfUpdate {
            needed,
            version,
            notes,
        } => say!(
            texts,
            "update-too-old",
            "new" => version,
            "old" => print_app().current,
            "needed" => needed,
            "notes" => notes,
        ),
        UpdateError::Busy(lock) => say!(texts, "update-busy", "path" => shown(lock)),
        UpdateError::Signature | UpdateError::UnknownKey => {
            say!(texts, "update-bad-signature", "error" => error)
        }
        UpdateError::NoAssetForPlatform(platform) => say!(
            texts,
            "update-no-asset",
            "platform" => platform,
            "page" => format!("{}/releases", print_app().repo_url),
        ),
        other => say!(texts, "update-failed", "error" => other),
    }
}

/// `bytes` in MB (2^20 bytes, as the disk space messages count) with one decimal, written with
/// the decimal separator of the language.
fn megabytes(texts: &Texts, bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / f64::from(1u32 << 20))
        .replace('.', &texts.plain("update-decimal-separator"))
}

/// A path for a message: on Windows without the `\\?\` prefix that resolving the executable adds.
fn shown(path: &Path) -> String {
    let text = path.display().to_string();
    match text.strip_prefix(r"\\?\") {
        Some(plain) => plain.to_string(),
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::Lang;

    fn english() -> Texts {
        Texts::new(&Lang::english())
    }

    #[test]
    fn only_yes_installs() {
        for yes in ["y\n", "Y", " yes \r\n", "j", "Ja"] {
            assert!(is_yes(yes), "{yes:?}");
        }
        for no in ["", "\n", "n", "no", "nein", "yess", "maybe"] {
            assert!(!is_yes(no), "{no:?}");
        }
    }

    #[test]
    fn sizes_are_megabytes_with_one_decimal() {
        let texts = english();
        assert_eq!(megabytes(&texts, 12_345_678), "11.8");
        assert_eq!(megabytes(&texts, 0), "0.0");
        assert_eq!(megabytes(&texts, 1 << 20), "1.0");
    }

    #[test]
    fn errors_the_user_can_act_on_have_their_own_text() {
        let texts = english();
        let folder = Path::new("/opt/freemapper-print");
        assert_eq!(
            describe(&texts, &UpdateError::NotWritable(folder.to_path_buf())),
            say!(texts, "update-not-writable", "path" => folder.display())
        );
        let too_old = describe(
            &texts,
            &UpdateError::TooOldToSelfUpdate {
                needed: "0.12.0".parse().unwrap(),
                version: "0.13.0".parse().unwrap(),
                notes: "https://github.com/HenryJobst/freemapper-print/releases/tag/v0.13.0".into(),
            },
        );
        assert!(
            too_old.contains("0.13.0") && too_old.contains("0.12.0"),
            "{too_old}"
        );
        assert!(too_old.contains("releases/tag/v0.13.0"), "{too_old}");
        assert!(too_old.contains("the self-update of"), "{too_old}");
        for unverified in [UpdateError::Signature, UpdateError::UnknownKey] {
            let detail = unverified.to_string();
            assert_eq!(
                describe(&texts, &unverified),
                say!(texts, "update-bad-signature", "error" => detail)
            );
        }
        assert_eq!(
            describe(&texts, &UpdateError::Signature),
            "the release could not be verified (the update signature is not valid); nothing was installed"
        );
        let no_asset = describe(
            &texts,
            &UpdateError::NoAssetForPlatform("linux-riscv".into()),
        );
        assert!(no_asset.contains("(linux-riscv)"), "{no_asset}");
        assert!(
            no_asset.contains("https://github.com/HenryJobst/freemapper-print/releases"),
            "{no_asset}"
        );
        let lock = Path::new("/opt/freemapper-print/freemapper-print.update-lock");
        assert_eq!(
            describe(&texts, &UpdateError::Busy(lock.to_path_buf())),
            say!(texts, "update-busy", "path" => lock.display())
        );
        assert_eq!(
            shown(Path::new(r"\\?\C:\bin\freemapper-print.exe")),
            r"C:\bin\freemapper-print.exe"
        );
        assert_eq!(shown(Path::new("/usr/bin/x")), "/usr/bin/x");
    }

    #[test]
    fn a_missing_file_is_no_previous_only_when_previous_is_missing() {
        let texts = english();
        let dir = tempfile::tempdir().unwrap();
        let previous = dir.path().join("freemapper-print.previous");
        let not_found = || UpdateError::Io(std::io::ErrorKind::NotFound.into());
        let no_previous = say!(texts, "update-no-previous", "path" => previous.display());
        assert_eq!(
            rollback_error(&texts, not_found(), &previous).to_string(),
            no_previous
        );
        // Another file that is missing (the previous is there) is not "no previous".
        std::fs::write(&previous, "x").unwrap();
        let other = rollback_error(&texts, not_found(), &previous).to_string();
        assert_ne!(other, no_previous);
        assert!(other.starts_with("the update failed"), "{other}");
    }
}
