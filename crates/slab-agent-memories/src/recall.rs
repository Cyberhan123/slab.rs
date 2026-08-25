//! Read-side relevant-memory recall.
//!
//! Two-phase selection in the Claude memdir style: SCAN a manifest of the
//! project's memory files (frontmatter from `raw_memories.md` unioned with a
//! `rollout_summaries/` mtime scan), then let a small model side query PICK
//! the handful of summaries relevant to the current request. No embeddings,
//! no vector store — the manifest is small and the model is the judge.
//!
//! Everything here is pure + filesystem-read-only so the host can wrap it in
//! caching/timeouts without the crate learning about models or pools.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, Duration, Utc};

use crate::{Result, error::fs_error};

/// Maximum files the side query may choose.
pub const RECALL_TOP_K: usize = 5;
/// Manifest cap (Claude's MAX_MEMORY_FILES equivalent).
pub const RECALL_MANIFEST_CAP: usize = 200;
/// Entries older than this render a staleness warning (point-in-time records).
pub const RECALL_STALENESS_DAYS: i64 = 1;
/// Token budget for the always-on `memory_summary.md` injection (Codex parity).
pub const SUMMARY_TOKEN_BUDGET: usize = 2500;
/// Per-entry token budget for selected summaries.
pub const RECALL_ENTRY_TOKEN_BUDGET: usize = SUMMARY_TOKEN_BUDGET / RECALL_TOP_K;
/// chars-per-token heuristic — the same one `slab-agent`'s compaction uses.
const CHARS_PER_TOKEN: usize = 4;
const TRUNCATION_MARKER: &str = "<!-- truncated to memory token budget -->";

/// One memory file as the side query sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecallManifestEntry {
    /// Workspace-relative path, always `rollout_summaries/<name>.md`.
    pub filename: String,
    /// One-line description (frontmatter `description:` or first heading).
    pub title: String,
    pub keywords: String,
    pub cwd: String,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Build the selection manifest for a project memory root.
///
/// Sources, unioned by filename (frontmatter wins, dir scan fills gaps):
/// 1. `raw_memories.md` `## Thread` sections — the `summary_file:` /
///    `description:` / `keywords:` / `cwd:` / `source_updated_at:` header
///    lines phase1 + phase2 sync write.
/// 2. `rollout_summaries/*.md` readdir — mtime + first `# ` heading, so
///    summaries that predate the routing line still surface.
///
/// Sorted newest-first, capped at [`RECALL_MANIFEST_CAP`].
pub fn build_manifest(project_root: &Path) -> Result<Vec<RecallManifestEntry>> {
    let mut by_file: BTreeMap<String, RecallManifestEntry> = BTreeMap::new();

    if let Some(raw) = read_optional(&project_root.join(crate::fs::RAW_MEMORIES_FILE))? {
        for entry in parse_raw_memories_manifest(&raw) {
            by_file.insert(entry.filename.clone(), entry);
        }
    }

    let summaries_dir = project_root.join("rollout_summaries");
    if let Ok(entries) = std::fs::read_dir(&summaries_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let filename = format!("rollout_summaries/{name}");
            if by_file.contains_key(&filename) {
                continue;
            }
            let updated_at = entry
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .map(DateTime::<Utc>::from);
            let title = read_optional(&path)?
                .and_then(|body| {
                    body.lines()
                        .find_map(|line| line.strip_prefix("# ").map(str::trim).map(str::to_owned))
                })
                .unwrap_or_default();
            by_file.insert(
                filename.clone(),
                RecallManifestEntry {
                    filename,
                    title,
                    keywords: String::new(),
                    cwd: String::new(),
                    updated_at,
                },
            );
        }
    }

    let mut manifest: Vec<RecallManifestEntry> = by_file.into_values().collect();
    manifest.sort_by_key(|entry| std::cmp::Reverse(entry.updated_at));
    manifest.truncate(RECALL_MANIFEST_CAP);
    Ok(manifest)
}

