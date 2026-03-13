use crate::context::ModelState;
use crate::error::ServerError;
use crate::infra::db::ConfigStore;

#[derive(Debug, Clone)]
pub struct ConfigEntryView {
    pub key: String,
    pub value: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct SetConfigValueCommand {
    pub name: Option<String>,
    pub value: String,
}

#[derive(Clone)]
pub struct ConfigService {
    state: ModelState,
}

impl ConfigService {
    pub fn new(state: ModelState) -> Self {
        Self { state }
    }

    pub async fn list_config(&self) -> Result<Vec<ConfigEntryView>, ServerError> {
        let entries = self.state.store().list_config_values().await?;
        Ok(entries
            .into_iter()
            .map(|(key, name, value)| ConfigEntryView { key, name, value })
            .collect())
    }

    pub async fn get_config_value(&self, key: String) -> Result<ConfigEntryView, ServerError> {
        let (name, value) = self
            .state
            .store()
            .get_config_entry(&key)
            .await?
            .ok_or_else(|| ServerError::NotFound(format!("config key '{key}' not found")))?;
        Ok(ConfigEntryView { key, name, value })
    }

    pub async fn set_config_value(
        &self,
        key: String,
        body: SetConfigValueCommand,
    ) -> Result<ConfigEntryView, ServerError> {
        self.state
            .store()
            .set_config_entry(&key, body.name.as_deref(), &body.value)
            .await?;
        self.get_config_value(key).await
    }
}

#[cfg(test)]
mod test {
    use super::ConfigEntryView;

    #[test]
    fn config_entry_fields() {
        let entry = ConfigEntryView {
            key: "foo".into(),
            name: "Foo".into(),
            value: "bar".into(),
        };
        assert_eq!(entry.key, "foo");
        assert_eq!(entry.name, "Foo");
        assert_eq!(entry.value, "bar");
    }
}
