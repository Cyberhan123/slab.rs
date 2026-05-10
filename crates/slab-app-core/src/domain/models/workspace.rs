use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGitStatusEntry {
    pub path: String,
    pub original_path: Option<String>,
    pub status: WorkspaceGitFileStatus,
    pub staged: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceConsoleOutput {
    pub command: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceWriteFileCommand {
    pub relative_path: String,
    pub content: String,
    pub expected_hash: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceWriteFileView {
    pub relative_path: String,
    pub size_bytes: u64,
    pub content_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGitPathCommand {
    pub path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGitCommitCommand {
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGitOperationView {
    pub status: WorkspaceGitStatusView,
}
