use std::fmt;

#[cfg(test)]
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SettingPmid(String);

impl SettingPmid {
    pub fn from_segments<const N: usize>(segments: [&'static str; N]) -> Self {
        Self(segments.join("."))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[cfg(test)]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl AsRef<str> for SettingPmid {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for SettingPmid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PmidCatalog {
    pub setup: SetupPmids,
    pub runtime: RuntimePmids,
    pub chat: ChatPmids,
    pub diffusion: DiffusionPmids,
}

impl PmidCatalog {
    pub const fn new() -> Self {
        Self {
            setup: SetupPmids::new(),
            runtime: RuntimePmids::new(),
            chat: ChatPmids::new(),
            diffusion: DiffusionPmids::new(),
        }
    }

    #[cfg(test)]
    pub fn all(self) -> Vec<SettingPmid> {
        vec![
            self.setup.initialized(),
            self.setup.ffmpeg.auto_download(),
            self.setup.ffmpeg.dir(),
            self.setup.backends.dir(),
            self.setup.backends.llama.tag(),
            self.setup.backends.llama.asset(),
            self.setup.backends.whisper.tag(),
            self.setup.backends.whisper.asset(),
            self.setup.backends.diffusion.tag(),
            self.setup.backends.diffusion.asset(),
            self.runtime.model_cache_dir(),
            self.runtime.llama.num_workers(),
            self.runtime.llama.context_length(),
            self.runtime.whisper.num_workers(),
            self.runtime.diffusion.num_workers(),
            self.runtime.model_auto_unload.enabled(),
            self.runtime.model_auto_unload.idle_minutes(),
            self.chat.providers(),
            self.diffusion.paths.model(),
            self.diffusion.paths.vae(),
            self.diffusion.paths.taesd(),
            self.diffusion.paths.lora_model_dir(),
            self.diffusion.paths.clip_l(),
            self.diffusion.paths.clip_g(),
            self.diffusion.paths.t5xxl(),
            self.diffusion.performance.flash_attn(),
            self.diffusion.performance.keep_vae_on_cpu(),
            self.diffusion.performance.keep_clip_on_cpu(),
            self.diffusion.performance.offload_params_to_cpu(),
        ]
    }
}

impl Default for PmidCatalog {
    fn default() -> Self {
        Self::new()
    }
}

pub const PMID: PmidCatalog = PmidCatalog::new();

#[derive(Debug, Clone, Copy)]
pub struct SetupPmids {
    pub ffmpeg: SetupFfmpegPmids,
    pub backends: SetupBackendPmids,
}

impl SetupPmids {
    pub const fn new() -> Self {
        Self {
            ffmpeg: SetupFfmpegPmids::new(),
            backends: SetupBackendPmids::new(),
        }
    }

    pub fn initialized(self) -> SettingPmid {
        SettingPmid::from_segments(["setup", "initialized"])
    }
}

impl Default for SetupPmids {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SetupFfmpegPmids;

impl SetupFfmpegPmids {
    pub const fn new() -> Self {
        Self
    }

    pub fn auto_download(self) -> SettingPmid {
        SettingPmid::from_segments(["setup", "ffmpeg", "auto_download"])
    }

    pub fn dir(self) -> SettingPmid {
        SettingPmid::from_segments(["setup", "ffmpeg", "dir"])
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SetupBackendPmids {
    pub llama: SetupBackendReleasePmids,
    pub whisper: SetupBackendReleasePmids,
    pub diffusion: SetupBackendReleasePmids,
}

impl SetupBackendPmids {
    pub const fn new() -> Self {
        Self {
            llama: SetupBackendReleasePmids::new("llama"),
            whisper: SetupBackendReleasePmids::new("whisper"),
            diffusion: SetupBackendReleasePmids::new("diffusion"),
        }
    }

    pub fn dir(self) -> SettingPmid {
        SettingPmid::from_segments(["setup", "backends", "dir"])
    }
}

impl Default for SetupBackendPmids {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SetupBackendReleasePmids {
    backend: &'static str,
}

impl SetupBackendReleasePmids {
    pub const fn new(backend: &'static str) -> Self {
        Self { backend }
    }

    pub fn tag(self) -> SettingPmid {
        SettingPmid::from_segments(["setup", "backends", self.backend, "tag"])
    }

    pub fn asset(self) -> SettingPmid {
        SettingPmid::from_segments(["setup", "backends", self.backend, "asset"])
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimePmids {
    pub llama: RuntimeLlamaPmids,
    pub whisper: RuntimeWorkerPmids,
    pub diffusion: RuntimeWorkerPmids,
    pub model_auto_unload: RuntimeModelAutoUnloadPmids,
}

impl RuntimePmids {
    pub const fn new() -> Self {
        Self {
            llama: RuntimeLlamaPmids::new(),
            whisper: RuntimeWorkerPmids::new("whisper"),
            diffusion: RuntimeWorkerPmids::new("diffusion"),
            model_auto_unload: RuntimeModelAutoUnloadPmids::new(),
        }
    }

    pub fn model_cache_dir(self) -> SettingPmid {
        SettingPmid::from_segments(["runtime", "model_cache_dir"])
    }
}

impl Default for RuntimePmids {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RuntimeLlamaPmids;

impl RuntimeLlamaPmids {
    pub const fn new() -> Self {
        Self
    }

    pub fn num_workers(self) -> SettingPmid {
        SettingPmid::from_segments(["runtime", "llama", "num_workers"])
    }

    pub fn context_length(self) -> SettingPmid {
        SettingPmid::from_segments(["runtime", "llama", "context_length"])
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeWorkerPmids {
    backend: &'static str,
}

impl RuntimeWorkerPmids {
    pub const fn new(backend: &'static str) -> Self {
        Self { backend }
    }

    pub fn num_workers(self) -> SettingPmid {
        SettingPmid::from_segments(["runtime", self.backend, "num_workers"])
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RuntimeModelAutoUnloadPmids;

impl RuntimeModelAutoUnloadPmids {
    pub const fn new() -> Self {
        Self
    }

    pub fn enabled(self) -> SettingPmid {
        SettingPmid::from_segments(["runtime", "model_auto_unload", "enabled"])
    }

    pub fn idle_minutes(self) -> SettingPmid {
        SettingPmid::from_segments(["runtime", "model_auto_unload", "idle_minutes"])
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ChatPmids;

impl ChatPmids {
    pub const fn new() -> Self {
        Self
    }

    pub fn providers(self) -> SettingPmid {
        SettingPmid::from_segments(["chat", "providers"])
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DiffusionPmids {
    pub paths: DiffusionPathPmids,
    pub performance: DiffusionPerformancePmids,
}

impl DiffusionPmids {
    pub const fn new() -> Self {
        Self {
            paths: DiffusionPathPmids::new(),
            performance: DiffusionPerformancePmids::new(),
        }
    }
}

impl Default for DiffusionPmids {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DiffusionPathPmids;

impl DiffusionPathPmids {
    pub const fn new() -> Self {
        Self
    }

    pub fn model(self) -> SettingPmid {
        SettingPmid::from_segments(["diffusion", "paths", "model"])
    }

    pub fn vae(self) -> SettingPmid {
        SettingPmid::from_segments(["diffusion", "paths", "vae"])
    }

    pub fn taesd(self) -> SettingPmid {
        SettingPmid::from_segments(["diffusion", "paths", "taesd"])
    }

    pub fn lora_model_dir(self) -> SettingPmid {
        SettingPmid::from_segments(["diffusion", "paths", "lora_model_dir"])
    }

    pub fn clip_l(self) -> SettingPmid {
        SettingPmid::from_segments(["diffusion", "paths", "clip_l"])
    }

    pub fn clip_g(self) -> SettingPmid {
        SettingPmid::from_segments(["diffusion", "paths", "clip_g"])
    }

    pub fn t5xxl(self) -> SettingPmid {
        SettingPmid::from_segments(["diffusion", "paths", "t5xxl"])
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DiffusionPerformancePmids;

impl DiffusionPerformancePmids {
    pub const fn new() -> Self {
        Self
    }

    pub fn flash_attn(self) -> SettingPmid {
        SettingPmid::from_segments(["diffusion", "performance", "flash_attn"])
    }

    pub fn keep_vae_on_cpu(self) -> SettingPmid {
        SettingPmid::from_segments(["diffusion", "performance", "keep_vae_on_cpu"])
    }

    pub fn keep_clip_on_cpu(self) -> SettingPmid {
        SettingPmid::from_segments(["diffusion", "performance", "keep_clip_on_cpu"])
    }

    pub fn offload_params_to_cpu(self) -> SettingPmid {
        SettingPmid::from_segments(["diffusion", "performance", "offload_params_to_cpu"])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::embedded_settings_schema;

    #[test]
    fn nested_builder_generates_expected_pmid() {
        assert_eq!(
            PMID.setup.backends.llama.tag().as_str(),
            "setup.backends.llama.tag"
        );
        assert_eq!(
            PMID.runtime.model_auto_unload.idle_minutes().as_str(),
            "runtime.model_auto_unload.idle_minutes"
        );
    }

    #[test]
    fn structured_pmids_cover_embedded_schema() {
        let schema = embedded_settings_schema().expect("schema");
        let expected: BTreeSet<String> = schema
            .sections()
            .iter()
            .flat_map(|section| section.subsections.iter())
            .flat_map(|subsection| subsection.properties.iter())
            .map(|property| property.pmid.clone())
            .collect();
        let actual: BTreeSet<String> = PMID
            .all()
            .into_iter()
            .map(SettingPmid::into_string)
            .collect();

        assert_eq!(actual, expected);
    }
}
