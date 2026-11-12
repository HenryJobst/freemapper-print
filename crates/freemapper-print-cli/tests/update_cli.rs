//! `freemapper-print update` (spec 3.4 to 3.6, ADR 0060). The tests that reach the update code need a
//! build with the `test-stub` feature and talk to the stub of `tests/support`:
//! `cargo test -p freemapper-print-cli --features test-stub`.

use std::process::Command;

#[cfg(all(feature = "test-stub", debug_assertions))]
mod support;
#[cfg(all(unix, feature = "test-stub", debug_assertions))]
use support::installable_release;
#[cfg(all(feature = "test-stub", debug_assertions))]
use support::{MANIFEST_PATH, UpdateStub, copied_program};

const PROGRAM: &str = env!("CARGO_BIN_EXE_freemapper-print");

/// `freemapper-print update` of `program` in English, without the update switches of the environment and
/// with both folders (settings, record) in a folder of this test (the thread's, removed when the test ends)
/// instead of the home of the user or a folder the tests share.
fn update_of(program: &std::path::Path) -> Command {
    thread_local! {
        static SCRATCH: tempfile::TempDir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    }
    let scratch = SCRATCH.with(|dir| dir.path().to_path_buf());
    let mut command = Command::new(program);
    command
        .env("FREEMAPPER_LANG", "en")
        .env_remove("CI")
        .env_remove("FREEMAPPER_PRINT_UPDATE")
        .env_remove("FREEMAPPER_PRINT_UPDATE_TEST_BASE")
        .env("FREEMAPPER_PRINT_CONFIG_DIR", scratch.join("config"))
        .env("FREEMAPPER_PRINT_CACHE_DIR", scratch.join("cache"))
        .arg("update");
    command
}

#[test]
fn update_in_source_build_says_how_to_update() {
    for extra in [&[][..], &["--check"], &["--yes"], &["--rollback"]] {
        let output = update_of(std::path::Path::new(PROGRAM))
            .args(extra)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{extra:?}: {output:?}");
        let stdout = String::from_utf8(output.stdout).unwrap();
        let text = stdout.trim_end();
        assert!(
            text.contains("https://github.com/HenryJobst/freemapper-print/releases/latest"),
            "{extra:?}: {text}"
        );
        assert!(
            text.contains("git pull and cargo build --release --locked"),
            "{extra:?}: {text}"
        );
        assert!(text.contains("built from source"), "{extra:?}: {text}");
    }
}

