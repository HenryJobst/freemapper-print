//! The daily check after the commands that make a person look at the program (spec 3.3, ADR 0060
//! P3), and in mode `install` the install that follows it (spec 3.4, P5).
//!
//! `main` starts the check on a thread before `init`, `check`, `export` and `render`, and finishes
//! it after the command's own output. The check gets 3 seconds from its start; a slower one is
//! dropped and recorded as failed, so it is tried again the next day.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use freemapper_update::{
    CheckRecord, Mode, Outcome, ProcessRunner, RecordResult, UpdateError, UpdateSource, Version,
    check_for_update, due, exe_path, install, load_mode, load_record, remove_stale, save_record,
};

use super::command::describe;
use super::{mode_from_env, print_app, record_path, release_build, settings_dir, update_source};
use crate::text::Texts;

/// How long after its start the check may take (spec 3.3).
const LIMIT: Duration = Duration::from_secs(3);

/// Everything the daily check depends on, so tests can set each part.
pub struct CheckContext {
    /// The file of the record (`update-check.json`); `None` when the system has no cache folder.
    pub record: Option<PathBuf>,
    /// `CI` is set.
    pub ci: bool,
    pub mode: Mode,
    /// [`release_build`].
    pub release: bool,
    /// Unix seconds now.
    pub now: i64,
    pub source: Arc<dyn UpdateSource + Send + Sync>,
}

impl CheckContext {
    /// The context of this process: the record in the cache folder, the mode from the environment
    /// and the settings, the clock and the update source.
    pub fn of_process() -> Self {
        let env = mode_from_env();
        let mode = match settings_dir() {
            Some(dir) => load_mode(&dir, env.as_deref()),
            None => env.and_then(|m| m.parse().ok()).unwrap_or(Mode::Notify),
        };
        Self {
            record: record_path(),
            ci: std::env::var_os("CI").is_some(),
            mode,
            release: release_build(),
            now: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()
                .and_then(|elapsed| i64::try_from(elapsed.as_secs()).ok())
                .unwrap_or(0),
            source: Arc::new(update_source()),
        }
    }
}

/// A check running on its thread.
pub struct BackgroundCheck {
    record: PathBuf,
    started: Instant,
    now: i64,
    mode: Mode,
    source: Arc<dyn UpdateSource + Send + Sync>,
    result: Receiver<Result<Outcome, UpdateError>>,
}

/// What the check has to say after the command.
#[derive(Debug, PartialEq, Eq)]
pub enum Report {
    /// A newer version exists (stderr).
    Notice(String),
    /// Mode `install` installed it (stdout, like other progress).
    Installed(String),
    /// Mode `install` could not install it, or the newest release does not verify (stderr).
    Warning(String),
}

impl Report {
    pub fn print(&self) {
        match self {
            Report::Installed(text) => println!("{text}"),
            Report::Notice(text) | Report::Warning(text) => eprintln!("{text}"),
        }
    }
}

/// A text with variables, each given as a name and anything printable.
fn say(texts: &Texts, id: &str, args: &[(&str, String)]) -> String {
    let mut fluent = fluent_bundle::FluentArgs::new();
    for (name, value) in args {
        fluent.set(*name, value.clone());
    }
    texts.get(id, Some(&fluent))
}

fn v(value: impl std::fmt::Display) -> String {
    value.to_string()
}

/// Starts the check, or `None` when none is due: a build from source, `CI` set, mode `off`, no
/// record path, or an attempt less than 24 hours ago. A record that is not JSON, or lies in the
/// future, counts as due.
pub fn start_background_check(ctx: &CheckContext) -> Option<BackgroundCheck> {
    if !ctx.release || ctx.ci || ctx.mode == Mode::Off {
        return None;
    }
    let record = ctx.record.clone()?;
    if !due(load_record(&record).as_ref(), ctx.now) {
        return None;
    }
    let (sender, result) = std::sync::mpsc::channel();
    let source = Arc::clone(&ctx.source);
    std::thread::Builder::new()
        .name("daily check".into())
        .spawn(move || {
            // Nobody listens any more after a timeout; that is fine.
            let _ = sender.send(check_for_update(&print_app(), source.as_ref(), None));
        })
        .ok()?;
    Some(BackgroundCheck {
        record,
        started: Instant::now(),
        now: ctx.now,
        mode: ctx.mode,
        source: Arc::clone(&ctx.source),
        result,
    })
}

