use crate::context::ModelState;
use crate::domain::models::{SettingPropertyView, SettingsDocumentView, UpdateSettingCommand};
use crate::error::AppCoreError;

#[derive(Clone)]
pub struct SettingsService {
    state: ModelState,
}

impl SettingsService {
    pub fn new(state: ModelState) -> Self {
        Self { state }
    }

    pub async fn list_settings(&self) -> Result<SettingsDocumentView, AppCoreError> {
        Ok(self.state.pmid().document().await)
    }

    pub async fn get_setting(&self, pmid: &str) -> Result<SettingPropertyView, AppCoreError> {
        self.state.pmid().property(pmid).await
    }

    pub async fn update_setting(
        &self,
        pmid: &str,
        command: UpdateSettingCommand,
    ) -> Result<SettingPropertyView, AppCoreError> {
        self.state.pmid().update_setting(pmid, command).await
    }
}
