//! Settings, launch configuration, and PMID catalog for Slab.

pub mod app_config;
mod descriptor;
mod error;
pub mod launch;
mod pmid_service;
mod provider;
mod settings;
mod view;

pub use app_config::{
    AppConfig, Config, default_app_dir, default_database_path, default_database_url,
    default_exec_rules_dir, default_exec_rules_dir_for_settings_path, default_model_config_dir,
    default_model_config_dir_for_settings_path, default_output_dir_for_settings_path,
    default_plugin_install_dir_for_settings_path, default_plugins_dir, default_runtime_ipc_dir,
    default_runtime_log_dir, default_session_state_dir, default_settings_path,
};
pub use error::ConfigError;
pub use launch::{
    LaunchHostPaths, LaunchProfile, ResolvedGatewaySpec, ResolvedLaunchSpec,
    ResolvedRuntimeChildSpec, ResolvedRuntimeEndpoints, resolve_launch_spec,
};
pub use pmid_service::{PmidService, change_effect_for};
pub use provider::{
    SettingsDocumentProvider, seed_settings_document_from_env_if_missing,
    settings_document_to_json_value,
};
pub use settings::*;
pub use view::{
    SettingChangeEffect, SettingOverrideSource, SettingPropertySchema, SettingPropertyView,
    SettingValidationErrorData, SettingValue, SettingValueType, SettingsDocumentView,
    SettingsSectionView, SettingsSubsectionView, UpdateSettingCommand, UpdateSettingOperation,
};
