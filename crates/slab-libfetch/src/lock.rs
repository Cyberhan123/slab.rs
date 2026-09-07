//! Cross-process advisory locking for artifact installs.
//!
//! Cargo runs build scripts in parallel, and several `*-sys` crates share one
//! vendored artifact (e.g. `slab-whisper-sys` and `slab-parakeet-sys` both
//! install `vendor/whisper`). Without mutual exclusion two build scripts can
//! simultaneously decide "not installed", both download (one may exhaust its
//! retries), or one `remove_dir_all` the files the other just extracted.
//!
//! Installs into the same directory are serialized with a blocking exclusive
//! advisory lock (`flock` on Unix, `LockFileEx` on Windows) on a sibling file
//! of the install directory (`vendor/whisper` -> `vendor/whisper.lock`). The
//! lock is per artifact directory, so unrelated artifacts never block each
//! other. It is held per open file handle, released on drop and automatically
//! when a process exits, and deliberately not reentrant.

use crate::error::FetchError;
use fd_lock::RwLock;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

/// Path of the advisory lock file guarding `install_dir`.
///
/// The lock file is a *sibling* of the install directory, never inside it:
/// installs remove and recreate that directory on version changes, and a lock
/// file inside it would be deleted while held — silently breaking mutual
/// exclusion on Unix and failing `remove_dir_all` outright on Windows.
fn lock_path(install_dir: &Path) -> PathBuf {
    match install_dir.file_name() {
        Some(name) => install_dir.with_file_name(format!("{}.lock", name.to_string_lossy())),
        None => PathBuf::from(".slab-libfetch.lock"),
    }
}

/// Open (creating if necessary) the advisory lock file guarding `install_dir`.
///
/// The lock is not held yet. Call [`RwLock::write`] on the result in the same
/// scope and keep the guard alive for the whole critical section — the
/// `version.json` check, download, extraction, `remove_dir_all`, and the
/// final `version.json` write:
///
/// ```ignore
/// let mut install_lock = open_install_lock(&install_dir)?;
/// let _install_guard = install_lock.write()?; // blocks until free
/// // re-check version.json, then download/extract/write under the lock
/// ```
pub(crate) fn open_install_lock(install_dir: &Path) -> Result<RwLock<File>, FetchError> {
    let path = lock_path(install_dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file =
        OpenOptions::new().read(true).write(true).create(true).truncate(false).open(&path)?;
    Ok(RwLock::new(file))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn lock_path_is_sibling_of_install_dir() {
        assert_eq!(lock_path(Path::new("vendor/whisper")), PathBuf::from("vendor/whisper.lock"));
        assert_eq!(
            lock_path(Path::new("/opt/artifacts/llama")),
            PathBuf::from("/opt/artifacts/llama.lock")
        );
    }

    #[test]
    fn write_lock_excludes_a_second_handle_until_released() {
        let dir = std::env::temp_dir().join(format!(
            "slab_libfetch_lock_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let install_dir = dir.join("artifact");

        let mut first = open_install_lock(&install_dir).unwrap();
        let first_guard = first.write().unwrap();

        // Locks are per open file handle, so a second handle of the same
        // process is excluded exactly like a second build-script process.
        let mut second = open_install_lock(&install_dir).unwrap();
        assert!(second.try_write().is_err());

        drop(first_guard);
        let second_guard = second.write().unwrap();
        drop(second_guard);

        assert!(first.try_write().is_ok());
        fs::remove_dir_all(&dir).unwrap();
    }
}
