use std::path::Path;
use std::sync::LazyLock;
use std::time::UNIX_EPOCH;

use async_trait::async_trait;
use slab_file::{
    CopyOptions, DirectoryEntry, ExecutorFileSystem, FileMetadata, FileSystemError,
    FileSystemSandboxContext, RemoveOptions, resolve_path,
};

pub(crate) struct LocalFs;

pub(crate) static LOCAL_FS: LocalFs = LocalFs;

impl AsRef<LocalExecutorFileSystem> for LocalFs {
    fn as_ref(&self) -> &LocalExecutorFileSystem {
        static INSTANCE: LazyLock<LocalExecutorFileSystem> =
            LazyLock::new(|| LocalExecutorFileSystem);
        &INSTANCE
    }
}

pub(crate) struct LocalExecutorFileSystem;

#[async_trait]
impl ExecutorFileSystem for LocalExecutorFileSystem {
    async fn read_file(
        &self,
        context: &FileSystemSandboxContext,
        path: &str,
    ) -> Result<Vec<u8>, FileSystemError> {
        let path = resolve_path(context.workspace_root.as_deref(), path)?;
        Ok(tokio::fs::read(path).await?)
    }

    async fn write_file(
        &self,
        context: &FileSystemSandboxContext,
        path: &str,
        content: &[u8],
    ) -> Result<(), FileSystemError> {
        let path = resolve_path(context.workspace_root.as_deref(), path)?;
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
        let path = resolve_path(context.workspace_root.as_deref(), path)?;
        tokio::fs::create_dir_all(path).await?;
        Ok(())
    }

    async fn get_metadata(
        &self,
        context: &FileSystemSandboxContext,
        path: &str,
    ) -> Result<FileMetadata, FileSystemError> {
        let path = resolve_path(context.workspace_root.as_deref(), path)?;
        metadata_for_path(&path).await
    }

    async fn read_directory(
        &self,
        context: &FileSystemSandboxContext,
        path: &str,
    ) -> Result<Vec<DirectoryEntry>, FileSystemError> {
        let path = resolve_path(context.workspace_root.as_deref(), path)?;
        let mut read_dir = tokio::fs::read_dir(path).await?;
        let mut entries = Vec::new();
        while let Some(entry) = read_dir.next_entry().await? {
            let metadata = metadata_for_path(&entry.path()).await?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = name.clone();
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
        let path = resolve_path(context.workspace_root.as_deref(), path)?;
        if path.is_dir() {
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
        let from = resolve_path(context.workspace_root.as_deref(), from)?;
        let to = resolve_path(context.workspace_root.as_deref(), to)?;
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

async fn metadata_for_path(path: &Path) -> Result<FileMetadata, FileSystemError> {
    let metadata = tokio::fs::metadata(path).await?;
    let symlink_metadata = tokio::fs::symlink_metadata(path).await?;
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
