#![allow(dead_code)]

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use uuid::Uuid;

use crate::domain::models::TaskStatus;
use crate::error::AppCoreError;
use crate::infra::db::{TaskRecord, TaskStore};

#[derive(Default)]
pub struct OperationManager {
    handles: Mutex<HashMap<String, tokio::task::AbortHandle>>,
}

impl std::fmt::Debug for OperationManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let count = self.handles.lock().map(|h| h.len()).unwrap_or(0);
        write!(f, "OperationManager({count} handles)")
    }
}

impl OperationManager {
    pub fn new() -> Self {
        Self { handles: Mutex::new(HashMap::new()) }
    }

    pub fn insert(&self, id: impl Into<String>, handle: tokio::task::AbortHandle) {
        match self.handles.lock() {
            Ok(mut map) => {
                map.insert(id.into(), handle);
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "OperationManager mutex poisoned on insert; handle leaked"
                )
            }
        }
    }

    pub fn cancel(&self, id: &str) -> bool {
        match self.handles.lock() {
            Ok(mut map) => {
                if let Some(h) = map.remove(id) {
                    h.abort();
                    return true;
                }
            }
            Err(e) => tracing::warn!(error = %e, "OperationManager mutex poisoned on cancel"),
        }
        false
    }

    pub fn remove(&self, id: &str) {
        match self.handles.lock() {
            Ok(mut map) => {
                map.remove(id);
            }
            Err(e) => tracing::warn!(error = %e, "OperationManager mutex poisoned on remove"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SubmitOperation {
    pub task_type: String,
    pub initial_status: TaskStatus,
    pub model_id: Option<String>,
    pub input_data: Option<String>,
}

impl SubmitOperation {
    pub fn pending(
        task_type: impl Into<String>,
        model_id: Option<String>,
        input_data: Option<String>,
    ) -> Self {
        Self {
            task_type: task_type.into(),
            initial_status: TaskStatus::Pending,
            model_id,
            input_data,
        }
    }

    pub fn running(
        task_type: impl Into<String>,
        model_id: Option<String>,
        input_data: Option<String>,
    ) -> Self {
        Self {
            task_type: task_type.into(),
            initial_status: TaskStatus::Running,
            model_id,
            input_data,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OperationContext {
    operation_id: String,
    store: Arc<crate::infra::db::AnyStore>,
}

impl OperationContext {
    pub fn id(&self) -> &str {
        &self.operation_id
    }

    pub async fn update_status(
        &self,
        status: TaskStatus,
        result_data: Option<&str>,
        error_msg: Option<&str>,
    ) -> Result<(), AppCoreError> {
        self.store.update_task_status(&self.operation_id, status, result_data, error_msg).await?;
        Ok(())
    }

    pub async fn mark_running(&self) -> Result<(), AppCoreError> {
        self.update_status(TaskStatus::Running, None, None).await
    }

    pub async fn mark_succeeded(&self, payload: &str) -> Result<(), AppCoreError> {
        self.update_status(TaskStatus::Succeeded, Some(payload), None).await
    }

    pub async fn mark_failed(&self, error_msg: &str) -> Result<(), AppCoreError> {
        self.update_status(TaskStatus::Failed, None, Some(error_msg)).await
    }

    pub async fn is_cancelled(&self) -> bool {
        matches!(
            self.store.get_task(&self.operation_id).await,
            Ok(Some(record)) if record.status == TaskStatus::Cancelled
        )
    }
}

#[derive(Clone, Debug)]
pub struct WorkerState {
    store: Arc<crate::infra::db::AnyStore>,
    grpc: Arc<crate::infra::rpc::gateway::GrpcGateway>,
    runtime_status: Arc<crate::runtime_supervisor::RuntimeSupervisorStatus>,
    model_auto_unload: Arc<crate::model_auto_unload::ModelAutoUnloadManager>,
    operations: Arc<OperationManager>,
}

impl WorkerState {
    pub fn new(
        store: Arc<crate::infra::db::AnyStore>,
        grpc: Arc<crate::infra::rpc::gateway::GrpcGateway>,
        runtime_status: Arc<crate::runtime_supervisor::RuntimeSupervisorStatus>,
        model_auto_unload: Arc<crate::model_auto_unload::ModelAutoUnloadManager>,
        operations: Arc<OperationManager>,
    ) -> Self {
        Self { store, grpc, runtime_status, model_auto_unload, operations }
    }

    pub fn store(&self) -> &Arc<crate::infra::db::AnyStore> {
        &self.store
    }

    pub fn grpc(&self) -> &Arc<crate::infra::rpc::gateway::GrpcGateway> {
        &self.grpc
    }

    pub fn runtime_status(&self) -> &Arc<crate::runtime_supervisor::RuntimeSupervisorStatus> {
        &self.runtime_status
    }

    pub fn auto_unload(&self) -> &Arc<crate::model_auto_unload::ModelAutoUnloadManager> {
        &self.model_auto_unload
    }

    pub fn operations(&self) -> &Arc<OperationManager> {
        &self.operations
    }

    pub fn cancel_operation(&self, operation_id: &str) -> bool {
        self.operations.cancel(operation_id)
    }

    pub async fn submit_operation<F, Fut>(
        &self,
        operation: SubmitOperation,
        task: F,
    ) -> Result<String, AppCoreError>
    where
        F: FnOnce(OperationContext) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let operation_id = Uuid::new_v4().to_string();
        let now = Utc::now();

        self.store
            .insert_task(TaskRecord {
                id: operation_id.clone(),
                task_type: operation.task_type,
                status: operation.initial_status,
                model_id: operation.model_id,
                input_data: operation.input_data,
                result_data: None,
                error_msg: None,
                core_task_id: None,
                created_at: now,
                updated_at: now,
            })
            .await?;

        let context =
            OperationContext { operation_id: operation_id.clone(), store: Arc::clone(&self.store) };
        self.spawn_tracked(operation_id.clone(), task(context));
        Ok(operation_id)
    }

    pub fn spawn_tracked<F>(&self, operation_id: String, task: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let operations = Arc::clone(&self.operations);
        let completion_id = operation_id.clone();
        let join = tokio::spawn(async move {
            task.await;
            operations.remove(&completion_id);
        });
        self.operations.insert(operation_id, join.abort_handle());
    }
}