#[test]
fn update_mode_writes_settings() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    let set = |mode: &str| {
        let output = update_of(std::path::Path::new(PROGRAM))
            .env("FREEMAPPER_PRINT_CONFIG_DIR", &config)
            .args(["--mode", mode])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(
            stdout.contains(&format!("Update mode set to {mode}")),
            "{stdout}"
        );
        std::fs::read_to_string(config.join("settings.toml")).unwrap()
    };
    assert!(set("install").contains("mode = \"install\""));
    assert!(set("off").contains("mode = \"off\""));
    // An unknown mode is a usage error and leaves the file alone.
    let output = update_of(std::path::Path::new(PROGRAM))
        .env("FREEMAPPER_PRINT_CONFIG_DIR", &config)
        .args(["--mode", "sometimes"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(
        std::fs::read_to_string(config.join("settings.toml"))
            .unwrap()
            .contains("mode = \"off\"")
    );
    // The environment overrides the stored mode, and the command says so.
    let output = update_of(std::path::Path::new(PROGRAM))
        .env("FREEMAPPER_PRINT_CONFIG_DIR", &config)
        .env("FREEMAPPER_PRINT_UPDATE", "install")
        .args(["--mode", "off"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("FREEMAPPER_PRINT_UPDATE=install overrides"),
        "{stdout}"
    );
}

/// `freemapper-print update` of `program`, talking to `stub`.
#[cfg(all(feature = "test-stub", debug_assertions))]
fn update_with(program: &std::path::Path, stub: &UpdateStub) -> Command {
    let mut command = update_of(program);
    command.env("FREEMAPPER_PRINT_UPDATE_TEST_BASE", &stub.base);
    command
}

#[cfg(all(feature = "test-stub", debug_assertions))]
fn fixtures() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/update")
}

#[test]
#[cfg(all(feature = "test-stub", debug_assertions))]
fn update_check_with_stub_reports_available_exit_10() {
    let stub = UpdateStub::with_fixture();
    let output = update_with(std::path::Path::new(PROGRAM), &stub)
        .arg("--check")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(10), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let current = env!("CARGO_PKG_VERSION");
    for expected in [
        format!("freemapper-print 99.0.0 is available (you have {current}), released 2026-11-20."),
        "Release notes: https://github.com/HenryJobst/freemapper-print/releases/tag/v99.0.0"
            .to_string(),
        // 12345678 bytes.
        "Download: 11.8 MB".to_string(),
    ] {
        assert!(stdout.contains(&expected), "{expected}\n{stdout}");
    }
    assert_eq!(
        stub.paths(),
        [
            MANIFEST_PATH.to_string(),
            format!("{MANIFEST_PATH}.minisig")
        ]
    );
}

#[test]
#[cfg(all(feature = "test-stub", debug_assertions))]
fn update_check_with_a_bad_signature_is_an_error() {
    let dir = fixtures();
    let mut manifest = std::fs::read(dir.join("manifest-99.json")).unwrap();
    manifest.extend_from_slice(b"\n");
    let stub = UpdateStub::serving(
        manifest,
        std::fs::read(dir.join("manifest-99.json.minisig")).unwrap(),
        Vec::new(),
    );
    let output = update_with(std::path::Path::new(PROGRAM), &stub)
        .arg("--check")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains(
            "Error: the release could not be verified (the update signature is not valid); nothing was installed"
        ),
        "{stderr}"
    );
}

/// A release without `freemapper-print-update.json` (the manifest URL answers 404) cannot install
/// itself; the message says so and links the release page, in the language of the run.
#[test]
#[cfg(all(feature = "test-stub", debug_assertions))]
fn update_of_a_release_without_a_manifest_says_so() {
    let stub = UpdateStub::start(Vec::new());
    let folder = tempfile::tempdir().unwrap();
    let program = copied_program(folder.path());
    let program = program.as_path();
    let no_manifest = "Error: the newest release cannot install itself (it has no update manifest); \
                       download it from https://github.com/HenryJobst/freemapper-print/releases/latest\n";
    for extra in [&["--check"][..], &["--yes"]] {
        let output = update_with(program, &stub).args(extra).output().unwrap();
        assert_eq!(output.status.code(), Some(1), "{extra:?}: {output:?}");
        assert_eq!(String::from_utf8(output.stderr).unwrap(), no_manifest);
    }
    let output = update_with(program, &stub)
        .env("FREEMAPPER_LANG", "de")
        .arg("--check")
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "Fehler: Die neueste Version kann sich nicht selbst installieren (ihr fehlt das \
         Aktualisierungsmanifest). Laden Sie sie von \
         https://github.com/HenryJobst/freemapper-print/releases/latest herunter.\n"
    );
    // A release asked for by its version: it may not exist at all.
    let output = update_with(program, &stub)
        .args(["--version", "0.9.9"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "Error: release 0.9.9 does not exist or cannot install itself (it has no update manifest); \
         see https://github.com/HenryJobst/freemapper-print/releases\n"
    );
    assert!(!stub.paths().iter().any(|path| path.ends_with(".minisig")));
}

/// Only the manifest's 404 means "no manifest": a missing signature next to it is another error.
#[test]
#[cfg(all(feature = "test-stub", debug_assertions))]
fn update_with_a_missing_signature_is_no_missing_manifest() {
    let stub = UpdateStub::start(vec![(
        MANIFEST_PATH.to_string(),
        std::fs::read(fixtures().join("manifest-99.json")).unwrap(),
    )]);
    let output = update_with(std::path::Path::new(PROGRAM), &stub)
        .arg("--check")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.starts_with("Error: the update failed: "), "{stderr}");
    assert!(stderr.contains(".minisig: HTTP 404"), "{stderr}");
}

