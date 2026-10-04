use minijinja::{Environment, context};

use crate::recall::{RecallManifestEntry, freshness_label};
use crate::{MemoryError, Result};

pub const PHASE1_SYSTEM_TEMPLATE: &str = include_str!("../templates/memories/system.md");
pub const PHASE1_INPUT_TEMPLATE: &str = include_str!("../templates/memories/input.md");
pub const PHASE2_CONSOLIDATION_TEMPLATE: &str =
    include_str!("../templates/memories/consolidation.md");
/// The read-side developer prompt: usage policy, memory-workspace layout, the
/// `<oai-mem-citation>` telemetry contract (parsed back by
/// `crate::read::parse_memory_citations`), the update policy, and the wrapped
/// MEMORY_SUMMARY. This crate owns the complete `slab_memory` body — the
/// context hook injects it verbatim.
pub const MEMORY_READ_TEMPLATE: &str = include_str!("../templates/memories/read.md");
/// The recall side-query user prompt (manifest lines + the request).
pub const RECALL_MANIFEST_TEMPLATE: &str = include_str!("../templates/memories/recall-manifest.md");
/// The static `memory` built-in agent stub prompt (see
/// `render_memory_agent_prompt`).
pub const MEMORY_AGENT_TEMPLATE: &str = include_str!("../templates/memories/agent.md");
pub const RECALL_TEMPLATE: &str = include_str!("../templates/memories/recall.md");
pub const RECALL_SELECT_TEMPLATE: &str = include_str!("../templates/memories/recall-select.md");
pub const HOOK_INSTRUCTIONS_TEMPLATE: &str = include_str!("../templates/hooks/instructions.md");

pub fn render_phase1_input(
    rollout_path: &str,
    rollout_cwd: &str,
    rollout_contents: &str,
) -> Result<String> {
    render(
        PHASE1_INPUT_TEMPLATE,
        context! {
            rollout_path => rollout_path,
            rollout_cwd => rollout_cwd,
            rollout_contents => rollout_contents,
        },
    )
}

pub fn render_phase2_consolidation(
    memory_root: &str,
    phase2_workspace_diff_file: &str,
    memory_extensions_folder_structure: &str,
    memory_extensions_primary_inputs: &str,
) -> Result<String> {
    let mut rendered = render(
        PHASE2_CONSOLIDATION_TEMPLATE,
        context! {
            memory_root => memory_root,
            phase2_workspace_diff_file => phase2_workspace_diff_file,
            memory_extensions_folder_structure => memory_extensions_folder_structure,
            memory_extensions_primary_inputs => memory_extensions_primary_inputs,
        },
    )?;
    // The ad-hoc notes extension instructions ride with the consolidation
    // prompt: the phase2 sub-agent is transient and never receives the
    // read-side memory fragment, so this is their only consumer.
    rendered.push_str("\n\n");
    rendered.push_str(HOOK_INSTRUCTIONS_TEMPLATE);
    Ok(rendered)
}

/// Wrap the recall-selected summaries as the `slab_memory_relevant` body.
pub fn render_memory_relevant(base_path: &str, body: &str) -> Result<String> {
    render(
        RECALL_TEMPLATE,
        context! {
            base_path => base_path,
            body => body,
        },
    )
}

/// The side-query system prompt for recall selection.
pub fn render_recall_select(top_k: usize) -> Result<String> {
    render(
        RECALL_SELECT_TEMPLATE,
        context! {
            top_k => top_k,
        },
    )
}

/// The complete `slab_memory` developer-message body: usage policy,
/// memory-workspace layout, the `<oai-mem-citation>` telemetry contract, the
/// update policy, and the wrapped MEMORY_SUMMARY.
///
/// The output must stay byte-stable — the context hook injects it under the
/// `slab_memory` tag and the rollout merge / prompt-cache prefix rely on the
/// rendered body not drifting between runs.
pub fn render_memory_read(base_path: &str, memory_summary: &str) -> Result<String> {
    render(
        MEMORY_READ_TEMPLATE,
        context! {
            base_path => base_path,
            memory_summary => memory_summary,
        },
    )
}

