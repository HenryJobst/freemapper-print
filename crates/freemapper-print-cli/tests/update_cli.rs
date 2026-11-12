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
/// with both folders (settings, record) inside the target folder instead of the home of the user.
fn update_of(program: &std::path::Path) -> Command {
    let scratch = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("update_cli");
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
    let program = std::path::Path::new(PROGRAM);
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
    let output = update_with(std::path::Path::new(PROGRAM), &stub)
        .arg("--yes")
        .output()
        .unwrap();
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
