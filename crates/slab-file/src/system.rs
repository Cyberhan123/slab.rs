//! Workspace-safe filesystem helpers.

use std::{
    fs,
    path::{Path, PathBuf},
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

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
    #[error("filesystem access denied: {0}")]
    PermissionDenied(String),
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

pub use slab_sandboxing::SandboxPolicy as FileSystemSandboxPolicy;

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
    if Path::new(raw).is_absolute() {
        return Err(FileSystemError::AbsolutePath(raw.to_string()));
    }

    slab_utils::path::normalize_relative_path_allow_empty(raw)
        .map_err(|_| FileSystemError::InvalidPath(raw.to_string()))
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
        let canonical_parent = slab_utils::fs::existing_ancestor(parent)?;
        if !canonical_parent.starts_with(&canonical_root) {
            return Err(FileSystemError::PathEscapesRoot(path.to_string()));
        }
    }

    Ok(candidate)
}

pub fn resolve_sandbox_path_for_read(
    context: &FileSystemSandboxContext,
    path: &str,
) -> Result<PathBuf, FileSystemError> {
    resolve_sandbox_path(context, path, SandboxAccess::Read)
}

pub fn resolve_sandbox_path_for_write(
    context: &FileSystemSandboxContext,
    path: &str,
) -> Result<PathBuf, FileSystemError> {
    resolve_sandbox_path(context, path, SandboxAccess::Write)
}

#[derive(Clone, Copy)]
enum SandboxAccess {
    Read,
    Write,
}

fn resolve_sandbox_path(
    context: &FileSystemSandboxContext,
    path: &str,
    access: SandboxAccess,
) -> Result<PathBuf, FileSystemError> {
    if matches!(access, SandboxAccess::Write)
        && matches!(context.policy, FileSystemSandboxPolicy::ReadOnly)
    {
        return Err(FileSystemError::PermissionDenied(
            "read-only filesystem sandbox refused mutation".to_string(),
        ));
    }

    let path_buf = PathBuf::from(path);
    let candidate = if path_buf.is_absolute() {
        path_buf
    } else {
        resolve_path(context.workspace_root.as_deref().or(context.cwd.as_deref()), path)?
    };
    ensure_sandbox_path_allowed(context, &candidate, access)?;
    Ok(candidate)
}

fn ensure_sandbox_path_allowed(
    context: &FileSystemSandboxContext,
    candidate: &Path,
    access: SandboxAccess,
) -> Result<(), FileSystemError> {
    ensure_sandbox_path_not_denied(context, candidate)?;
    if matches!(context.policy, FileSystemSandboxPolicy::DangerFullAccess) {
        return Ok(());
    }

    let allowed_roots = allowed_roots_for_access(context, access);
    if allowed_roots.iter().any(|root| path_is_within_policy_root(candidate, root)) {
        return Ok(());
    }

    let access_name = match access {
        SandboxAccess::Read => "read",
        SandboxAccess::Write => "write",
    };
    Err(FileSystemError::PermissionDenied(format!(
        "path is outside sandbox {access_name} roots: {}",
        candidate.display()
    )))
}

fn allowed_roots_for_access(
    context: &FileSystemSandboxContext,
    access: SandboxAccess,
) -> Vec<&Path> {
    let mut roots = Vec::new();
    if let Some(root) = context.workspace_root.as_deref() {
        roots.push(root);
    }
    match access {
        SandboxAccess::Read => {
            roots.extend(context.readable_roots.iter().map(PathBuf::as_path));
            if matches!(context.policy, FileSystemSandboxPolicy::WorkspaceWrite) {
                roots.extend(context.writable_roots.iter().map(PathBuf::as_path));
            }
        }
        SandboxAccess::Write => {
            if matches!(context.policy, FileSystemSandboxPolicy::WorkspaceWrite) {
                roots.extend(context.writable_roots.iter().map(PathBuf::as_path));
            }
        }
    }
    roots
}

fn ensure_sandbox_path_not_denied(
    context: &FileSystemSandboxContext,
    candidate: &Path,
) -> Result<(), FileSystemError> {
    for denied in &context.denied_paths {
        let denied = resolve_policy_path(context, denied)?;
        if path_is_within_policy_root(candidate, &denied) {
            return Err(FileSystemError::PermissionDenied(format!(
                "path is denied by filesystem sandbox policy: {}",
                candidate.display()
            )));
        }
    }
    Ok(())
}

fn resolve_policy_path(
    context: &FileSystemSandboxContext,
    path: &Path,
) -> Result<PathBuf, FileSystemError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    let path = path.to_string_lossy();
    resolve_path(context.workspace_root.as_deref().or(context.cwd.as_deref()), &path)
}

