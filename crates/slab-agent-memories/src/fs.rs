use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Utc};

use crate::{Result, error::fs_error, phase2::Phase2Input};

pub const RAW_MEMORIES_FILE: &str = "raw_memories.md";
pub const PHASE2_WORKSPACE_DIFF_FILE: &str = "phase2_workspace_diff.md";
pub const PROJECTS_DIR: &str = "projects";
const LEGACY_ADOPTED_MARKER: &str = ".projects-adopted";
const PROJECT_KEY_MAX_LEN: usize = 120;
/// MEMORY.md registry guard (Claude's dual truncation): both a line cap and
/// a byte cap, because long single lines slip past a line-only limit.
pub const MEMORY_REGISTRY_FILE: &str = "MEMORY.md";
pub const MEMORY_MAX_LINES: usize = 200;
pub const MEMORY_MAX_BYTES: usize = 25 * 1024;

/// Sanitize a project identity (canonical git root or workspace root) into a
/// single filesystem path segment: lowercase, every non-`[a-z0-9]` run
/// collapsed to one `-`, trimmed of leading/trailing `-`, capped in length.
/// An empty identity (no workspace bound) sanitizes to `_global`.
pub fn sanitize_project_key(project: &str) -> String {
    let mut sanitized = String::with_capacity(project.len().min(PROJECT_KEY_MAX_LEN));
    let mut pending_dash = false;
    // Lowercase FIRST so drive letters / PascalCase segments survive as
    // information instead of being dropped as non-[a-z] characters.
    for character in project.to_lowercase().chars() {
        if sanitized.len() >= PROJECT_KEY_MAX_LEN {
            break;
        }
        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            if pending_dash && !sanitized.is_empty() {
                sanitized.push('-');
            }
            pending_dash = false;
            sanitized.push(character);
        } else {
            pending_dash = true;
        }
    }
    if sanitized.is_empty() { "_global".to_owned() } else { sanitized }
}

/// The per-project memory workspace: `<memory_root>/projects/<key>`. The
/// parent `memory_root` stays the configured anchor (tool extra-roots,
/// config) while each project owns the full legacy workspace shape.
pub fn project_memory_root(memory_root: &Path, project_key: &str) -> PathBuf {
    memory_root.join(PROJECTS_DIR).join(sanitize_project_key(project_key))
}

/// Enforce the MEMORY.md registry limits in CODE (not just prompt): over
/// `MEMORY_MAX_LINES` lines OR `MEMORY_MAX_BYTES` bytes triggers a
/// deterministic truncation — byte-safe, cut back to the last newline, with
/// a marker line appended. Returns `true` when the file was truncated.
///
/// The phase2 consolidation agent is instructed to stay within the budget,
/// but a prompt is not a guarantee; this is the Claude-style post-write
/// guard that makes the invariant hold regardless of model behavior.
pub fn enforce_memory_registry_limits(project_root: &Path) -> Result<bool> {
    let registry_path = project_root.join(MEMORY_REGISTRY_FILE);
    let Ok(contents) = std::fs::read_to_string(&registry_path) else {
        return Ok(false);
    };
    let line_count = contents.lines().count();
    let byte_len = contents.len();
    if line_count <= MEMORY_MAX_LINES && byte_len <= MEMORY_MAX_BYTES {
        return Ok(false);
    }
    let mut truncated = String::new();
    for (index, line) in contents.lines().enumerate() {
        if index >= MEMORY_MAX_LINES || truncated.len() + line.len() + 1 > MEMORY_MAX_BYTES {
            break;
        }
        truncated.push_str(line);
        truncated.push('\n');
    }
    truncated.push_str("<!-- memory registry truncated to line/byte budget -->\n");
    std::fs::write(&registry_path, &truncated).map_err(|error| fs_error(&registry_path, error))?;
    Ok(true)
}

