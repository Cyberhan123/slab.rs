pub mod absolute;

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use anyhow::{Result, bail};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EnsureWithinRootError {
    #[error("failed to resolve root path {path}: {source}")]
    RootCanonicalize { path: PathBuf, source: std::io::Error },
    #[error("failed to resolve path {path}: {source}")]
    PathCanonicalize { path: PathBuf, source: std::io::Error },
    #[error("path {path} escapes root {root}")]
    OutsideRoot { root: PathBuf, path: PathBuf },
}

#[derive(Debug, Error)]
pub enum ValidateAbsolutePathError {
    #[error("{label} must not be empty")]
    Empty { label: String },
    #[error("{label} must be an absolute path (got: {path})")]
    NotAbsolute { label: String, path: PathBuf },
    #[error("{label} must not contain '..' components")]
    ContainsParentDir { label: String },
}

pub fn ensure_within_root(
    root: &Path,
    path: &Path,
) -> std::result::Result<PathBuf, EnsureWithinRootError> {
    let canonical_root = dunce::canonicalize(root).map_err(|source| {
        EnsureWithinRootError::RootCanonicalize { path: root.to_path_buf(), source }
    })?;
    let canonical_path = dunce::canonicalize(path).map_err(|source| {
        EnsureWithinRootError::PathCanonicalize { path: path.to_path_buf(), source }
    })?;
    ensure_canonical_path_stays_within_root(canonical_root, canonical_path)
}

pub fn ensure_within_root_or_nearest(
    root: &Path,
    path: &Path,
) -> std::result::Result<PathBuf, EnsureWithinRootError> {
    let canonical_root = dunce::canonicalize(root).map_err(|source| {
        EnsureWithinRootError::RootCanonicalize { path: root.to_path_buf(), source }
    })?;
    let absolute_path =
        if path.is_absolute() { path.to_path_buf() } else { canonical_root.join(path) };
    let resolved_path = canonicalize_existing_or_nearest(&absolute_path);
    ensure_canonical_path_stays_within_root(canonical_root, resolved_path)
}

pub fn validate_absolute_path(
    label: &str,
    path: &Path,
) -> std::result::Result<PathBuf, ValidateAbsolutePathError> {
    if path.as_os_str().is_empty() {
        return Err(ValidateAbsolutePathError::Empty { label: label.to_owned() });
    }
    if !path.is_absolute() {
        return Err(ValidateAbsolutePathError::NotAbsolute {
            label: label.to_owned(),
            path: path.to_path_buf(),
        });
    }
    if path.components().any(|component| component == Component::ParentDir) {
        return Err(ValidateAbsolutePathError::ContainsParentDir { label: label.to_owned() });
    }
    Ok(path.to_path_buf())
}

fn ensure_canonical_path_stays_within_root(
    root: PathBuf,
    path: PathBuf,
) -> std::result::Result<PathBuf, EnsureWithinRootError> {
    if path.starts_with(&root) {
        Ok(path)
    } else {
        Err(EnsureWithinRootError::OutsideRoot { root, path })
    }
}

fn canonicalize_existing_or_nearest(path: &Path) -> PathBuf {
    if let Ok(canonical) = dunce::canonicalize(path) {
        return canonical;
    }

    let mut current = path;
    let mut tail = Vec::<OsString>::new();
    while let (Some(parent), Some(file_name)) = (current.parent(), current.file_name()) {
        tail.push(file_name.to_os_string());
        if let Ok(canonical_parent) = dunce::canonicalize(parent) {
            let mut resolved = canonical_parent;
            for segment in tail.iter().rev() {
                resolved.push(segment);
            }
            return resolved;
        }
        current = parent;
    }

    path.to_path_buf()
}

pub fn normalize_relative_path(raw: &str) -> Result<String> {
    normalize_relative_path_impl(raw, false)
}

pub fn normalize_relative_path_allow_empty(raw: &str) -> Result<String> {
    normalize_relative_path_impl(raw, true)
}

fn normalize_relative_path_impl(raw: &str, allow_empty: bool) -> Result<String> {
    let trimmed = raw.trim().trim_matches(['/', '\\']);
    if trimmed.is_empty() {
        if allow_empty {
            return Ok(String::new());
        }
        bail!("empty path is not allowed");
    }

    let mut components = Vec::new();
    for component in Path::new(trimmed).components() {
        match component {
            Component::Normal(segment) => components.push(segment.to_string_lossy().into_owned()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!("path `{raw}` is invalid")
            }
        }
    }

    if components.is_empty() {
        if allow_empty {
            return Ok(String::new());
        }
        bail!("path is invalid");
    }

    Ok(components.join("/"))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{
        ensure_within_root, ensure_within_root_or_nearest, normalize_relative_path,
        normalize_relative_path_allow_empty, validate_absolute_path,
    };

    #[test]
    fn normalizes_non_empty_relative_paths() {
        assert_eq!(normalize_relative_path("ui/index.html").expect("normalize"), "ui/index.html");
    }

    #[test]
    fn rejects_parent_segments() {
        assert!(normalize_relative_path("../plugin.json").is_err());
    }

    #[test]
    fn allows_empty_after_trimming_current_dir() {
        assert_eq!(normalize_relative_path_allow_empty(" ./ ").expect("normalize"), "");
    }

    #[test]
    fn ensure_within_root_returns_existing_canonical_child() {
        let root = tempdir().expect("root");
        let child = root.path().join("nested").join("file.txt");
        fs::create_dir_all(child.parent().expect("child parent")).expect("create child parent");
        fs::write(&child, "ok").expect("write child");

        let resolved = ensure_within_root(root.path(), &child).expect("child within root");

        assert_eq!(resolved, dunce::canonicalize(&child).expect("canonical child"));
    }

    #[test]
    fn ensure_within_root_rejects_missing_and_outside_paths() {
        let root = tempdir().expect("root");
        let outside = tempdir().expect("outside");

        assert!(ensure_within_root(root.path(), &root.path().join("missing.txt")).is_err());
        assert!(ensure_within_root(root.path(), outside.path()).is_err());
    }

    #[test]
    fn ensure_within_root_or_nearest_accepts_missing_descendant() {
        let root = tempdir().expect("root");
        let child = root.path().join("missing").join("asset.txt");

        let resolved =
            ensure_within_root_or_nearest(root.path(), &child).expect("missing child within root");

        assert!(
            resolved.ends_with("missing/asset.txt") || resolved.ends_with("missing\\asset.txt")
        );
    }

    #[test]
    fn validate_absolute_path_rejects_relative_and_parent_segments() {
        assert!(validate_absolute_path("model_path", std::path::Path::new("model.gguf")).is_err());

        let path_with_parent =
            std::env::temp_dir().join("slab-utils-path").join("..").join("model.gguf");
        assert!(validate_absolute_path("model_path", &path_with_parent).is_err());
        assert!(validate_absolute_path("model_path", std::env::temp_dir().as_path()).is_ok());
    }
}
