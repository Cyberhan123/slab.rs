//! `model_quantize` async task — wraps the runtime `quantize_model` RPC as a
//! pollable task (mirrors the `model_download` task, but without the model
//! resolution / dedup machinery: quantize takes explicit input/output paths).

use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::context::worker_state::OperationContext;
use crate::domain::models::{AcceptedOperation, QuantizeModelCommand, TaskStatus};
use crate::domain::ports::{RuntimeInferenceGateway, RuntimeQuantizeRequest};
use crate::error::AppCoreError;
use crate::infra::db::{TaskRecord, TaskStore};

use super::ModelService;

pub(crate) const MODEL_QUANTIZE_TASK_TYPE: &str = "model_quantize";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QuantizeTaskInput {
    input_path: String,
    output_path: String,
    ftype: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    nthread: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    allow_requantize: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    quantize_output_tensor: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    only_copy: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pure: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    keep_split: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dry_run: Option<bool>,
}

#[derive(Serialize)]
struct QuantizeTaskResult {
    output_path: String,
    layers_processed: u32,
}

impl From<QuantizeModelCommand> for QuantizeTaskInput {
    fn from(command: QuantizeModelCommand) -> Self {
        Self {
            input_path: command.input_path,
            output_path: command.output_path,
            ftype: command.ftype,
            nthread: command.nthread,
            allow_requantize: command.allow_requantize,
            quantize_output_tensor: command.quantize_output_tensor,
            only_copy: command.only_copy,
            pure: command.pure,
            keep_split: command.keep_split,
            dry_run: command.dry_run,
        }
    }
}

impl ModelService {
    /// Accept a quantize request, persist a pending task row, spawn the worker,
    /// and return the operation id immediately. Poll via `GET /v1/tasks/{id}`.
    pub async fn quantize_model(
        &self,
        command: QuantizeModelCommand,
    ) -> Result<AcceptedOperation, AppCoreError> {
        let input: QuantizeTaskInput = command.into();
        let input_data = serde_json::to_string(&input).map_err(|error| {
            AppCoreError::Internal(format!("failed to serialize quantize task input: {error}"))
        })?;

        let operation_id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        self.model_state
            .store()
            .insert_task(TaskRecord {
                id: operation_id.clone(),
                task_type: MODEL_QUANTIZE_TASK_TYPE.to_owned(),
                status: TaskStatus::Pending,
                model_id: None,
                input_data: Some(input_data.clone()),
                result_data: None,
                error_msg: None,
                core_task_id: None,
                created_at: now,
                updated_at: now,
            })
            .await
            .map_err(|error| {
                AppCoreError::Internal(format!("failed to insert quantize task: {error}"))
            })?;

        self.spawn_quantize_operation(operation_id.clone(), input_data);
        tracing::info!(task_id = %operation_id, "model quantize task accepted");
        Ok(AcceptedOperation { operation_id })
    }

    fn spawn_quantize_operation(&self, operation_id: String, input_data: String) {
        let runtime = Arc::clone(self.model_state.runtime());
        self.worker_state.spawn_existing_operation(operation_id, move |operation| async move {
            run_quantize_operation(operation, runtime, input_data).await;
        });
    }
}

async fn run_quantize_operation(
    operation: OperationContext,
    runtime: Arc<dyn RuntimeInferenceGateway>,
    input_data: String,
) {
    if let Err(error) = operation.mark_running().await {
        warn!(error = %error, "failed to mark quantize task running");
        return;
    }

    let input: QuantizeTaskInput = match serde_json::from_str(&input_data) {
        Ok(input) => input,
        Err(error) => {
            let message = format!("invalid quantize task input: {error}");
            if let Err(mark_error) = operation.mark_failed(&message).await {
                warn!(error = %mark_error, "failed to mark quantize task failed");
            }
            return;
        }
    };

    let request = RuntimeQuantizeRequest {
        input_path: input.input_path.clone(),
        output_path: input.output_path.clone(),
        ftype: input.ftype,
        nthread: input.nthread,
        allow_requantize: input.allow_requantize,
        quantize_output_tensor: input.quantize_output_tensor,
        only_copy: input.only_copy,
        pure: input.pure,
        keep_split: input.keep_split,
        dry_run: input.dry_run,
    };

    match runtime.quantize_model(request).await {
        Ok(result) => {
            let payload = serde_json::to_string(&QuantizeTaskResult {
                output_path: result.output_path,
                layers_processed: result.layers_processed,
            })
            .unwrap_or_default();
            if let Err(error) = operation.mark_succeeded(&payload).await {
                warn!(error = %error, "failed to mark quantize task succeeded");
            }
        }
        Err(error) => {
            let message = error.to_string();
            if let Err(mark_error) = operation.mark_failed(&message).await {
                warn!(error = %mark_error, "failed to mark quantize task failed");
            }
        }
    }
}
