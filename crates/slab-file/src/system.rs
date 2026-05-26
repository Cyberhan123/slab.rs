//! Workspace-safe filesystem helpers.

use std::{
    fs,
    path::{Path, PathBuf},
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FileSystemError {
    #[error("absolute path `{0}` is not allowed when a workspace root is configured")]
    AbsolutePath(String),
    #[error("path `{0}` escapes the workspace root")]
    PathEscapesRoot(String),
    #[error("workspace path `{0}` is invalid")]
    InvalidPath(String),
    #[error("failed to resolve workspace root: {0}")]
    Root(std::io::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid patch: {0}")]
    InvalidPatch(String),
    #[error("patch does not apply to `{path}` at line {line}")]
    PatchMismatch { path: String, line: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirEntryView {
    pub name: String,
    pub is_dir: bool,
    pub size_bytes: u64,
    pub modified: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchApplyResult {
    pub applied_files: Vec<String>,
    pub result: String,
    pub error_message: Option<String>,
}

/// Sandbox policy metadata attached to filesystem operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FileSystemSandboxPolicy {
    #[default]
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

/// Filesystem permission context supplied by host layers.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileSystemSandboxContext {
    pub policy: FileSystemSandboxPolicy,
    pub cwd: Option<PathBuf>,
    pub workspace_root: Option<PathBuf>,
    pub readable_roots: Vec<PathBuf>,
    pub writable_roots: Vec<PathBuf>,
    pub denied_paths: Vec<PathBuf>,
}

/// Metadata returned by filesystem implementations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMetadata {
    pub is_file: bool,
    pub is_directory: bool,
    pub is_symlink: bool,
    pub size_bytes: u64,
    pub modified_at: u64,
    pub created_at: u64,
}

/// Directory entry returned by filesystem implementations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
    pub metadata: FileMetadata,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateDirectoryOptions {
    pub recursive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RemoveOptions {
    pub recursive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CopyOptions {
    pub overwrite: bool,
}

/// Abstract filesystem access used by local and remote execution environments.
#[async_trait]
pub trait ExecutorFileSystem: Send + Sync {
    async fn read_file(
        &self,
        context: &FileSystemSandboxContext,
        path: &str,
    ) -> Result<Vec<u8>, FileSystemError>;

    async fn write_file(
        &self,
        context: &FileSystemSandboxContext,
        path: &str,
        content: &[u8],
    ) -> Result<(), FileSystemError>;

    async fn create_directory(
        &self,
        context: &FileSystemSandboxContext,
        path: &str,
    ) -> Result<(), FileSystemError>;

    async fn get_metadata(
        &self,
        context: &FileSystemSandboxContext,
        path: &str,
    ) -> Result<FileMetadata, FileSystemError>;

    async fn read_directory(
        &self,
        context: &FileSystemSandboxContext,
        path: &str,
    ) -> Result<Vec<DirectoryEntry>, FileSystemError>;

    async fn remove(
        &self,
        context: &FileSystemSandboxContext,
        path: &str,
        options: RemoveOptions,
    ) -> Result<(), FileSystemError>;

    async fn copy(
        &self,
        context: &FileSystemSandboxContext,
        from: &str,
        to: &str,
        options: CopyOptions,
    ) -> Result<(), FileSystemError>;
}

pub fn normalize_relative_path(raw: &str) -> Result<String, FileSystemError> {
    let normalized = raw.replace('\\', "/");
    let path = Path::new(&normalized);
    if path.is_absolute() {
        return Err(FileSystemError::AbsolutePath(raw.to_string()));
    }

    let mut parts = Vec::new();
    for part in normalized.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err(FileSystemError::InvalidPath(raw.to_string()));
        }
        parts.push(part);
    }

    Ok(parts.join("/"))
}

pub fn resolve_path(workspace_root: Option<&Path>, path: &str) -> Result<PathBuf, FileSystemError> {
    let path_buf = PathBuf::from(path);
    let Some(root) = workspace_root else {
        return Ok(path_buf);
    };

    if path_buf.is_absolute() {
        return Err(FileSystemError::AbsolutePath(path.to_string()));
    }

    let relative = normalize_relative_path(path)?;
    let canonical_root = root.canonicalize().map_err(FileSystemError::Root)?;
    let candidate = canonical_root.join(&relative);
    if candidate.exists() {
        let canonical_candidate = candidate.canonicalize()?;
        if !canonical_candidate.starts_with(&canonical_root) {
            return Err(FileSystemError::PathEscapesRoot(path.to_string()));
        }
        return Ok(canonical_candidate);
    }

    if let Some(parent) = candidate.parent() {
        let canonical_parent = existing_parent(parent)?;
        if !canonical_parent.starts_with(&canonical_root) {
            return Err(FileSystemError::PathEscapesRoot(path.to_string()));
        }
    }

    Ok(candidate)
}

pub async fn read_to_string(
    workspace_root: Option<&Path>,
    path: &str,
) -> Result<String, FileSystemError> {
    let path = resolve_path(workspace_root, path)?;
    Ok(tokio::fs::read_to_string(path).await?)
}

pub async fn write_string(
    workspace_root: Option<&Path>,
    path: &str,
    content: &str,
) -> Result<(), FileSystemError> {
    let path = resolve_path(workspace_root, path)?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmp_path = {
        let mut file_name = path
            .file_name()
            .ok_or_else(|| FileSystemError::InvalidPath(path.display().to_string()))?
            .to_owned();
        file_name.push(".tmp");
        path.with_file_name(file_name)
    };
    tokio::fs::write(&tmp_path, content).await?;
    tokio::fs::rename(&tmp_path, &path).await?;
    Ok(())
}

pub async fn list_dir(
    workspace_root: Option<&Path>,
    path: &str,
) -> Result<Vec<DirEntryView>, FileSystemError> {
    let path = resolve_path(workspace_root, path)?;
    let mut read_dir = tokio::fs::read_dir(path).await?;
    let mut entries = Vec::new();
    while let Some(entry) = read_dir.next_entry().await? {
        let metadata = entry.metadata().await.ok();
        let modified = metadata
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        entries.push(DirEntryView {
            name: entry.file_name().to_string_lossy().into_owned(),
            is_dir: metadata.as_ref().is_some_and(std::fs::Metadata::is_dir),
            size_bytes: metadata.as_ref().map(std::fs::Metadata::len).unwrap_or(0),
            modified,
        });
    }
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));
    Ok(entries)
}

pub fn apply_unified_patch(
    workspace_root: &Path,
    patch: &str,
) -> Result<PatchApplyResult, FileSystemError> {
    let file_patches = parse_patch(patch)?;
    let mut applied_files = Vec::new();
    for file_patch in file_patches {
        let display_path = file_patch.display_path();
        let target_path = if file_patch.is_delete() {
            resolve_path(Some(workspace_root), file_patch.old_path.as_deref().unwrap_or(""))?
        } else {
            resolve_path(Some(workspace_root), file_patch.new_path.as_deref().unwrap_or(""))?
        };

        if file_patch.is_delete() {
            if target_path.exists() {
                fs::remove_file(&target_path)?;
            }
            applied_files.push(display_path);
            continue;
        }

        let original =
            if file_patch.is_create() { String::new() } else { fs::read_to_string(&target_path)? };
        let updated = apply_file_patch(&display_path, &original, &file_patch)?;
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&target_path, updated)?;
        applied_files.push(display_path);
    }

    Ok(PatchApplyResult { applied_files, result: "ok".to_string(), error_message: None })
}

