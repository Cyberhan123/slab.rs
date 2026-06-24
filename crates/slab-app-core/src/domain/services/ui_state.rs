use std::collections::{HashMap, HashSet};

use chrono::Utc;

use crate::context::ModelState;
use crate::domain::models::{
    DeleteUiStateView, UiStateBatchEntryView, UiStateBatchView, UiStateValueView,
    UpdateUiStateCommand,
};
use crate::error::AppCoreError;
use crate::infra::db::{UiStateRecord, UiStateStore};

/// Maximum number of keys accepted by a batched UI-state read. Bounds the
/// query length; the frontend persists a small fixed set of store keys.
const MAX_UI_STATE_BATCH_KEYS: usize = 32;

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

    /// Reads many UI-state keys in one query. Returns an entry for every
    /// requested key (deduped, order-preserving), with `value: None` for keys
    /// that are absent. Never returns `NotFound` — a missing key is a normal
    /// "no saved state yet" result.
    pub async fn get_ui_state_batch(
        &self,
        keys: &[String],
    ) -> Result<UiStateBatchView, AppCoreError> {
        // Dedupe while preserving first-seen order; cap the request size.
        let mut unique_keys: Vec<String> = Vec::new();
        let mut seen = HashSet::new();
        for key in keys {
            let trimmed = key.trim();
            if trimmed.is_empty() || !seen.insert(trimmed.to_owned()) {
                continue;
            }
            unique_keys.push(trimmed.to_owned());
            if unique_keys.len() >= MAX_UI_STATE_BATCH_KEYS {
                break;
            }
        }

        if unique_keys.is_empty() {
            return Ok(UiStateBatchView { entries: Vec::new() });
        }

        let records = self.state.store().get_ui_state_batch(&unique_keys).await?;
        let by_key: HashMap<&str, &UiStateRecord> =
            records.iter().map(|record| (record.key.as_str(), record)).collect();

        let entries = unique_keys
            .iter()
            .map(|key| match by_key.get(key.as_str()) {
                Some(record) => UiStateBatchEntryView::from(*record),
                None => UiStateBatchEntryView { key: key.clone(), value: None, updated_at: None },
            })
            .collect();

        Ok(UiStateBatchView { entries })
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
