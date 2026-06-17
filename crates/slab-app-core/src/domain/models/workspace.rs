use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub type WorkspaceGitStatusView = slab_git::GitStatus;
pub type WorkspaceGitStatusSummary = slab_git::GitStatusSummary;
pub type WorkspaceGitStatusEntry = slab_git::GitStatusEntry;
pub type WorkspaceGitFileStatus = slab_git::GitFileStatus;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceConsoleOutput {
    pub command: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceWriteFileCommand {
    pub relative_path: String,
    pub content: String,
    pub expected_hash: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceWriteFileView {
    pub relative_path: String,
    pub size_bytes: u64,
    pub content_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDirectoryView {
    pub relative_path: String,
    pub entries: Vec<WorkspaceFileEntry>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFileSearchView {
    pub query: String,
    pub entries: Vec<WorkspaceFileEntry>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTextSearchView {
    pub query: String,
    pub matches: Vec<WorkspaceTextSearchFileMatch>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTextSearchFileMatch {
    pub relative_path: String,
    pub name: String,
    pub line_matches: Vec<WorkspaceTextSearchLineMatch>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTextSearchLineMatch {
    pub line_number: usize,
    pub line_text: String,
    pub match_start: usize,
    pub match_end: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFileEntry {
    pub id: String,
    pub name: String,
    pub relative_path: String,
    pub kind: WorkspaceFileKind,
    pub has_children: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceFileKind {
    Directory,
    File,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePathMetadata {
    pub relative_path: String,
    pub kind: WorkspaceFileKind,
    pub size_bytes: u64,
    pub modified_at: u64,
    pub created_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFileContent {
    pub relative_path: String,
    pub name: String,
    pub content: String,
    pub size_bytes: u64,
    pub content_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCreateFileCommand {
    pub relative_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCreateDirectoryCommand {
    pub relative_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRenamePathCommand {
    pub from_relative_path: String,
    pub to_relative_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDeletePathCommand {
    pub relative_path: String,
    pub recursive: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePathView {
    pub relative_path: String,
}

pub type WorkspaceGitPathCommand = slab_git::GitPathCommand;
pub type WorkspaceGitCommitCommand = slab_git::GitCommitCommand;
pub type WorkspaceGitDiffCommand = slab_git::GitDiffCommand;
pub type WorkspaceGitDiffView = slab_git::GitPathDiff;
pub type WorkspaceGitOperationView = slab_git::GitOperationResult;
