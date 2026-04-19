use std::marker::PhantomData;
use std::sync::Arc;

use futures::StreamExt;
use futures::stream::{self, BoxStream};
use slab_runtime_core::Payload;
use slab_runtime_core::backend::{RequestRoute, StreamChunk};

use crate::domain::runtime::{
    CoreError, DEFAULT_WAIT_TIMEOUT, Orchestrator, STREAM_INIT_TIMEOUT, TaskId,
};

pub(crate) trait TaskCodec<R, C>: Send + Sync + 'static {
    fn route(&self) -> RequestRoute;
    fn decode_result(&self, payload: Payload) -> Result<R, CoreError>;
    fn decode_chunk(&self, chunk: StreamChunk) -> Result<Option<C>, CoreError>;
}

#[derive(Clone)]
pub struct TaskHandle<R, C> {
    orchestrator: Orchestrator,
    task_id: TaskId,
    codec: Arc<dyn TaskCodec<R, C>>,
    _types: PhantomData<(R, C)>,
}

impl<R: 'static, C: 'static> std::fmt::Debug for TaskHandle<R, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskHandle")
            .field("task_id", &self.task_id)
            .field("route", &self.codec.route())
            .finish()
    }
}

impl<R, C> TaskHandle<R, C>
where
    R: Send + 'static,
    C: Send + 'static,
{
    pub(crate) fn new(
        orchestrator: Orchestrator,
        task_id: TaskId,
        codec: Arc<dyn TaskCodec<R, C>>,
    ) -> Self {
        Self { orchestrator, task_id, codec, _types: PhantomData }
    }

    pub async fn cancel_and_purge(&self) {
        self.orchestrator.cancel_and_purge(self.task_id).await;
    }

    pub async fn purge(&self) {
        self.orchestrator.purge_task(self.task_id).await;
    }

    pub async fn result(&self) -> Result<R, CoreError> {
        self.result_timeout(DEFAULT_WAIT_TIMEOUT).await
    }

    pub async fn result_timeout(&self, timeout: std::time::Duration) -> Result<R, CoreError> {
        let payload = self.orchestrator.wait_result(self.task_id, timeout).await?;
        self.codec.decode_result(payload)
    }

    pub async fn take_stream(&self) -> Result<BoxStream<'static, Result<C, CoreError>>, CoreError> {
        self.take_stream_timeout(STREAM_INIT_TIMEOUT).await
    }

    pub async fn take_stream_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> Result<BoxStream<'static, Result<C, CoreError>>, CoreError> {
        let handle = self.orchestrator.wait_stream(self.task_id, timeout).await?;
        let codec = Arc::clone(&self.codec);

        Ok(stream::unfold((handle, codec), |(mut rx, codec)| async move {
            match rx.recv().await {
                Some(chunk) => match codec.decode_chunk(chunk) {
                    Ok(Some(decoded)) => Some((Ok(decoded), (rx, codec))),
                    Ok(None) => None,
                    Err(error) => Some((Err(error), (rx, codec))),
                },
                None => None,
            }
        })
        .boxed())
    }
}
