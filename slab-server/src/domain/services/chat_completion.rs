use futures::stream::BoxStream;

use crate::domain::models::ChatCompletionResult;

pub enum ChatStreamChunk {
    Data(String),
    Comment(String),
}

pub enum ChatCompletionOutput {
    Json(ChatCompletionResult),
    Stream(BoxStream<'static, ChatStreamChunk>),
}
