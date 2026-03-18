use std::sync::{Arc, RwLock};

use crate::domain::models::{
    CloudProviderSettingValue, SettingPropertyView, SettingsDocumentView, UpdateSettingCommand,
    UpdateSettingOperation, PMID,
};
use crate::error::ServerError;
use crate::infra::settings::SettingsProvider;

#[derive(Debug, Clone, Default)]
pub struct PmidConfig {
    pub setup: SetupConfig,
    pub runtime: RuntimeConfig,
    pub chat: ChatConfig,
    pub diffusion: DiffusionConfig,
}

#[derive(Debug, Clone, Default)]
pub struct SetupConfig {
    pub initialized: bool,
    pub ffmpeg: SetupFfmpegConfig,
    pub backends: SetupBackendsConfig,
}

#[derive(Debug, Clone, Default)]
pub struct SetupFfmpegConfig {
    #[allow(dead_code)]
    pub auto_download: bool,
    pub dir: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SetupBackendsConfig {
    pub dir: Option<String>,
    pub ggml_llama: SetupBackendReleaseConfig,
    pub ggml_whisper: SetupBackendReleaseConfig,
    pub ggml_diffusion: SetupBackendReleaseConfig,
    pub candle_llama: SetupBackendReleaseConfig,
    pub candle_whisper: SetupBackendReleaseConfig,
    pub candle_diffusion: SetupBackendReleaseConfig,
    pub onnx: SetupBackendReleaseConfig,
}

#[derive(Debug, Clone, Default)]
pub struct SetupBackendReleaseConfig {
    pub tag: Option<String>,
    pub asset: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeConfig {
    pub model_cache_dir: Option<String>,
    pub llama: RuntimeLlamaConfig,
    pub whisper: RuntimeWorkerConfig,
    pub diffusion: RuntimeWorkerConfig,
    pub model_auto_unload: RuntimeModelAutoUnloadConfig,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeLlamaConfig {
    pub num_workers: u32,
    pub context_length: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeWorkerConfig {
    pub num_workers: u32,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeModelAutoUnloadConfig {
    pub enabled: bool,
    pub idle_minutes: u32,
}

#[derive(Debug, Clone, Default)]
pub struct ChatConfig {
    pub providers: Vec<CloudProviderSettingValue>,
}

#[derive(Debug, Clone, Default)]
pub struct DiffusionConfig {
    pub paths: DiffusionPathsConfig,
    pub performance: DiffusionPerformanceConfig,
}

#[derive(Debug, Clone, Default)]
pub struct DiffusionPathsConfig {
    pub model: Option<String>,
    pub vae: Option<String>,
    pub taesd: Option<String>,
    pub lora_model_dir: Option<String>,
    pub clip_l: Option<String>,
    pub clip_g: Option<String>,
    pub t5xxl: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct DiffusionPerformanceConfig {
    pub flash_attn: bool,
    pub keep_vae_on_cpu: bool,
    pub keep_clip_on_cpu: bool,
    pub offload_params_to_cpu: bool,
}

#[derive(Debug, Clone)]
pub struct PmidService {
    settings: Arc<SettingsProvider>,
    config: Arc<RwLock<PmidConfig>>,
}

impl PmidService {
    pub async fn load(settings: Arc<SettingsProvider>) -> Result<Self, ServerError> {
        let config = load_config(&settings).await?;
        Ok(Self {
            settings,
            config: Arc::new(RwLock::new(config)),
        })
    }

    pub fn config(&self) -> PmidConfig {
        self.config
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub async fn document(&self) -> SettingsDocumentView {
        self.settings.document().await
    }

    pub async fn property(&self, pmid: &str) -> Result<SettingPropertyView, ServerError> {
        self.settings.property(pmid).await
    }

    pub async fn refresh(&self) -> Result<PmidConfig, ServerError> {
        let next = load_config(&self.settings).await?;
        *self
            .config
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = next.clone();
        Ok(next)
    }

    pub async fn update_setting(
        &self,
        pmid: impl AsRef<str>,
        command: UpdateSettingCommand,
    ) -> Result<SettingPropertyView, ServerError> {
        let property = self.settings.update(pmid.as_ref(), command).await?;
        self.refresh().await?;
        Ok(property)
    }

    pub async fn set_setup_initialized(
        &self,
        initialized: bool,
    ) -> Result<SettingPropertyView, ServerError> {
        self.update_setting(
            PMID.setup.initialized(),
            UpdateSettingCommand {
                op: UpdateSettingOperation::Set,
                value: Some(serde_json::Value::Bool(initialized)),
            },
        )
        .await
    }
}

async fn load_config(settings: &SettingsProvider) -> Result<PmidConfig, ServerError> {
    Ok(PmidConfig {
        setup: SetupConfig {
            initialized: settings.get_bool(PMID.setup.initialized()).await?,
            ffmpeg: SetupFfmpegConfig {
                auto_download: settings.get_bool(PMID.setup.ffmpeg.auto_download()).await?,
                dir: settings
                    .get_optional_string(PMID.setup.ffmpeg.dir())
                    .await?,
            },
            backends: SetupBackendsConfig {
                dir: settings
                    .get_optional_string(PMID.setup.backends.dir())
                    .await?,
                ggml_llama: SetupBackendReleaseConfig {
                    tag: settings
                        .get_optional_string(PMID.setup.backends.ggml_llama.tag())
                        .await?,
                    asset: settings
                        .get_optional_string(PMID.setup.backends.ggml_llama.asset())
                        .await?,
                },
                ggml_whisper: SetupBackendReleaseConfig {
                    tag: settings
                        .get_optional_string(PMID.setup.backends.ggml_whisper.tag())
                        .await?,
                    asset: settings
                        .get_optional_string(PMID.setup.backends.ggml_whisper.asset())
                        .await?,
                },
                ggml_diffusion: SetupBackendReleaseConfig {
                    tag: settings
                        .get_optional_string(PMID.setup.backends.ggml_diffusion.tag())
                        .await?,
                    asset: settings
                        .get_optional_string(PMID.setup.backends.ggml_diffusion.asset())
                        .await?,
                },
                candle_llama: SetupBackendReleaseConfig {
                    tag: settings
                        .get_optional_string(PMID.setup.backends.candle_llama.tag())
                        .await?,
                    asset: settings
                        .get_optional_string(PMID.setup.backends.candle_llama.asset())
                        .await?,
                },
                candle_whisper: SetupBackendReleaseConfig {
                    tag: settings
                        .get_optional_string(PMID.setup.backends.candle_whisper.tag())
                        .await?,
                    asset: settings
                        .get_optional_string(PMID.setup.backends.candle_whisper.asset())
                        .await?,
                },
                candle_diffusion: SetupBackendReleaseConfig {
                    tag: settings
                        .get_optional_string(PMID.setup.backends.candle_diffusion.tag())
                        .await?,
                    asset: settings
                        .get_optional_string(PMID.setup.backends.candle_diffusion.asset())
                        .await?,
                },
                onnx: SetupBackendReleaseConfig {
                    tag: settings
                        .get_optional_string(PMID.setup.backends.onnx.tag())
                        .await?,
                    asset: settings
                        .get_optional_string(PMID.setup.backends.onnx.asset())
                        .await?,
                },
            },
        },
        runtime: RuntimeConfig {
            model_cache_dir: settings
                .get_optional_string(PMID.runtime.model_cache_dir())
                .await?,
            llama: RuntimeLlamaConfig {
                num_workers: required_u32(settings, PMID.runtime.llama.num_workers()).await?,
                context_length: settings
                    .get_optional_u32(PMID.runtime.llama.context_length())
                    .await?,
            },
            whisper: RuntimeWorkerConfig {
                num_workers: required_u32(settings, PMID.runtime.whisper.num_workers()).await?,
            },
            diffusion: RuntimeWorkerConfig {
                num_workers: required_u32(settings, PMID.runtime.diffusion.num_workers()).await?,
            },
            model_auto_unload: RuntimeModelAutoUnloadConfig {
                enabled: settings
                    .get_bool(PMID.runtime.model_auto_unload.enabled())
                    .await?,
                idle_minutes: required_u32(settings, PMID.runtime.model_auto_unload.idle_minutes())
                    .await?,
            },
        },
        chat: ChatConfig {
            providers: settings.get_chat_providers(PMID.chat.providers()).await?,
        },
        diffusion: DiffusionConfig {
            paths: DiffusionPathsConfig {
                model: settings
                    .get_optional_string(PMID.diffusion.paths.model())
                    .await?,
                vae: settings
                    .get_optional_string(PMID.diffusion.paths.vae())
                    .await?,
                taesd: settings
                    .get_optional_string(PMID.diffusion.paths.taesd())
                    .await?,
                lora_model_dir: settings
                    .get_optional_string(PMID.diffusion.paths.lora_model_dir())
                    .await?,
                clip_l: settings
                    .get_optional_string(PMID.diffusion.paths.clip_l())
                    .await?,
                clip_g: settings
                    .get_optional_string(PMID.diffusion.paths.clip_g())
                    .await?,
                t5xxl: settings
                    .get_optional_string(PMID.diffusion.paths.t5xxl())
                    .await?,
            },
            performance: DiffusionPerformanceConfig {
                flash_attn: settings
                    .get_bool(PMID.diffusion.performance.flash_attn())
                    .await?,
                keep_vae_on_cpu: settings
                    .get_bool(PMID.diffusion.performance.keep_vae_on_cpu())
                    .await?,
                keep_clip_on_cpu: settings
                    .get_bool(PMID.diffusion.performance.keep_clip_on_cpu())
                    .await?,
                offload_params_to_cpu: settings
                    .get_bool(PMID.diffusion.performance.offload_params_to_cpu())
                    .await?,
            },
        },
    })
}

async fn required_u32(
    settings: &SettingsProvider,
    pmid: impl AsRef<str>,
) -> Result<u32, ServerError> {
    let pmid = pmid.as_ref();
    settings
        .get_optional_u32(pmid)
        .await?
        .ok_or_else(|| ServerError::Internal(format!("setting '{}' resolved to null", pmid)))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;

    use serde_json::json;
    use uuid::Uuid;

    use super::*;
    use crate::domain::models::SettingsValuesFile;

    fn temp_settings_path() -> PathBuf {
        let base = std::env::temp_dir().join(format!("slab-pmid-test-{}", Uuid::new_v4()));
        base.join("settings.json")
    }

    #[tokio::test]
    async fn loads_typed_snapshot_from_settings_provider() {
        let path = temp_settings_path();
        let settings = Arc::new(
            SettingsProvider::load(path.clone())
                .await
                .expect("provider"),
        );

        settings
            .update(
                PMID.setup.ffmpeg.dir(),
                UpdateSettingCommand {
                    op: UpdateSettingOperation::Set,
                    value: Some(json!("C:/ffmpeg")),
                },
            )
            .await
            .expect("set ffmpeg dir");

        let service = PmidService::load(Arc::clone(&settings))
            .await
            .expect("pmid service");
        let config = service.config();

        assert_eq!(config.setup.ffmpeg.dir.as_deref(), Some("C:/ffmpeg"));

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn update_setting_refreshes_cached_snapshot() {
        let path = temp_settings_path();
        let settings = Arc::new(
            SettingsProvider::load(path.clone())
                .await
                .expect("provider"),
        );
        let service = PmidService::load(Arc::clone(&settings))
            .await
            .expect("pmid service");

        service
            .update_setting(
                PMID.setup.initialized(),
                UpdateSettingCommand {
                    op: UpdateSettingOperation::Set,
                    value: Some(json!(true)),
                },
            )
            .await
            .expect("update");

        assert!(service.config().setup.initialized);

        let file: SettingsValuesFile =
            serde_json::from_str(&fs::read_to_string(&path).expect("file")).expect("json");
        assert_eq!(
            file.values.get(PMID.setup.initialized().as_str()),
            Some(&json!(true))
        );

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn refresh_picks_up_external_settings_changes() {
        let path = temp_settings_path();
        let settings = Arc::new(
            SettingsProvider::load(path.clone())
                .await
                .expect("provider"),
        );
        let _service = PmidService::load(Arc::clone(&settings))
            .await
            .expect("pmid service");

        fs::write(
            &path,
            serde_json::to_string_pretty(&SettingsValuesFile {
                version: 1,
                values: BTreeMap::from([(
                    PMID.runtime.model_cache_dir().into_string(),
                    json!("C:/models"),
                )]),
            })
            .expect("serialize"),
        )
        .expect("write");

        let reloaded_settings =
            Arc::new(SettingsProvider::load(path.clone()).await.expect("reload"));
        let refreshed_service = PmidService::load(reloaded_settings)
            .await
            .expect("reloaded pmid service");

        assert_eq!(
            refreshed_service
                .config()
                .runtime
                .model_cache_dir
                .as_deref(),
            Some("C:/models")
        );

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn set_setup_initialized_refreshes_cached_snapshot() {
        let path = temp_settings_path();
        let settings = Arc::new(
            SettingsProvider::load(path.clone())
                .await
                .expect("provider"),
        );
        let service = PmidService::load(Arc::clone(&settings))
            .await
            .expect("pmid service");

        service
            .set_setup_initialized(true)
            .await
            .expect("set setup initialized");

        assert!(service.config().setup.initialized);

        let file: SettingsValuesFile =
            serde_json::from_str(&fs::read_to_string(&path).expect("file")).expect("json");
        assert_eq!(
            file.values.get(PMID.setup.initialized().as_str()),
            Some(&json!(true))
        );

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn update_setting_uses_provider_not_found_for_unknown_pmid() {
        let path = temp_settings_path();
        let settings = Arc::new(
            SettingsProvider::load(path.clone())
                .await
                .expect("provider"),
        );
        let service = PmidService::load(Arc::clone(&settings))
            .await
            .expect("pmid service");

        let error = service
            .update_setting(
                "missing.setting",
                UpdateSettingCommand {
                    op: UpdateSettingOperation::Set,
                    value: Some(json!(true)),
                },
            )
            .await
            .expect_err("missing pmid should fail");

        assert!(matches!(error, ServerError::NotFound(_)));
        assert!(error.to_string().contains("missing.setting"));

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }
}
