use async_trait::async_trait;
use slab_types::{ConversationMessage, ConversationMessageContent};

use crate::{error::AgentError, event::AgentEventKind};

/// Estimates the token pressure of agent history and optionally compacts it.
#[async_trait]
pub trait CompactPort: Send + Sync {
    /// Human-readable policy name for trace metadata.
    fn policy_name(&self) -> &'static str {
        "custom"
    }

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
    fn policy_name(&self) -> &'static str {
        "noop"
    }

    fn threshold_tokens(&self) -> usize {
        self.threshold_tokens
    }

    fn estimate_tokens(&self, messages: &[ConversationMessage]) -> usize {
        estimate_tokens(messages)
    }
}

#[derive(Debug, Clone)]
pub struct SlidingWindowCompactPort {
    threshold_tokens: usize,
    target_tokens: usize,
}

impl SlidingWindowCompactPort {
    pub fn new(threshold_tokens: usize, target_tokens: usize) -> Self {
        Self { threshold_tokens, target_tokens: target_tokens.min(threshold_tokens) }
    }
}

impl Default for SlidingWindowCompactPort {
    fn default() -> Self {
        Self::new(16_000, 12_000)
    }
}

#[async_trait]
impl CompactPort for SlidingWindowCompactPort {
    fn policy_name(&self) -> &'static str {
        "sliding_window"
    }

    fn threshold_tokens(&self) -> usize {
        self.threshold_tokens
    }

    fn estimate_tokens(&self, messages: &[ConversationMessage]) -> usize {
        estimate_tokens(messages)
    }

    async fn compact(
        &self,
        messages: &[ConversationMessage],
    ) -> Result<CompactOutcome, AgentError> {
        let mut compacted = trailing_window(messages, self.target_tokens);
        remove_leading_orphan_tool_results(&mut compacted);

        if let Some(system) = messages.first().filter(|message| message.role == "system")
            && compacted.first() != Some(system)
        {
            compacted.insert(0, system.clone());
            trim_to_target_after_system(&mut compacted, self.target_tokens);
            remove_leading_orphan_tool_results(&mut compacted);
        }

        if compacted.is_empty() || compacted.len() >= messages.len() {
            return Ok(CompactOutcome::Skipped {
                reason: "sliding window kept the full history".to_owned(),
            });
        }

        let output_tokens = estimate_tokens(&compacted);
        Ok(CompactOutcome::Replaced {
            replaced_messages: messages.len() - compacted.len(),
            messages: compacted,
            output_tokens,
        })
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
    let chars = messages.iter().map(estimate_message_chars).sum::<usize>();
    chars.div_ceil(4)
}

fn estimate_message_tokens(message: &ConversationMessage) -> usize {
    estimate_message_chars(message).div_ceil(4)
}

fn estimate_message_chars(message: &ConversationMessage) -> usize {
    match &message.content {
        ConversationMessageContent::Text(text) => text.chars().count(),
        ConversationMessageContent::Parts(parts) => parts
            .iter()
            .map(|part| serde_json::to_string(part).map_or(0, |text| text.chars().count()))
            .sum(),
    }
}

fn trailing_window(
    messages: &[ConversationMessage],
    target_tokens: usize,
) -> Vec<ConversationMessage> {
    let mut selected = Vec::new();
    let mut tokens = 0usize;
    for message in messages.iter().rev() {
        let message_tokens = estimate_message_tokens(message);
        if !selected.is_empty() && tokens + message_tokens > target_tokens {
            break;
        }
        tokens += message_tokens;
        selected.push(message.clone());
    }
    selected.reverse();
    selected
}

fn remove_leading_orphan_tool_results(messages: &mut Vec<ConversationMessage>) {
    let index =
        if messages.first().is_some_and(|message| message.role == "system") { 1 } else { 0 };
    while messages.get(index).is_some_and(|message| message.role == "tool") {
        messages.remove(index);
    }
}

fn trim_to_target_after_system(messages: &mut Vec<ConversationMessage>, target_tokens: usize) {
    while messages.len() > 1 && estimate_tokens(messages) > target_tokens {
        messages.remove(1);
    }
}
