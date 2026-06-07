use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use tempfile::NamedTempFile;

#[derive(Debug, Clone, Copy, Default)]
pub struct AtomicWriteOptions {
    pub unix_mode: Option<u32>,
    pub sync_parent_dir: bool,
}

pub fn atomic_write_bytes(path: &Path, bytes: &[u8]) -> io::Result<()> {
    atomic_write_bytes_with_options(path, bytes, AtomicWriteOptions::default())
}

pub fn atomic_write_bytes_with_options(
    path: &Path,
    bytes: &[u8],
    options: AtomicWriteOptions,
) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;

    let mut temp_file = NamedTempFile::new_in(parent)?;
    set_unix_mode(&temp_file, options.unix_mode)?;
    temp_file.write_all(bytes)?;
    temp_file.flush()?;
    temp_file.as_file().sync_all()?;

    let temp_path = temp_file.into_temp_path();
    replace_file(temp_path.as_ref(), path)?;
    std::mem::forget(temp_path);

    if options.sync_parent_dir {
        sync_parent_dir(parent)?;
    }

    Ok(())
}

pub fn existing_ancestor(path: &Path) -> io::Result<PathBuf> {
    let mut current = path;
    while !current.exists() {
        current = current.parent().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "path has no existing ancestor")
        })?;
    }
    current.canonicalize()
}

#[cfg(unix)]
fn set_unix_mode(temp_file: &NamedTempFile, mode: Option<u32>) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    if let Some(mode) = mode {
        temp_file.as_file().set_permissions(fs::Permissions::from_mode(mode))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn set_unix_mode(_temp_file: &NamedTempFile, _mode: Option<u32>) -> io::Result<()> {
    Ok(())
}

#[cfg(windows)]
fn replace_file(from: &Path, to: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let from_wide: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to_wide: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    let result = unsafe {
        MoveFileExW(
            from_wide.as_ptr(),
            to_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
}

#[cfg(not(windows))]
fn replace_file(from: &Path, to: &Path) -> io::Result<()> {
    fs::rename(from, to)
}

#[cfg(unix)]
fn sync_parent_dir(parent: &Path) -> io::Result<()> {
    fs::File::open(parent)?.sync_all()
}

#[cfg(not(unix))]
fn sync_parent_dir(_parent: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_bytes_creates_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        atomic_write_bytes(&path, b"first").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"first");
    }

    #[test]
    fn atomic_write_bytes_overwrites_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, b"old").unwrap();

        atomic_write_bytes(&path, b"new").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"new");
    }

    #[test]
    fn atomic_write_bytes_fails_without_orphan_temp_file_when_target_parent_is_file() {
        let dir = tempfile::tempdir().unwrap();
        let parent = dir.path().join("not-a-dir");
        fs::write(&parent, b"file").unwrap();
        let path = parent.join("settings.json");

        assert!(atomic_write_bytes(&path, b"payload").is_err());
        let entries = fs::read_dir(dir.path()).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path(), parent);
    }

    #[test]
    fn existing_ancestor_returns_closest_existing_parent() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("a").join("b").join("file.txt");

        let ancestor = existing_ancestor(&missing).unwrap();

        assert_eq!(ancestor, dir.path().canonicalize().unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_bytes_applies_unix_mode() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        atomic_write_bytes_with_options(
            &path,
            b"payload",
            AtomicWriteOptions { unix_mode: Some(0o600), sync_parent_dir: false },
        )
        .unwrap();

        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    }
}
