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

impl From<&UiStateRecord> for UiStateValueView {
    fn from(record: &UiStateRecord) -> Self {
        Self {
            key: record.key.clone(),
            value: record.value.clone(),
            updated_at: record.updated_at.to_rfc3339(),
        }
    }
}