impl BackgroundCheck {
    /// Waits for the check, at most until 3 s after its start, and records the attempt (a
    /// dropped one as failed with reason `timeout`). Only this writes the record, so a check
    /// that ends after the drop changes nothing.
    ///
    /// A newer version gives the notice. In mode `install`, after a command that succeeded, it
    /// is installed instead (`update-installing` is printed here, before the download); a failed
    /// install is one warning and never changes the exit code of the command.
    pub fn finish(self, texts: &Texts, command_ok: bool) -> Option<Report> {
        match &self.settle()? {
            Ok(outcome @ Outcome::Available { version, notes, .. }) => {
                if self.mode == Mode::Install && command_ok {
                    return Some(self.install(texts, outcome, version, notes));
                }
                Some(Report::Notice(notice(texts, *version)))
            }
            // An update exists, but it needs a manual install: say so, with the link.
            Err(too_old @ UpdateError::TooOldToSelfUpdate { .. }) => {
                Some(Report::Notice(describe(texts, too_old)))
            }
            // A release that does not verify is an error that says so (spec 3.2), but only a
            // warning here: the command ran. The record keeps it to once a day.
            Err(unverified @ (UpdateError::Signature | UpdateError::UnknownKey)) => {
                Some(Report::Warning(bad_signature_note(texts, unverified)))
            }
            // Anything else stays silent, a missing manifest or program for this system too; the
            // record says it failed.
            _ => None,
        }
    }

    /// Waits for the check as [`Self::finish`] does and records the attempt. `None` when it was
    /// dropped or stopped.
    fn settle(&self) -> Option<Result<Outcome, UpdateError>> {
        let result = match self
            .result
            .recv_timeout(LIMIT.saturating_sub(self.started.elapsed()))
        {
            Ok(result) => result,
            Err(RecvTimeoutError::Timeout) => {
                self.save(RecordResult::Failed {
                    reason: "timeout".into(),
                });
                return None;
            }
            Err(RecvTimeoutError::Disconnected) => {
                self.save(RecordResult::Failed {
                    reason: "the check stopped".into(),
                });
                return None;
            }
        };
        self.save(match &result {
            Ok(Outcome::Available { version, notes, .. })
            | Err(UpdateError::TooOldToSelfUpdate { version, notes, .. }) => {
                RecordResult::Available {
                    version: *version,
                    notes: notes.clone(),
                }
            }
            Err(e) => RecordResult::Failed {
                reason: e.to_string(),
            },
            // `Current`, and any case a later core adds.
            Ok(_) => RecordResult::Current,
        });
        Some(result)
    }

    /// Mode `install`: installs `available` over the running program.
    fn install(
        &self,
        texts: &Texts,
        available: &Outcome,
        version: &Version,
        notes: &str,
    ) -> Report {
        let app = print_app();
        println!(
            "{}",
            say(texts, "update-installing", &[("version", v(version))])
        );
        let installed = exe_path().map_err(UpdateError::Io).and_then(|exe| {
            install(
                &app,
                self.source.as_ref(),
                &ProcessRunner::new(app.name),
                &exe,
                available,
            )
        });
        match installed {
            Ok(new) => Report::Installed(format!(
                "{}\n{}",
                say(
                    texts,
                    "update-done",
                    &[
                        ("old", v(app.current)),
                        ("new", v(new)),
                        ("notes", notes.to_string()),
                    ],
                ),
                texts.plain("update-applies-next-start")
            )),
            Err(e) => Report::Warning(say(
                texts,
                "update-install-failed",
                &[("new", v(version)), ("error", describe(texts, &e))],
            )),
        }
    }

    /// Records the attempt; a record that cannot be written only means the next check comes sooner.
    fn save(&self, result: RecordResult) {
        let record = CheckRecord {
            at: self.now,
            by: print_app().current,
            result,
        };
        if let Some(folder) = self.record.parent() {
            let _ = std::fs::create_dir_all(folder);
        }
        let _ = save_record(&self.record, &record);
    }
}