fn existing_parent(path: &Path) -> Result<PathBuf, FileSystemError> {
    let mut current = path;
    while !current.exists() {
        current = current
            .parent()
            .ok_or_else(|| FileSystemError::InvalidPath(path.display().to_string()))?;
    }
    Ok(current.canonicalize()?)
}

#[derive(Debug)]
struct FilePatch {
    old_path: Option<String>,
    new_path: Option<String>,
    hunks: Vec<Hunk>,
}

impl FilePatch {
    fn is_create(&self) -> bool {
        self.old_path.is_none()
    }

    fn is_delete(&self) -> bool {
        self.new_path.is_none()
    }

    fn display_path(&self) -> String {
        self.new_path.as_ref().or(self.old_path.as_ref()).cloned().unwrap_or_default()
    }
}

#[derive(Debug)]
struct Hunk {
    old_start: usize,
    lines: Vec<HunkLine>,
}

#[derive(Debug)]
enum HunkLine {
    Context(String),
    Remove(String),
    Add(String),
}

fn parse_patch(patch: &str) -> Result<Vec<FilePatch>, FileSystemError> {
    let lines: Vec<&str> = patch.lines().collect();
    let mut patches = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        if !lines[index].starts_with("--- ") {
            index += 1;
            continue;
        }
        let old_path = parse_patch_path(lines[index].trim_start_matches("--- "));
        index += 1;
        if index >= lines.len() || !lines[index].starts_with("+++ ") {
            return Err(FileSystemError::InvalidPatch("missing +++ file header".to_string()));
        }
        let new_path = parse_patch_path(lines[index].trim_start_matches("+++ "));
        index += 1;

        let mut hunks = Vec::new();
        while index < lines.len() {
            if lines[index].starts_with("--- ") {
                break;
            }
            if !lines[index].starts_with("@@ ") {
                index += 1;
                continue;
            }
            let old_start = parse_hunk_old_start(lines[index])?;
            index += 1;
            let mut hunk_lines = Vec::new();
            while index < lines.len()
                && !lines[index].starts_with("@@ ")
                && !lines[index].starts_with("--- ")
            {
                let line = lines[index];
                if let Some(rest) = line.strip_prefix(' ') {
                    hunk_lines.push(HunkLine::Context(rest.to_string()));
                } else if let Some(rest) = line.strip_prefix('-') {
                    hunk_lines.push(HunkLine::Remove(rest.to_string()));
                } else if let Some(rest) = line.strip_prefix('+') {
                    hunk_lines.push(HunkLine::Add(rest.to_string()));
                } else if line != r"\ No newline at end of file" {
                    return Err(FileSystemError::InvalidPatch(format!(
                        "invalid hunk line `{line}`"
                    )));
                }
                index += 1;
            }
            hunks.push(Hunk { old_start, lines: hunk_lines });
        }

        patches.push(FilePatch { old_path, new_path, hunks });
    }

    if patches.is_empty() {
        return Err(FileSystemError::InvalidPatch("no file patches found".to_string()));
    }
    Ok(patches)
}