/// One-time move of a legacy flat memory workspace (`<memory_root>/MEMORY.md`
/// and friends, pre-project-sharding layout) into `projects/<project_key>/`.
///
/// Guarded by a `.projects-adopted` marker file: after the first run (or when
/// no legacy artifacts exist) this is a cheap marker stat. The move is
/// same-volume `fs::rename` per entry — never a copy-then-delete — and any
/// failure propagates so the caller can retry on the next startup instead of
/// silently half-adopting.
///
/// Returns `true` when a legacy workspace was moved (the caller should then
/// backfill `project_key` on existing DB rows), `false` when there was
/// nothing to do.
pub fn adopt_legacy_layout(memory_root: &Path, project_key: &str) -> Result<bool> {
    let marker = memory_root.join(LEGACY_ADOPTED_MARKER);
    if marker.exists() {
        return Ok(false);
    }
    let legacy_entries = [
        "memory_summary.md",
        "MEMORY.md",
        RAW_MEMORIES_FILE,
        "rollout_summaries",
        "skills",
        "extensions",
        ".git",
        PHASE2_WORKSPACE_DIFF_FILE,
    ];
    let has_legacy = legacy_entries.iter().any(|entry| memory_root.join(entry).exists());
    let projects_dir = memory_root.join(PROJECTS_DIR);
    if !has_legacy || projects_dir.exists() {
        // Nothing to adopt (fresh install) or the sharded layout already
        // exists — write the marker so future calls are a single stat.
        std::fs::create_dir_all(memory_root).map_err(|error| fs_error(memory_root, error))?;
        std::fs::write(&marker, b"adopted\n").map_err(|error| fs_error(&marker, error))?;
        return Ok(false);
    }
    let target = project_memory_root(memory_root, project_key);
    std::fs::create_dir_all(&target).map_err(|error| fs_error(&target, error))?;
    for entry in legacy_entries {
        let source = memory_root.join(entry);
        if !source.exists() {
            continue;
        }
        let destination = target.join(entry);
        std::fs::rename(&source, &destination).map_err(|error| fs_error(&source, error))?;
    }
    std::fs::write(&marker, b"adopted\n").map_err(|error| fs_error(&marker, error))?;
    Ok(true)
}

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
            "## Thread {}\n\nsession_id: {}\nsummary_file: rollout_summaries/{}\nsource_updated_at: {}\ngenerated_at: {}\n\n{}\n\n",
            input.thread_id,
            input.session_id,
            summary_filename(&input),
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
    fn raw_memories_include_summary_file_routing_line() {
        let now = Utc.with_ymd_and_hms(2026, 6, 11, 0, 0, 0).unwrap();
        let mut slug_input = input("slug-thread", now);
        slug_input.rollout_slug = Some("my-slug".to_owned());

        let rendered = render_raw_memories(&[slug_input, input("plain-thread", now)]);

        assert!(rendered.contains("summary_file: rollout_summaries/my-slug.md"));
        assert!(rendered.contains("summary_file: rollout_summaries/plain-thread.md"));
    }

    #[test]
    fn sanitize_project_key_collapses_and_lowercases() {
        assert_eq!(sanitize_project_key("C:\\Users\\han\\Repo"), "c-users-han-repo");
        assert_eq!(sanitize_project_key("MyRepo"), "myrepo");
        assert_eq!(sanitize_project_key("/home/han/../slab.rs"), "home-han-slab-rs");
        assert_eq!(sanitize_project_key("---"), "_global");
        assert_eq!(sanitize_project_key(""), "_global");
        let long = "a".repeat(500);
        assert_eq!(sanitize_project_key(&long).len(), PROJECT_KEY_MAX_LEN);
    }

    #[test]
    fn project_root_uses_global_for_empty_key() {
        assert_eq!(
            project_memory_root(Path::new("memories"), ""),
            Path::new("memories").join("projects").join("_global")
        );
        assert_eq!(
            project_memory_root(Path::new("memories"), "My-Repo"),
            Path::new("memories").join("projects").join("my-repo")
        );
    }

    #[test]
    fn adopt_moves_legacy_files_once() {
        let root = tempfile::tempdir().expect("tempdir");
        std::fs::write(root.path().join("MEMORY.md"), "# legacy").expect("legacy registry");
        std::fs::write(root.path().join("memory_summary.md"), "v1\nlegacy").expect("summary");
        let summaries = root.path().join("rollout_summaries");
        std::fs::create_dir_all(&summaries).expect("summaries");
        std::fs::write(summaries.join("old.md"), "old").expect("summary file");

        let adopted = adopt_legacy_layout(root.path(), "my-repo").expect("adopt");
        assert!(adopted);

        let project = root.path().join("projects").join("my-repo");
        assert!(project.join("MEMORY.md").exists());
        assert!(project.join("memory_summary.md").exists());
        assert!(project.join("rollout_summaries").join("old.md").exists());
        assert!(!root.path().join("MEMORY.md").exists());

        // Second call is a marker-stat no-op, even if new legacy-looking
        // files appear at the parent level afterwards.
        std::fs::write(root.path().join("MEMORY.md"), "# stray").expect("stray");
        let again = adopt_legacy_layout(root.path(), "other-repo").expect("adopt again");
        assert!(!again);
        assert!(root.path().join("MEMORY.md").exists(), "marker guards re-adoption");
    }

    #[test]
    fn adopt_is_noop_for_fresh_install() {
        let root = tempfile::tempdir().expect("tempdir");

        let adopted = adopt_legacy_layout(root.path(), "my-repo").expect("adopt");

        assert!(!adopted);
        assert!(root.path().join(".projects-adopted").exists());
        assert!(!root.path().join("projects").exists(), "no empty project dir created");
    }

    #[test]
    fn truncates_oversized_memory_md_at_line_boundary() {
        let root = tempfile::tempdir().expect("tempdir");
        let oversized: String =
            (0..MEMORY_MAX_LINES + 50).map(|index| format!("line {index}\n")).collect();
        std::fs::write(root.path().join(MEMORY_REGISTRY_FILE), &oversized).expect("registry");

        let truncated = enforce_memory_registry_limits(root.path()).expect("enforce limits");

        assert!(truncated);
        let contents =
            std::fs::read_to_string(root.path().join(MEMORY_REGISTRY_FILE)).expect("read back");
        assert!(contents.lines().count() <= MEMORY_MAX_LINES + 1);
        assert!(contents.ends_with("<!-- memory registry truncated to line/byte budget -->\n"));
        // Cut lands on a line boundary: the marker is its own line.
        assert!(contents.contains("line 199\n"));
    }

    #[test]
    fn truncates_memory_md_on_byte_budget_with_long_lines() {
        let root = tempfile::tempdir().expect("tempdir");
        // Few lines, but each huge: a line-only cap would let this through.
        let oversized: String =
            (0..10).map(|index| format!("{index}: {}\n", "x".repeat(5000))).collect();
        std::fs::write(root.path().join(MEMORY_REGISTRY_FILE), &oversized).expect("registry");

        let truncated = enforce_memory_registry_limits(root.path()).expect("enforce limits");

        assert!(truncated);
        let contents =
            std::fs::read_to_string(root.path().join(MEMORY_REGISTRY_FILE)).expect("read back");
        assert!(contents.len() <= MEMORY_MAX_BYTES + 100, "byte budget holds");
        assert!(contents.contains("truncated to line/byte budget"));
    }

    #[test]
    fn keeps_small_memory_md() {
        let root = tempfile::tempdir().expect("tempdir");
        std::fs::write(root.path().join(MEMORY_REGISTRY_FILE), "# Memory\nsmall\n")
            .expect("registry");

        let truncated = enforce_memory_registry_limits(root.path()).expect("enforce limits");

        assert!(!truncated);
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