/// The MCP server's and the editor's daily check (spec 3.7, P4): due and recorded as in 3.3, on a
/// thread of its own. It prints nothing itself and never installs, whatever the mode (D14).
/// `tell` gets the note once the check finds a newer version, or at once when the record of an
/// earlier check knows one. A release that needs a manual install gets a note that says so, with
/// its link, and one that does not verify a note that nothing was installed (spec 3.2). The
/// wording follows `texts`: `Texts::for_agent` gives the server's.
pub fn check_for_server(
    ctx: &CheckContext,
    texts: Texts,
    tell: impl FnOnce(String) + Send + 'static,
) {
    if let Some(check) = start_background_check(ctx) {
        let _ = std::thread::Builder::new()
            .name("update note".into())
            .spawn(move || {
                let note = match check.settle() {
                    Some(Ok(Outcome::Available { version, .. })) => Some(notice(&texts, version)),
                    Some(Err(UpdateError::TooOldToSelfUpdate {
                        needed,
                        version,
                        notes,
                    })) => Some(say(
                        &texts,
                        "update-note-manual",
                        &[
                            ("new", v(version)),
                            ("old", v(print_app().current)),
                            ("needed", v(needed)),
                            ("notes", notes),
                        ],
                    )),
                    Some(Err(unverified @ (UpdateError::Signature | UpdateError::UnknownKey))) => {
                        Some(bad_signature_note(&texts, &unverified))
                    }
                    _ => None,
                };
                if let Some(note) = note {
                    tell(note);
                }
            });
    } else if let Some((version, _)) = recorded_update(ctx) {
        tell(notice(&texts, version));
    }
}

/// A newer version than this one, with its release notes link, that the record of an earlier
/// check holds, under the same conditions as the check itself (apart from the 24 hours). Reads
/// the record only.
pub fn recorded_update(ctx: &CheckContext) -> Option<(Version, String)> {
    if !ctx.release || ctx.ci || ctx.mode == Mode::Off {
        return None;
    }
    let record = load_record(ctx.record.as_ref()?)?;
    match record.result {
        RecordResult::Available { version, notes } if version > print_app().current => {
            Some((version, notes))
        }
        _ => None,
    }
}

fn notice(texts: &Texts, version: Version) -> String {
    say(
        texts,
        "update-notice",
        &[("new", v(version)), ("old", v(print_app().current))],
    )
}

fn bad_signature_note(texts: &Texts, error: &UpdateError) -> String {
    say(texts, "update-note-bad-signature", &[("error", v(error))])
}