/// Spec 3.3: `--check` exits 10 whenever a newer release exists, also one that needs a manual
/// install; the message links its release page.
#[test]
#[cfg(all(feature = "test-stub", debug_assertions))]
fn update_check_of_a_release_that_needs_a_manual_install_exits_10() {
    let dir = fixtures();
    let stub = UpdateStub::serving(
        std::fs::read(dir.join("manifest-99-manual.json")).unwrap(),
        std::fs::read(dir.join("manifest-99-manual.json.minisig")).unwrap(),
        Vec::new(),
    );
    let output = update_with(std::path::Path::new(PROGRAM), &stub)
        .arg("--check")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(10), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains(&format!(
            "freemapper-print 99.0.0 cannot be installed by the self-update of {}; install 98.0.0 or newer by hand, see https://github.com/HenryJobst/freemapper-print/releases/tag/v99.0.0",
            env!("CARGO_PKG_VERSION")
        )),
        "{stdout}"
    );
}

/// Review focus 5: a release without a program for this system names the system and the releases
/// page, and exits 1.
#[test]
#[cfg(all(feature = "test-stub", debug_assertions))]
fn update_without_an_asset_for_this_platform_names_it() {
    let dir = fixtures();
    let stub = UpdateStub::serving(
        std::fs::read(dir.join("manifest-99-noasset.json")).unwrap(),
        std::fs::read(dir.join("manifest-99-noasset.json.minisig")).unwrap(),
        Vec::new(),
    );
    let folder = tempfile::tempdir().unwrap();
    let program = copied_program(folder.path());
    let output = update_with(&program, &stub).arg("--yes").output().unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    let platform = freemapper_update::current_platform().unwrap();
    assert!(stderr.contains(&format!("({platform})")), "{stderr}");
    assert!(
        stderr.contains("https://github.com/HenryJobst/freemapper-print/releases"),
        "{stderr}"
    );
    assert!(!stub.asked_for_an_asset());
}

#[test]
#[cfg(all(feature = "test-stub", debug_assertions))]
fn update_without_yes_and_no_terminal_refuses() {
    let dir = tempfile::tempdir().unwrap();
    let program = copied_program(dir.path());
    let stub = UpdateStub::with_fixture();
    let output = update_with(&program, &stub)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("freemapper-print 99.0.0 is available"),
        "{stdout}"
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("freemapper-print update --yes"), "{stderr}");
    assert!(!stub.asked_for_an_asset());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

/// Review focus 1: a program folder the user cannot write is named, and nothing is downloaded.
#[test]
#[cfg(all(unix, feature = "test-stub", debug_assertions))]
fn update_in_a_read_only_folder_names_it() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("bin");
    std::fs::create_dir(&folder).unwrap();
    let program = copied_program(&folder);
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o555)).unwrap();
    let stub = UpdateStub::with_fixture();
    let output = update_with(&program, &stub).arg("--yes").output().unwrap();
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    let canonical = std::fs::canonicalize(&folder).unwrap();
    assert!(
        stderr.contains(&format!(
            "cannot write to the program folder {}",
            canonical.display()
        )),
        "{stderr}"
    );
    assert!(!stub.asked_for_an_asset());
    assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 1);
}

