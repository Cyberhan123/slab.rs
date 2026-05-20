use serde::{Deserialize, Serialize};

pub type WorkspaceGitStatusView = slab_git::GitStatus;
pub type WorkspaceGitStatusSummary = slab_git::GitStatusSummary;
pub type WorkspaceGitStatusEntry = slab_git::GitStatusEntry;
pub type WorkspaceGitFileStatus = slab_git::GitFileStatus;

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
pub struct WorkspaceCreateFileCommand {
    pub relative_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCreateDirectoryCommand {
    pub relative_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRenamePathCommand {
    pub from_relative_path: String,
    pub to_relative_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDeletePathCommand {
    pub relative_path: String,
    pub recursive: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePathView {
    pub relative_path: String,
}

pub type WorkspaceGitPathCommand = slab_git::GitPathCommand;
pub type WorkspaceGitCommitCommand = slab_git::GitCommitCommand;
pub type WorkspaceGitDiffCommand = slab_git::GitDiffCommand;
pub type WorkspaceGitDiffView = slab_git::GitPathDiff;
pub type WorkspaceGitOperationView = slab_git::GitOperationResult;
