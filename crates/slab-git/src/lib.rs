//! Git helpers shared by Slab agent and workspace integrations.

mod repository;
mod types;

pub use repository::GitRepository;
pub use types::{
    GitCommitCommand, GitCommitOptions, GitCommitResult, GitDiff, GitDiffCommand, GitError,
    GitFileStatus, GitOperationResult, GitPathCommand, GitPathDiff, GitRepositoryMetadata,
    GitStatus, GitStatusEntry, GitStatusSummary,
};
