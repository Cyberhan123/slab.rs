use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use async_trait::async_trait;
use slab_file::{
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
        slab_file::resolve_sandbox_path_for_read(&self.context, relative_path)
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
        let path = slab_file::resolve_sandbox_path_for_read(context, path)?;
        Ok(tokio::fs::read(path).await?)
    }

    async fn write_file(
        &self,
        context: &FileSystemSandboxContext,
        path: &str,
        content: &[u8],
    ) -> Result<(), FileSystemError> {
        let path = slab_file::resolve_sandbox_path_for_write(context, path)?;
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
        let path = slab_file::resolve_sandbox_path_for_write(context, path)?;
        tokio::fs::create_dir_all(path).await?;
        Ok(())
    }

    async fn get_metadata(
        &self,
        context: &FileSystemSandboxContext,
        path: &str,
    ) -> Result<FileMetadata, FileSystemError> {
        let path = slab_file::resolve_sandbox_path_for_read(context, path)?;
        metadata_for_path(&path)
    }

    async fn read_directory(
        &self,
        context: &FileSystemSandboxContext,
        path: &str,
    ) -> Result<Vec<DirectoryEntry>, FileSystemError> {
        let relative_path = slab_file::normalize_relative_path(path)?;
        let directory = slab_file::resolve_sandbox_path_for_read(context, &relative_path)?;
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
        let path = slab_file::resolve_sandbox_path_for_write(context, path)?;
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
        let from = slab_file::resolve_sandbox_path_for_read(context, from)?;
        let to = slab_file::resolve_sandbox_path_for_write(context, to)?;
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

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[tokio::test]
    async fn local_executor_file_system_round_trips_workspace_files() {
        let root = temp_root("round_trip");
        let file_system = LocalExecutorFileSystem::new(&root).expect("filesystem");
        let context = file_system.context.clone();

        file_system.write_file(&context, "notes/today.md", b"hello").await.expect("write");
        let content = file_system.read_file(&context, "notes/today.md").await.expect("read");
        let metadata =
            file_system.get_metadata(&context, "notes/today.md").await.expect("metadata");
        let entries = file_system.read_directory(&context, "notes").await.expect("directory");

        assert_eq!(content, b"hello");
        assert!(metadata.is_file);
        assert!(!metadata.is_directory);
        assert!(entries.iter().any(|entry| entry.path == "notes/today.md"));
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn local_executor_file_system_rejects_workspace_escapes() {
        let root = temp_root("escape");
        let file_system = LocalExecutorFileSystem::new(&root).expect("filesystem");
        let context = file_system.context.clone();

        let read = file_system.read_file(&context, "../outside.txt").await;
        let write = file_system.write_file(&context, "../outside.txt", b"outside").await;

        assert!(matches!(read, Err(FileSystemError::InvalidPath(_))));
        assert!(matches!(write, Err(FileSystemError::InvalidPath(_))));
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn local_executor_file_system_read_only_rejects_mutations() {
        let root = temp_root("read_only");
        fs::write(root.join("source.txt"), "source").expect("seed source");
        fs::write(root.join("delete.txt"), "delete").expect("seed delete");
        let file_system = LocalExecutorFileSystem::new(&root).expect("filesystem");
        let mut context = file_system.context.clone();
        context.policy = FileSystemSandboxPolicy::ReadOnly;

        let write = file_system.write_file(&context, "created.txt", b"created").await;
        let create_dir = file_system.create_directory(&context, "created-dir").await;
        let remove = file_system.remove(&context, "delete.txt", RemoveOptions::default()).await;
        let copy =
            file_system.copy(&context, "source.txt", "copied.txt", CopyOptions::default()).await;

        assert!(matches!(write, Err(FileSystemError::PermissionDenied(_))));
        assert!(matches!(create_dir, Err(FileSystemError::PermissionDenied(_))));
        assert!(matches!(remove, Err(FileSystemError::PermissionDenied(_))));
        assert!(matches!(copy, Err(FileSystemError::PermissionDenied(_))));
        assert!(root.join("delete.txt").is_file());
        assert!(!root.join("copied.txt").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn local_executor_file_system_denied_paths_override_workspace_access() {
        let root = temp_root("denied");
        fs::create_dir_all(root.join("secret")).expect("secret dir");
        fs::write(root.join("secret").join("note.md"), "secret").expect("secret file");
        let file_system = LocalExecutorFileSystem::new(&root).expect("filesystem");
        let mut context = file_system.context.clone();
        context.denied_paths.push(root.join("secret"));

        let read = file_system.read_file(&context, "secret/note.md").await;
        let write = file_system.write_file(&context, "secret/new.md", b"new").await;

        assert!(matches!(read, Err(FileSystemError::PermissionDenied(_))));
        assert!(matches!(write, Err(FileSystemError::PermissionDenied(_))));
        assert!(!root.join("secret").join("new.md").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn local_executor_file_system_allows_configured_writable_roots() {
        let root = temp_root("workspace_root");
        let writable = temp_root("writable_root");
        let file_system = LocalExecutorFileSystem::new(&root).expect("filesystem");
        let mut context = file_system.context.clone();
        context.writable_roots.push(writable.clone());
        let writable_path = writable.join("created.txt");

        file_system
            .write_file(&context, &writable_path.to_string_lossy(), b"created")
            .await
            .expect("write into configured root");
        let content = file_system
            .read_file(&context, &writable_path.to_string_lossy())
            .await
            .expect("read from configured root");

        assert_eq!(content, b"created");
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(writable);
    }

    #[tokio::test]
    async fn local_executor_file_system_denied_paths_override_writable_roots() {
        let root = temp_root("workspace_root_denied");
        let writable = temp_root("writable_root_denied");
        fs::create_dir_all(writable.join("secret")).expect("secret dir");
        let file_system = LocalExecutorFileSystem::new(&root).expect("filesystem");
        let mut context = file_system.context.clone();
        context.writable_roots.push(writable.clone());
        context.denied_paths.push(writable.join("secret"));
        let denied_path = writable.join("secret").join("created.txt");

        let write =
            file_system.write_file(&context, &denied_path.to_string_lossy(), b"created").await;

        assert!(matches!(write, Err(FileSystemError::PermissionDenied(_))));
        assert!(!denied_path.exists());
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(writable);
    }

    fn temp_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!(
            "slab_workspace_fs_{name}_{}_{}",
            std::process::id(),
            nonce
        ));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }
}