/// The whole install against the stub (see `installable_release`). Without the `minisign` CLI
/// the test says so and passes.
#[test]
#[cfg(all(unix, feature = "test-stub", debug_assertions))]
fn update_yes_installs_over_a_copied_binary() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("bin");
    let work = dir.path().join("work");
    std::fs::create_dir(&folder).unwrap();
    std::fs::create_dir(&work).unwrap();
    let program = copied_program(&folder);
    let Some(stub) = installable_release(&work) else {
        return;
    };

    let output = update_with(&program, &stub).arg("--yes").output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let current = env!("CARGO_PKG_VERSION");
    // Progress goes to stdout like the other progress lines.
    assert!(
        stdout.contains("downloading and checking freemapper-print 99.0.0"),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!(
            "freemapper-print updated from {current} to 99.0.0. Release notes: https://github.com/HenryJobst/freemapper-print/releases/tag/v99.0.0"
        )),
        "{stdout}"
    );
    assert!(stdout.contains("MCP server"), "{stdout}");
    let version = |path: &std::path::Path| {
        let output = Command::new(path).arg("--version").output().unwrap();
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    };
    assert_eq!(version(&program), "freemapper-print 99.0.0");
    assert_eq!(
        version(&folder.join("freemapper-print.previous")),
        format!("freemapper-print {current}")
    );
}

#[test]
#[cfg(all(feature = "test-stub", debug_assertions))]
fn update_rollback_restores_previous() {
    let dir = tempfile::tempdir().unwrap();
    let program = copied_program(dir.path());
    let previous = dir.path().join(if cfg!(windows) {
        "freemapper-print.previous.exe"
    } else {
        "freemapper-print.previous"
    });
    std::fs::copy(&program, &previous).unwrap();
    // Both are the same program; the time stamp tells them apart.
    let marked = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000);
    std::fs::File::options()
        .write(true)
        .open(&previous)
        .unwrap()
        .set_modified(marked)
        .unwrap();
    let modified = |path: &std::path::Path| std::fs::metadata(path).unwrap().modified().unwrap();
    let stub = UpdateStub::start(Vec::new());
    let output = update_with(&program, &stub)
        .arg("--rollback")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains(&format!(
            "Went back to freemapper-print {}",
            env!("CARGO_PKG_VERSION")
        )),
        "{stdout}"
    );
    assert_eq!(modified(&program), marked);
    assert_ne!(modified(&previous), marked);
    assert!(stub.paths().is_empty(), "a rollback sends no request");
    // Without a previous version there is nothing to go back to.
    std::fs::remove_file(&previous).unwrap();
    let output = update_with(&program, &stub)
        .arg("--rollback")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("there is no previous version to go back to"),
        "{stderr}"
    );
}

/// The daily check after the commands (spec 3.3, ADR 0060 P3 to P6). Every test has its own folders for the
/// settings, the record and the home of the program, and talks to a stub on 127.0.0.1.
#[cfg(all(feature = "test-stub", debug_assertions))]
mod daily {
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use freemapper_update::{CheckRecord, RecordResult, load_record, save_record};

    use super::{Command, MANIFEST_PATH, UpdateStub, copied_program};

    const PROGRAM: &str = env!("CARGO_BIN_EXE_freemapper-print");
    const NOTES: &str = "https://github.com/HenryJobst/freemapper-print/releases/tag/v99.0.0";

    /// `program` in English, talking to `stub`, without `CI` and the update switches of the environment, with
    /// its settings, its record and its home folder in `dir`.
    fn checking(program: &Path, stub: &UpdateStub, dir: &Path) -> Command {
        let mut command = Command::new(program);
        command
            .env("FREEMAPPER_LANG", "en")
            .env_remove("CI")
            .env_remove("FREEMAPPER_PRINT_UPDATE")
            .env("FREEMAPPER_PRINT_CONFIG_DIR", dir.join("config"))
            .env("FREEMAPPER_PRINT_CACHE_DIR", dir.join("cache"))
            .env("FREEMAPPER_PRINT_UPDATE_TEST_BASE", &stub.base)
            .env("HOME", dir.join("home"))
            .env("XDG_CACHE_HOME", dir.join("home/.cache"));
        command
    }

    fn record(dir: &Path) -> Option<RecordResult> {
        load_record(&dir.join("cache/update-check.json")).map(|record| record.result)
    }

    fn notice() -> String {
        format!(
            "freemapper-print 99.0.0 is available (you have {}). Update with: freemapper-print update\n",
            env!("CARGO_PKG_VERSION")
        )
    }

