//! Safe writing of finished PDFs: free space is checked first, the bytes go to a `.part` file next
//! to the target (`a4.pdf` is written as `a4.pdf.part`), and the target appears only by renaming a
//! fully written, synced file. A full disk, any other error, an early return or a panic leaves no
//! half-written PDF and no leftover `.part` file (ADR 0030).
//!
//! One writer per target is assumed: two exports into the same folder at the same time would
//! share the `.part` name of a layout they both write.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

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

/// Writes `bytes` to `path`, replacing an existing file only when the whole PDF is on disk.
///
/// Fails with [`OutputError::DiskFull`] when the volume has less than `bytes.len()` plus
/// [`DISK_RESERVE_BYTES`] free. If the free space cannot be determined, the check is skipped and a
/// full disk is still caught while writing.
pub fn write_pdf_file(path: &Path, bytes: &[u8]) -> Result<(), OutputError> {
    #[cfg(any(test, feature = "test-hooks"))]
    let fake = std::env::var(FAKE_FREE_SPACE_ENV)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok());
    #[cfg(not(any(test, feature = "test-hooks")))]
    let fake: Option<u64> = None;
    write_checked(
        path,
        bytes,
        |folder| match fake {
            Some(bytes) => Ok(bytes),
            None => fs4::available_space(folder),
        },
        |file, bytes| file.write_all(bytes),
    )
}

/// [`write_pdf_file`] with the free-space query and the byte writer replaceable for tests.
fn write_checked(
    path: &Path,
    bytes: &[u8],
    available_space: impl Fn(&Path) -> io::Result<u64>,
    write: impl Fn(&mut File, &[u8]) -> io::Result<()>,
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
    let mut part = PartFile {
        path: part_path(path),
        done: false,
    };
    let result = (|| {
        let mut file = File::create(&part.path)?;
        write(&mut file, bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&part.path, path)
    })();
    match result {
        Ok(()) => {
            part.done = true;
            Ok(())
        }
        Err(source) => {
            // The `.part` file goes first, so that the free space counts without it.
            drop(part);
            if source.kind() == io::ErrorKind::StorageFull {
                Err(disk_full(query()))
            } else {
                Err(OutputError::Io {
                    path: path.to_path_buf(),
                    source,
                })
            }
        }
    }
}

/// The `.part` file of one write. Dropping it removes the file unless the write is `done` (renamed
/// onto the target): on an error, an early return and a panic alike.
struct PartFile {
    path: PathBuf,
    done: bool,
}

impl Drop for PartFile {
    fn drop(&mut self) {
        if !self.done {
            // Best effort: the original error is the one to report.
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// `<name>.part` next to `path`: `a4.pdf.part` for `a4.pdf`.
fn part_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leftovers(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
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
            |f, b| f.write_all(b),
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
        write_checked(
            &target,
            b"x",
            |_| Ok(1 + DISK_RESERVE_BYTES),
            |f, b| f.write_all(b),
        )
        .unwrap();
        assert_eq!(leftovers(dir.path()), ["map.pdf"]);
    }

    #[test]
    fn cleans_up_on_write_error() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("map.pdf");
        fs::write(&target, b"old").unwrap();
        let err = write_checked(
            &target,
            b"new",
            |_| Ok(u64::MAX),
            |f, b| {
                f.write_all(&b[..1])?;
                Err(io::Error::other("simulated"))
            },
        )
        .unwrap_err();
        assert!(matches!(err, OutputError::Io { .. }), "{err}");
        assert_eq!(fs::read(&target).unwrap(), b"old", "target untouched");
        assert_eq!(leftovers(dir.path()), ["map.pdf"]);
    }

    #[test]
    fn storage_full_while_writing_is_disk_full() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("map.pdf");
        let err = write_checked(
            &target,
            b"new",
            |_| Ok(u64::MAX),
            |_, _| Err(io::ErrorKind::StorageFull.into()),
        )
        .unwrap_err();
        assert!(matches!(err, OutputError::DiskFull { .. }), "{err}");
        assert!(leftovers(dir.path()).is_empty());
    }

    #[test]
    fn failing_rename_leaves_no_part_file() {
        let dir = tempfile::tempdir().unwrap();
        // A folder with something in it where the PDF should go: the rename fails.
        let target = dir.path().join("map.pdf");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("keep"), b"x").unwrap();
        let err =
            write_checked(&target, b"%PDF", |_| Ok(u64::MAX), |f, b| f.write_all(b)).unwrap_err();
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
            |_, _| Err(io::ErrorKind::StorageFull.into()),
        )
        .unwrap_err();
        match &err {
            OutputError::DiskFull { available, .. } => assert_eq!(*available, None),
            other => panic!("{other}"),
        }
        let text = err.to_string();
        assert!(text.contains("unknown") && !text.contains("0 MB"), "{text}");
        assert!(leftovers(dir.path()).is_empty());
    }

    #[test]
    fn panic_while_writing_leaves_no_part_file() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("map.pdf");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            write_checked(
                &target,
                b"new",
                |_| Ok(u64::MAX),
                |f, b| {
                    f.write_all(b).unwrap();
                    panic!("simulated");
                },
            )
        }));
        assert!(result.is_err());
        assert!(leftovers(dir.path()).is_empty());
    }

    #[test]
    fn part_file_is_named_after_the_target() {
        assert_eq!(
            part_path(Path::new("/x/a4.pdf")),
            PathBuf::from("/x/a4.pdf.part")
        );
    }

    #[test]
    fn real_free_space_query_works() {
        let dir = tempfile::tempdir().unwrap();
        assert!(fs4::available_space(dir.path()).is_ok());
    }
}