fn parse_patch_path(raw: &str) -> Option<String> {
    let path = raw.split_whitespace().next().unwrap_or(raw);
    if path == "/dev/null" {
        return None;
    }
    let stripped = path.strip_prefix("a/").or_else(|| path.strip_prefix("b/")).unwrap_or(path);
    Some(stripped.to_string())
}

fn parse_hunk_old_start(header: &str) -> Result<usize, FileSystemError> {
    let Some(rest) = header.strip_prefix("@@ -") else {
        return Err(FileSystemError::InvalidPatch(format!("invalid hunk header `{header}`")));
    };
    let Some((range, _)) = rest.split_once(' ') else {
        return Err(FileSystemError::InvalidPatch(format!("invalid hunk header `{header}`")));
    };
    let start = range.split(',').next().unwrap_or(range);
    let parsed = start
        .parse::<usize>()
        .map_err(|_| FileSystemError::InvalidPatch(format!("invalid hunk header `{header}`")))?;
    Ok(parsed.max(1))
}

fn apply_file_patch(
    path: &str,
    original: &str,
    patch: &FilePatch,
) -> Result<String, FileSystemError> {
    let original_lines: Vec<String> = original.lines().map(str::to_string).collect();
    let mut output = Vec::new();
    let mut cursor = 0usize;

    for hunk in &patch.hunks {
        let target = hunk.old_start.saturating_sub(1);
        while cursor < target && cursor < original_lines.len() {
            output.push(original_lines[cursor].clone());
            cursor += 1;
        }

        for line in &hunk.lines {
            match line {
                HunkLine::Context(text) => {
                    verify_line(path, &original_lines, cursor, text)?;
                    output.push(text.clone());
                    cursor += 1;
                }
                HunkLine::Remove(text) => {
                    verify_line(path, &original_lines, cursor, text)?;
                    cursor += 1;
                }
                HunkLine::Add(text) => output.push(text.clone()),
            }
        }
    }

    while cursor < original_lines.len() {
        output.push(original_lines[cursor].clone());
        cursor += 1;
    }

    let mut content = output.join("\n");
    if !content.is_empty() {
        content.push('\n');
    }
    Ok(content)
}

