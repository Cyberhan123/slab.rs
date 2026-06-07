use std::path::{Path, PathBuf};

pub(crate) fn settings_path_from_env_or_default() -> PathBuf {
    std::env::var("SLAB_SETTINGS_PATH")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(slab_utils::app_home::settings_path)
}

#[cfg(windows)]
pub(crate) fn remove_windows_extended_path_prefix(path: &Path) -> PathBuf {
    let raw = path.to_string_lossy();
    if let Some(path) = raw.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{path}"));
    }
    if let Some(path) = raw.strip_prefix(r"\\?\") {
        return PathBuf::from(path);
    }
    path.to_path_buf()
}

#[cfg(not(windows))]
pub(crate) fn remove_windows_extended_path_prefix(path: &Path) -> PathBuf {
    path.to_path_buf()
}

pub(crate) fn display_path_string(path: &Path) -> String {
    remove_windows_extended_path_prefix(path).to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[cfg(windows)]
    #[test]
    fn removes_windows_extended_path_prefix() {
        assert_eq!(
            super::remove_windows_extended_path_prefix(Path::new(r"\\?\C:\Users\example\repo")),
            Path::new(r"C:\Users\example\repo")
        );
        assert_eq!(
            super::remove_windows_extended_path_prefix(Path::new(r"\\?\UNC\server\share\repo")),
            Path::new(r"\\server\share\repo")
        );
    }

    #[test]
    fn display_path_string_returns_lossy_path() {
        assert!(!super::display_path_string(Path::new("workspace")).is_empty());
    }
}