/// Parse `## Thread` header sections out of `raw_memories.md`.
///
/// Only the metadata lines between the heading and the first blank-line
/// separator are consumed — the raw memory bodies below them are the phase2
/// agent's business, not the manifest's.
fn parse_raw_memories_manifest(raw: &str) -> Vec<RecallManifestEntry> {
    let mut entries = Vec::new();
    let mut current: Option<RecallManifestEntry> = None;
    let mut in_header = false;
    // A section is `## Thread <id>` / blank / sync metadata (session_id,
    // summary_file, …) / blank / the raw memory's own pseudo-frontmatter
    // (description, keywords, cwd, …) / blank / prose body. The metadata
    // region extends across blanks until the first line that is neither
    // blank nor `key: value` — the prose body — so both metadata blocks are
    // harvested for the manifest while the body is ignored.
    for line in raw.lines() {
        if let Some(thread) = line.strip_prefix("## Thread ") {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
            current = Some(RecallManifestEntry {
                filename: format!("rollout_summaries/{thread}.md"),
                title: String::new(),
                keywords: String::new(),
                cwd: String::new(),
                updated_at: None,
            });
            in_header = true;
            continue;
        }
        let Some(entry) = current.as_mut() else { continue };
        if !in_header || line.trim().is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            in_header = false;
            continue;
        };
        let value = value.trim();
        match key {
            "summary_file" => entry.filename = value.to_owned(),
            "description" => entry.title = value.to_owned(),
            "keywords" => entry.keywords = value.to_owned(),
            "cwd" => entry.cwd = value.to_owned(),
            "source_updated_at" => {
                entry.updated_at = DateTime::parse_from_rfc3339(value)
                    .ok()
                    .map(|parsed| parsed.with_timezone(&Utc))
            }
            _ => {}
        }
    }
    if let Some(entry) = current {
        entries.push(entry);
    }
    entries
}

/// Render the side-query user prompt: manifest lines + the request.
pub fn render_manifest_prompt(
    entries: &[RecallManifestEntry],
    input_message: &str,
    cwd: &str,
    now: DateTime<Utc>,
) -> String {
    let mut prompt = String::from("Memory manifest (newest first):\n");
    for entry in entries {
        prompt.push_str(&format!(
            "- {} | {} | keywords: {} | cwd: {} | {}\n",
            entry.filename,
            if entry.title.is_empty() { "(no description)" } else { &entry.title },
            if entry.keywords.is_empty() { "-" } else { &entry.keywords },
            if entry.cwd.is_empty() { "-" } else { &entry.cwd },
            freshness_label(entry.updated_at, now),
        ));
    }
    prompt.push_str(&format!(
        "\nWorkspace: {cwd}\n\nUser request:\n{input_message}\n\n\
         Reply ONLY with JSON: {{\"filenames\": [...]}} listing up to {RECALL_TOP_K} \
         manifest filenames most relevant to the request, most relevant first. \
         Use EXACT filenames from the manifest; if none are relevant reply \
         with an empty list."
    ));
    prompt
}

/// Parse the side-query model output into a valid selection.
///
/// Accepts either a bare JSON array or `{"filenames": [...]}`. Hallucinated
/// filenames (not in the manifest) are dropped — the manifest is the
/// ground truth (Claude's hallucination filter); model order is kept; the
/// result is capped at [`RECALL_TOP_K`].
pub fn parse_recall_selection(model_output: &str, manifest: &[RecallManifestEntry]) -> Vec<String> {
    let valid: std::collections::BTreeSet<&str> =
        manifest.iter().map(|entry| entry.filename.as_str()).collect();
    // Tolerate markdown-fenced model output ("```json\n{...}\n```").
    let trimmed = model_output.trim();
    let trimmed = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed)
        .trim();
    let trimmed = trimmed.strip_suffix("```").unwrap_or(trimmed).trim();
    let parsed: Option<Vec<String>> = serde_json::from_str(trimmed).ok().or_else(|| {
        serde_json::from_str::<serde_json::Value>(trimmed).ok().and_then(|value| {
            value
                .get("filenames")
                .and_then(|names| serde_json::from_value::<Vec<String>>(names.clone()).ok())
        })
    });
    let Some(names) = parsed else {
        return Vec::new();
    };
    names.into_iter().filter(|name| valid.contains(name.as_str())).take(RECALL_TOP_K).collect()
}

/// Precomputed freshness label ("saved 3 hours ago").
///
/// Computed ONCE at render time and frozen into the injected text so the
/// label cannot drift between turns and churn the prompt cache (Claude's
/// memoryAge rationale).
pub fn freshness_label(updated: Option<DateTime<Utc>>, now: DateTime<Utc>) -> String {
    let Some(updated) = updated else {
        return "saved at unknown time".to_owned();
    };
    let elapsed = now.signed_duration_since(updated);
    if elapsed < Duration::zero() {
        return "saved just now".to_owned();
    }
    let minutes = elapsed.num_minutes();
    if minutes < 1 {
        "saved just now".to_owned()
    } else if minutes < 60 {
        format!("saved {minutes} minutes ago")
    } else if minutes < 24 * 60 {
        format!("saved {} hours ago", minutes / 60)
    } else {
        format!("saved {} days ago", minutes / (24 * 60))
    }
}

