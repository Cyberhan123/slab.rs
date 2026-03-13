use std::future::Future;
use std::pin::Pin;

use futures::stream::BoxStream;

use crate::domain::models::{ChatCompletionCommand, ChatCompletionResult};
use crate::error::ServerError;

pub enum ChatStreamChunk {
    Data(String),
    Comment(String),
}

pub enum ChatCompletionOutput {
    Json(ChatCompletionResult),
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
