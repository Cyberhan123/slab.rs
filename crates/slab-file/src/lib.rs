//! File helpers for Slab agent and workspace integrations.

pub mod search;
pub mod watcher;

mod system;

pub use system::{
    CopyOptions, DirEntryView, DirectoryEntry, ExecutorFileSystem, FileMetadata, FileSystemError,
    FileSystemSandboxContext, FileSystemSandboxPolicy, PatchApplyResult, RemoveOptions,
    apply_unified_patch, list_dir, normalize_relative_path, read_to_string, resolve_path,
    resolve_sandbox_path_for_read, resolve_sandbox_path_for_write, write_string,
};
