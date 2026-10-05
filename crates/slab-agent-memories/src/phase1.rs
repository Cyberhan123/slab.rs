use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{MemoryError, Result, redaction::redact_secrets, templates};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RolloutCandidate {
    pub thread_id: String,
    pub session_id: String,
    pub rollout_path: Option<String>,
    pub rollout_cwd: Option<String>,
    pub source_updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RolloutResponseItem {
    pub role: String,
    pub content: String,
    pub created_at: String,
    /// The message's harness tag (`name`), when it carries one. Not part of
    /// the rendered prompt — it exists so
    /// [`filter_memory_relevant_items`] can drop harness-injected messages
    /// (init-context fragments, subagent notices) from the extraction input.
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Phase1RolloutInput {
    pub candidate: RolloutCandidate,
    pub items: Vec<RolloutResponseItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Phase1MemoryOutput {
    pub thread_id: String,
    pub session_id: String,
    pub raw_memory: String,
    pub rollout_summary: String,
    pub rollout_slug: Option<String>,
    pub source_updated_at: DateTime<Utc>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Phase1ModelOutput {
    pub raw_memory: String,
    pub rollout_summary: String,
    #[serde(default)]
    pub rollout_slug: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase1JobOutcome {
    Succeeded,
    SucceededNoOutput,
    Failed,
}

impl Phase1RolloutInput {
    pub fn render_user_prompt(&self) -> Result<String> {
        templates::render_phase1_input(
            self.candidate.rollout_path.as_deref().unwrap_or("state-db"),
            self.candidate.rollout_cwd.as_deref().unwrap_or("unknown"),
            &self.render_items(),
        )
    }

    pub fn render_items(&self) -> String {
        self.items
            .iter()
            .map(|item| {
                format!(
                    "[{}] role={}\n{}\n",
                    item.created_at,
                    item.role,
                    redact_secrets(&item.content)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Phase1ModelOutput {
    pub fn from_model_json(value: &str) -> Result<Self> {
        let parsed: Self = serde_json::from_str(value)
            .map_err(|error| MemoryError::InvalidModelOutput(error.to_string()))?;
        Ok(parsed.redacted())
    }

    pub fn into_memory_output(
        self,
        candidate: &RolloutCandidate,
        generated_at: DateTime<Utc>,
    ) -> Option<Phase1MemoryOutput> {
        let raw_memory = self.raw_memory.trim().to_owned();
        let rollout_summary = self.rollout_summary.trim().to_owned();
        if raw_memory.is_empty() && rollout_summary.is_empty() {
            return None;
        }
        Some(Phase1MemoryOutput {
            thread_id: candidate.thread_id.clone(),
            session_id: candidate.session_id.clone(),
            raw_memory,
            rollout_summary,
            rollout_slug: self
                .rollout_slug
                .and_then(|slug| (!slug.trim().is_empty()).then(|| sanitize_slug(&slug))),
            source_updated_at: candidate.source_updated_at,
            generated_at,
        })
    }

    fn redacted(self) -> Self {
        Self {
            raw_memory: redact_secrets(&self.raw_memory),
            rollout_summary: redact_secrets(&self.rollout_summary),
            rollout_slug: self.rollout_slug.map(|slug| redact_secrets(&slug)),
        }
    }
}

/// Keep only the conversation-proper items for the phase1 extraction input:
/// `user` / `assistant` / `tool` roles, non-empty, and NOT harness-injected.
///
/// Harness injections carry a `slab_`-prefixed tag (the init-context batch —
/// `slab_agents_md` is user-role — and the `slab_subagent_notice`
/// completions); learning from them pollutes the memory with the project's
/// own instructions and delegated-subtask output re-read as user speech
/// (Codex strips the same injection classes before extraction). The
/// `slab_` prefix rule covers future tags too — the namespace is
/// harness-owned.
pub fn filter_memory_relevant_items(items: Vec<RolloutResponseItem>) -> Vec<RolloutResponseItem> {
    items
        .into_iter()
        .filter(|item| {
            !item.name.as_deref().is_some_and(|name| name.starts_with("slab_"))
                && matches!(item.role.as_str(), "user" | "assistant" | "tool")
                && !item.content.trim().is_empty()
        })
        .collect()
}

pub fn sanitize_slug(value: &str) -> String {
    let mut output = String::with_capacity(value.len().min(80));
    let mut last_dash = false;
    for ch in value.chars() {
        let next = if ch.is_ascii_alphanumeric() {
            last_dash = false;
            Some(ch.to_ascii_lowercase())
        } else if ch == '-' || ch == '_' || ch.is_ascii_whitespace() {
            if last_dash {
                None
            } else {
                last_dash = true;
                Some('-')
            }
        } else {
            None
        };
        if let Some(ch) = next {
            output.push(ch);
        }
        if output.len() >= 80 {
            break;
        }
    }
    output.trim_matches('-').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_redacts_phase1_model_output() {
        let parsed = Phase1ModelOutput::from_model_json(
            r#"{"raw_memory":"api_key=abcdefghijklmnop","rollout_summary":"ok","rollout_slug":"My Slug!"}"#,
        )
        .expect("json");

        assert!(parsed.raw_memory.contains("[REDACTED_SECRET]"));
        assert_eq!(parsed.rollout_slug.as_deref(), Some("My Slug!"));
    }

    #[test]
    fn sanitizes_rollout_slug() {
        assert_eq!(sanitize_slug("My Slug! 2026"), "my-slug-2026");
    }

    #[test]
    fn filters_memory_relevant_items() {
        let items = filter_memory_relevant_items(vec![
            RolloutResponseItem {
                role: "system".into(),
                content: "ignore".into(),
                created_at: "1".into(),
                name: None,
            },
            RolloutResponseItem {
                role: "developer".into(),
                content: "ignore".into(),
                created_at: "2".into(),
                name: None,
            },
            RolloutResponseItem {
                role: "user".into(),
                content: " ".into(),
                created_at: "3".into(),
                name: None,
            },
            RolloutResponseItem {
                role: "assistant".into(),
                content: "keep".into(),
                created_at: "4".into(),
                name: None,
            },
            RolloutResponseItem {
                role: "tool".into(),
                content: "keep".into(),
                created_at: "5".into(),
                name: None,
            },
        ]);

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].role, "assistant");
        assert_eq!(items[1].role, "tool");
    }

    /// Harness-injected messages never reach the extraction input, whatever
    /// their role: the init-context fragments (`slab_agents_md` is user-role)
    /// and the subagent completion notices would otherwise be re-learned as
    /// conversation (Codex filters the same injection classes).
    #[test]
    fn filters_harness_tagged_injections() {
        let items = filter_memory_relevant_items(vec![
            RolloutResponseItem {
                role: "user".into(),
                content: "# Project rules\nUse bun.".into(),
                created_at: "1".into(),
                name: Some("slab_agents_md".into()),
            },
            RolloutResponseItem {
                role: "user".into(),
                content: "[subagent task finished] task_id=bg-1\n<subagent-result>findings</subagent-result>".into(),
                created_at: "2".into(),
                name: Some("slab_subagent_notice".into()),
            },
            RolloutResponseItem {
                role: "assistant".into(),
                content: "injected as developer-role too".into(),
                created_at: "3".into(),
                name: Some("slab_memory".into()),
            },
            RolloutResponseItem {
                role: "user".into(),
                content: "real user turn".into(),
                created_at: "4".into(),
                name: None,
            },
            // A non-harness name (API multi-name chats) is conversation, not
            // injection — it stays.
            RolloutResponseItem {
                role: "user".into(),
                content: "named user turn".into(),
                created_at: "5".into(),
                name: Some("alice".into()),
            },
        ]);

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].content, "real user turn");
        assert_eq!(items[1].content, "named user turn");
    }
}
