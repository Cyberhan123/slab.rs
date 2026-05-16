use std::path::Path;

pub fn sqlite_url_for_path(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let prefix = if normalized.starts_with('/') { "sqlite://" } else { "sqlite:///" };
    format!("{prefix}{normalized}?mode=rwc")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::sqlite_url_for_path;

    #[test]
    fn sqlite_url_for_relative_path_uses_relative_url_shape() {
        assert_eq!(sqlite_url_for_path(Path::new("slab.db")), "sqlite:///slab.db?mode=rwc");
    }

    #[test]
    fn sqlite_url_for_absolute_path_uses_absolute_url_shape() {
        assert_eq!(
            sqlite_url_for_path(Path::new("/var/lib/slab/slab.db")),
            "sqlite:///var/lib/slab/slab.db?mode=rwc"
        );
    }

    #[test]
    fn sqlite_url_for_windows_path_normalizes_separators() {
        assert_eq!(
            sqlite_url_for_path(Path::new(r"C:\Project\.slab\slab.db")),
            "sqlite:///C:/Project/.slab/slab.db?mode=rwc"
        );
    }
}
