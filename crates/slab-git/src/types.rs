use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

#[derive(Debug, Error)]
pub enum GitError {
    #[error("Git is not available on PATH")]
    GitUnavailable,
    #[error("not a Git repository: {0}")]
    NotRepository(String),
    #[error("invalid path: {0}")]
    InvalidPath(String),
    #[error("git command failed: {0}")]
    CommandFailed(String),
    #[error("repository discovery failed: {0}")]
    RepositoryDiscovery(String),
    #[error("repository status failed: {0}")]
    RepositoryStatus(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub enum GitFileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Untracked,
    Conflicted,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct GitStatusSummary {
    pub added: usize,
    pub modified: usize,
    pub deleted: usize,
    pub renamed: usize,
    pub copied: usize,
    pub untracked: usize,
    pub conflicted: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct GitStatusEntry {
    pub path: String,
    pub original_path: Option<String>,
    pub status: GitFileStatus,
    pub staged: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    pub available: bool,
    pub is_repository: bool,
    pub branch: Option<String>,
    pub repository_root: Option<String>,
    pub message: Option<String>,
    pub summary: GitStatusSummary,
    pub entries: Vec<GitStatusEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct GitRepositoryMetadata {
    pub repository_root: String,
    pub git_dir: String,
    pub branch: Option<String>,
    pub discovered_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct GitDiff {
    pub path: Option<String>,
    pub staged: bool,
    pub diff: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct GitPathDiff {
    pub path: String,
    pub staged: bool,
    pub diff: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitResult {
    pub status: GitStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct GitOperationResult {
    pub status: GitStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct GitPathCommand {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitCommand {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct GitDiffCommand {
    pub path: String,
    pub staged: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GitCommitOptions {
    pub auto_stage_when_index_empty: bool,
    pub push_after_clean_commit: bool,
}

impl GitCommitOptions {
    pub const fn workspace_default() -> Self {
        Self { auto_stage_when_index_empty: true, push_after_clean_commit: true }
    }

    pub const fn agent_default() -> Self {
        Self { auto_stage_when_index_empty: false, push_after_clean_commit: false }
    }
}
