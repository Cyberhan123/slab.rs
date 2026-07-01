use crate::context::ModelState;
use crate::domain::models::{
    SettingChangeEffect, SettingPropertyView, SettingsDocumentView, UpdateSettingCommand,
};
use crate::domain::services::cloud_activation;
use crate::domain::services::model::ModelService;
use crate::domain::services::pmid::change_effect_for;
use crate::error::AppCoreError;

#[derive(Clone)]
pub struct SettingsService {
    state: ModelState,
    agent_runtime: Option<crate::infra::agent::runtime::AgentRuntimeReloader>,
    model_service: Option<ModelService>,
}

impl SettingsService {
    pub fn new(state: ModelState) -> Self {
        Self::new_with(state, None, None)
    }

    pub(crate) fn new_with(
        state: ModelState,
        agent_runtime: Option<crate::infra::agent::runtime::AgentRuntimeReloader>,
        model_service: Option<ModelService>,
    ) -> Self {
        Self { state, agent_runtime, model_service }
    }

    pub async fn list_settings(&self) -> Result<SettingsDocumentView, AppCoreError> {
        Ok(self.state.pmid().document().await)
    }

    pub async fn get_setting(&self, pmid: &str) -> Result<SettingPropertyView, AppCoreError> {
        self.state.pmid().property(pmid).await.map_err(Into::into)
    }

    pub async fn update_setting(
        &self,
        pmid: &str,
        command: UpdateSettingCommand,
    ) -> Result<SettingPropertyView, AppCoreError> {
        let property = self.state.pmid().update_setting(pmid, command).await?;
        if setting_affects_agent_runtime(pmid)
            && let Some(agent_runtime) = &self.agent_runtime
        {
            agent_runtime.reload().await?;
        }
        if pmid == "providers.registry"
            && let Some(model_service) = &self.model_service
        {
            // Best-effort: never fail the settings save because of cloud activation.
            cloud_activation::sync_provider_models(model_service, &self.state).await;
        }
        Ok(property)
    }
}

fn setting_affects_agent_runtime(pmid: &str) -> bool {
    change_effect_for(pmid) == SettingChangeEffect::Live
        && (pmid.starts_with("agent.hooks.") || pmid.starts_with("agent.memories."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pmid_change_effects_keep_agent_reload_scope_narrow() {
        assert_eq!(change_effect_for("agent.hooks.enabled"), SettingChangeEffect::Live);
        assert_eq!(change_effect_for("agent.memories.enabled"), SettingChangeEffect::Live);
        assert_eq!(change_effect_for("runtime.capacity.queue"), SettingChangeEffect::NeedsRestart);
        assert_eq!(change_effect_for("server.admin.token"), SettingChangeEffect::Live);
        assert_eq!(change_effect_for("providers.registry"), SettingChangeEffect::Live);
        assert_eq!(change_effect_for("models.download_source"), SettingChangeEffect::Live);
        assert_eq!(change_effect_for("models.auto_unload.enabled"), SettingChangeEffect::Live);
        assert_eq!(
            change_effect_for("runtime.ggml.backends.llama.context_length"),
            SettingChangeEffect::NeedsModelReload
        );
        assert_eq!(change_effect_for("agent.tools.mcp.servers"), SettingChangeEffect::NeedsRestart);
        assert_eq!(change_effect_for("general.language"), SettingChangeEffect::None);
    }

    #[test]
    fn agent_runtime_reload_scope_stays_agent_only() {
        assert!(setting_affects_agent_runtime("agent.hooks.enabled"));
        assert!(setting_affects_agent_runtime("agent.memories.enabled"));
        assert!(!setting_affects_agent_runtime("server.admin.token"));
        assert!(!setting_affects_agent_runtime("providers.registry"));
        assert!(!setting_affects_agent_runtime("models.auto_unload.enabled"));
    }
}
