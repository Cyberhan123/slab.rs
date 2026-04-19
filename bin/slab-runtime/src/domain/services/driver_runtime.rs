use std::sync::Arc;

use slab_runtime_core::Payload;
use slab_runtime_core::backend::{BackendOp, StreamChunk};
use slab_types::{Capability, ModelSpec};
use tokio::sync::Mutex;

use super::ExecutionHub;
use crate::domain::models::{InvocationPlan, ResolvedBackend, TaskCodec, TaskHandle};
use crate::domain::runtime::{CoreError, CpuStage, PipelineBuilder};

#[derive(Clone, Debug)]
pub(crate) struct DriverRuntime {
    execution: ExecutionHub,
    spec: Arc<ModelSpec>,
    backend_target: Arc<str>,
    load_payload: Payload,
    deployment: Arc<Mutex<Option<LoadedDeployment>>>,
}

#[derive(Clone, Debug)]
struct LoadedDeployment {
    resolved: ResolvedBackend,
}

impl DriverRuntime {
    pub(crate) fn new(
        execution: ExecutionHub,
        spec: ModelSpec,
        backend_target: impl Into<String>,
        load_payload: Payload,
    ) -> Self {
        Self {
            execution,
            spec: Arc::new(spec),
            backend_target: Arc::from(backend_target.into()),
            load_payload,
            deployment: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) async fn load(&self) -> Result<(), CoreError> {
        let _ = self.ensure_loaded_for(self.spec.capability, false).await?;
        Ok(())
    }

    pub(crate) async fn unload(&self) -> Result<(), CoreError> {
        let resolved = {
            let guard = self.deployment.lock().await;
            guard.as_ref().map(|deployment| deployment.resolved.clone())
        };

        if let Some(resolved) = resolved {
            self.execution.orchestrator().unload_model_backend(&resolved.backend_id).await?;
            let mut guard = self.deployment.lock().await;
            *guard = None;
        }

        Ok(())
    }

    pub(crate) async fn submit(
        &self,
        capability: Capability,
        streaming: bool,
        input: Payload,
        preprocess_stages: Vec<CpuStage>,
        op_options: Payload,
    ) -> Result<TaskHandle<Payload, StreamChunk>, CoreError> {
        let deployment = self.ensure_loaded_for(capability, streaming).await?;
        let plan = InvocationPlan::new(
            deployment.resolved,
            capability,
            streaming,
            input,
            preprocess_stages,
            op_options,
        )?;
        submit_plan(&self.execution, plan, RawPayloadTaskCodec { capability }).await
    }

    async fn ensure_loaded_for(
        &self,
        capability: Capability,
        streaming: bool,
    ) -> Result<LoadedDeployment, CoreError> {
        {
            let guard = self.deployment.lock().await;
            if let Some(existing) = guard.as_ref() {
                validate_loaded(existing, capability, streaming)?;
                return Ok(existing.clone());
            }
        }

        let mut guard = self.deployment.lock().await;
        if let Some(existing) = guard.as_ref() {
            validate_loaded(existing, capability, streaming)?;
            return Ok(existing.clone());
        }

        let resolved = self.execution.catalog().bind_for_target(
            self.spec.as_ref(),
            self.backend_target.as_ref(),
            capability,
            streaming,
        )?;
        self.execution
            .orchestrator()
            .load_model_backend(&resolved.backend_id, self.load_payload.clone())
            .await?;

        let deployment = LoadedDeployment { resolved };
        *guard = Some(deployment.clone());
        Ok(deployment)
    }
}

fn validate_loaded(
    deployment: &LoadedDeployment,
    capability: Capability,
    streaming: bool,
) -> Result<(), CoreError> {
    if streaming && !deployment.resolved.supports_streaming {
        return Err(CoreError::UnsupportedOperation {
            backend: deployment.resolved.driver_id.clone(),
            op: "stream".to_owned(),
        });
    }
    if deployment.resolved.capability != capability {
        return Err(CoreError::UnsupportedCapability {
            family: format!("{:?}", deployment.resolved.family),
            capability: format!("{:?}", capability),
        });
    }
    Ok(())
}

struct RawPayloadTaskCodec {
    capability: Capability,
}

impl TaskCodec<Payload, StreamChunk> for RawPayloadTaskCodec {
    fn capability(&self) -> Capability {
        self.capability
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

async fn submit_plan(
    execution: &ExecutionHub,
    plan: InvocationPlan,
    codec: impl TaskCodec<Payload, StreamChunk>,
) -> Result<TaskHandle<Payload, StreamChunk>, CoreError> {
    let task_id = submit_invocation_plan(execution, plan).await?;
    Ok(TaskHandle::new(execution.orchestrator(), task_id, Arc::new(codec)))
}

async fn submit_invocation_plan(
    execution: &ExecutionHub,
    plan: InvocationPlan,
) -> Result<u64, CoreError> {
    let op = BackendOp { name: plan.invocation.op_name.clone(), options: plan.op_options };

    let mut builder = PipelineBuilder::new(execution.orchestrator(), plan.initial_payload);
    for stage in plan.preprocess_stages {
        builder = builder.cpu_stage(stage);
    }

    if plan.streaming {
        builder
            .gpu_stream(
                plan.invocation.op_name.clone(),
                plan.invocation.backend.backend_id.clone(),
                op,
            )
            .run_stream()
            .await
    } else {
        builder
            .gpu(plan.invocation.op_name.clone(), plan.invocation.backend.backend_id.clone(), op)
            .run()
            .await
    }
}
