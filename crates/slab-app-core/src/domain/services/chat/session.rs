use chrono::Utc;
use uuid::Uuid;

use crate::context::ModelState;
use crate::domain::models::{
    ConversationMessage as DomainConversationMessage, serialize_session_message,
};
use crate::error::AppCoreError;
use crate::infra::db::{ChatMessage, ChatStore};

pub(super) async fn persist_session_message(
    state: &ModelState,
    session_id: &str,
    message: &DomainConversationMessage,
) {
    state
        .store()
        .append_message(ChatMessage {
            id: Uuid::new_v4().to_string(),
            session_id: session_id.to_owned(),
            role: message.role.clone(),
            content: serialize_session_message(message),
            created_at: Utc::now(),
        })
        .await
        .unwrap_or_else(|error| {
            tracing::warn!(
                error = %error,
                role = %message.role,
                session_id,
                "failed to persist session message"
            )
        });
}

/// Merge history from DB and current request messages while avoiding duplicates.
pub(super) async fn build_messages(
    state: &ModelState,
    session_id: Option<&str>,
    current_messages: &[DomainConversationMessage],
) -> Result<Vec<DomainConversationMessage>, AppCoreError> {
    let current: Vec<DomainConversationMessage> = current_messages
        .iter()
        .filter(|message| message.has_meaningful_content())
        .cloned()
        .collect();
    let client_sent_history = current.len() > 1;

    let mut merged = Vec::new();
    if !client_sent_history && let Some(session_id) = session_id {
        let history = state.store().list_messages(session_id).await?;
        for message in history {
            if message.content.trim().is_empty() {
                continue;
            }
            merged.push(message.into());
        }
    }
    merged.extend(current);
    Ok(merged)
}
