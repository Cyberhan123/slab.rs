#![allow(dead_code)]

use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct ModelState {
    config: Arc<crate::context::AppConfig>,
    pmid: Arc<crate::domain::services::PmidService>,
    store: Arc<crate::infra::db::AnyStore>,
    settings: Arc<crate::infra::settings::SettingsProvider>,
    grpc: Arc<crate::infra::rpc::gateway::GrpcGateway>,
    model_auto_unload: Arc<crate::model_auto_unload::ModelAutoUnloadManager>,
}

impl ModelState {
    pub fn new(
        config: Arc<crate::context::AppConfig>,
        pmid: Arc<crate::domain::services::PmidService>,
        store: Arc<crate::infra::db::AnyStore>,
        settings: Arc<crate::infra::settings::SettingsProvider>,
        grpc: Arc<crate::infra::rpc::gateway::GrpcGateway>,
        model_auto_unload: Arc<crate::model_auto_unload::ModelAutoUnloadManager>,
    ) -> Self {
        Self {
            config,
            pmid,
            store,
            settings,
            grpc,
            model_auto_unload,
        }
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

    pub fn settings(&self) -> &Arc<crate::infra::settings::SettingsProvider> {
        &self.settings
    }

    pub fn grpc(&self) -> &Arc<crate::infra::rpc::gateway::GrpcGateway> {
        &self.grpc
    }

    pub fn auto_unload(&self) -> &Arc<crate::model_auto_unload::ModelAutoUnloadManager> {
        &self.model_auto_unload
    }
}
