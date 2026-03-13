use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;

use axum::response::sse::Event;
use futures::stream::BoxStream;

use crate::error::ServerError;
use crate::schemas::v1::chat::{ChatCompletionRequest, ChatCompletionResponse};

/// Application-level output of the chat completion use-case.
/// Routes are responsible for converting this into an Axum `Response`.
pub enum ChatCompletionOutput {
    /// A complete JSON response payload.
    Json(ChatCompletionResponse),
    /// A server-sent events stream.
    Stream(BoxStream<'static, Result<Event, Infallible>>),
}

pub trait ChatCompletionPort: Send + Sync {
    fn create_chat_completion(
        &self,
        req: ChatCompletionRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ChatCompletionOutput, ServerError>> + Send + '_>>;
}

pub struct CreateChatCompletionUseCase<P> {
    port: P,
}

impl<P> CreateChatCompletionUseCase<P>
where
    P: ChatCompletionPort,
{
    pub fn new(port: P) -> Self {
        Self { port }
    }

    pub async fn execute(
        &self,
        req: ChatCompletionRequest,
    ) -> Result<ChatCompletionOutput, ServerError> {
        self.port.create_chat_completion(req).await
    }
}
