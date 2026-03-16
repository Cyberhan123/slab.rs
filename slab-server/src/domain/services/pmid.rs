use std::str::FromStr;
use std::sync::{Arc, RwLock};

use crate::domain::models::{
    pmid::Pmid, CloudProviderSettingValue, SettingPropertyView, UpdateSettingCommand,
    CHAT_PROVIDERS_PMID, DIFFUSION_CLIP_G_PATH_PMID, DIFFUSION_CLIP_L_PATH_PMID,
    DIFFUSION_FLASH_ATTN_PMID, DIFFUSION_KEEP_CLIP_ON_CPU_PMID, DIFFUSION_KEEP_VAE_ON_CPU_PMID,
    DIFFUSION_LORA_MODEL_DIR_PMID, DIFFUSION_MODEL_PATH_PMID, DIFFUSION_NUM_WORKERS_PMID,
    DIFFUSION_OFFLOAD_PARAMS_TO_CPU_PMID, DIFFUSION_T5XXL_PATH_PMID, DIFFUSION_TAESD_PATH_PMID,
    DIFFUSION_VAE_PATH_PMID, LLAMA_CONTEXT_LENGTH_PMID, LLAMA_NUM_WORKERS_PMID,
    MODEL_AUTO_UNLOAD_ENABLED_PMID, MODEL_AUTO_UNLOAD_IDLE_MINUTES_PMID, MODEL_CACHE_DIR_PMID,
    SETUP_BACKENDS_DIFFUSION_ASSET_PMID, SETUP_BACKENDS_DIFFUSION_TAG_PMID,
    SETUP_BACKENDS_DIR_PMID, SETUP_BACKENDS_LLAMA_ASSET_PMID, SETUP_BACKENDS_LLAMA_TAG_PMID,
    SETUP_BACKENDS_WHISPER_ASSET_PMID, SETUP_BACKENDS_WHISPER_TAG_PMID,
    SETUP_FFMPEG_AUTO_DOWNLOAD_PMID, SETUP_FFMPEG_DIR_PMID, SETUP_INITIALIZED_PMID,
    WHISPER_NUM_WORKERS_PMID,
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
    pub llama: SetupBackendReleaseConfig,
    pub whisper: SetupBackendReleaseConfig,
    pub diffusion: SetupBackendReleaseConfig,
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
        let pmid = Pmid::from_str(pmid.as_ref())
            .map_err(|_| ServerError::NotFound(format!("setting pmid '{}' not found", pmid.as_ref())))?;
        let pmid_path = pmid.to_path();
        let property = self.settings.update(&pmid_path, command).await?;
        self.refresh().await?;
        Ok(property)
    }
}

