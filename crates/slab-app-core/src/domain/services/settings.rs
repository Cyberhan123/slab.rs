use crate::context::ModelState;
use crate::domain::models::{SettingPropertyView, SettingsDocumentView, UpdateSettingCommand};
use crate::error::AppCoreError;

#[derive(Clone)]
pub struct SettingsService {
    state: ModelState,
    agent_runtime: Option<crate::infra::agent::runtime::AgentRuntimeReloader>,
}

impl SettingsService {
    pub fn new(state: ModelState) -> Self {
        Self::new_with_agent_runtime(state, None)
    }

    pub(crate) fn new_with_agent_runtime(
        state: ModelState,
        agent_runtime: Option<crate::infra::agent::runtime::AgentRuntimeReloader>,
    ) -> Self {
        Self { state, agent_runtime }
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
        if affects_agent_runtime(pmid)
            && let Some(agent_runtime) = &self.agent_runtime
        {
            agent_runtime.reload().await?;
        }
        Ok(property)
    }
}

fn affects_agent_runtime(pmid: &str) -> bool {
    pmid.starts_with("agent.hooks.") || pmid.starts_with("agent.memories.")
}
