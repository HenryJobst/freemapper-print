//! Safe writing of finished PDFs (ADR 0030, ADR 0041). Free space is checked first. Then
//! `freemapper-fs` writes the bytes to a new partial file next to the target
//! (`<file>.<process id>.<counter>.part`, created new, so no existing entry of that name is used),
//! flushes it to the disk and renames it over the target. A full disk or any other error leaves
//! no half-written PDF and no partial file. A link at the target's name is replaced, never
//! written through (core ADR 0009).

use std::io;
use std::path::{Component, Path, PathBuf};

/// Free space that must remain after the PDF is written, in bytes.
pub const DISK_RESERVE_BYTES: u64 = 50 * 1024 * 1024;

const MB: u64 = 1024 * 1024;

/// Why a PDF could not be written.
#[derive(Debug, thiserror::Error)]
pub enum OutputError {
    /// The volume of `path` has too little room (or filled up while writing).
    #[error(
        "not enough disk space to write {}: {} MB needed, {} available",
        path.display(),
        needed.div_ceil(MB),
        available.map_or_else(|| "an unknown amount".to_string(), |a| format!("{} MB", a / MB))
    )]
    DiskFull {
        /// Bytes needed: the PDF plus [`DISK_RESERVE_BYTES`].
        needed: u64,
        /// Bytes free on the volume; `None` when the volume did not say (after it filled up while
        /// writing).
        available: Option<u64>,
        /// The file that was to be written.
        path: PathBuf,
    },
    /// Any other I/O error. The operating system's error is the `source`, not part of the
    /// message.
    #[error("could not write {}", path.display())]
    Io {
        /// The file that was to be written.
        path: PathBuf,
        /// The underlying error.
        source: io::Error,
    },
}

/// Test hook (feature `test-hooks`): when this environment variable holds a number,
/// [`write_pdf_file`] takes it as the free space in bytes instead of asking the volume. The CLI
/// tests use it to fake a full disk in the binary they run. Builds without the feature never read
/// it.
#[cfg(any(test, feature = "test-hooks"))]
#[doc(hidden)]
pub const FAKE_FREE_SPACE_ENV: &str = "FREEMAPPER_PRINT_TEST_FREE_SPACE";

/// Writes `bytes` to `path` outside a project folder (`render --out`, an `--out-dir` outside the
/// project), replacing an existing file only when the whole PDF is on disk. A link at `path` is
/// replaced by the PDF; the file it pointed to stays as it was. The folder of `path` must exist:
/// a missing one is the operating system's `NotFound` error, and no folder is created.
///
/// Fails with [`OutputError::DiskFull`] when the volume has less than `bytes.len()` plus
/// [`DISK_RESERVE_BYTES`] free. If the free space cannot be determined, the check is skipped and a
/// full disk is still caught while writing.
pub fn write_pdf_file(path: &Path, bytes: &[u8]) -> Result<(), OutputError> {
    write_checked(path, bytes, free_space, freemapper_fs::write_cache_file)
}

/// Writes the PDF `path` of the project in `project_dir`. A `path` whose name lies in the project
/// folder (`out/` below it, or an `--out-dir` there) goes through [`freemapper_fs::write_inside`]:
/// it is refused with [`freemapper_fs::OUTSIDE_PROJECT`] when a link leads its folder out of the
/// project. Any other `path` is written like [`write_pdf_file`]. The folder of `path` must exist,
/// as for [`write_pdf_file`]. Which paths count as in the project folder: see [`names_inside`].
pub fn write_project_pdf(project_dir: &Path, path: &Path, bytes: &[u8]) -> Result<(), OutputError> {
    if names_inside(project_dir, path) {
        write_checked(path, bytes, free_space, |path, bytes| {
            freemapper_fs::write_inside(project_dir, path, bytes)
        })
    } else {
        write_pdf_file(path, bytes)
    }
}

/// Refuses the output folder `folder` of the project in `project_dir` when a link leads it out of
/// the project folder. A folder outside the project ([`names_inside`]), or one that does not exist
/// yet, passes. The commands call it once before they write anything.
pub fn check_out_folder(project_dir: &Path, folder: &Path) -> Result<(), OutputError> {
    if !names_inside(project_dir, &folder.join("file")) {
        return Ok(());
    }
    freemapper_fs::check_folder_inside(project_dir, folder)
        .map_err(|(path, source)| OutputError::Io { path, source })
}

/// Removes partial files older than an hour that interrupted runs left in the output folder
/// `folder` of the project in `project_dir`. Only the project's own `out` folder is cleaned, not any other
/// folder inside the project, and only when no link leads it out; partial files that are links stay.
/// Best effort.
pub fn remove_stale_parts(project_dir: &Path, folder: &Path) {
    const STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(60 * 60);
    let is_out = match (
        std::path::absolute(project_dir.join("out")),
        std::path::absolute(folder),
    ) {
        (Ok(out), Ok(folder)) => by_name(&out) == by_name(&folder),
        _ => false,
    };
    if is_out && names_inside(project_dir, &folder.join("file")) {
        let _ = freemapper_fs::remove_stale_parts_inside(project_dir, folder, STALE_AFTER);
    }
}

