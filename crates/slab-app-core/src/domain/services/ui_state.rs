use chrono::Utc;

use crate::context::ModelState;
use crate::domain::models::{DeleteUiStateView, UiStateValueView, UpdateUiStateCommand};
use crate::error::AppCoreError;
use crate::infra::db::{UiStateRecord, UiStateStore};

#[derive(Clone)]
pub struct UiStateService {
    state: ModelState,
}

impl UiStateService {
    pub fn new(state: ModelState) -> Self {
        Self { state }
    }

    pub async fn get_ui_state(&self, key: &str) -> Result<UiStateValueView, AppCoreError> {
        let record = self
            .state
            .store()
            .get_ui_state(key)
            .await?
            .ok_or_else(|| AppCoreError::NotFound(format!("ui state '{key}' not found")))?;

        Ok(UiStateValueView::from(&record))
    }

    pub async fn update_ui_state(
        &self,
        key: &str,
        command: UpdateUiStateCommand,
    ) -> Result<UiStateValueView, AppCoreError> {
        let record =
            UiStateRecord { key: key.to_owned(), value: command.value, updated_at: Utc::now() };

        self.state.store().upsert_ui_state(record.clone()).await?;
        Ok(UiStateValueView::from(&record))
    }

    pub async fn delete_ui_state(&self, key: &str) -> Result<DeleteUiStateView, AppCoreError> {
        self.state.store().delete_ui_state(key).await?;
        Ok(DeleteUiStateView { key: key.to_owned(), deleted: true })
    }
}
