use std::future::Future;
use std::pin::Pin;

use axum::response::Response;

use crate::error::ServerError;
use crate::schemas::v1::chat::ChatCompletionRequest;

pub trait ChatCompletionPort: Send + Sync {
    fn create_chat_completion(
        &self,
        req: ChatCompletionRequest,
    ) -> Pin<Box<dyn Future<Output = Result<Response, ServerError>> + Send + '_>>;
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

    pub async fn execute(&self, req: ChatCompletionRequest) -> Result<Response, ServerError> {
        self.port.create_chat_completion(req).await
    }
}