fn verify_line(
    path: &str,
    original_lines: &[String],
    cursor: usize,
    expected: &str,
) -> Result<(), FileSystemError> {
    if original_lines.get(cursor).is_some_and(|line| line == expected) {
        return Ok(());
    }
    Err(FileSystemError::PatchMismatch { path: path.to_string(), line: cursor + 1 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn rejects_parent_segments() {
        assert!(normalize_relative_path("../outside.txt").is_err());
    }

    #[test]
    fn resolve_path_rejects_workspace_escape() {
        let root = temp_root("escape");
        let result = resolve_path(Some(&root), "../outside.txt");
        assert!(matches!(result, Err(FileSystemError::InvalidPath(_))));
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn write_string_uses_sibling_tmp_file_name() {
        let root = temp_root("write");
        fs::write(root.join("note.tmp"), "keep").expect("seed sibling tmp");

        write_string(Some(&root), "note.md", "new content").await.expect("write should succeed");

        assert_eq!(fs::read_to_string(root.join("note.md")).unwrap(), "new content");
        assert_eq!(fs::read_to_string(root.join("note.tmp")).unwrap(), "keep");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn applies_simple_patch() {
        let patch = FilePatch {
            old_path: Some("a.txt".to_string()),
            new_path: Some("a.txt".to_string()),
            hunks: vec![Hunk {
                old_start: 1,
                lines: vec![
                    HunkLine::Context("one".to_string()),
                    HunkLine::Remove("two".to_string()),
                    HunkLine::Add("three".to_string()),
                ],
            }],
        };

        let applied = apply_file_patch("a.txt", "one\ntwo\n", &patch).expect("patch applies");
        assert_eq!(applied, "one\nthree\n");
    }

    #[test]
    fn apply_unified_patch_updates_file_and_reports_mismatch() {
        let root = temp_root("patch");
        fs::write(root.join("a.txt"), "one\ntwo\n").expect("seed file");
        let patch = "\
--- a/a.txt
+++ b/a.txt
@@ -1,2 +1,2 @@
 one
-two
+three
";

        let result = apply_unified_patch(&root, patch).expect("patch applies");
        assert_eq!(result.applied_files, vec!["a.txt"]);
        assert_eq!(fs::read_to_string(root.join("a.txt")).unwrap(), "one\nthree\n");

        let mismatch = apply_unified_patch(&root, patch).expect_err("patch should not reapply");
        assert!(matches!(mismatch, FileSystemError::PatchMismatch { .. }));
        let _ = fs::remove_dir_all(root);
    }

    fn temp_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root =
            std::env::temp_dir().join(format!("slab_file_{name}_{}_{}", std::process::id(), nonce));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }
}