/// A display-normalized manifest line for [`RECALL_MANIFEST_TEMPLATE`]:
/// fallbacks (`(no description)` / `-`) and the frozen freshness label are
/// precomputed so the template only lays out fields.
#[derive(serde::Serialize)]
struct RecallManifestLine {
    filename: String,
    title: String,
    keywords: String,
    cwd: String,
    freshness: String,
}

/// The recall side-query user prompt: manifest lines + the request. Replaces
/// the former hardcoded formatter in `crate::recall`; the output is
/// byte-identical to it.
pub fn render_recall_manifest_prompt(
    entries: &[RecallManifestEntry],
    input_message: &str,
    cwd: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<String> {
    let lines: Vec<RecallManifestLine> = entries
        .iter()
        .map(|entry| RecallManifestLine {
            filename: entry.filename.clone(),
            title: if entry.title.is_empty() {
                "(no description)".to_owned()
            } else {
                entry.title.clone()
            },
            keywords: if entry.keywords.is_empty() {
                "-".to_owned()
            } else {
                entry.keywords.clone()
            },
            cwd: if entry.cwd.is_empty() { "-".to_owned() } else { entry.cwd.clone() },
            freshness: freshness_label(entry.updated_at, now),
        })
        .collect();
    render(
        RECALL_MANIFEST_TEMPLATE,
        context! {
            lines => lines,
            cwd => cwd,
            input_message => input_message,
            top_k => crate::recall::RECALL_TOP_K,
        },
    )
}

/// The static stub system prompt for the `memory` built-in agent type. The
/// real consolidation runs always override it with the per-run
/// [`render_phase2_consolidation`] prompt; the stub only keeps a stray
/// `agent_type="memory"` selection harmless.
pub fn render_memory_agent_prompt() -> Result<String> {
    render(MEMORY_AGENT_TEMPLATE, context! {})
}

fn render(template: &str, context: minijinja::Value) -> Result<String> {
    let env = Environment::new();
    env.render_str(template, context).map_err(|error| MemoryError::Template(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read::{MemoryCitationSourceKind, parse_memory_citations, parse_memory_rollout_ids};
    use chrono::TimeZone;

    #[test]
    fn renders_phase1_input_template() {
        let rendered =
            render_phase1_input("rollout.jsonl", "C:/repo", "user: hi").expect("rendered");

        assert!(rendered.contains("rollout_path: rollout.jsonl"));
        assert!(rendered.contains("rollout_cwd: C:/repo"));
        assert!(rendered.contains("user: hi"));
    }

    #[test]
    fn renders_consolidation_with_ad_hoc_note_instructions() {
        let rendered =
            render_phase2_consolidation("C:/memories/projects/p", "diff.md", "", "").expect("r");

        assert!(rendered.contains("Memory Writing Agent"));
        // The ad-hoc notes extension instructions must reach the (transient)
        // consolidation agent — it never sees the read-side memory fragment.
        assert!(rendered.contains("# Ad-hoc notes"));
        assert!(rendered.contains("Never delete a note file."));
        assert!(rendered.contains("[ad-hoc note]"));
    }

    #[test]
    fn renders_memory_read_with_base_path_and_summary() {
        let body = render_memory_read(
            "C:/memories/projects/slab-rs",
            "v1\n# Summary\nprefers minimal diffs",
        )
        .expect("renders");

        // The memory-workspace routes interpolate the base path.
        assert!(body.contains("C:/memories/projects/slab-rs/MEMORY.md"));
        assert!(body.contains("C:/memories/projects/slab-rs/rollout_summaries/"));
        // The summary is wrapped verbatim.
        assert!(body.contains("========= MEMORY_SUMMARY BEGINS ========="));
        assert!(body.contains("prefers minimal diffs"));
        assert!(body.contains("========= MEMORY_SUMMARY ENDS ========="));
    }

    #[test]
    fn keeps_the_parseable_citation_and_update_contracts() {
        let body = render_memory_read("/memories/projects/p", "v1\ns").expect("renders");

        // The citation block must stay parseable by this crate's
        // `parse_memory_citations` regexes.
        assert!(body.contains("<oai-mem-citation>"));
        assert!(body.contains("<citation_entries>"));
        assert!(
            body.contains("MEMORY.md:234-236|note=[responsesapi citation extraction code pointer]")
        );
        assert!(body.contains("<rollout_ids>"));
        assert!(body.contains("VERY LAST content"));
        assert!(body.contains("Do not cite `memory_summary.md`"));
        // The write path routes through the memory_note tool.
        assert!(body.contains("`memory_note` tool"));
        assert!(body.contains("consolidation applies"));
    }

    /// The prompt (this crate's template) and the parser (`crate::read`) now
    /// live in one crate: the example block the prompt shows must round-trip
    /// through the parser unchanged — previously this contract was split
    /// across `slab-agent-context` (render) and here (parse).
    #[test]
    fn memory_read_citation_example_round_trips_through_the_parser() {
        let body = render_memory_read("/memories/projects/p", "v1\ns").expect("renders");

        let citations = parse_memory_citations(&body);
        assert_eq!(citations.len(), 2, "both example entries parse");
        assert_eq!(citations[0].source, "MEMORY.md:234-236");
        assert_eq!(citations[0].source_kind, MemoryCitationSourceKind::MemoryRegistry);
        assert_eq!(
            citations[0].note.as_deref(),
            Some("responsesapi citation extraction code pointer")
        );
        assert_eq!(
            citations[1].source,
            "rollout_summaries/2026-02-17T21-23-02-LN3m-example.md:10-12"
        );
        assert_eq!(citations[1].source_kind, MemoryCitationSourceKind::RolloutSummary);
        assert_eq!(citations[1].note.as_deref(), Some("weekly report format"));

        let rollout_ids = parse_memory_rollout_ids(&body);
        assert_eq!(rollout_ids, vec!["019c6e27-e55b-73d1-87d8-4e01f1f75043".to_owned()]);
    }

    /// Full-body byte pin: the injected `slab_memory` developer body must not
    /// drift silently — it is keyed by tag in the rollout merge and feeds the
    /// prompt-cache prefix. The expected literal below pins content (LF) and
    /// the CRLF line-ending style of `read.md` at the same time. If this test
    /// fails, the template changed on purpose: update the pin consciously.
    #[test]
    fn memory_read_body_is_byte_stable() {
        let rendered = render_memory_read(
            "C:/memories/projects/slab-rs",
            "v1\n# Summary\nprefers minimal diffs",
        )
        .expect("renders");
        assert!(rendered.contains("\r\n"), "read.md keeps CRLF line endings");

        let expected_lf = r#"## Memory

Use the injected MEMORY_SUMMARY as historical context: apply the user's actual
preferences, corrections, decisions, and supported task scope. Its exact
rollout, source, pull-request, discussion, and document pointers can guide
independently useful work without an extra lookup merely to rediscover them.

Memory layout (general -> specific), relative to `C:/memories/projects/slab-rs`:

- memory_summary.md (already provided below; do NOT open again)
- MEMORY.md (searchable registry; the primary file to query)
- skills/<skill-name>/ (reusable procedures; SKILL.md entrypoint, optional
  scripts/, templates/, examples/)
- rollout_summaries/<slug>.md (per-rollout recaps + evidence snippets)
  - The source rollout paths appear in MEMORY.md and the summaries as `rollout_path`
  - Those rollout files are append-only `jsonl`: `session_meta.payload.id` identifies the session, `turn_context` marks turn boundaries, and `response_item` contains actual messages, tool calls, and tool outputs.
  - For efficient lookup, prefer matching the filename suffix or `session_meta.payload.id`; avoid broad full-content scans unless needed.

Read a matching rollout under `C:/memories/projects/slab-rs/rollout_summaries/` when its
additional evidence, wording, chronology, or uncertainty could change your
answer; otherwise do not retrieve history speculatively. Search selectively
when a genuinely needed route is missing: query `C:/memories/projects/slab-rs/MEMORY.md`
first, then open only the 1-2 most relevant rollout summaries or skills it
points to. Keep the pass lightweight (ideally <= 4-6 search steps before the
main work, no broad scans of all rollout summaries) and batch independent
useful lookups. If there are no relevant hits, stop memory lookup and continue
normally; redo the quick pass if repeated errors or confusing behavior suggest
relevant prior context.

Memory is not proof of current behavior. For consequential or changeable
claims, use judgment about drift, verification cost, and harm; inspect the
actual owning source when warranted and acknowledge material uncertainty. When
you rely on a fact you did not verify this turn, say so briefly and do not
present it as confirmed-current.

Memory citations:

When any memory file actually used informs the answer, append exactly one
citation block as the VERY LAST content of the final reply, outside code
fences. Do not cite `memory_summary.md` (it is injected, not read) or
workspace files, and never include memory citations inside pull-request
messages. Use this exact structure for programmatic parsing:

```
<oai-mem-citation>
<citation_entries>
MEMORY.md:234-236|note=[responsesapi citation extraction code pointer]
rollout_summaries/2026-02-17T21-23-02-LN3m-example.md:10-12|note=[weekly report format]
</citation_entries>
<rollout_ids>
019c6e27-e55b-73d1-87d8-4e01f1f75043
</rollout_ids>
</oai-mem-citation>
```

Use actual source paths relative to `C:/memories/projects/slab-rs` and line ranges from the
search or read, with one entry per line and short single-line notes
(`<file>:<line_start>-<line_end>|note=[<how memory was used>]`, most important
first, never blank lines). Cite every memory file you actually used. Include
unique relevant rollout UUIDs already available (rollout summary files and
MEMORY.md carry them); leave `rollout_ids` empty if none are available. Do not
reread files or make extra tool calls solely to construct or check citations
or obtain rollout IDs.

Updating memories:

Update memory only when the user explicitly asks — never unprompted. For an
explicit remember, forget, or correction request, call the `memory_note` tool
once with the requested addition, deletion, or correction (one small note per
request). Do not edit generated memory files directly; consolidation applies
these notes.

========= MEMORY_SUMMARY BEGINS =========
__SUMMARY__
========= MEMORY_SUMMARY ENDS ========="#;
        // Template lines are CRLF; the substituted summary keeps its own LF
        // endings verbatim; minijinja strips the template's trailing newline
        // (so the body ends right after the ENDS marker).
        let expected = expected_lf
            .replace('\n', "\r\n")
            .replace("__SUMMARY__", "v1\n# Summary\nprefers minimal diffs");
        assert_eq!(rendered, expected);
    }

    /// The templatized manifest prompt must be byte-identical to the former
    /// hardcoded formatter in `crate::recall` (same lines, same trailing JSON
    /// instruction, no trailing newline).
    #[test]
    fn recall_manifest_prompt_matches_legacy_formatter() {
        let now = chrono::Utc.with_ymd_and_hms(2026, 10, 1, 12, 0, 0).unwrap();
        let entries = vec![
            RecallManifestEntry {
                filename: "rollout_summaries/2026-10-01T10-00-00-abc-thread-one.md".to_owned(),
                title: "Fixed flaky auth test".to_owned(),
                keywords: "auth, tests".to_owned(),
                cwd: "C:/repo".to_owned(),
                updated_at: Some(now - chrono::Duration::hours(3)),
            },
            RecallManifestEntry {
                filename: "rollout_summaries/other.md".to_owned(),
                title: String::new(),
                keywords: String::new(),
                cwd: String::new(),
                updated_at: None,
            },
        ];

        let rendered = render_recall_manifest_prompt(
            &entries,
            "How did we fix the flaky test?",
            "C:/repo",
            now,
        )
        .expect("renders");

        let expected = "Memory manifest (newest first):\n\
- rollout_summaries/2026-10-01T10-00-00-abc-thread-one.md | Fixed flaky auth test | keywords: auth, tests | cwd: C:/repo | saved 3 hours ago\n\
- rollout_summaries/other.md | (no description) | keywords: - | cwd: - | saved at unknown time\n\
\n\
Workspace: C:/repo\n\
\n\
User request:\n\
How did we fix the flaky test?\n\
\n\
Reply ONLY with JSON: {\"filenames\": [...]} listing up to 5 manifest filenames most relevant to the request, most relevant first. Use EXACT filenames from the manifest; if none are relevant reply with an empty list.";
        assert_eq!(rendered, expected);
    }

    #[test]
    fn renders_memory_agent_stub_prompt() {
        let rendered = render_memory_agent_prompt().expect("renders");

        assert!(rendered.contains("memory consolidation agent"));
        assert!(rendered.contains("no consolidation was requested"));
    }
}