    /// The Viktoriapark example in `dir/project`, with `extra` appended to the project file (which ends in the
    /// assignment of the course "Training"). Returns the project file.
    fn viktoriapark(dir: &Path, extra: &str) -> PathBuf {
        let example = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/berlin-viktoriapark-training");
        let project = dir.join("project");
        std::fs::create_dir_all(&project).unwrap();
        for name in [
            "freemapper-print.toml",
            "berlin-viktoriapark.omap",
            "freemapper.toml",
            "training.ppen",
        ] {
            std::fs::copy(example.join(name), project.join(name)).unwrap();
        }
        let file = project.join("freemapper-print.toml");
        let text = std::fs::read_to_string(&file).unwrap();
        std::fs::write(&file, format!("{text}{extra}")).unwrap();
        file
    }

    /// A mask over the whole map: `check` then has notes and exits 1.
    #[cfg(unix)]
    const MASK_ALL: &str = "mask = \"all\"\n[[masks]]\nid = \"all\"\npolygons = \
        [[[[-1000000, -1000000], [1000000, -1000000], [1000000, 1000000], [-1000000, 1000000]]]]\n";

    #[test]
    fn notice_line_on_stderr_after_command_output() {
        let dir = tempfile::tempdir().unwrap();
        let project = viktoriapark(dir.path(), "");
        let stub = UpdateStub::with_fixture();
        let without = checking(Path::new(PROGRAM), &stub, dir.path())
            .env("CI", "1")
            .arg("check")
            .arg(&project)
            .output()
            .unwrap();
        assert!(stub.paths().is_empty(), "no check when CI is set");
        let with = checking(Path::new(PROGRAM), &stub, dir.path())
            .arg("check")
            .arg(&project)
            .output()
            .unwrap();
        assert_eq!(with.status.code(), Some(0), "{with:?}");
        assert_eq!(with.status.code(), without.status.code());
        assert_eq!(with.stdout, without.stdout);
        let before = String::from_utf8(without.stderr).unwrap();
        let stderr = String::from_utf8(with.stderr).unwrap();
        assert_eq!(stderr, format!("{before}{}", notice()));
        assert_eq!(
            record(dir.path()),
            Some(RecordResult::Available {
                version: "99.0.0".parse().unwrap(),
                notes: NOTES.into(),
            })
        );
    }

    #[test]
    fn no_second_check_within_24_h() {
        let dir = tempfile::tempdir().unwrap();
        let project = viktoriapark(dir.path(), "");
        let stub = UpdateStub::with_fixture();
        for round in 0..2 {
            let output = checking(Path::new(PROGRAM), &stub, dir.path())
                .arg("check")
                .arg(&project)
                .output()
                .unwrap();
            let stderr = String::from_utf8(output.stderr).unwrap();
            // The first run asks (manifest and signature) and tells; the second neither asks nor tells.
            assert_eq!(stderr.contains("99.0.0"), round == 0, "{stderr}");
            assert_eq!(stub.paths().len(), 2, "round {round}");
        }
    }

    #[test]
    fn numbers_and_masks_do_not_check() {
        let dir = tempfile::tempdir().unwrap();
        let project = viktoriapark(dir.path(), "");
        let stub = UpdateStub::with_fixture();
        let masks = checking(Path::new(PROGRAM), &stub, dir.path())
            .args(["masks", "window"])
            .arg(&project)
            .args(["--course", "Training"])
            .output()
            .unwrap();
        assert_eq!(masks.status.code(), Some(0), "{masks:?}");
        let numbers = checking(Path::new(PROGRAM), &stub, dir.path())
            .args(["numbers", "accept"])
            .arg(&project)
            .args([
                "--course",
                "Training",
                "--control",
                "34",
                "--layout-wide",
                "--direction",
                "90",
                "--gap",
                "2",
            ])
            .output()
            .unwrap();
        assert_eq!(numbers.status.code(), Some(0), "{numbers:?}");
        assert!(stub.paths().is_empty(), "{:?}", stub.paths());
        assert_eq!(record(dir.path()), None);
    }

