use async_trait::async_trait;
use slab_types::{ConversationMessage, ConversationMessageContent};

use crate::{error::AgentError, event::AgentEventKind};

/// Estimates the token pressure of agent history and optionally compacts it.
#[async_trait]
pub trait CompactPort: Send + Sync {
    /// Return the threshold at which compaction should be considered.
    fn threshold_tokens(&self) -> usize;

    /// Estimate token usage for the current message history.
    fn estimate_tokens(&self, messages: &[ConversationMessage]) -> usize;

    /// Compact messages when implemented by the host.
    async fn compact(
        &self,
        _messages: &[ConversationMessage],
    ) -> Result<CompactOutcome, AgentError> {
        Ok(CompactOutcome::Skipped { reason: "no compact provider configured".to_owned() })
    }
}

#[derive(Debug, Clone)]
pub enum CompactOutcome {
    Replaced { messages: Vec<ConversationMessage>, output_tokens: usize, replaced_messages: usize },
    Skipped { reason: String },
}

#[derive(Debug, Clone)]
pub struct NoopCompactPort {
    threshold_tokens: usize,
}

impl NoopCompactPort {
    pub fn new(threshold_tokens: usize) -> Self {
        Self { threshold_tokens }
    }
}

impl Default for NoopCompactPort {
    fn default() -> Self {
        Self::new(16_000)
    }
}

#[async_trait]
impl CompactPort for NoopCompactPort {
    fn threshold_tokens(&self) -> usize {
        self.threshold_tokens
    }

    fn estimate_tokens(&self, messages: &[ConversationMessage]) -> usize {
        estimate_tokens(messages)
    }
}

pub(crate) fn compact_skipped_event(
    input_tokens: usize,
    threshold_tokens: usize,
    reason: String,
) -> AgentEventKind {
    AgentEventKind::ResponseContextCompactSkipped { input_tokens, threshold_tokens, reason }
}

fn estimate_tokens(messages: &[ConversationMessage]) -> usize {
    let chars = messages
        .iter()
        .map(|message| match &message.content {
            ConversationMessageContent::Text(text) => text.chars().count(),
            ConversationMessageContent::Parts(parts) => parts
                .iter()
                .map(|part| serde_json::to_string(part).map_or(0, |text| text.chars().count()))
                .sum(),
        })
        .sum::<usize>();
    chars.div_ceil(4)
}
