use std::future::Future;
use std::pin::Pin;

use futures::stream::BoxStream;

use crate::contexts::chat::domain::{ChatCompletionCommand, ChatCompletionResult};
use crate::error::ServerError;

/// A single chunk yielded by a streaming chat completion response.
pub enum ChatStreamChunk {
    /// A data frame (token or `[DONE]` sentinel) to be sent as an SSE `data:` line.
    Data(String),
    /// An SSE comment line used to signal errors or metadata without closing the stream.
    Comment(String),
}

/// The output of a chat completion request.
pub enum ChatCompletionOutput {
    /// A complete, non-streaming completion serialised as JSON.
    Json(ChatCompletionResult),
    /// A stream of SSE chunks for incremental token delivery.
    Stream(BoxStream<'static, ChatStreamChunk>),
}

pub trait ChatCompletionPort: Send + Sync {
    fn create_chat_completion(
        &self,
        command: ChatCompletionCommand,
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
        command: ChatCompletionCommand,
    ) -> Result<ChatCompletionOutput, ServerError> {
        self.port.create_chat_completion(command).await
    }
}
