#![allow(dead_code)]

use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct ModelState {
    config: Arc<crate::context::AppConfig>,
    store: Arc<crate::infra::db::AnyStore>,
    grpc: Arc<crate::infra::rpc::gateway::GrpcGateway>,
    model_auto_unload: Arc<crate::model_auto_unload::ModelAutoUnloadManager>,
}

impl ModelState {
    pub fn new(
        config: Arc<crate::context::AppConfig>,
        store: Arc<crate::infra::db::AnyStore>,
        grpc: Arc<crate::infra::rpc::gateway::GrpcGateway>,
        model_auto_unload: Arc<crate::model_auto_unload::ModelAutoUnloadManager>,
    ) -> Self {
        Self {
            config,
            store,
            grpc,
            model_auto_unload,
        }
    }

    pub fn config(&self) -> &Arc<crate::context::AppConfig> {
        &self.config
    }

    pub fn store(&self) -> &Arc<crate::infra::db::AnyStore> {
        &self.store
    }

    pub fn grpc(&self) -> &Arc<crate::infra::rpc::gateway::GrpcGateway> {
        &self.grpc
    }

    pub fn auto_unload(&self) -> &Arc<crate::model_auto_unload::ModelAutoUnloadManager> {
        &self.model_auto_unload
    }
}