    /// Review focus 3: a host that never answers costs at most 3 s (3.5 s with the margin of a slow machine),
    /// leaves output and exit code as they are and is recorded as failed.
    #[test]
    fn export_against_a_silent_stub_ends_within_the_budget() {
        let dir = tempfile::tempdir().unwrap();
        let project = viktoriapark(dir.path(), "");
        let out = dir.path().join("pdf");
        std::fs::create_dir(&out).unwrap();
        let export = |program: &Path, stub: &UpdateStub, ci: bool| {
            let mut command = checking(program, stub, dir.path());
            if ci {
                command.env("CI", "1");
            }
            let started = Instant::now();
            let output = command
                .arg("export")
                .arg(&project)
                .arg("--out-dir")
                .arg(&out)
                .output()
                .unwrap();
            (output, started.elapsed())
        };
        let stub = UpdateStub::gated_fixture();
        let (without, own) = export(Path::new(PROGRAM), &stub, true);
        assert_eq!(without.status.code(), Some(0), "{without:?}");
        assert!(stub.paths().is_empty());
        let (with, took) = export(Path::new(PROGRAM), &stub, false);
        assert_eq!(with.status.code(), Some(0), "{with:?}");
        assert_eq!(with.stdout, without.stdout);
        assert_eq!(with.stderr, without.stderr);
        assert!(
            took < own + Duration::from_millis(3500),
            "{took:?} against {own:?} without the check"
        );
        assert_eq!(
            record(dir.path()),
            Some(RecordResult::Failed {
                reason: "timeout".into()
            })
        );
        assert_eq!(stub.paths(), [MANIFEST_PATH]);
    }