fn path_is_within_policy_root(path: &Path, root: &Path) -> bool {
    let Ok(path) = canonical_policy_path(path) else {
        return false;
    };
    let Ok(root) = canonical_policy_path(root) else {
        return false;
    };
    path.starts_with(root)
}

fn canonical_policy_path(path: &Path) -> Result<PathBuf, std::io::Error> {
    if path.exists() {
        return path.canonicalize();
    }

    let Some(parent) = path.parent() else {
        return Ok(path.to_path_buf());
    };
    let ancestor = slab_utils::fs::existing_ancestor(parent)?;
    let canonical_ancestor = ancestor.canonicalize()?;
    let suffix = path.strip_prefix(&ancestor).unwrap_or(Path::new(""));
    Ok(canonical_ancestor.join(suffix))
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
        file_name.push(format!(".{}.tmp", Uuid::new_v4()));
        path.with_file_name(file_name)
    };
    if let Err(error) = tokio::fs::write(&tmp_path, content).await {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return Err(FileSystemError::Io(error));
    }
    if let Err(error) = tokio::fs::rename(&tmp_path, &path).await {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return Err(FileSystemError::Io(error));
    }
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
    let path = raw.trim().split_once('\t').map_or(raw.trim(), |(path, _)| path);
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

    #[test]
    fn resolve_path_rejects_absolute_paths_when_workspace_root_is_set() {
        let root = temp_root("absolute");
        let absolute = root.join("inside.txt");
        let result = resolve_path(Some(&root), &absolute.to_string_lossy());

        assert!(matches!(result, Err(FileSystemError::AbsolutePath(_))));

        #[cfg(windows)]
        {
            let result = resolve_path(Some(&root), r"\\?\C:\Windows\system.ini");
            assert!(matches!(result, Err(FileSystemError::AbsolutePath(_))));
        }

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolve_path_rejects_parent_segments_even_when_they_stay_in_root() {
        let root = temp_root("normalize");
        fs::write(root.join("inside.txt"), "inside").expect("seed file");

        assert!(matches!(
            resolve_path(Some(&root), "dir/../inside.txt"),
            Err(FileSystemError::InvalidPath(_))
        ));
        assert!(matches!(
            resolve_path(Some(&root), "dir/../../outside.txt"),
            Err(FileSystemError::InvalidPath(_))
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolve_path_rejects_symlink_escapes_for_existing_and_new_files() {
        let root = temp_root("symlink_escape");
        let outside = temp_root("symlink_outside");
        fs::write(outside.join("secret.txt"), "secret").expect("seed outside file");
        let link = root.join("outside-link");
        if !create_dir_symlink(&outside, &link) {
            let _ = fs::remove_dir_all(root);
            let _ = fs::remove_dir_all(outside);
            return;
        }

        let existing = resolve_path(Some(&root), "outside-link/secret.txt");
        let missing = resolve_path(Some(&root), "outside-link/new.txt");

        assert!(matches!(existing, Err(FileSystemError::PathEscapesRoot(_))));
        assert!(matches!(missing, Err(FileSystemError::PathEscapesRoot(_))));
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(outside);
    }

    #[test]
    fn filesystem_sandbox_policy_uses_shared_sandbox_policy() {
        assert_eq!(FileSystemSandboxContext::default().policy, FileSystemSandboxPolicy::ReadOnly);
        assert_eq!(
            FileSystemSandboxPolicy::WorkspaceWrite,
            slab_sandboxing::SandboxPolicy::WorkspaceWrite
        );
    }

    #[test]
    fn sandbox_context_read_only_rejects_write_resolution() {
        let root = temp_root("sandbox_read_only");
        let context = sandbox_context(&root, FileSystemSandboxPolicy::ReadOnly);

        let error = resolve_sandbox_path_for_write(&context, "note.md").expect_err("write denied");

        assert!(matches!(error, FileSystemError::PermissionDenied(_)));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sandbox_context_denied_paths_override_read_and_write_roots() {
        let root = temp_root("sandbox_denied");
        let denied = root.join("secret");
        fs::create_dir_all(&denied).expect("denied dir");
        fs::write(denied.join("note.md"), "secret").expect("denied file");
        let mut context = sandbox_context(&root, FileSystemSandboxPolicy::WorkspaceWrite);
        context.writable_roots.push(root.clone());
        context.denied_paths.push(denied);

        let read =
            resolve_sandbox_path_for_read(&context, "secret/note.md").expect_err("read denied");
        let write =
            resolve_sandbox_path_for_write(&context, "secret/new.md").expect_err("write denied");

        assert!(matches!(read, FileSystemError::PermissionDenied(_)));
        assert!(matches!(write, FileSystemError::PermissionDenied(_)));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sandbox_context_workspace_write_allows_workspace_and_writable_roots() {
        let root = temp_root("sandbox_workspace");
        let writable = temp_root("sandbox_writable");
        let outside = temp_root("sandbox_outside");
        fs::write(writable.join("readable.txt"), "ok").expect("writable file");
        let mut context = sandbox_context(&root, FileSystemSandboxPolicy::WorkspaceWrite);
        context.writable_roots.push(writable.clone());

        let workspace_path =
            resolve_sandbox_path_for_write(&context, "note.md").expect("workspace write");
        let writable_path = resolve_sandbox_path_for_write(
            &context,
            &writable.join("created.txt").to_string_lossy(),
        )
        .expect("writable root write");
        let readable_path = resolve_sandbox_path_for_read(
            &context,
            &writable.join("readable.txt").to_string_lossy(),
        )
        .expect("writable root read");
        let outside_error =
            resolve_sandbox_path_for_write(&context, &outside.join("denied.txt").to_string_lossy())
                .expect_err("outside write denied");

        assert_eq!(
            workspace_path.parent().expect("workspace parent").canonicalize().expect("root"),
            root.canonicalize().expect("root")
        );
        assert_eq!(
            writable_path.parent().expect("writable parent").canonicalize().expect("writable"),
            writable.canonicalize().expect("writable")
        );
        assert_eq!(
            readable_path.canonicalize().expect("readable"),
            writable.join("readable.txt").canonicalize().expect("readable")
        );
        assert!(matches!(outside_error, FileSystemError::PermissionDenied(_)));
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(writable);
        let _ = fs::remove_dir_all(outside);
    }

    #[tokio::test]
    async fn write_string_uses_unique_sibling_tmp_file_name() {
        let root = temp_root("write");
        fs::write(root.join("note.md.tmp"), "keep").expect("seed sibling tmp");

        write_string(Some(&root), "note.md", "new content").await.expect("write should succeed");

        assert_eq!(fs::read_to_string(root.join("note.md")).unwrap(), "new content");
        assert_eq!(fs::read_to_string(root.join("note.md.tmp")).unwrap(), "keep");
        assert_tmp_files(&root, &["note.md.tmp"]);
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn write_string_removes_tmp_file_after_failed_rename() {
        let root = temp_root("write_fail");
        fs::create_dir(root.join("note.md")).expect("seed target directory");

        let error =
            write_string(Some(&root), "note.md", "new content").await.expect_err("rename fails");

        assert!(matches!(error, FileSystemError::Io(_)));
        assert_tmp_files(&root, &[]);
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
    fn parses_patch_paths_with_spaces_and_tab_metadata() {
        assert_eq!(parse_patch_path("a/dir/file name.txt"), Some("dir/file name.txt".to_string()));
        assert_eq!(
            parse_patch_path("b/dir/file name.txt\t2026-01-01 00:00:00"),
            Some("dir/file name.txt".to_string())
        );
        assert_eq!(parse_patch_path("/dev/null"), None);
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

    #[test]
    fn apply_unified_patch_handles_paths_with_spaces() {
        let root = temp_root("patch_spaces");
        fs::create_dir_all(root.join("dir")).expect("create dir");
        fs::write(root.join("dir").join("file name.txt"), "one\ntwo\n").expect("seed file");
        let patch = "\
--- a/dir/file name.txt
+++ b/dir/file name.txt
@@ -1,2 +1,2 @@
 one
-two
+three
";

        let result = apply_unified_patch(&root, patch).expect("patch applies");

        assert_eq!(result.applied_files, vec!["dir/file name.txt"]);
        assert_eq!(
            fs::read_to_string(root.join("dir").join("file name.txt")).unwrap(),
            "one\nthree\n"
        );
        let _ = fs::remove_dir_all(root);
    }

    fn temp_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root =
            std::env::temp_dir().join(format!("slab_file_{name}_{}_{}", std::process::id(), nonce));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn sandbox_context(root: &Path, policy: FileSystemSandboxPolicy) -> FileSystemSandboxContext {
        FileSystemSandboxContext {
            policy,
            cwd: Some(root.to_path_buf()),
            workspace_root: Some(root.to_path_buf()),
            readable_roots: Vec::new(),
            writable_roots: Vec::new(),
            denied_paths: Vec::new(),
        }
    }

    #[cfg(unix)]
    fn create_dir_symlink(target: &Path, link: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    fn create_dir_symlink(target: &Path, link: &Path) -> bool {
        std::os::windows::fs::symlink_dir(target, link).is_ok()
    }

    fn assert_tmp_files(root: &Path, expected: &[&str]) {
        let mut tmp_files: Vec<String> = fs::read_dir(root)
            .expect("read temp root")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".tmp"))
            .collect();
        tmp_files.sort();
        let expected: Vec<String> = expected.iter().map(|name| (*name).to_owned()).collect();
        assert_eq!(tmp_files, expected);
    }
}
