use crate::context::ModelState;
use crate::domain::models::{SettingPropertyView, SettingsDocumentView, UpdateSettingCommand};
use crate::error::ServerError;

#[derive(Clone)]
pub struct SettingsService {
    state: ModelState,
}

impl SettingsService {
    pub fn new(state: ModelState) -> Self {
        Self { state }
    }

    pub async fn list_settings(&self) -> Result<SettingsDocumentView, ServerError> {
        Ok(self.state.settings().document().await)
    }

    pub async fn get_setting(&self, pmid: &str) -> Result<SettingPropertyView, ServerError> {
        self.state.settings().property(pmid).await
    }

    pub async fn update_setting(
        &self,
        pmid: &str,
        command: UpdateSettingCommand,
    ) -> Result<SettingPropertyView, ServerError> {
        self.state.pmid().update_setting(pmid, command).await
    }
}