    /// P5: mode `install` installs after a command that exits 0 and only then.
    #[test]
    #[cfg(unix)]
    fn install_mode_installs_only_after_exit_0() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("work");
        std::fs::create_dir(&work).unwrap();
        let Some(stub) = super::installable_release(&work) else {
            return;
        };
        // `export` exits 0: the install follows the command's own output.
        let ok = dir.path().join("ok");
        let bin = ok.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let program = copied_program(&bin);
        let project = viktoriapark(&ok, "");
        let out = ok.join("pdf");
        std::fs::create_dir(&out).unwrap();
        let output = checking(&program, &stub, &ok)
            .env("FREEMAPPER_PRINT_UPDATE", "install")
            .arg("export")
            .arg(&project)
            .arg("--out-dir")
            .arg(&out)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        let stdout = String::from_utf8(output.stdout).unwrap();
        let lines: Vec<&str> = stdout.lines().collect();
        let position = |start: &str| {
            lines
                .iter()
                .position(|line| line.starts_with(start))
                .unwrap_or_else(|| panic!("{start}\n{stdout}"))
        };
        let current = env!("CARGO_PKG_VERSION");
        assert!(
            position("downloading and checking freemapper-print 99.0.0")
                < position(&format!(
                    "freemapper-print updated from {current} to 99.0.0"
                ))
        );
        assert_eq!(position("downloading and checking"), 0, "{stdout}");
        assert_eq!(
            lines.last(),
            Some(&"The new version applies from the next start of freemapper-print.")
        );
        // `export` writes its own report to stderr, and the install adds nothing there.
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("Wrote "), "{stderr}");
        assert!(!stderr.contains("99.0.0"), "{stderr}");
        let version = Command::new(&program).arg("--version").output().unwrap();
        assert_eq!(
            String::from_utf8(version.stdout).unwrap().trim(),
            "freemapper-print 99.0.0"
        );

        // `check` with notes exits 1: told, not installed.
        let bad = dir.path().join("bad");
        let bin = bad.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let program = copied_program(&bin);
        let project = viktoriapark(&bad, MASK_ALL);
        let asked_before = stub.paths().len();
        let output = checking(&program, &stub, &bad)
            .env("FREEMAPPER_PRINT_UPDATE", "install")
            .arg("check")
            .arg(&project)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.ends_with(&notice()), "{stderr}");
        assert!(
            !String::from_utf8(output.stdout)
                .unwrap()
                .contains("downloading")
        );
        // Only the manifest and its signature were asked for, no program.
        assert_eq!(stub.paths().len(), asked_before + 2);
        assert_eq!(std::fs::read_dir(&bin).unwrap().count(), 1);
        let version = Command::new(&program).arg("--version").output().unwrap();
        assert_eq!(
            String::from_utf8(version.stdout).unwrap().trim(),
            format!("freemapper-print {current}")
        );
    }

    /// Every start deletes the stale files an update left next to the program (spec 3.5 step 7).
    #[test]
    fn stale_files_are_removed_at_start() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let program = copied_program(&bin);
        let stale = bin.join(if cfg!(windows) {
            "freemapper-print.stale-1700000000.exe"
        } else {
            "freemapper-print.stale-1700000000"
        });
        std::fs::write(&stale, "old").unwrap();
        let stub = UpdateStub::start(Vec::new());
        // A build from source leaves them.
        checking(&program, &stub, dir.path())
            .env_remove("FREEMAPPER_PRINT_UPDATE_TEST_BASE")
            .args(["update", "--mode", "notify"])
            .output()
            .unwrap();
        assert!(stale.exists());
        let output = checking(&program, &stub, dir.path())
            .args(["update", "--mode", "notify"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        assert!(!stale.exists());
        assert!(stub.paths().is_empty());
    }

    /// P4: the editor says it once on stderr at its start; the page and the endpoints know nothing of it.
    #[test]
    fn edit_prints_the_notice_on_stderr_at_start() {
        use std::io::{BufRead as _, Read as _, Write as _};

        let dir = tempfile::tempdir().unwrap();
        let project = viktoriapark(dir.path(), "");
        let stub = UpdateStub::with_fixture();
        // An earlier check found 99.0.0 a moment ago.
        let now = i64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("cache")).unwrap();
        save_record(
            &dir.path().join("cache/update-check.json"),
            &CheckRecord {
                at: now,
                by: env!("CARGO_PKG_VERSION").parse().unwrap(),
                result: RecordResult::Available {
                    version: "99.0.0".parse().unwrap(),
                    notes: NOTES.into(),
                },
            },
        )
        .unwrap();
        let mut child = checking(Path::new(PROGRAM), &stub, dir.path())
            .arg("edit")
            .arg(&project)
            .args(["--no-browser", "--port", "0"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        struct Kill(std::process::Child);
        impl Drop for Kill {
            fn drop(&mut self) {
                let _ = self.0.kill();
            }
        }
        let stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();
        let mut child = Kill(child);
        let mut first = String::new();
        let mut stdout = std::io::BufReader::new(stdout);
        stdout.read_line(&mut first).unwrap();
        // Keep reading, so that a later line of the editor never meets a closed pipe.
        let drain = std::thread::spawn(move || {
            let mut rest = String::new();
            let _ = stdout.read_to_string(&mut rest);
        });
        let url = first
            .split_whitespace()
            .find(|word| word.starts_with("http://"))
            .unwrap_or_else(|| panic!("{first}"))
            .trim_end_matches('.');
        assert!(!first.contains("99.0.0"), "{first}");
        let rest = url.strip_prefix("http://").unwrap();
        let (host, path) = rest.split_once('/').unwrap();
        let path = path.trim_end_matches('/');
        let mut stream = std::net::TcpStream::connect(host).unwrap();
        let request = format!(
            "POST /{path}/api/done HTTP/1.1\r\nHost: {host}\r\nOrigin: http://{host}\r\n\
             Content-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}"
        );
        stream.write_all(request.as_bytes()).unwrap();
        let mut answer = String::new();
        let _ = stream.read_to_string(&mut answer);
        assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
        let status = child.0.wait().unwrap();
        assert_eq!(status.code(), Some(0));
        drain.join().unwrap();
        let mut text = String::new();
        stderr.read_to_string(&mut text).unwrap();
        assert_eq!(text, notice());
        // The record was fresh: no request went to the stub.
        assert!(stub.paths().is_empty());
    }
}
