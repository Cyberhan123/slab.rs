use crate::infra::db::UiStateRecord;

#[derive(Debug, Clone)]
pub struct UiStateValueView {
    pub key: String,
    pub value: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct UpdateUiStateCommand {
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct DeleteUiStateView {
    pub key: String,
    pub deleted: bool,
}

/// One entry in a batched UI-state read. `value` is `None` when the requested
/// key is absent so callers can resolve every requested key from a single
/// response without distinguishing "missing" from "not asked for".
#[derive(Debug, Clone)]
pub struct UiStateBatchEntryView {
    pub key: String,
    pub value: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UiStateBatchView {
    pub entries: Vec<UiStateBatchEntryView>,
}

impl From<&UiStateRecord> for UiStateValueView {
    fn from(record: &UiStateRecord) -> Self {
        Self {
            key: record.key.clone(),
            value: record.value.clone(),
            updated_at: record.updated_at.to_rfc3339(),
        }
    }
}

impl From<&UiStateRecord> for UiStateBatchEntryView {
    fn from(record: &UiStateRecord) -> Self {
        Self {
            key: record.key.clone(),
            value: Some(record.value.clone()),
            updated_at: Some(record.updated_at.to_rfc3339()),
        }
    }
}
