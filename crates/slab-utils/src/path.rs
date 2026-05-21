use std::path::{Component, Path};

use anyhow::{Result, bail};

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
    use super::{normalize_relative_path, normalize_relative_path_allow_empty};

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
}
