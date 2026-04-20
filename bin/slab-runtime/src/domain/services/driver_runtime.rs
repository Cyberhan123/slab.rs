use std::sync::Arc;

use serde::de::DeserializeOwned;
use slab_runtime_core::Payload;
use slab_runtime_core::backend::{BackendOp, RequestRoute, StreamChunk};
use tokio::sync::Mutex;

use super::ExecutionHub;
use crate::domain::models::{TaskCodec, TaskHandle};
use crate::domain::runtime::{CoreError, CpuStage, PipelineBuilder};

#[derive(Clone, Debug)]
pub(crate) struct DriverRuntime {
    execution: ExecutionHub,
    capability_id: Arc<str>,
    deployment_id: Arc<str>,
    load_payload: Payload,
    loaded: Arc<Mutex<bool>>,
}

impl DriverRuntime {
    pub(crate) fn new(
        execution: ExecutionHub,
        capability_id: impl Into<String>,
        deployment_id: impl Into<String>,
        load_payload: Payload,
    ) -> Self {
        Self {
            execution,
            capability_id: Arc::from(capability_id.into()),
            deployment_id: Arc::from(deployment_id.into()),
            load_payload,
            loaded: Arc::new(Mutex::new(false)),
        }
    }

    pub(crate) fn new_typed<T>(
        execution: ExecutionHub,
        capability_id: impl Into<String>,
        deployment_id: impl Into<String>,
        load_payload: T,
    ) -> Self
    where
        T: Send + Sync + 'static,
    {
        Self::new(execution, capability_id, deployment_id, Payload::typed(load_payload))
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
            self.execution.orchestrator().unload_model_backend(&self.deployment_id).await?;
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
            self.deployment_id.as_ref(),
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

    #[allow(dead_code)]
    pub(crate) async fn submit_typed<TInput, TOptions>(
        &self,
        route: RequestRoute,
        input: TInput,
        preprocess_stages: Vec<CpuStage>,
        op_options: TOptions,
    ) -> Result<TaskHandle<Payload, StreamChunk>, CoreError>
    where
        TInput: Send + Sync + 'static,
        TOptions: Send + Sync + 'static,
    {
        self.submit(route, Payload::typed(input), preprocess_stages, Payload::typed(op_options))
            .await
    }

    pub(crate) async fn submit_payload<TOptions>(
        &self,
        route: RequestRoute,
        input: impl Into<Payload>,
        preprocess_stages: Vec<CpuStage>,
        op_options: TOptions,
    ) -> Result<TaskHandle<Payload, StreamChunk>, CoreError>
    where
        TOptions: Send + Sync + 'static,
    {
        self.submit(route, input.into(), preprocess_stages, Payload::typed(op_options)).await
    }

    pub(crate) async fn submit_preprocessed<TOptions>(
        &self,
        route: RequestRoute,
        preprocess_stages: Vec<CpuStage>,
        op_options: TOptions,
    ) -> Result<TaskHandle<Payload, StreamChunk>, CoreError>
    where
        TOptions: Send + Sync + 'static,
    {
        self.submit(route, Payload::None, preprocess_stages, Payload::typed(op_options)).await
    }

    pub(crate) async fn submit_without_options<TInput>(
        &self,
        route: RequestRoute,
        input: TInput,
        preprocess_stages: Vec<CpuStage>,
    ) -> Result<TaskHandle<Payload, StreamChunk>, CoreError>
    where
        TInput: Send + Sync + 'static,
    {
        self.submit(route, Payload::typed(input), preprocess_stages, Payload::None).await
    }

    #[allow(dead_code)]
    pub(crate) async fn submit_preprocessed_without_options(
        &self,
        route: RequestRoute,
        preprocess_stages: Vec<CpuStage>,
    ) -> Result<TaskHandle<Payload, StreamChunk>, CoreError> {
        self.submit(route, Payload::None, preprocess_stages, Payload::None).await
    }

    #[allow(dead_code)]
    pub(crate) async fn invoke_typed<TInput, TOptions, TOutput>(
        &self,
        route: RequestRoute,
        input: TInput,
        preprocess_stages: Vec<CpuStage>,
        op_options: TOptions,
    ) -> Result<TOutput, CoreError>
    where
        TInput: Send + Sync + 'static,
        TOptions: Send + Sync + 'static,
        TOutput: DeserializeOwned + Clone + Send + Sync + 'static,
    {
        let payload =
            self.submit_typed(route, input, preprocess_stages, op_options).await?.result().await?;
        decode_typed_output(payload, self.capability_id.as_ref())
    }

    pub(crate) async fn invoke_preprocessed_typed<TOptions, TOutput>(
        &self,
        route: RequestRoute,
        preprocess_stages: Vec<CpuStage>,
        op_options: TOptions,
    ) -> Result<TOutput, CoreError>
    where
        TOptions: Send + Sync + 'static,
        TOutput: DeserializeOwned + Clone + Send + Sync + 'static,
    {
        let payload =
            self.submit_preprocessed(route, preprocess_stages, op_options).await?.result().await?;
        decode_typed_output(payload, self.capability_id.as_ref())
    }

    pub(crate) async fn invoke_without_options<TInput, TOutput>(
        &self,
        route: RequestRoute,
        input: TInput,
        preprocess_stages: Vec<CpuStage>,
    ) -> Result<TOutput, CoreError>
    where
        TInput: Send + Sync + 'static,
        TOutput: DeserializeOwned + Clone + Send + Sync + 'static,
    {
        let payload =
            self.submit_without_options(route, input, preprocess_stages).await?.result().await?;
        decode_typed_output(payload, self.capability_id.as_ref())
    }

    #[allow(dead_code)]
    pub(crate) async fn invoke_preprocessed_without_options<TOutput>(
        &self,
        route: RequestRoute,
        preprocess_stages: Vec<CpuStage>,
    ) -> Result<TOutput, CoreError>
    where
        TOutput: DeserializeOwned + Clone + Send + Sync + 'static,
    {
        let payload = self
            .submit_preprocessed_without_options(route, preprocess_stages)
            .await?
            .result()
            .await?;
        decode_typed_output(payload, self.capability_id.as_ref())
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
            .load_model_backend(&self.deployment_id, self.load_payload.clone())
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

fn decode_typed_output<T>(payload: Payload, task_kind: &str) -> Result<T, CoreError>
where
    T: DeserializeOwned + Clone + Send + Sync + 'static,
{
    payload.to_typed().map_err(|error| CoreError::ResultDecodeFailed {
        task_kind: task_kind.to_owned(),
        message: format!("invalid typed result payload: {error}"),
    })
}

#[cfg(test)]
mod tests {
    use super::decode_typed_output;
    use slab_runtime_core::Payload;

    #[test]
    fn decode_typed_output_reads_typed_payload() {
        let value: String =
            decode_typed_output(Payload::typed("hello".to_owned()), "onnx.text").unwrap();
        assert_eq!(value, "hello");
    }
}
