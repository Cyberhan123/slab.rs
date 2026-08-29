//! Disk spill for oversized tool results (context-budget system).
//!
//! Tools whose output exceeds the context-injection budget write the FULL
//! payload under `.slab/artifacts/<thread_id>/` and inject only a bounded
//! preview plus the artifact reference — the model can re-read the artifact
//! file when it genuinely needs the details. Mirrors the subagent artifact
//! convention so every spill lands in one place.

use std::path::Path;

use tracing::warn;

/// Write `bytes` to `<workspace>/.slab/artifacts/<thread_id>/<file_name>` and
/// return the forward-slash workspace-relative reference.
///
/// Best-effort by design: returns `None` (and the caller keeps its bounded
/// output) when there is no workspace or the write fails — a spill failure
/// must never fail the tool call itself.
pub(crate) async fn write_tool_artifact(
    workspace_root: Option<&Path>,
    thread_id: &str,
    file_name: &str,
    bytes: &[u8],
) -> Option<String> {
    let workspace_root = workspace_root?;
    let artifact_ref = format!(".slab/artifacts/{thread_id}/{file_name}");
    let artifact_path = workspace_root.join(&artifact_ref);
    if let Some(parent) = artifact_path.parent()
        && let Err(error) = tokio::fs::create_dir_all(parent).await
    {
        warn!(%error, artifact = %artifact_ref, "tool artifact parent create failed; skipping spill");
        return None;
    }
    if let Err(error) = tokio::fs::write(&artifact_path, bytes).await {
        warn!(%error, artifact = %artifact_ref, "tool artifact write failed; skipping spill");
        return None;
    }
    Some(artifact_ref)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    #[tokio::test]
    async fn artifact_helper_writes_under_slab_artifacts_and_returns_relative_ref() {
        let root = std::env::temp_dir().join(format!(
            "slab_artifact_helper_{}_{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create temp root");

        let reference = super::write_tool_artifact(
            Some(&root),
            "thread-1",
            "shell-stdout-turn2.txt",
            b"full output".as_slice(),
        )
        .await
        .expect("artifact written");

        assert_eq!(reference, ".slab/artifacts/thread-1/shell-stdout-turn2.txt");
        let written = std::fs::read(
            root.join(".slab").join("artifacts").join("thread-1").join("shell-stdout-turn2.txt"),
        )
        .expect("artifact file exists");
        assert_eq!(written, b"full output".as_slice());

        // No workspace: spill silently skipped.
        assert!(
            super::write_tool_artifact(None, "thread-1", "x.txt", b"x".as_slice()).await.is_none()
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn artifact_helper_accepts_pathbuf_roots() {
        // Compile-time sanity for the call shape used by the tools.
        fn assert_root_type(root: Option<&PathBuf>) -> Option<&Path> {
            root.map(|path| path.as_path())
        }
        assert!(assert_root_type(None).is_none());
    }
}
