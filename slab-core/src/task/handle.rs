use std::marker::PhantomData;
use std::sync::Arc;

use futures::stream::{self, BoxStream};
use futures::StreamExt;

use crate::base::error::CoreError;
use crate::base::types::{Payload, StageStatus, StreamChunk, TaskId, TaskStatus};
use crate::scheduler::kernel::{
    ExecutionKernel, DEFAULT_WAIT_TIMEOUT, STREAM_INIT_TIMEOUT,
};
use crate::scheduler::storage::TaskStatusView;
use crate::spec::TaskKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NeverChunk {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskState {
    Pending,
    Running {
        stage_index: usize,
        stage_name: String,
    },
    Succeeded,
    ResultConsumed,
    SucceededStreaming,
    Failed {
        message: String,
    },
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStageState {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct TaskSnapshot {
    pub task_id: TaskId,
    pub task_kind: TaskKind,
    pub status: TaskState,
    pub stage_statuses: Vec<TaskStageState>,
}

impl TaskSnapshot {
    pub(crate) fn from_view(task_kind: TaskKind, view: TaskStatusView) -> Self {
        Self {
            task_id: view.task_id,
            task_kind,
            status: match view.status {
                TaskStatus::Pending => TaskState::Pending,
                TaskStatus::Running {
                    stage_index,
                    stage_name,
                } => TaskState::Running {
                    stage_index,
                    stage_name,
                },
                TaskStatus::Succeeded { .. } => TaskState::Succeeded,
                TaskStatus::ResultConsumed => TaskState::ResultConsumed,
                TaskStatus::SucceededStreaming => TaskState::SucceededStreaming,
                TaskStatus::Failed { error } => TaskState::Failed {
                    message: error.to_string(),
                },
                TaskStatus::Cancelled => TaskState::Cancelled,
            },
            stage_statuses: view
                .stage_statuses
                .into_iter()
                .map(|status| match status {
                    StageStatus::StagePending => TaskStageState::Pending,
                    StageStatus::StageRunning => TaskStageState::Running,
                    StageStatus::StageCompleted => TaskStageState::Completed,
                    StageStatus::StageFailed => TaskStageState::Failed,
                    StageStatus::StageCancelled => TaskStageState::Cancelled,
                })
                .collect(),
        }
    }
}

pub(crate) trait TaskCodec<R, C>: Send + Sync + 'static {
    fn task_kind(&self) -> TaskKind;
    fn decode_result(&self, payload: Payload) -> Result<R, CoreError>;
    fn decode_chunk(&self, chunk: StreamChunk) -> Result<Option<C>, CoreError>;
}

#[derive(Clone)]
pub struct TaskHandle<R, C> {
    kernel: ExecutionKernel,
    task_id: TaskId,
    codec: Arc<dyn TaskCodec<R, C>>,
    _types: PhantomData<(R, C)>,
}

impl<R: 'static, C: 'static> std::fmt::Debug for TaskHandle<R, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskHandle")
            .field("task_id", &self.task_id)
            .field("task_kind", &self.codec.task_kind())
            .finish()
    }
}

impl<R, C> TaskHandle<R, C>
where
    R: Send + 'static,
    C: Send + 'static,
{
    pub(crate) fn new(
        kernel: ExecutionKernel,
        task_id: TaskId,
        codec: Arc<dyn TaskCodec<R, C>>,
    ) -> Self {
        Self {
            kernel,
            task_id,
            codec,
            _types: PhantomData,
        }
    }

    pub fn task_id(&self) -> TaskId {
        self.task_id
    }

    pub async fn status(&self) -> Result<TaskSnapshot, CoreError> {
        let view = self.kernel.snapshot(self.task_id).await?;
        Ok(TaskSnapshot::from_view(self.codec.task_kind(), view))
    }

    pub fn cancel(&self) {
        self.kernel.cancel(self.task_id);
    }

    pub async fn cancel_and_purge(&self) {
        self.kernel.cancel_and_purge(self.task_id).await;
    }

    pub async fn purge(&self) {
        self.kernel.purge(self.task_id).await;
    }

    pub async fn result(&self) -> Result<R, CoreError> {
        self.result_timeout(DEFAULT_WAIT_TIMEOUT).await
    }

    pub async fn result_timeout(&self, timeout: std::time::Duration) -> Result<R, CoreError> {
        let payload = self.kernel.wait_result(self.task_id, timeout).await?;
        self.codec.decode_result(payload)
    }

    pub async fn take_stream(&self) -> Result<BoxStream<'static, Result<C, CoreError>>, CoreError> {
        self.take_stream_timeout(STREAM_INIT_TIMEOUT).await
    }

    pub async fn take_stream_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> Result<BoxStream<'static, Result<C, CoreError>>, CoreError> {
        let handle = self.kernel.wait_stream(self.task_id, timeout).await?;
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
