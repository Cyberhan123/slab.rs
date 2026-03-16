use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use strum::{AsRefStr, EnumString, ParseError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pmid {
    Setup(Setup),
    Runtime(Runtime),
    Chat(Chat),
    Diffusion(Diffusion),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Setup {
    Initialized,
    Ffmpeg(SetupFfmpeg),
    Backends(SetupBackends),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, AsRefStr, EnumString, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SetupFfmpeg {
    AutoDownload,
    Dir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SetupBackends {
    Dir,
    Llama(BackendAsset),
    Whisper(BackendAsset),
    Diffusion(BackendAsset),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, AsRefStr, EnumString, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum BackendAsset {
    Tag,
    Asset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Runtime {
    ModelCacheDir,
    Llama(RuntimeLlama),
    Whisper(RuntimeWorker),
    Diffusion(RuntimeWorker),
    ModelAutoUnload(RuntimeModelAutoUnload),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, AsRefStr, EnumString, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum RuntimeLlama {
    NumWorkers,
    ContextLength,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, AsRefStr, EnumString, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum RuntimeWorker {
    NumWorkers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, AsRefStr, EnumString, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum RuntimeModelAutoUnload {
    Enabled,
    IdleMinutes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, AsRefStr, EnumString, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum Chat {
    Providers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Diffusion {
    Paths(DiffusionPaths),
    Performance(DiffusionPerformance),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, AsRefStr, EnumString, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum DiffusionPaths {
    Model,
    Vae,
    Taesd,
    LoraModelDir,
    ClipL,
    ClipG,
    #[strum(serialize = "t5xxl")]
    #[serde(rename = "t5xxl")]
    T5xxl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, AsRefStr, EnumString, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum DiffusionPerformance {
    FlashAttn,
    KeepVaeOnCpu,
    KeepClipOnCpu,
    OffloadParamsToCpu,
}

macro_rules! define_pmids {
    ($( $const_name:ident => ($pmid:expr) = $path:literal; )+ ) => {
        const PMID_ENTRIES: &[(Pmid, &str)] = &[
            $(
                ($pmid, $path),
            )+
        ];

        $(
            pub const $const_name: &str = $path;
        )+
    };
}

define_pmids! {
    SETUP_INITIALIZED_PMID => (Pmid::Setup(Setup::Initialized)) = "setup.initialized";
    SETUP_FFMPEG_AUTO_DOWNLOAD_PMID => (Pmid::Setup(Setup::Ffmpeg(SetupFfmpeg::AutoDownload))) = "setup.ffmpeg.auto_download";
    SETUP_FFMPEG_DIR_PMID => (Pmid::Setup(Setup::Ffmpeg(SetupFfmpeg::Dir))) = "setup.ffmpeg.dir";
    SETUP_BACKENDS_DIR_PMID => (Pmid::Setup(Setup::Backends(SetupBackends::Dir))) = "setup.backends.dir";
    SETUP_BACKENDS_LLAMA_TAG_PMID => (Pmid::Setup(Setup::Backends(SetupBackends::Llama(BackendAsset::Tag)))) = "setup.backends.llama.tag";
    SETUP_BACKENDS_LLAMA_ASSET_PMID => (Pmid::Setup(Setup::Backends(SetupBackends::Llama(BackendAsset::Asset)))) = "setup.backends.llama.asset";
    SETUP_BACKENDS_WHISPER_TAG_PMID => (Pmid::Setup(Setup::Backends(SetupBackends::Whisper(BackendAsset::Tag)))) = "setup.backends.whisper.tag";
    SETUP_BACKENDS_WHISPER_ASSET_PMID => (Pmid::Setup(Setup::Backends(SetupBackends::Whisper(BackendAsset::Asset)))) = "setup.backends.whisper.asset";
    SETUP_BACKENDS_DIFFUSION_TAG_PMID => (Pmid::Setup(Setup::Backends(SetupBackends::Diffusion(BackendAsset::Tag)))) = "setup.backends.diffusion.tag";
    SETUP_BACKENDS_DIFFUSION_ASSET_PMID => (Pmid::Setup(Setup::Backends(SetupBackends::Diffusion(BackendAsset::Asset)))) = "setup.backends.diffusion.asset";
    MODEL_CACHE_DIR_PMID => (Pmid::Runtime(Runtime::ModelCacheDir)) = "runtime.model_cache_dir";
    LLAMA_NUM_WORKERS_PMID => (Pmid::Runtime(Runtime::Llama(RuntimeLlama::NumWorkers))) = "runtime.llama.num_workers";
    LLAMA_CONTEXT_LENGTH_PMID => (Pmid::Runtime(Runtime::Llama(RuntimeLlama::ContextLength))) = "runtime.llama.context_length";
    WHISPER_NUM_WORKERS_PMID => (Pmid::Runtime(Runtime::Whisper(RuntimeWorker::NumWorkers))) = "runtime.whisper.num_workers";
    DIFFUSION_NUM_WORKERS_PMID => (Pmid::Runtime(Runtime::Diffusion(RuntimeWorker::NumWorkers))) = "runtime.diffusion.num_workers";
    MODEL_AUTO_UNLOAD_ENABLED_PMID => (Pmid::Runtime(Runtime::ModelAutoUnload(RuntimeModelAutoUnload::Enabled))) = "runtime.model_auto_unload.enabled";
    MODEL_AUTO_UNLOAD_IDLE_MINUTES_PMID => (Pmid::Runtime(Runtime::ModelAutoUnload(RuntimeModelAutoUnload::IdleMinutes))) = "runtime.model_auto_unload.idle_minutes";
    CHAT_PROVIDERS_PMID => (Pmid::Chat(Chat::Providers)) = "chat.providers";
    DIFFUSION_MODEL_PATH_PMID => (Pmid::Diffusion(Diffusion::Paths(DiffusionPaths::Model))) = "diffusion.paths.model";
    DIFFUSION_VAE_PATH_PMID => (Pmid::Diffusion(Diffusion::Paths(DiffusionPaths::Vae))) = "diffusion.paths.vae";
    DIFFUSION_TAESD_PATH_PMID => (Pmid::Diffusion(Diffusion::Paths(DiffusionPaths::Taesd))) = "diffusion.paths.taesd";
    DIFFUSION_LORA_MODEL_DIR_PMID => (Pmid::Diffusion(Diffusion::Paths(DiffusionPaths::LoraModelDir))) = "diffusion.paths.lora_model_dir";
    DIFFUSION_CLIP_L_PATH_PMID => (Pmid::Diffusion(Diffusion::Paths(DiffusionPaths::ClipL))) = "diffusion.paths.clip_l";
    DIFFUSION_CLIP_G_PATH_PMID => (Pmid::Diffusion(Diffusion::Paths(DiffusionPaths::ClipG))) = "diffusion.paths.clip_g";
    DIFFUSION_T5XXL_PATH_PMID => (Pmid::Diffusion(Diffusion::Paths(DiffusionPaths::T5xxl))) = "diffusion.paths.t5xxl";
    DIFFUSION_FLASH_ATTN_PMID => (Pmid::Diffusion(Diffusion::Performance(DiffusionPerformance::FlashAttn))) = "diffusion.performance.flash_attn";
    DIFFUSION_KEEP_VAE_ON_CPU_PMID => (Pmid::Diffusion(Diffusion::Performance(DiffusionPerformance::KeepVaeOnCpu))) = "diffusion.performance.keep_vae_on_cpu";
    DIFFUSION_KEEP_CLIP_ON_CPU_PMID => (Pmid::Diffusion(Diffusion::Performance(DiffusionPerformance::KeepClipOnCpu))) = "diffusion.performance.keep_clip_on_cpu";
    DIFFUSION_OFFLOAD_PARAMS_TO_CPU_PMID => (Pmid::Diffusion(Diffusion::Performance(DiffusionPerformance::OffloadParamsToCpu))) = "diffusion.performance.offload_params_to_cpu";
}

impl Pmid {
    #[cfg(test)]
    pub fn iter() -> impl Iterator<Item = Pmid> {
        PMID_ENTRIES.iter().map(|(pmid, _)| *pmid)
    }

    pub fn as_str(&self) -> &'static str {
        PMID_ENTRIES
            .iter()
            .find_map(|(pmid, path)| (*pmid == *self).then_some(*path))
            .expect("missing PMID path mapping")
    }

    pub fn to_path(self) -> String {
        self.as_str().to_owned()
    }
}

impl AsRef<str> for Pmid {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for Pmid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for Pmid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Pmid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::from_str(&raw).map_err(serde::de::Error::custom)
    }
}

impl FromStr for Pmid {
    type Err = ParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        PMID_ENTRIES
            .iter()
            .find_map(|(pmid, path)| (*path == raw).then_some(*pmid))
            .ok_or(ParseError::VariantNotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::embedded_settings_schema;

    #[test]
    fn pmid_round_trips_from_str() {
        let parsed = Pmid::from_str(SETUP_INITIALIZED_PMID).expect("parse pmid");

        assert_eq!(parsed, Pmid::Setup(Setup::Initialized));
        assert_eq!(parsed.to_string(), SETUP_INITIALIZED_PMID);
        assert_eq!(parsed.to_path(), SETUP_INITIALIZED_PMID);
    }

    #[test]
    fn nested_variants_produce_expected_paths() {
        let pmid = Pmid::Setup(Setup::Backends(SetupBackends::Llama(BackendAsset::Tag)));

        assert_eq!(pmid.to_path(), SETUP_BACKENDS_LLAMA_TAG_PMID);
    }

    #[test]
    fn embedded_schema_pmids_are_covered_by_enum() {
        let schema = embedded_settings_schema().expect("schema");

        for section in schema.sections() {
            for subsection in &section.subsections {
                for property in &subsection.properties {
                    let parsed = Pmid::from_str(&property.pmid)
                        .unwrap_or_else(|_| panic!("missing PMID enum for '{}'", property.pmid));
                    assert_eq!(parsed.as_str(), property.pmid);
                }
            }
        }
    }

    #[test]
    fn enum_pmids_exist_in_embedded_schema() {
        let schema = embedded_settings_schema().expect("schema");

        for pmid in Pmid::iter() {
            assert!(
                schema.property(pmid.as_str()).is_some(),
                "embedded schema missing '{}'",
                pmid.as_str()
            );
        }
    }
}
