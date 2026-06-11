use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{MemoryError, Result, fs::PHASE2_WORKSPACE_DIFF_FILE};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryGitDiff {
    pub diff_path: PathBuf,
    pub diff: String,
}

pub fn ensure_memory_git_baseline(memory_root: &Path) -> Result<()> {
    std::fs::create_dir_all(memory_root)
        .map_err(|error| crate::error::fs_error(memory_root, error))?;
    if !memory_root.join(".git").exists() {
        run_git(memory_root, &["init"])?;
        run_git(memory_root, &["config", "user.email", "memory@slab.local"])?;
        run_git(memory_root, &["config", "user.name", "Slab Memory"])?;
    }
    Ok(())
}

pub fn write_workspace_diff(memory_root: &Path) -> Result<MemoryGitDiff> {
    ensure_memory_git_baseline(memory_root)?;
    run_git(memory_root, &["add", "-N", "."])?;
    let diff = git_output(memory_root, &["diff", "--", "."])?;
    let diff_path = memory_root.join(PHASE2_WORKSPACE_DIFF_FILE);
    std::fs::write(&diff_path, &diff).map_err(|error| crate::error::fs_error(&diff_path, error))?;
    Ok(MemoryGitDiff { diff_path, diff })
}

pub fn reset_memory_git_baseline(memory_root: &Path) -> Result<()> {
    remove_workspace_diff_file(memory_root)?;
    ensure_memory_git_baseline(memory_root)?;
    run_git(memory_root, &["add", "-A"])?;
    let status = git_output(memory_root, &["status", "--porcelain"])?;
    if status.trim().is_empty() {
        return Ok(());
    }
    run_git(memory_root, &["commit", "-m", "slab memory baseline"])?;
    Ok(())
}

pub fn remove_workspace_diff_file(memory_root: &Path) -> Result<()> {
    let diff_path = memory_root.join(PHASE2_WORKSPACE_DIFF_FILE);
    match std::fs::remove_file(&diff_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(crate::error::fs_error(&diff_path, error)),
    }
}

fn git_output(root: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git").arg("-C").arg(root).args(args).output().map_err(|error| {
        MemoryError::Git(format!("failed to run git {}: {error}", args.join(" ")))
    })?;
    if !output.status.success() {
        return Err(MemoryError::Git(output_message(&output)));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn run_git(root: &Path, args: &[&str]) -> Result<()> {
    git_output(root, args).map(|_| ())
}

fn output_message(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if !stderr.is_empty() {
        return stderr;
    }
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_diff_and_resets_baseline_when_git_is_available() {
        let root = tempfile::tempdir().expect("tempdir");
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }

        ensure_memory_git_baseline(root.path()).expect("baseline");
        std::fs::write(root.path().join("MEMORY.md"), "hello\n").expect("write");
        let diff = write_workspace_diff(root.path()).expect("diff");
        assert!(diff.diff.contains("+hello"));
        reset_memory_git_baseline(root.path()).expect("reset");
        let clean = git_output(root.path(), &["status", "--porcelain"]).expect("status");
        assert!(clean.trim().is_empty());
    }

    #[test]
    fn removes_workspace_diff_file_if_present() {
        let root = tempfile::tempdir().expect("tempdir");
        let diff_path = root.path().join(PHASE2_WORKSPACE_DIFF_FILE);
        std::fs::write(&diff_path, "diff").expect("diff");

        remove_workspace_diff_file(root.path()).expect("remove");
        remove_workspace_diff_file(root.path()).expect("remove missing");

        assert!(!diff_path.exists());
    }
}
