use std::path::Path;

use slab_config::SettingsMigrationResult;

use crate::domain::services::setup::{SETUP_INITIALIZED_CONFIG_KEY, SETUP_INITIALIZED_CONFIG_NAME};
use crate::error::AppCoreError;
use crate::infra::db::repository::config::ConfigStore;

pub async fn migrate_legacy_settings_if_needed<S>(
    path: &Path,
    store: &S,
) -> Result<SettingsMigrationResult, AppCoreError>
where
    S: ConfigStore,
{
    let result = slab_config::migrate_legacy_settings_file_if_needed(path)?;

    if let Some(initialized) = result.setup_initialized {
        let value = if initialized { "true" } else { "false" };
        store
            .set_config_entry(
                SETUP_INITIALIZED_CONFIG_KEY,
                Some(SETUP_INITIALIZED_CONFIG_NAME),
                value,
            )
            .await
            .map_err(|error| {
                AppCoreError::Internal(format!(
                    "failed to persist migrated setup state in config_store: {error}"
                ))
            })?;
    }

    Ok(result)
}