/// Point-in-time records older than [`RECALL_STALENESS_DAYS`] get a warning.
pub fn is_stale(updated: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
    updated.is_some_and(|updated| {
        now.signed_duration_since(updated) > Duration::days(RECALL_STALENESS_DAYS)
    })
}

/// Cut `text` to `budget_tokens` (chars/4 heuristic), backing off to the last
/// newline so the cut never lands mid-line, and stamping a marker.
pub fn truncate_to_token_budget(text: &str, budget_tokens: usize) -> String {
    let max_chars = budget_tokens.saturating_mul(CHARS_PER_TOKEN);
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let prefix: String = text.chars().take(max_chars).collect();
    let cut = match prefix.char_indices().rev().find(|(_, c)| *c == '\n').map(|(idx, _)| idx) {
        Some(newline) => &prefix[..newline],
        None => prefix.as_str(),
    };
    format!("{cut}\n{TRUNCATION_MARKER}\n")
}

/// Render the selected summaries as the `slab_memory_relevant` fragment body.
///
/// Each entry gets a heading with its (frozen) freshness label and a
/// staleness warning when older than a day, and is individually capped at
/// [`RECALL_ENTRY_TOKEN_BUDGET`].
pub fn render_selected_entries(
    project_root: &Path,
    filenames: &[String],
    now: DateTime<Utc>,
) -> Result<String> {
    let mut rendered = String::new();
    for filename in filenames {
        let Some(name) = filename.strip_prefix("rollout_summaries/") else {
            continue;
        };
        let path = project_root.join("rollout_summaries").join(name);
        let Some(body) = read_optional(&path)? else {
            continue;
        };
        let metadata = std::fs::metadata(&path).ok().and_then(|meta| meta.modified().ok());
        let updated = metadata.map(DateTime::<Utc>::from);
        let stale = if is_stale(updated, now) {
            " — STALE: may no longer match the codebase"
        } else {
            ""
        };
        rendered.push_str(&format!(
            "### {filename} ({}{})\n\n{}\n\n",
            freshness_label(updated, now),
            stale,
            truncate_to_token_budget(body.trim(), RECALL_ENTRY_TOKEN_BUDGET).trim_end()
        ));
    }
    Ok(rendered)
}