/// Deletes the stale files an update left next to the program (spec 3.5 step 7), silently. Only
/// a release build, which is the only one that updates itself.
pub fn remove_stale_files() {
    if release_build()
        && let Ok(exe) = exe_path()
    {
        remove_stale(&exe);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::Lang;

    const MANIFEST: &[u8] = include_bytes!("../../tests/fixtures/update/manifest-99.json");
    const SIGNATURE: &[u8] = include_bytes!("../../tests/fixtures/update/manifest-99.json.minisig");
    const NO_ASSET: &[u8] = include_bytes!("../../tests/fixtures/update/manifest-99-noasset.json");
    const NO_ASSET_SIGNATURE: &[u8] =
        include_bytes!("../../tests/fixtures/update/manifest-99-noasset.json.minisig");
    const MANUAL: &[u8] = include_bytes!("../../tests/fixtures/update/manifest-99-manual.json");
    const MANUAL_SIGNATURE: &[u8] =
        include_bytes!("../../tests/fixtures/update/manifest-99-manual.json.minisig");

    /// Serves one signed manifest after `delay`, or fails every request; downloads nothing.
    struct FakeSource {
        manifest: Vec<u8>,
        signature: &'static [u8],
        serve: bool,
        delay: Duration,
    }

    impl FakeSource {
        fn release() -> Self {
            Self::of(MANIFEST, SIGNATURE)
        }

        fn of(manifest: &[u8], signature: &'static [u8]) -> Self {
            Self {
                manifest: manifest.to_vec(),
                signature,
                serve: true,
                delay: Duration::ZERO,
            }
        }

        fn failing() -> Self {
            Self {
                serve: false,
                ..Self::release()
            }
        }
    }

    impl UpdateSource for FakeSource {
        fn get(&self, url: &str, _max_bytes: u64) -> Result<Vec<u8>, UpdateError> {
            std::thread::sleep(self.delay);
            match (self.serve, url.ends_with(".minisig")) {
                (false, _) => Err(UpdateError::Download(format!("{url}: 503"))),
                (true, false) => Ok(self.manifest.clone()),
                (true, true) => Ok(self.signature.to_vec()),
            }
        }

        fn download_to(
            &self,
            url: &str,
            _size: u64,
            _file: &mut std::fs::File,
        ) -> Result<(), UpdateError> {
            panic!("the check downloads nothing: {url}")
        }
    }

    fn texts() -> Texts {
        Texts::new(&Lang::english())
    }

    /// A due check in mode `notify` against `source`, recording in `cache` (a folder of the test).
    fn context_with(cache: &std::path::Path, source: FakeSource) -> CheckContext {
        CheckContext {
            record: Some(cache.join("update-check.json")),
            ci: false,
            mode: Mode::Notify,
            release: true,
            now: 1_800_000_000,
            source: Arc::new(source),
        }
    }

    fn context(cache: &std::path::Path, serve: bool) -> CheckContext {
        context_with(
            cache,
            if serve {
                FakeSource::release()
            } else {
                FakeSource::failing()
            },
        )
    }

    fn record_of(cache: &std::path::Path) -> Option<RecordResult> {
        load_record(&cache.join("update-check.json")).map(|record| record.result)
    }

    fn notice() -> String {
        format!(
            "freemapper-print 99.0.0 is available (you have {}). Update with: freemapper-print update",
            env!("CARGO_PKG_VERSION")
        )
    }

    #[test]
    fn no_check_when_not_due() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context(dir.path(), true);
        let recent = CheckRecord {
            at: ctx.now - 23 * 3600,
            by: print_app().current,
            result: RecordResult::Current,
        };
        save_record(&dir.path().join("update-check.json"), &recent).unwrap();
        assert!(start_background_check(&ctx).is_none());
        // A day later it is due again.
        let ctx = CheckContext {
            now: ctx.now + 2 * 3600,
            ..ctx
        };
        assert!(start_background_check(&ctx).is_some());
    }

    #[test]
    fn no_check_with_ci_env() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = CheckContext {
            ci: true,
            ..context(dir.path(), true)
        };
        assert!(start_background_check(&ctx).is_none());
        assert_eq!(record_of(dir.path()), None);
    }

    #[test]
    fn no_check_in_mode_off_in_a_source_build_or_without_a_record_path() {
        let dir = tempfile::tempdir().unwrap();
        for ctx in [
            CheckContext {
                mode: Mode::Off,
                ..context(dir.path(), true)
            },
            CheckContext {
                release: false,
                ..context(dir.path(), true)
            },
            CheckContext {
                record: None,
                ..context(dir.path(), true)
            },
        ] {
            assert!(start_background_check(&ctx).is_none());
        }
        assert_eq!(record_of(dir.path()), None);
    }

    #[test]
    fn a_failed_check_is_recorded_and_not_shown() {
        let dir = tempfile::tempdir().unwrap();
        // The record's folder does not exist yet; saving creates it.
        let cache = dir.path().join("cache");
        let check = start_background_check(&context(&cache, false)).unwrap();
        assert_eq!(check.finish(&texts(), true), None);
        let record = load_record(&cache.join("update-check.json")).unwrap();
        assert_eq!(record.at, 1_800_000_000);
        assert_eq!(record.by, print_app().current);
        let RecordResult::Failed { reason } = record.result else {
            panic!("{:?}", record.result);
        };
        assert!(reason.contains("503"), "{reason}");
    }

    /// Review focus 3: a source that is slower than 3 s is dropped and recorded as failed.
    #[test]
    fn slow_source_is_dropped_after_3_s_and_recorded_failed() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context_with(
            dir.path(),
            FakeSource {
                delay: Duration::from_secs(5),
                ..FakeSource::release()
            },
        );
        let started = Instant::now();
        let check = start_background_check(&ctx).unwrap();
        assert_eq!(check.finish(&texts(), true), None);
        let took = started.elapsed();
        assert!(
            took >= Duration::from_millis(2900) && took < Duration::from_millis(3500),
            "{took:?}"
        );
        assert_eq!(
            record_of(dir.path()),
            Some(RecordResult::Failed {
                reason: "timeout".into()
            })
        );
        // Recorded: the next check is a day away.
        assert!(start_background_check(&ctx).is_none());
    }

    /// Review focus 5: a release without a program for this system is no news for the person.
    #[test]
    fn no_asset_is_recorded_failed_and_not_shown() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context_with(dir.path(), FakeSource::of(NO_ASSET, NO_ASSET_SIGNATURE));
        let check = start_background_check(&ctx).unwrap();
        assert_eq!(check.finish(&texts(), true), None);
        let Some(RecordResult::Failed { reason }) = record_of(dir.path()) else {
            panic!("{:?}", record_of(dir.path()));
        };
        assert!(
            reason.contains("platform") || reason.contains("asset"),
            "{reason}"
        );
    }

    /// Review focus 4: a record that is not JSON, or from the future (the clock went back), counts as
    /// due and is replaced.
    #[test]
    fn a_garbage_or_future_record_is_due_and_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("update-check.json");
        let ctx = context(dir.path(), true);
        let future = CheckRecord {
            at: ctx.now + 86_400,
            by: print_app().current,
            result: RecordResult::Current,
        };
        for before in [
            "not json".to_string(),
            serde_json::to_string(&future).unwrap(),
        ] {
            std::fs::write(&file, before).unwrap();
            let check = start_background_check(&ctx).expect("due");
            assert!(check.finish(&texts(), true).is_some());
            assert_eq!(load_record(&file).unwrap().at, ctx.now);
        }
    }

    #[test]
    fn a_bad_signature_is_a_warning_and_recorded_failed() {
        let dir = tempfile::tempdir().unwrap();
        // The manifest changed after it was signed. Mode `install` installs nothing that does not
        // verify; it warns like `notify`.
        let mut changed = MANIFEST.to_vec();
        changed.push(b'\n');
        for mode in [Mode::Notify, Mode::Install] {
            let ctx = CheckContext {
                mode,
                ..context_with(dir.path(), FakeSource::of(&changed, SIGNATURE))
            };
            let check = start_background_check(&ctx).unwrap();
            assert_eq!(
                check.finish(&texts(), true),
                Some(Report::Warning(
                    "Warning: the newest freemapper-print release could not be verified (the \
                     update signature is not valid). Nothing was installed."
                        .into()
                ))
            );
            assert_eq!(
                record_of(dir.path()),
                Some(RecordResult::Failed {
                    reason: "the update signature is not valid".into()
                })
            );
            // Recorded: the next check is a day away.
            assert!(start_background_check(&ctx).is_none());
            std::fs::remove_file(dir.path().join("update-check.json")).unwrap();
        }
    }

    #[test]
    fn an_available_update_is_recorded_and_noticed() {
        let dir = tempfile::tempdir().unwrap();
        // Mode `notify` tells after any command; mode `install` tells after a failed one.
        for (mode, command_ok) in [
            (Mode::Notify, true),
            (Mode::Notify, false),
            (Mode::Install, false),
        ] {
            let ctx = CheckContext {
                mode,
                ..context(dir.path(), true)
            };
            let check = start_background_check(&ctx).unwrap();
            assert_eq!(
                check.finish(&texts(), command_ok),
                Some(Report::Notice(notice()))
            );
            assert_eq!(
                record_of(dir.path()),
                Some(RecordResult::Available {
                    version: "99.0.0".parse().unwrap(),
                    notes: "https://github.com/HenryJobst/freemapper-print/releases/tag/v99.0.0"
                        .into(),
                })
            );
            std::fs::remove_file(dir.path().join("update-check.json")).unwrap();
        }
    }

    /// Spec 3.4 and P5: mode `install` does not install after a command that failed.
    #[test]
    fn install_mode_skips_after_failed_command() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = CheckContext {
            mode: Mode::Install,
            ..context(dir.path(), true)
        };
        // The source panics on a download, so a try to install would fail this test.
        let check = start_background_check(&ctx).unwrap();
        assert_eq!(
            check.finish(&texts(), false),
            Some(Report::Notice(notice()))
        );
    }

    /// D14: the server tells the person and never installs, whatever the mode.
    #[test]
    fn server_check_never_installs_in_install_mode() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = CheckContext {
            mode: Mode::Install,
            ..context(dir.path(), true)
        };
        let (sender, receiver) = std::sync::mpsc::channel();
        check_for_server(&ctx, Texts::for_agent(&Lang::english()), move |note| {
            let _ = sender.send(note);
        });
        let note = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(
            note.contains("99.0.0") && note.contains("install_update"),
            "{note}"
        );
        assert_eq!(
            record_of(dir.path()),
            Some(RecordResult::Available {
                version: "99.0.0".parse().unwrap(),
                notes: "https://github.com/HenryJobst/freemapper-print/releases/tag/v99.0.0".into(),
            })
        );
        // The record of that check tells at once next time, without a second check.
        let (sender, receiver) = std::sync::mpsc::channel();
        check_for_server(&ctx, Texts::for_agent(&Lang::english()), move |note| {
            let _ = sender.send(note);
        });
        assert!(receiver.recv_timeout(Duration::from_secs(5)).is_ok());
    }

    #[test]
    fn server_notes_for_a_manual_install_and_a_bad_signature() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context_with(dir.path(), FakeSource::of(MANUAL, MANUAL_SIGNATURE));
        let (sender, receiver) = std::sync::mpsc::channel();
        check_for_server(&ctx, Texts::for_agent(&Lang::english()), move |note| {
            let _ = sender.send(note);
        });
        let note = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(
            note.contains("98.0.0") && note.contains("by hand"),
            "{note}"
        );
        std::fs::remove_file(dir.path().join("update-check.json")).unwrap();

        let mut changed = MANIFEST.to_vec();
        changed.push(b'\n');
        let ctx = context_with(dir.path(), FakeSource::of(&changed, SIGNATURE));
        let (sender, receiver) = std::sync::mpsc::channel();
        check_for_server(&ctx, Texts::for_agent(&Lang::english()), move |note| {
            let _ = sender.send(note);
        });
        let note = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(note.contains("could not be verified"), "{note}");
    }

    #[test]
    fn the_note_is_worded_for_the_reader() {
        let lang = Lang::english();
        let args = || [("new", v("99.0.0")), ("old", v("0.1.0"))];
        let cli = say(&Texts::new(&lang), "update-notice", &args());
        assert!(cli.contains("freemapper-print update"), "{cli}");
        assert!(!cli.contains("install_update"), "{cli}");
        let agent = say(&Texts::for_agent(&lang), "update-notice", &args());
        assert!(agent.contains("install_update"), "{agent}");
        assert!(!agent.contains("freemapper-print update"), "{agent}");
    }

    #[test]
    fn recorded_update_reads_only_a_newer_version() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context(dir.path(), true);
        assert_eq!(recorded_update(&ctx), None);
        let file = dir.path().join("update-check.json");
        let record = |version: &str| CheckRecord {
            at: ctx.now,
            by: print_app().current,
            result: RecordResult::Available {
                version: version.parse().unwrap(),
                notes: "notes".into(),
            },
        };
        save_record(&file, &record("99.0.0")).unwrap();
        assert_eq!(
            recorded_update(&ctx),
            Some(("99.0.0".parse().unwrap(), "notes".into()))
        );
        save_record(&file, &record("0.0.1")).unwrap();
        assert_eq!(recorded_update(&ctx), None);
        let off = CheckContext {
            mode: Mode::Off,
            ..ctx
        };
        save_record(&file, &record("99.0.0")).unwrap();
        assert_eq!(recorded_update(&off), None);
    }
}