async fn load_config(settings: &SettingsProvider) -> Result<PmidConfig, ServerError> {
    Ok(PmidConfig {
        setup: SetupConfig {
            initialized: settings.get_bool(SETUP_INITIALIZED_PMID).await?,
            ffmpeg: SetupFfmpegConfig {
                auto_download: settings.get_bool(SETUP_FFMPEG_AUTO_DOWNLOAD_PMID).await?,
                dir: settings.get_optional_string(SETUP_FFMPEG_DIR_PMID).await?,
            },
            backends: SetupBackendsConfig {
                dir: settings.get_optional_string(SETUP_BACKENDS_DIR_PMID).await?,
                llama: SetupBackendReleaseConfig {
                    tag: settings.get_optional_string(SETUP_BACKENDS_LLAMA_TAG_PMID).await?,
                    asset: settings
                        .get_optional_string(SETUP_BACKENDS_LLAMA_ASSET_PMID)
                        .await?,
                },
                whisper: SetupBackendReleaseConfig {
                    tag: settings
                        .get_optional_string(SETUP_BACKENDS_WHISPER_TAG_PMID)
                        .await?,
                    asset: settings
                        .get_optional_string(SETUP_BACKENDS_WHISPER_ASSET_PMID)
                        .await?,
                },
                diffusion: SetupBackendReleaseConfig {
                    tag: settings
                        .get_optional_string(SETUP_BACKENDS_DIFFUSION_TAG_PMID)
                        .await?,
                    asset: settings
                        .get_optional_string(SETUP_BACKENDS_DIFFUSION_ASSET_PMID)
                        .await?,
                },
            },
        },
        runtime: RuntimeConfig {
            model_cache_dir: settings.get_optional_string(MODEL_CACHE_DIR_PMID).await?,
            llama: RuntimeLlamaConfig {
                num_workers: required_u32(settings, LLAMA_NUM_WORKERS_PMID).await?,
                context_length: settings.get_optional_u32(LLAMA_CONTEXT_LENGTH_PMID).await?,
            },
            whisper: RuntimeWorkerConfig {
                num_workers: required_u32(settings, WHISPER_NUM_WORKERS_PMID).await?,
            },
            diffusion: RuntimeWorkerConfig {
                num_workers: required_u32(settings, DIFFUSION_NUM_WORKERS_PMID).await?,
            },
            model_auto_unload: RuntimeModelAutoUnloadConfig {
                enabled: settings.get_bool(MODEL_AUTO_UNLOAD_ENABLED_PMID).await?,
                idle_minutes: required_u32(settings, MODEL_AUTO_UNLOAD_IDLE_MINUTES_PMID).await?,
            },
        },
        chat: ChatConfig {
            providers: settings.get_chat_providers(CHAT_PROVIDERS_PMID).await?,
        },
        diffusion: DiffusionConfig {
            paths: DiffusionPathsConfig {
                model: settings.get_optional_string(DIFFUSION_MODEL_PATH_PMID).await?,
                vae: settings.get_optional_string(DIFFUSION_VAE_PATH_PMID).await?,
                taesd: settings.get_optional_string(DIFFUSION_TAESD_PATH_PMID).await?,
                lora_model_dir: settings
                    .get_optional_string(DIFFUSION_LORA_MODEL_DIR_PMID)
                    .await?,
                clip_l: settings.get_optional_string(DIFFUSION_CLIP_L_PATH_PMID).await?,
                clip_g: settings.get_optional_string(DIFFUSION_CLIP_G_PATH_PMID).await?,
                t5xxl: settings.get_optional_string(DIFFUSION_T5XXL_PATH_PMID).await?,
            },
            performance: DiffusionPerformanceConfig {
                flash_attn: settings.get_bool(DIFFUSION_FLASH_ATTN_PMID).await?,
                keep_vae_on_cpu: settings.get_bool(DIFFUSION_KEEP_VAE_ON_CPU_PMID).await?,
                keep_clip_on_cpu: settings.get_bool(DIFFUSION_KEEP_CLIP_ON_CPU_PMID).await?,
                offload_params_to_cpu: settings
                    .get_bool(DIFFUSION_OFFLOAD_PARAMS_TO_CPU_PMID)
                    .await?,
            },
        },
    })
}

async fn required_u32(settings: &SettingsProvider, pmid: &str) -> Result<u32, ServerError> {
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
    use crate::domain::models::{SettingsValuesFile, UpdateSettingOperation};

    fn temp_settings_path() -> PathBuf {
        let base = std::env::temp_dir().join(format!("slab-pmid-test-{}", Uuid::new_v4()));
        base.join("settings.json")
    }

    #[tokio::test]
    async fn loads_typed_snapshot_from_settings_provider() {
        let path = temp_settings_path();
        let settings = Arc::new(SettingsProvider::load(path.clone()).await.expect("provider"));

        settings
            .update(
                SETUP_FFMPEG_DIR_PMID,
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
        let settings = Arc::new(SettingsProvider::load(path.clone()).await.expect("provider"));
        let service = PmidService::load(Arc::clone(&settings))
            .await
            .expect("pmid service");

        service
            .update_setting(
                SETUP_INITIALIZED_PMID,
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
            file.values.get(SETUP_INITIALIZED_PMID),
            Some(&json!(true))
        );

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn refresh_picks_up_external_settings_changes() {
        let path = temp_settings_path();
        let settings = Arc::new(SettingsProvider::load(path.clone()).await.expect("provider"));
        let _service = PmidService::load(Arc::clone(&settings))
            .await
            .expect("pmid service");

        fs::write(
            &path,
            serde_json::to_string_pretty(&SettingsValuesFile {
                version: 1,
                values: BTreeMap::from([(MODEL_CACHE_DIR_PMID.to_owned(), json!("C:/models"))]),
            })
            .expect("serialize"),
        )
        .expect("write");

        let reloaded_settings = Arc::new(SettingsProvider::load(path.clone()).await.expect("reload"));
        let refreshed_service = PmidService::load(reloaded_settings)
            .await
            .expect("reloaded pmid service");

        assert_eq!(
            refreshed_service.config().runtime.model_cache_dir.as_deref(),
            Some("C:/models")
        );

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }
}