fn read_optional(path: &Path) -> Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
        Ok(_) => Ok(None),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(fs_error(path, error)),
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone as _;

    use super::*;

    fn entry(filename: &str, updated_at: Option<DateTime<Utc>>) -> RecallManifestEntry {
        RecallManifestEntry {
            filename: filename.to_owned(),
            title: format!("about {filename}"),
            keywords: "rust".to_owned(),
            cwd: "C:/repo".to_owned(),
            updated_at,
        }
    }

    #[test]
    fn manifest_parses_raw_memories_sections() {
        let raw = "# Raw Memories\n\n\
            ## Thread thread-1\n\n\
            session_id: s1\n\
            summary_file: rollout_summaries/my-slug.md\n\
            source_updated_at: 2026-08-20T10:00:00Z\n\
            generated_at: 2026-08-20T10:05:00Z\n\n\
            description: fixed the parser bug\n\
            task: parser fix\n\
            cwd: C:/repo\n\
            keywords: parser, regex\n\n\
            raw body line\n\n\
            ## Thread thread-2\n\n\
            session_id: s2\n\n\
            other body\n";

        let parsed = parse_raw_memories_manifest(raw);

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].filename, "rollout_summaries/my-slug.md");
        assert_eq!(parsed[0].title, "fixed the parser bug");
        assert_eq!(parsed[0].keywords, "parser, regex");
        assert_eq!(parsed[0].cwd, "C:/repo");
        assert!(parsed[0].updated_at.is_some());
        // Section without routing falls back to the thread-id guess.
        assert_eq!(parsed[1].filename, "rollout_summaries/thread-2.md");
    }

    #[test]
    fn manifest_unions_dir_scan_and_orders_by_recency() {
        let root = tempfile::tempdir().expect("tempdir");
        let summaries = root.path().join("rollout_summaries");
        std::fs::create_dir_all(&summaries).expect("dir");
        std::fs::write(summaries.join("older.md"), "# Older summary\nbody").expect("older");
        std::fs::write(summaries.join("newer.md"), "# Newer summary\nbody").expect("newer");
        std::fs::write(
            root.path().join(crate::fs::RAW_MEMORIES_FILE),
            "# Raw Memories\n\n## Thread routed\n\nsummary_file: rollout_summaries/routed.md\nsource_updated_at: 2026-08-01T00:00:00Z\n\nbody\n",
        )
        .expect("raw");

        let manifest = build_manifest(root.path()).expect("manifest");

        let names: Vec<&str> = manifest.iter().map(|e| e.filename.as_str()).collect();
        assert!(names.contains(&"rollout_summaries/routed.md"));
        assert!(names.contains(&"rollout_summaries/older.md"));
        assert!(names.contains(&"rollout_summaries/newer.md"));
        // Dir-scan entries carry their first heading as the title.
        let newer = manifest.iter().find(|e| e.filename.ends_with("newer.md")).expect("newer");
        assert_eq!(newer.title, "Newer summary");
    }

    #[test]
    fn selection_filters_hallucinated_filenames_and_caps_top_k() {
        let manifest = (0..8)
            .map(|index| entry(&format!("rollout_summaries/file-{index}.md"), None))
            .collect::<Vec<_>>();

        let selected = parse_recall_selection(
            "{\"filenames\": [\"rollout_summaries/file-7.md\", \"made/up.md\", \
              \"rollout_summaries/file-2.md\", \"rollout_summaries/file-0.md\", \
              \"rollout_summaries/file-1.md\", \"rollout_summaries/file-3.md\", \
              \"rollout_summaries/file-4.md\"]}",
            &manifest,
        );

        assert_eq!(
            selected,
            vec![
                "rollout_summaries/file-7.md".to_owned(),
                "rollout_summaries/file-2.md".to_owned(),
                "rollout_summaries/file-0.md".to_owned(),
                "rollout_summaries/file-1.md".to_owned(),
                "rollout_summaries/file-3.md".to_owned(),
            ],
            "hallucinated dropped, model order kept, capped at RECALL_TOP_K"
        );
    }

    #[test]
    fn selection_accepts_bare_array_and_garbage_is_empty() {
        let manifest = vec![entry("rollout_summaries/a.md", None)];
        assert_eq!(
            parse_recall_selection("[\"rollout_summaries/a.md\"]", &manifest),
            vec!["rollout_summaries/a.md".to_owned()]
        );
        assert!(parse_recall_selection("I think a.md is relevant", &manifest).is_empty());
        // Fenced output is unwrapped before parsing.
        assert_eq!(
            parse_recall_selection(
                "```json\n{\"filenames\": [\"rollout_summaries/a.md\"]}\n```",
                &manifest
            ),
            vec!["rollout_summaries/a.md".to_owned()]
        );
    }

    #[test]
    fn truncation_cuts_at_last_newline() {
        let text = "line one\nline two\nline three\n";

        let truncated = truncate_to_token_budget(text, 3);

        // 3 tokens * 4 chars = 12 chars budget; the cut backs off to the end
        // of line one and stamps the marker.
        assert!(truncated.starts_with("line one\n"));
        assert!(truncated.contains(TRUNCATION_MARKER));
        assert!(!truncated.contains("line three"));
        // Within budget: unchanged.
        assert_eq!(truncate_to_token_budget("short", 100), "short");
    }

    #[test]
    fn freshness_labels_days_and_staleness() {
        let now = Utc.with_ymd_and_hms(2026, 8, 26, 12, 0, 0).unwrap();
        assert_eq!(freshness_label(Some(now - Duration::minutes(30)), now), "saved 30 minutes ago");
        assert_eq!(freshness_label(Some(now - Duration::hours(3)), now), "saved 3 hours ago");
        assert_eq!(freshness_label(Some(now - Duration::days(2)), now), "saved 2 days ago");
        assert_eq!(freshness_label(None, now), "saved at unknown time");
        assert!(!is_stale(Some(now - Duration::hours(3)), now));
        assert!(is_stale(Some(now - Duration::days(2)), now));
        assert!(!is_stale(None, now));
    }

    #[test]
    fn selected_entries_render_headings_and_stale_warning() {
        let root = tempfile::tempdir().expect("tempdir");
        let summaries = root.path().join("rollout_summaries");
        std::fs::create_dir_all(&summaries).expect("dir");
        std::fs::write(summaries.join("a.md"), "# Did a thing\nthe body").expect("a");
        // Far-future "now": the file's real mtime is unambiguously >1 day old.
        let now = Utc.with_ymd_and_hms(2100, 1, 1, 0, 0, 0).unwrap();

        let rendered = render_selected_entries(
            root.path(),
            &["rollout_summaries/a.md".to_owned(), "rollout_summaries/missing.md".to_owned()],
            now,
        )
        .expect("rendered");

        assert!(rendered.contains("### rollout_summaries/a.md (saved"));
        assert!(rendered.contains("the body"));
        assert!(rendered.contains("STALE"), "entries older than the staleness window warn");
    }
}
