use std::marker::PhantomData;
use std::sync::Arc;

use futures::StreamExt;
use futures::stream::{self, BoxStream};

use crate::base::error::CoreError;
use crate::base::types::{Payload, StreamChunk, TaskId, TaskStatus};
use crate::internal::scheduler::orchestrator::{
    DEFAULT_WAIT_TIMEOUT, Orchestrator, STREAM_INIT_TIMEOUT,
};
use crate::internal::scheduler::storage::TaskStatusView;
use crate::model::Capability;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskState {
    Pending,
    Running { stage_index: usize, stage_name: String },
    Succeeded,
    ResultConsumed,
    SucceededStreaming,
    Failed { message: String },
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct TaskSnapshot {
    pub task_id: TaskId,
    pub capability: Capability,
    pub status: TaskState,
}

impl TaskSnapshot {
    pub(crate) fn from_view(capability: Capability, view: TaskStatusView) -> Self {
        Self {
            task_id: view.task_id,
            capability,
            status: match view.status {
                TaskStatus::Pending => TaskState::Pending,
                TaskStatus::Running { stage_index, stage_name } => {
                    TaskState::Running { stage_index, stage_name }
                }
                TaskStatus::Succeeded { .. } => TaskState::Succeeded,
                TaskStatus::ResultConsumed => TaskState::ResultConsumed,
                TaskStatus::SucceededStreaming => TaskState::SucceededStreaming,
                TaskStatus::Failed { error } => TaskState::Failed { message: error.to_string() },
                TaskStatus::Cancelled => TaskState::Cancelled,
            },
        }
    }
}

pub(crate) trait TaskCodec<R, C>: Send + Sync + 'static {
    fn capability(&self) -> Capability;
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
            .field("capability", &self.codec.capability())
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

    pub fn task_id(&self) -> TaskId {
        self.task_id
    }

    pub async fn status(&self) -> Result<TaskSnapshot, CoreError> {
        let view = self.orchestrator.get_status(self.task_id).await?;
        Ok(TaskSnapshot::from_view(self.codec.capability(), view))
    }

    pub fn cancel(&self) {
        self.orchestrator.cancel(self.task_id);
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