/// Whether `error` is the refusal of a write whose folder a link leads out of the project folder
/// ([`freemapper_fs::OUTSIDE_PROJECT`]).
pub fn is_outside_project(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::Other && error.to_string() == freemapper_fs::OUTSIDE_PROJECT
}

/// Whether `path` is a file of the project in `root`, so that [`freemapper_fs::write_inside`] must
/// write it. That is so when `path` lies in `root` by its name (both made absolute, `.` dropped and
/// `..` taken back by name), or when it does once the nearest existing folder above the folder of
/// `path` is resolved: an `--out-dir` that names the project folder by another link to it. The
/// folder of `path` itself is not resolved, so a link there that leads out of the project still
/// counts as inside and `write_inside` refuses it.
fn names_inside(root: &Path, path: &Path) -> bool {
    let (Ok(root), Ok(path)) = (std::path::absolute(root), std::path::absolute(path)) else {
        return false;
    };
    if by_name(&path).starts_with(by_name(&root)) {
        return true;
    }
    let (Ok(root), Some(above)) = (
        std::fs::canonicalize(&root),
        path.parent().and_then(Path::parent),
    ) else {
        return false;
    };
    above
        .ancestors()
        .find_map(|folder| Some((std::fs::canonicalize(folder).ok()?, folder)))
        .and_then(|(resolved, folder)| Some(resolved.join(path.strip_prefix(folder).ok()?)))
        .is_some_and(|resolved| by_name(&resolved).starts_with(&root))
}

