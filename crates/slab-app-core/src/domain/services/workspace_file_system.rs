use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use async_trait::async_trait;
use slab_file_system::{
    CopyOptions, DirectoryEntry, ExecutorFileSystem, FileMetadata, FileSystemError,
    FileSystemSandboxContext, FileSystemSandboxPolicy, RemoveOptions,
};

/// Local filesystem implementation for workspace-scoped operations.
#[derive(Debug, Clone)]
pub struct LocalExecutorFileSystem {
    context: FileSystemSandboxContext,
}

impl LocalExecutorFileSystem {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, FileSystemError> {
        let root = root.as_ref().canonicalize().map_err(FileSystemError::Root)?;
        Ok(Self {
            context: FileSystemSandboxContext {
                policy: FileSystemSandboxPolicy::WorkspaceWrite,
                cwd: Some(root.clone()),
                workspace_root: Some(root),
                readable_roots: Vec::new(),
                writable_roots: Vec::new(),
                denied_paths: Vec::new(),
            },
        })
    }

    pub fn resolve_existing(&self, relative_path: &str) -> Result<PathBuf, FileSystemError> {
        slab_file_system::resolve_path(self.context.workspace_root.as_deref(), relative_path)
    }

    pub fn read_file_bytes(&self, relative_path: &str) -> Result<Vec<u8>, FileSystemError> {
        Ok(fs::read(self.resolve_existing(relative_path)?)?)
    }

    pub fn metadata_sync(&self, relative_path: &str) -> Result<FileMetadata, FileSystemError> {
        metadata_for_path(&self.resolve_existing(relative_path)?)
    }

    pub fn read_directory_sync(
        &self,
        relative_path: &str,
    ) -> Result<Vec<DirectoryEntry>, FileSystemError> {
        let directory = self.resolve_existing(relative_path)?;
        let mut entries = Vec::new();
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let metadata = metadata_for_path(&entry.path())?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = if relative_path.is_empty() {
                name.clone()
            } else {
                format!("{relative_path}/{name}")
            };
            entries.push(DirectoryEntry { name, path, metadata });
        }
        Ok(entries)
    }
}

#[async_trait]
impl ExecutorFileSystem for LocalExecutorFileSystem {
    async fn read_file(
        &self,
        context: &FileSystemSandboxContext,
        path: &str,
    ) -> Result<Vec<u8>, FileSystemError> {
        let path = slab_file_system::resolve_path(context.workspace_root.as_deref(), path)?;
        Ok(tokio::fs::read(path).await?)
    }

    async fn write_file(
        &self,
        context: &FileSystemSandboxContext,
        path: &str,
        content: &[u8],
    ) -> Result<(), FileSystemError> {
        let path = slab_file_system::resolve_path(context.workspace_root.as_deref(), path)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(path, content).await?;
        Ok(())
    }

    async fn create_directory(
        &self,
        context: &FileSystemSandboxContext,
        path: &str,
    ) -> Result<(), FileSystemError> {
        let path = slab_file_system::resolve_path(context.workspace_root.as_deref(), path)?;
        tokio::fs::create_dir_all(path).await?;
        Ok(())
    }

    async fn get_metadata(
        &self,
        context: &FileSystemSandboxContext,
        path: &str,
    ) -> Result<FileMetadata, FileSystemError> {
        let path = slab_file_system::resolve_path(context.workspace_root.as_deref(), path)?;
        metadata_for_path(&path)
    }

    async fn read_directory(
        &self,
        context: &FileSystemSandboxContext,
        path: &str,
    ) -> Result<Vec<DirectoryEntry>, FileSystemError> {
        let root = context.workspace_root.as_deref();
        let relative_path = slab_file_system::normalize_relative_path(path)?;
        let directory = slab_file_system::resolve_path(root, &relative_path)?;
        let mut read_dir = tokio::fs::read_dir(directory).await?;
        let mut entries = Vec::new();
        while let Some(entry) = read_dir.next_entry().await? {
            let metadata = metadata_for_path(&entry.path())?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = if relative_path.is_empty() {
                name.clone()
            } else {
                format!("{relative_path}/{name}")
            };
            entries.push(DirectoryEntry { name, path, metadata });
        }
        Ok(entries)
    }

    async fn remove(
        &self,
        context: &FileSystemSandboxContext,
        path: &str,
        options: RemoveOptions,
    ) -> Result<(), FileSystemError> {
        let path = slab_file_system::resolve_path(context.workspace_root.as_deref(), path)?;
        let metadata = tokio::fs::metadata(&path).await?;
        if metadata.is_dir() {
            if options.recursive {
                tokio::fs::remove_dir_all(path).await?;
            } else {
                tokio::fs::remove_dir(path).await?;
            }
        } else {
            tokio::fs::remove_file(path).await?;
        }
        Ok(())
    }

    async fn copy(
        &self,
        context: &FileSystemSandboxContext,
        from: &str,
        to: &str,
        options: CopyOptions,
    ) -> Result<(), FileSystemError> {
        let from = slab_file_system::resolve_path(context.workspace_root.as_deref(), from)?;
        let to = slab_file_system::resolve_path(context.workspace_root.as_deref(), to)?;
        if to.exists() && !options.overwrite {
            return Err(FileSystemError::InvalidPath(to.display().to_string()));
        }
        if let Some(parent) = to.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::copy(from, to).await?;
        Ok(())
    }
}

fn metadata_for_path(path: &Path) -> Result<FileMetadata, FileSystemError> {
    let metadata = fs::metadata(path)?;
    let symlink_metadata = fs::symlink_metadata(path)?;
    Ok(FileMetadata {
        is_file: metadata.is_file(),
        is_directory: metadata.is_dir(),
        is_symlink: symlink_metadata.file_type().is_symlink(),
        size_bytes: metadata.len(),
        modified_at: system_time_millis(metadata.modified()),
        created_at: system_time_millis(metadata.created()),
    })
}

fn system_time_millis(time: Result<std::time::SystemTime, std::io::Error>) -> u64 {
    time.ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
