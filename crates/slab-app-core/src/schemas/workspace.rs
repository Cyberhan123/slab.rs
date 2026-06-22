use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

pub use crate::domain::models::{
    WorkspaceConsoleOutput, WorkspaceCreateDirectoryCommand, WorkspaceCreateFileCommand,
    WorkspaceDeletePathCommand, WorkspaceDirectoryView, WorkspaceFileContent, WorkspaceFileEntry,
    WorkspaceFileKind, WorkspaceFileSearchView, WorkspacePathMetadata, WorkspacePathView,
    WorkspaceRenamePathCommand, WorkspaceTextSearchFileMatch, WorkspaceTextSearchLineMatch,
    WorkspaceTextSearchView, WorkspaceWriteFileCommand, WorkspaceWriteFileView,
};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStateResponse {
    pub current: Option<WorkspaceInfoResponse>,
    pub recent: Vec<RecentWorkspaceResponse>,
    pub config: Option<WorkspaceConfigResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfoResponse {
    pub root_path: String,
    pub name: String,
    pub slab_dir: String,
    pub settings_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings_overlay_path: Option<String>,
    pub model_config_dir: String,
    pub session_state_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecentWorkspaceResponse {
    pub root_path: String,
    pub name: String,
    pub last_opened_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceConfigResponse {
    pub schema_version: u32,
    #[serde(default)]
    pub plugins: BTreeMap<String, WorkspacePluginConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePluginConfig {
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePluginPreferenceUpdate {
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceWatchEvent {
    pub sequence_number: u64,
    #[serde(rename = "type")]
    pub event_type: WorkspaceWatchEventType,
    pub relative_path: String,
    pub kind: WorkspaceWatchEntryKind,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceWatchEventType {
    Created,
    Changed,
    Deleted,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceWatchEntryKind {
    File,
    Directory,
    Unknown,
}

#[derive(Debug, Clone, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceOpenCommand {
    #[validate(custom(
        function = "crate::schemas::validation::validate_non_blank",
        message = "root path must not be empty"
    ))]
    pub root_path: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGitPathCommand {
    #[validate(custom(
        function = "crate::schemas::validation::validate_non_blank",
        message = "path must not be empty"
    ))]
    pub path: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGitCommitCommand {
    #[validate(custom(
        function = "crate::schemas::validation::validate_non_blank",
        message = "message must not be empty"
    ))]
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGitDiffCommand {
    #[validate(custom(
        function = "crate::schemas::validation::validate_non_blank",
        message = "path must not be empty"
    ))]
    pub path: String,
    pub staged: bool,
}

#[derive(Debug, Clone, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceConsoleRunCommand {
    #[validate(custom(
        function = "crate::schemas::validation::validate_non_blank",
        message = "command must not be empty"
    ))]
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGitStatusView {
    pub available: bool,
    pub is_repository: bool,
    pub branch: Option<String>,
    pub repository_root: Option<String>,
    pub message: Option<String>,
    pub summary: WorkspaceGitStatusSummary,
    pub entries: Vec<WorkspaceGitStatusEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGitStatusSummary {
    pub added: usize,
    pub modified: usize,
    pub deleted: usize,
    pub renamed: usize,
    pub copied: usize,
    pub untracked: usize,
    pub conflicted: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGitStatusEntry {
    pub path: String,
    pub original_path: Option<String>,
    pub status: WorkspaceGitFileStatus,
    pub staged: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceGitFileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Untracked,
    Conflicted,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGitOperationView {
    pub status: WorkspaceGitStatusView,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGitDiffView {
    pub path: String,
    pub staged: bool,
    pub diff: String,
}

impl From<slab_git::GitStatus> for WorkspaceGitStatusView {
    fn from(value: slab_git::GitStatus) -> Self {
        Self {
            available: value.available,
            is_repository: value.is_repository,
            branch: value.branch,
            repository_root: value.repository_root,
            message: value.message,
            summary: value.summary.into(),
            entries: value.entries.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<slab_git::GitStatusSummary> for WorkspaceGitStatusSummary {
    fn from(value: slab_git::GitStatusSummary) -> Self {
        Self {
            added: value.added,
            modified: value.modified,
            deleted: value.deleted,
            renamed: value.renamed,
            copied: value.copied,
            untracked: value.untracked,
            conflicted: value.conflicted,
        }
    }
}

impl From<slab_git::GitStatusEntry> for WorkspaceGitStatusEntry {
    fn from(value: slab_git::GitStatusEntry) -> Self {
        Self {
            path: value.path,
            original_path: value.original_path,
            status: value.status.into(),
            staged: value.staged,
        }
    }
}

impl From<slab_git::GitFileStatus> for WorkspaceGitFileStatus {
    fn from(value: slab_git::GitFileStatus) -> Self {
        match value {
            slab_git::GitFileStatus::Added => Self::Added,
            slab_git::GitFileStatus::Modified => Self::Modified,
            slab_git::GitFileStatus::Deleted => Self::Deleted,
            slab_git::GitFileStatus::Renamed => Self::Renamed,
            slab_git::GitFileStatus::Copied => Self::Copied,
            slab_git::GitFileStatus::Untracked => Self::Untracked,
            slab_git::GitFileStatus::Conflicted => Self::Conflicted,
        }
    }
}

impl From<slab_git::GitOperationResult> for WorkspaceGitOperationView {
    fn from(value: slab_git::GitOperationResult) -> Self {
        Self { status: value.status.into() }
    }
}

impl From<slab_git::GitPathDiff> for WorkspaceGitDiffView {
    fn from(value: slab_git::GitPathDiff) -> Self {
        Self { path: value.path, staged: value.staged, diff: value.diff }
    }
}
