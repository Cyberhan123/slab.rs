use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Utc};

use crate::{Result, error::fs_error, phase2::Phase2Input};

pub const RAW_MEMORIES_FILE: &str = "raw_memories.md";
pub const PHASE2_WORKSPACE_DIFF_FILE: &str = "phase2_workspace_diff.md";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryWorkspaceSync {
    pub raw_memories_path: PathBuf,
    pub summaries_dir: PathBuf,
    pub removed_summaries: Vec<PathBuf>,
    pub pruned_extension_resources: Vec<PathBuf>,
}

pub fn sync_phase2_workspace(
    memory_root: &Path,
    inputs: &[Phase2Input],
    extension_retention_days: i64,
    now: DateTime<Utc>,
) -> Result<MemoryWorkspaceSync> {
    ensure_dir(memory_root)?;
    let summaries_dir = memory_root.join("rollout_summaries");
    ensure_dir(&summaries_dir)?;

    let raw_memories_path = memory_root.join(RAW_MEMORIES_FILE);
    write_file(&raw_memories_path, &render_raw_memories(inputs))?;

    let mut expected_summaries = BTreeSet::new();
    for input in inputs {
        let filename = summary_filename(input);
        expected_summaries.insert(filename.clone());
        write_file(&summaries_dir.join(filename), &input.rollout_summary)?;
    }

    let removed_summaries = remove_stale_summaries(&summaries_dir, &expected_summaries)?;
    let pruned_extension_resources =
        prune_extension_resources(memory_root, extension_retention_days, now)?;

    Ok(MemoryWorkspaceSync {
        raw_memories_path,
        summaries_dir,
        removed_summaries,
        pruned_extension_resources,
    })
}

pub fn render_raw_memories(inputs: &[Phase2Input]) -> String {
    if inputs.is_empty() {
        return "# Raw Memories\n\nNo selected Phase 1 memories.\n".to_owned();
    }

    let mut sorted = inputs.to_vec();
    sorted.sort_by(|left, right| left.thread_id.cmp(&right.thread_id));
    let mut output = String::from("# Raw Memories\n\n");
    for input in sorted {
        output.push_str(&format!(
            "## Thread {}\n\nsession_id: {}\nsource_updated_at: {}\ngenerated_at: {}\n\n{}\n\n",
            input.thread_id,
            input.session_id,
            input.source_updated_at.to_rfc3339(),
            input.generated_at.to_rfc3339(),
            input.raw_memory.trim()
        ));
    }
    output
}

pub fn summary_filename(input: &Phase2Input) -> String {
    let stem = input
        .rollout_slug
        .as_deref()
        .filter(|slug| !slug.trim().is_empty())
        .unwrap_or(&input.thread_id);
    format!("{stem}.md")
}

fn ensure_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path).map_err(|error| fs_error(path, error))
}

fn write_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    let mut content = content.to_owned();
    if !content.ends_with('\n') {
        content.push('\n');
    }
    std::fs::write(path, content).map_err(|error| fs_error(path, error))
}

fn remove_stale_summaries(
    summaries_dir: &Path,
    expected: &BTreeSet<String>,
) -> Result<Vec<PathBuf>> {
    let mut removed = Vec::new();
    let entries =
        std::fs::read_dir(summaries_dir).map_err(|error| fs_error(summaries_dir, error))?;
    for entry in entries {
        let entry = entry.map_err(|error| fs_error(summaries_dir, error))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if expected.contains(file_name) {
            continue;
        }
        std::fs::remove_file(&path).map_err(|error| fs_error(&path, error))?;
        removed.push(path);
    }
    Ok(removed)
}

fn prune_extension_resources(
    memory_root: &Path,
    retention_days: i64,
    now: DateTime<Utc>,
) -> Result<Vec<PathBuf>> {
    if retention_days < 0 {
        return Ok(Vec::new());
    }
    let extensions_dir = memory_root.join("extensions");
    if !extensions_dir.exists() {
        return Ok(Vec::new());
    }
    let cutoff = now - Duration::days(retention_days);
    let mut pruned = Vec::new();
    for entry in
        walkdir::WalkDir::new(&extensions_dir).into_iter().filter_map(std::result::Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if !path
            .components()
            .any(|component| component.as_os_str().to_string_lossy() == "resources")
        {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        let modified = DateTime::<Utc>::from(modified);
        if modified >= cutoff {
            continue;
        }
        std::fs::remove_file(path).map_err(|error| fs_error(path, error))?;
        pruned.push(path.to_path_buf());
    }
    Ok(pruned)
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone as _;

    use super::*;

    #[test]
    fn raw_memories_are_rendered_by_thread_id() {
        let now = Utc.with_ymd_and_hms(2026, 6, 11, 0, 0, 0).unwrap();
        let rendered = render_raw_memories(&[input("b", now), input("a", now)]);

        assert!(rendered.find("Thread a").unwrap() < rendered.find("Thread b").unwrap());
    }

    #[test]
    fn sync_removes_stale_summary() {
        let root = tempfile::tempdir().expect("tempdir");
        let summaries = root.path().join("rollout_summaries");
        std::fs::create_dir_all(&summaries).expect("summaries");
        std::fs::write(summaries.join("stale.md"), "old").expect("stale");
        let now = Utc.with_ymd_and_hms(2026, 6, 11, 0, 0, 0).unwrap();

        let result =
            sync_phase2_workspace(root.path(), &[input("thread", now)], 30, now).expect("sync");

        assert_eq!(result.removed_summaries.len(), 1);
        assert!(root.path().join(RAW_MEMORIES_FILE).exists());
        assert!(summaries.join("thread.md").exists());
        assert!(!summaries.join("stale.md").exists());
    }

    #[test]
    fn sync_writes_empty_placeholder_when_selection_is_empty() {
        let root = tempfile::tempdir().expect("tempdir");
        let summaries = root.path().join("rollout_summaries");
        std::fs::create_dir_all(&summaries).expect("summaries");
        std::fs::write(summaries.join("stale.md"), "old").expect("stale");
        let now = Utc.with_ymd_and_hms(2026, 6, 11, 0, 0, 0).unwrap();

        sync_phase2_workspace(root.path(), &[], 30, now).expect("sync");

        let raw = std::fs::read_to_string(root.path().join(RAW_MEMORIES_FILE)).expect("raw");
        assert!(raw.contains("No selected Phase 1 memories."));
        assert!(!summaries.join("stale.md").exists());
    }

    #[test]
    fn sync_prunes_expired_extension_resources() {
        let root = tempfile::tempdir().expect("tempdir");
        let resources = root.path().join("extensions").join("ad_hoc").join("resources");
        std::fs::create_dir_all(&resources).expect("resources");
        let expired = resources.join("old.txt");
        std::fs::write(&expired, "old").expect("resource");
        let future = Utc.with_ymd_and_hms(2100, 1, 1, 0, 0, 0).unwrap();

        let result = sync_phase2_workspace(root.path(), &[], 0, future).expect("sync");

        assert_eq!(result.pruned_extension_resources, vec![expired.clone()]);
        assert!(!expired.exists());
    }

    fn input(thread_id: &str, now: DateTime<Utc>) -> Phase2Input {
        Phase2Input {
            thread_id: thread_id.to_owned(),
            session_id: "session".to_owned(),
            raw_memory: format!("memory {thread_id}"),
            rollout_summary: format!("summary {thread_id}"),
            rollout_slug: None,
            generated_at: now,
            source_updated_at: now,
            last_usage: None,
            usage_count: 0,
        }
    }
}
