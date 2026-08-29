//! Default search exclusions shared by the grep and file_glob tools.
//!
//! Both tools walk with `hidden(false)` so dot-files stay searchable, which
//! also lets the walk descend into `.git` — dangling objects there (whole
//! generated files collapsed into single lines) once injected hundreds of
//! kilobytes of garbage into agent context. These name-based excludes prune
//! the known non-source trees during traversal WITHOUT touching the
//! `.gitignore` machinery: an `ignore` Override whitelist would take
//! precedence over `.gitignore` and leak ignored directories (see the
//! `grep_tool_glob_does_not_override_gitignore` regression).

/// Directory names pruned during traversal.
const EXCLUDED_DIR_NAMES: &[&str] = &[".git", "node_modules", "target", "vendor", "dist"];

/// File names skipped outright (cargo-bazel generated aggregates).
const EXCLUDED_FILE_NAMES: &[&str] = &["crates.bzl", "defs.bzl"];

/// File name suffixes skipped outright.
const EXCLUDED_FILE_SUFFIXES: &[&str] = &[".lock"];

/// Name-based exclusion for a walk entry. The walk root (`depth == 0`) is
/// never excluded, so an explicit `path` into an excluded tree still searches
/// it.
pub(crate) fn is_default_excluded(entry: &ignore::DirEntry) -> bool {
    if entry.depth() == 0 {
        return false;
    }
    let Some(name) = entry.file_name().to_str() else { return false };
    if entry.file_type().is_some_and(|file_type| file_type.is_dir()) {
        return EXCLUDED_DIR_NAMES.contains(&name);
    }
    EXCLUDED_FILE_NAMES.contains(&name)
        || EXCLUDED_FILE_SUFFIXES.iter().any(|suffix| name.ends_with(suffix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excluded_vocabulary_is_stable() {
        assert!(EXCLUDED_DIR_NAMES.contains(&".git"));
        assert!(EXCLUDED_DIR_NAMES.contains(&"node_modules"));
        assert!(EXCLUDED_FILE_SUFFIXES.contains(&".lock"));
        assert!(EXCLUDED_FILE_NAMES.contains(&"crates.bzl"));
    }
}