/// `path` with `.` dropped and each `..` taking back the name before it.
fn by_name(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// The free space of the volume of `folder`, or the fake value of the test hook.
fn free_space(folder: &Path) -> io::Result<u64> {
    #[cfg(any(test, feature = "test-hooks"))]
    if let Some(bytes) = std::env::var(FAKE_FREE_SPACE_ENV)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
    {
        return Ok(bytes);
    }
    fs4::available_space(folder)
}

/// The writes of this module with the free-space query and the `freemapper-fs` write replaceable
/// for tests. Our check of the free space stays before the write (ADR 0030); the partial file and
/// the rename are `write`'s.
fn write_checked(
    path: &Path,
    bytes: &[u8],
    available_space: impl Fn(&Path) -> io::Result<u64>,
    write: impl FnOnce(&Path, &[u8]) -> Result<(), (PathBuf, io::Error)>,
) -> Result<(), OutputError> {
    let needed = bytes.len() as u64 + DISK_RESERVE_BYTES;
    let folder = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let query = || available_space(folder).ok();
    let disk_full = |available: Option<u64>| OutputError::DiskFull {
        needed,
        available,
        path: path.to_path_buf(),
    };
    if let Some(available) = query()
        && available < needed
    {
        return Err(disk_full(Some(available)));
    }
    // `freemapper-fs` would create a missing folder; the callers create theirs themselves.
    if let Err(source) = std::fs::metadata(folder) {
        return Err(OutputError::Io {
            path: path.to_path_buf(),
            source,
        });
    }
    // A failed write has removed its partial file, so the free space counts without it.
    write(path, bytes).map_err(|(path, source)| {
        if source.kind() == io::ErrorKind::StorageFull {
            disk_full(query())
        } else {
            OutputError::Io { path, source }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn leftovers(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// The real write of `freemapper-fs`, outside a project.
    fn core_write(path: &Path, bytes: &[u8]) -> Result<(), (PathBuf, io::Error)> {
        freemapper_fs::write_cache_file(path, bytes)
    }

    #[test]
    fn writes_via_temp_and_rename() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("map.pdf");
        fs::write(&target, b"old").unwrap();
        write_pdf_file(&target, b"%PDF new").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"%PDF new");
        assert_eq!(leftovers(dir.path()), ["map.pdf"]);
    }

    #[test]
    fn refuses_when_space_is_short() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("map.pdf");
        let bytes = [0u8; 1000];
        let err = write_checked(
            &target,
            &bytes,
            |_| Ok(1000 + DISK_RESERVE_BYTES - 1),
            core_write,
        )
        .unwrap_err();
        match &err {
            OutputError::DiskFull {
                needed, available, ..
            } => {
                assert_eq!(*needed, 1000 + DISK_RESERVE_BYTES);
                assert_eq!(*available, Some(1000 + DISK_RESERVE_BYTES - 1));
            }
            other => panic!("{other}"),
        }
        let text = err.to_string();
        assert!(
            text.contains("map.pdf") && text.contains("51 MB needed"),
            "{text}"
        );
        assert!(leftovers(dir.path()).is_empty());
    }

    #[test]
    fn enough_space_writes() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("map.pdf");
        write_checked(&target, b"x", |_| Ok(1 + DISK_RESERVE_BYTES), core_write).unwrap();
        assert_eq!(leftovers(dir.path()), ["map.pdf"]);
    }

    #[test]
    fn a_write_error_names_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("map.pdf");
        fs::write(&target, b"old").unwrap();
        let err = write_checked(
            &target,
            b"new",
            |_| Ok(u64::MAX),
            |path, _| Err((path.to_path_buf(), io::Error::other("simulated"))),
        )
        .unwrap_err();
        match &err {
            OutputError::Io { path, .. } => assert_eq!(path, &target),
            other => panic!("{other}"),
        }
        assert_eq!(fs::read(&target).unwrap(), b"old", "target untouched");
    }

    /// The disk is too full before writing, or fills up while writing: either way the error is
    /// [`OutputError::DiskFull`]. The first half runs the real write of `freemapper-fs` and shows
    /// that the check stops it before a partial file exists. The second half fakes the full disk
    /// in place of that write, so it checks only the mapping to `DiskFull`; that `freemapper-fs`
    /// removes its partial file when the disk fills up is its own test
    /// (`storage_full_while_writing_removes_the_part_file`).
    #[test]
    fn disk_full_leaves_no_part_file() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("map.pdf");
        fs::write(&target, b"old").unwrap();
        let before =
            write_checked(&target, b"new", |_| Ok(DISK_RESERVE_BYTES), core_write).unwrap_err();
        let full = |path: &Path, _: &[u8]| -> Result<(), (PathBuf, io::Error)> {
            Err((path.to_path_buf(), io::ErrorKind::StorageFull.into()))
        };
        let while_writing = write_checked(&target, b"new", |_| Ok(u64::MAX), full).unwrap_err();
        for err in [before, while_writing] {
            assert!(matches!(err, OutputError::DiskFull { .. }), "{err}");
        }
        assert_eq!(leftovers(dir.path()), ["map.pdf"]);
        assert_eq!(fs::read(&target).unwrap(), b"old");
    }

    #[test]
    fn a_missing_folder_is_not_created() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope");
        for err in [
            write_pdf_file(&missing.join("a4.pdf"), b"%PDF").unwrap_err(),
            write_project_pdf(dir.path(), &missing.join("a4.pdf"), b"%PDF").unwrap_err(),
        ] {
            match &err {
                OutputError::Io { path, source } => {
                    assert_eq!(path, &missing.join("a4.pdf"));
                    assert_eq!(source.kind(), io::ErrorKind::NotFound, "{source}");
                }
                other => panic!("{other}"),
            }
        }
        assert!(!missing.exists());
    }

    /// A folder named through another link to the project folder is inside it too; the link at
    /// `out` itself is not resolved, so `write_inside` sees and refuses it.
    #[cfg(unix)]
    #[test]
    fn another_name_of_the_project_folder_is_inside() {
        let dir = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let real = dir.path().join("event");
        fs::create_dir(&real).unwrap();
        let alias = dir.path().join("alias");
        std::os::unix::fs::symlink(&real, &alias).unwrap();
        std::os::unix::fs::symlink(elsewhere.path(), real.join("out")).unwrap();
        for (root, path) in [
            (&alias, real.join("out/a4.pdf")),
            (&real, alias.join("out/a4.pdf")),
            (&real, alias.join("new/deeper/a4.pdf")),
        ] {
            assert!(names_inside(root, &path), "{root:?} {path:?}");
        }
        for (root, folder) in [(&alias, real.join("out")), (&real, alias.join("out"))] {
            assert!(check_out_folder(root, &folder).is_err(), "{folder:?}");
            assert!(write_project_pdf(root, &folder.join("a4.pdf"), b"%PDF").is_err());
        }
        assert!(leftovers(elsewhere.path()).is_empty());
        // The folder the link leads to is outside.
        assert!(!names_inside(&real, &elsewhere.path().join("a4.pdf")));
    }

    #[test]
    fn failing_rename_leaves_no_part_file() {
        let dir = tempfile::tempdir().unwrap();
        // A folder with something in it where the PDF should go: the rename fails after the
        // partial file is written.
        let target = dir.path().join("map.pdf");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("keep"), b"x").unwrap();
        let err = write_checked(&target, b"%PDF", |_| Ok(u64::MAX), core_write).unwrap_err();
        assert!(matches!(err, OutputError::Io { .. }), "{err}");
        assert_eq!(leftovers(dir.path()), ["map.pdf"]);
        assert!(target.join("keep").is_file());
    }

    #[test]
    fn unknown_free_space_after_a_full_disk_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("map.pdf");
        // Room before writing, then the query fails once the disk is full.
        let calls = std::cell::Cell::new(0);
        let err = write_checked(
            &target,
            b"new",
            |_| {
                calls.set(calls.get() + 1);
                if calls.get() == 1 {
                    Ok(u64::MAX)
                } else {
                    Err(io::Error::other("no answer"))
                }
            },
            |path, _| Err((path.to_path_buf(), io::ErrorKind::StorageFull.into())),
        )
        .unwrap_err();
        match &err {
            OutputError::DiskFull { available, .. } => assert_eq!(*available, None),
            other => panic!("{other}"),
        }
        let text = err.to_string();
        assert!(text.contains("unknown") && !text.contains("0 MB"), "{text}");
    }

    #[test]
    fn project_pdfs_go_by_the_names_of_their_folders() {
        let root = Path::new("/p/event");
        assert!(names_inside(root, Path::new("/p/event/out/a4.pdf")));
        assert!(names_inside(root, Path::new("/p/event/./x/../out/a4.pdf")));
        assert!(names_inside(
            Path::new("/p/x/../event"),
            Path::new("/p/event/a.pdf")
        ));
        assert!(!names_inside(root, Path::new("/p/event/../maps/a4.pdf")));
        assert!(!names_inside(root, Path::new("/p/eventual/a4.pdf")));
        assert!(!names_inside(root, Path::new("/elsewhere/a4.pdf")));
    }

    #[test]
    fn a_project_pdf_is_written_inside_and_outside_the_project() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("event");
        fs::create_dir(&project).unwrap();
        fs::create_dir(project.join("out")).unwrap();
        let inside = project.join("out/a4.pdf");
        write_project_pdf(&project, &inside, b"%PDF in").unwrap();
        assert_eq!(fs::read(&inside).unwrap(), b"%PDF in");
        // An `--out-dir` outside the project folder.
        let outside = dir.path().join("maps/a4.pdf");
        fs::create_dir(dir.path().join("maps")).unwrap();
        write_project_pdf(&project, &outside, b"%PDF out").unwrap();
        assert_eq!(fs::read(&outside).unwrap(), b"%PDF out");
        assert_eq!(leftovers(&project.join("out")), ["a4.pdf"]);
    }

    #[cfg(unix)]
    #[test]
    fn a_linked_out_folder_of_the_project_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let out = dir.path().join("out");
        std::os::unix::fs::symlink(elsewhere.path(), &out).unwrap();
        let err = write_project_pdf(dir.path(), &out.join("a4.pdf"), b"%PDF").unwrap_err();
        match &err {
            OutputError::Io { path, source } => {
                assert_eq!(path, &out.join("a4.pdf"));
                assert!(is_outside_project(source), "{source}");
            }
            other => panic!("{other}"),
        }
        assert!(leftovers(elsewhere.path()).is_empty());
        let OutputError::Io { path, source } = check_out_folder(dir.path(), &out).unwrap_err()
        else {
            panic!("not an I/O error");
        };
        assert_eq!(path, out);
        assert!(is_outside_project(&source), "{source}");
        // A folder outside the project by name is the user's choice.
        assert!(check_out_folder(dir.path(), elsewhere.path()).is_ok());
        assert!(check_out_folder(dir.path(), &dir.path().join("new")).is_ok());
        assert!(!is_outside_project(&io::Error::other("other")));
    }

    #[test]
    fn stale_parts_are_removed_from_the_project_out_folder_only() {
        let dir = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let out = dir.path().join("out");
        fs::create_dir(&out).unwrap();
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 60 * 60);
        let plant = |folder: &Path| {
            for name in ["a4.pdf.1.0.part", "fresh.part"] {
                fs::write(folder.join(name), b"x").unwrap();
            }
            let file = fs::File::options()
                .write(true)
                .open(folder.join("a4.pdf.1.0.part"))
                .unwrap();
            file.set_modified(old).unwrap();
        };
        let other = dir.path().join("other");
        fs::create_dir(&other).unwrap();
        plant(&out);
        plant(elsewhere.path());
        plant(&other);
        remove_stale_parts(dir.path(), &other);
        remove_stale_parts(dir.path(), dir.path());
        assert_eq!(leftovers(&other), ["a4.pdf.1.0.part", "fresh.part"]);
        remove_stale_parts(dir.path(), &out);
        remove_stale_parts(dir.path(), elsewhere.path());
        assert_eq!(leftovers(&out), ["fresh.part"]);
        assert_eq!(
            leftovers(elsewhere.path()),
            ["a4.pdf.1.0.part", "fresh.part"]
        );
    }

    #[test]
    fn real_free_space_query_works() {
        let dir = tempfile::tempdir().unwrap();
        assert!(fs4::available_space(dir.path()).is_ok());
    }
}
