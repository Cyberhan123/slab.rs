#![allow(dead_code)]

use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct ModelState {
    config: Arc<crate::context::AppConfig>,
    pmid: Arc<crate::domain::services::PmidService>,
    store: Arc<crate::infra::db::AnyStore>,
    grpc: Arc<crate::infra::rpc::gateway::GrpcGateway>,
    runtime_status: Arc<crate::runtime_supervisor::RuntimeSupervisorStatus>,
    model_auto_unload: Arc<crate::model_auto_unload::ModelAutoUnloadManager>,
}

impl ModelState {
    pub fn new(
        config: Arc<crate::context::AppConfig>,
        pmid: Arc<crate::domain::services::PmidService>,
        store: Arc<crate::infra::db::AnyStore>,
        grpc: Arc<crate::infra::rpc::gateway::GrpcGateway>,
        runtime_status: Arc<crate::runtime_supervisor::RuntimeSupervisorStatus>,
        model_auto_unload: Arc<crate::model_auto_unload::ModelAutoUnloadManager>,
    ) -> Self {
        Self { config, pmid, store, grpc, runtime_status, model_auto_unload }
    }

    pub fn config(&self) -> &Arc<crate::context::AppConfig> {
        &self.config
    }

    pub fn pmid(&self) -> &Arc<crate::domain::services::PmidService> {
        &self.pmid
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
}
