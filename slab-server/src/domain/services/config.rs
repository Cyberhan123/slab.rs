use crate::context::ModelState;
use crate::domain::models::{ConfigEntryView, SetConfigValueCommand};
use crate::domain::services::settings::{get_config_entry, list_config_entries, set_config_entry};
use crate::error::ServerError;

#[derive(Clone)]
pub struct ConfigService {
    state: ModelState,
}

impl ConfigService {
    pub fn new(state: ModelState) -> Self {
        Self { state }
    }

    pub async fn list_config(&self) -> Result<Vec<ConfigEntryView>, ServerError> {
        list_config_entries(&self.state).await
    }

    pub async fn get_config_value(&self, key: String) -> Result<ConfigEntryView, ServerError> {
        get_config_entry(&self.state, &key).await
    }

    pub async fn set_config_value(
        &self,
        key: String,
        body: SetConfigValueCommand,
    ) -> Result<ConfigEntryView, ServerError> {
        set_config_entry(&self.state, &key, body.name.as_deref(), &body.value).await
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
