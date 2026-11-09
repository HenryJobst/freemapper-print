//! Safe writing of finished PDFs: free space is checked first, the bytes go to a `.part` file next
//! to the target, and the target appears only by renaming a fully written, synced file. A full
//! disk leaves no half-written PDF and no leftover `.part` file.

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
        "not enough disk space to write {}: {} MB needed, {} MB available",
        path.display(),
        needed.div_ceil(MB),
        available / MB
    )]
    DiskFull {
        /// Bytes needed: the PDF plus [`DISK_RESERVE_BYTES`].
        needed: u64,
        /// Bytes free on the volume.
        available: u64,
        /// The file that was to be written.
        path: PathBuf,
    },
    /// Any other I/O error.
    #[error("could not write {}: {source}", path.display())]
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
    let disk_full = |available: u64| OutputError::DiskFull {
        needed,
        available,
        path: path.to_path_buf(),
    };
    if let Some(available) = query()
        && available < needed
    {
        return Err(disk_full(available));
    }
    let part = part_path(path);
    let result = (|| {
        let mut file = File::create(&part)?;
        write(&mut file, bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&part, path)
    })();
    match result {
        Ok(()) => Ok(()),
        Err(source) => {
            // Best effort: the original error is the one to report.
            let _ = fs::remove_file(&part);
            if source.kind() == io::ErrorKind::StorageFull {
                Err(disk_full(query().unwrap_or(0)))
            } else {
                Err(OutputError::Io {
                    path: path.to_path_buf(),
                    source,
                })
            }
        }
    }
}

/// `<name>.part` next to `path`.
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
                assert_eq!(*available, 1000 + DISK_RESERVE_BYTES - 1);
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
    fn real_free_space_query_works() {
        let dir = tempfile::tempdir().unwrap();
        assert!(fs4::available_space(dir.path()).is_ok());
    }
}
