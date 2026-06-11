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
}
