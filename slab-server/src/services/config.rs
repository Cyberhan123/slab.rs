use crate::api::v1::config::schema::{ConfigEntry, SetConfigBody};
use crate::context::ModelState;
use crate::error::ServerError;
use crate::infra::db::ConfigStore;

#[derive(Clone)]
pub struct ConfigService {
    state: ModelState,
}

impl ConfigService {
    pub fn new(state: ModelState) -> Self {
        Self { state }
    }

    pub async fn list_config(&self) -> Result<Vec<ConfigEntry>, ServerError> {
        let entries = self.state.store().list_config_values().await?;
        Ok(entries
            .into_iter()
            .map(|(key, name, value)| ConfigEntry { key, name, value })
            .collect())
    }

    pub async fn get_config_value(&self, key: String) -> Result<ConfigEntry, ServerError> {
        let (name, value) = self
            .state
            .store()
            .get_config_entry(&key)
            .await?
            .ok_or_else(|| ServerError::NotFound(format!("config key '{key}' not found")))?;
        Ok(ConfigEntry { key, name, value })
    }

    pub async fn set_config_value(
        &self,
        key: String,
        body: SetConfigBody,
    ) -> Result<ConfigEntry, ServerError> {
        self.state
            .store()
            .set_config_entry(&key, body.name.as_deref(), &body.value)
            .await?;
        self.get_config_value(key).await
    }
}

#[cfg(test)]
mod test {
    use crate::api::v1::config::schema::ConfigEntry;

    #[test]
    fn config_entry_fields() {
        let entry = ConfigEntry {
            key: "foo".into(),
            name: "Foo".into(),
            value: "bar".into(),
        };
        assert_eq!(entry.key, "foo");
        assert_eq!(entry.name, "Foo");
        assert_eq!(entry.value, "bar");
    }
}
