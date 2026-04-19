use std::sync::Arc;

use slab_runtime_core::Payload;
use slab_runtime_core::backend::{BackendOp, RequestRoute, StreamChunk};
use tokio::sync::Mutex;

use super::ExecutionHub;
use crate::domain::models::{TaskCodec, TaskHandle};
use crate::domain::runtime::{CoreError, CpuStage, PipelineBuilder};

#[derive(Clone, Debug)]
pub(crate) struct DriverRuntime {
    execution: ExecutionHub,
    backend_id: Arc<str>,
    load_payload: Payload,
    loaded: Arc<Mutex<bool>>,
}

impl DriverRuntime {
    pub(crate) fn new(
        execution: ExecutionHub,
        backend_id: impl Into<String>,
        load_payload: Payload,
    ) -> Self {
        Self {
            execution,
            backend_id: Arc::from(backend_id.into()),
            load_payload,
            loaded: Arc::new(Mutex::new(false)),
        }
    }

    pub(crate) async fn load(&self) -> Result<(), CoreError> {
        self.ensure_loaded().await
    }

    pub(crate) async fn unload(&self) -> Result<(), CoreError> {
        let was_loaded = {
            let guard = self.loaded.lock().await;
            *guard
        };

        if was_loaded {
            self.execution.orchestrator().unload_model_backend(&self.backend_id).await?;
            let mut guard = self.loaded.lock().await;
            *guard = false;
        }

        Ok(())
    }

    pub(crate) async fn submit(
        &self,
        route: RequestRoute,
        input: Payload,
        preprocess_stages: Vec<CpuStage>,
        op_options: Payload,
    ) -> Result<TaskHandle<Payload, StreamChunk>, CoreError> {
        self.ensure_loaded().await?;
        let task_id = submit_invocation(
            &self.execution,
            self.backend_id.as_ref(),
            route,
            input,
            preprocess_stages,
            op_options,
        )
        .await?;
        Ok(TaskHandle::new(
            self.execution.orchestrator(),
            task_id,
            Arc::new(RawPayloadTaskCodec { route }),
        ))
    }

    async fn ensure_loaded(&self) -> Result<(), CoreError> {
        {
            let guard = self.loaded.lock().await;
            if *guard {
                return Ok(());
            }
        }

        let mut guard = self.loaded.lock().await;
        if *guard {
            return Ok(());
        }

        self.execution
            .orchestrator()
            .load_model_backend(&self.backend_id, self.load_payload.clone())
            .await?;
        *guard = true;
        Ok(())
    }
}

struct RawPayloadTaskCodec {
    route: RequestRoute,
}

impl TaskCodec<Payload, StreamChunk> for RawPayloadTaskCodec {
    fn route(&self) -> RequestRoute {
        self.route
    }

    fn decode_result(&self, payload: Payload) -> Result<Payload, CoreError> {
        Ok(payload)
    }

    fn decode_chunk(&self, chunk: StreamChunk) -> Result<Option<StreamChunk>, CoreError> {
        match chunk {
            StreamChunk::Done => Ok(None),
            other => Ok(Some(other)),
        }
    }
}

async fn submit_invocation(
    execution: &ExecutionHub,
    backend_id: &str,
    route: RequestRoute,
    initial_payload: Payload,
    preprocess_stages: Vec<CpuStage>,
    op_options: Payload,
) -> Result<u64, CoreError> {
    let op = BackendOp { name: route.as_str().to_owned(), options: op_options };

    let mut builder = PipelineBuilder::new(execution.orchestrator(), initial_payload);
    for stage in preprocess_stages {
        builder = builder.cpu_stage(stage);
    }

    if matches!(route, RequestRoute::InferenceStream) {
        builder.gpu_stream(route.as_str(), backend_id.to_owned(), op).run_stream().await
    } else {
        builder.gpu(route.as_str(), backend_id.to_owned(), op).run().await
    }
}
