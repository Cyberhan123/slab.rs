use crate::entities::contexts::chat::domain::{ChatMessage, ChatSession};
use std::future::Future;

pub trait ChatRepository: Send + Sync + 'static {
    fn create_session(
        &self,
        session: ChatSession,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn list_sessions(&self) -> impl Future<Output = Result<Vec<ChatSession>, sqlx::Error>> + Send;
    fn delete_session(&self, id: &str) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn append_message(
        &self,
        msg: ChatMessage,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn list_messages(
        &self,
        session_id: &str,
    ) -> impl Future<Output = Result<Vec<ChatMessage>, sqlx::Error>> + Send;
}
