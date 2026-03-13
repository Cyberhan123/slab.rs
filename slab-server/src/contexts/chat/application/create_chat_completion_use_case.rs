use std::future::Future;
use std::pin::Pin;

use futures::stream::BoxStream;

use crate::error::ServerError;
use crate::schemas::v1::chat::{ChatCompletionRequest, ChatCompletionResponse};

/// A single framework-agnostic chunk in a streaming chat completion.
/// The route layer is responsible for mapping this to the transport-specific
/// representation (e.g. an SSE `Event`).
pub enum ChatStreamChunk {
    /// A serialized data payload (maps to an SSE `data:` field).
    Data(String),
    /// An out-of-band signal or error description (maps to an SSE `:` comment field).
    /// This is used for backend-specific signals (e.g. gRPC done markers) and
    /// error descriptions that should not appear as data tokens to the client.
    Comment(String),
}

/// Application-level output of the chat completion use-case.
/// Routes are responsible for converting this into an Axum `Response`.
pub enum ChatCompletionOutput {
    /// A complete JSON response payload.
    Json(ChatCompletionResponse),
    /// A server-sent events stream of framework-agnostic chunks.
    Stream(BoxStream<'static, ChatStreamChunk>),
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
