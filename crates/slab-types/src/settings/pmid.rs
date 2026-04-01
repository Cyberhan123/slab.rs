use std::fmt;

/// A dot-separated Property-Management ID that uniquely identifies a setting.
///
/// PMIDs are composed of dot-separated segments (variable depth), such as
/// `"section.subsection.key"`, and serve as stable, machine-readable keys for
/// the settings system.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SettingPmid(String);

impl SettingPmid {
    /// Build a [`SettingPmid`] by joining N static segments with `.`.
    ///
    /// # Note
    /// TODO: This performs a heap allocation on every call because `segments.join(".")` is not
    /// a `const` operation. Since all PMIDs are compile-time known values, a future optimization
    /// could store `&'static str` directly (e.g., via a `const fn from_static(s: &'static str)`
    /// constructor or a proc-macro) to eliminate the per-call allocation.
    pub fn from_segments<const N: usize>(segments: [&'static str; N]) -> Self {
        Self(segments.join("."))
    }

    /// Return the PMID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume self and return the owned string.
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

// ── Top-level catalog ────────────────────────────────────────────────────────

/// The complete catalog of all known setting PMIDs.
#[derive(Debug, Clone, Copy)]
pub struct PmidCatalog {
    pub setup: SetupPmids,
    pub runtime: RuntimePmids,
    pub launch: LaunchPmids,
    pub chat: ChatPmids,
    pub diffusion: DiffusionPmids,
}

impl PmidCatalog {
    pub const fn new() -> Self {
        Self {
            setup: SetupPmids::new(),
            runtime: RuntimePmids::new(),
            launch: LaunchPmids::new(),
            chat: ChatPmids::new(),
            diffusion: DiffusionPmids::new(),
        }
    }

    /// Return every known [`SettingPmid`] in declaration order.
    pub fn all(self) -> Vec<SettingPmid> {
        vec![
            self.setup.initialized(),
            self.setup.ffmpeg.auto_download(),
            self.setup.ffmpeg.dir(),
            self.setup.backends.dir(),
            self.setup.backends.ggml_llama.tag(),
            self.setup.backends.ggml_llama.asset(),
            self.setup.backends.ggml_whisper.tag(),
            self.setup.backends.ggml_whisper.asset(),
            self.setup.backends.ggml_diffusion.tag(),
            self.setup.backends.ggml_diffusion.asset(),
            self.setup.backends.candle_llama.tag(),
            self.setup.backends.candle_llama.asset(),
            self.setup.backends.candle_whisper.tag(),
            self.setup.backends.candle_whisper.asset(),
            self.setup.backends.candle_diffusion.tag(),
            self.setup.backends.candle_diffusion.asset(),
            self.setup.backends.onnx.tag(),
            self.setup.backends.onnx.asset(),
            self.runtime.model_cache_dir(),
            self.runtime.llama.num_workers(),
            self.runtime.llama.context_length(),
            self.runtime.whisper.num_workers(),
            self.runtime.diffusion.num_workers(),
            self.runtime.model_auto_unload.enabled(),
            self.runtime.model_auto_unload.idle_minutes(),
            self.launch.transport(),
            self.launch.queue_capacity(),
            self.launch.backend_capacity(),
            self.launch.runtime_ipc_dir(),
            self.launch.runtime_log_dir(),
            self.launch.backends.llama.enabled(),
            self.launch.backends.whisper.enabled(),
            self.launch.backends.diffusion.enabled(),
            self.launch.profiles.server.gateway_bind(),
            self.launch.profiles.server.runtime_bind_host(),
            self.launch.profiles.server.runtime_bind_base_port(),
            self.launch.profiles.desktop.runtime_bind_host(),
            self.launch.profiles.desktop.runtime_bind_base_port(),
            self.chat.providers(),
            self.diffusion.paths.model(),
            self.diffusion.paths.vae(),
            self.diffusion.paths.taesd(),
            self.diffusion.paths.lora_model_dir(),
            self.diffusion.paths.clip_l(),
            self.diffusion.paths.clip_g(),
            self.diffusion.paths.t5xxl(),
            self.diffusion.performance.flash_attn(),
            self.diffusion.performance.vae_device(),
            self.diffusion.performance.clip_device(),
            self.diffusion.performance.offload_params_to_cpu(),
        ]
    }
}

impl Default for PmidCatalog {
    fn default() -> Self {
        Self::new()
    }
}

/// The global PMID catalog singleton.
pub const PMID: PmidCatalog = PmidCatalog::new();

// ── Setup PMIDs ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct SetupPmids {
    pub ffmpeg: SetupFfmpegPmids,
    pub backends: SetupBackendPmids,
}

impl SetupPmids {
    pub const fn new() -> Self {
        Self { ffmpeg: SetupFfmpegPmids::new(), backends: SetupBackendPmids::new() }
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
    pub ggml_llama: SetupBackendReleasePmids,
    pub ggml_whisper: SetupBackendReleasePmids,
    pub ggml_diffusion: SetupBackendReleasePmids,
    pub candle_llama: SetupBackendReleasePmids,
    pub candle_whisper: SetupBackendReleasePmids,
    pub candle_diffusion: SetupBackendReleasePmids,
    pub onnx: SetupBackendReleasePmids,
}

impl SetupBackendPmids {
    pub const fn new() -> Self {
        Self {
            ggml_llama: SetupBackendReleasePmids::new("ggml.llama"),
            ggml_whisper: SetupBackendReleasePmids::new("ggml.whisper"),
            ggml_diffusion: SetupBackendReleasePmids::new("ggml.diffusion"),
            candle_llama: SetupBackendReleasePmids::new("candle.llama"),
            candle_whisper: SetupBackendReleasePmids::new("candle.whisper"),
            candle_diffusion: SetupBackendReleasePmids::new("candle.diffusion"),
            onnx: SetupBackendReleasePmids::new("onnx"),
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

// ── Runtime PMIDs ────────────────────────────────────────────────────────────

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

// ── Chat PMIDs ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct LaunchPmids {
    pub backends: LaunchBackendPmids,
    pub profiles: LaunchProfilePmids,
}

impl LaunchPmids {
    pub const fn new() -> Self {
        Self { backends: LaunchBackendPmids::new(), profiles: LaunchProfilePmids::new() }
    }

    pub fn transport(self) -> SettingPmid {
        SettingPmid::from_segments(["launch", "transport"])
    }

    pub fn queue_capacity(self) -> SettingPmid {
        SettingPmid::from_segments(["launch", "queue_capacity"])
    }

    pub fn backend_capacity(self) -> SettingPmid {
        SettingPmid::from_segments(["launch", "backend_capacity"])
    }

    pub fn runtime_ipc_dir(self) -> SettingPmid {
        SettingPmid::from_segments(["launch", "runtime_ipc_dir"])
    }

    pub fn runtime_log_dir(self) -> SettingPmid {
        SettingPmid::from_segments(["launch", "runtime_log_dir"])
    }
}

impl Default for LaunchPmids {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LaunchBackendPmids {
    pub llama: LaunchBackendTogglePmids,
    pub whisper: LaunchBackendTogglePmids,
    pub diffusion: LaunchBackendTogglePmids,
}

impl LaunchBackendPmids {
    pub const fn new() -> Self {
        Self {
            llama: LaunchBackendTogglePmids::new("llama"),
            whisper: LaunchBackendTogglePmids::new("whisper"),
            diffusion: LaunchBackendTogglePmids::new("diffusion"),
        }
    }
}

impl Default for LaunchBackendPmids {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LaunchBackendTogglePmids {
    backend: &'static str,
}

impl LaunchBackendTogglePmids {
    pub const fn new(backend: &'static str) -> Self {
        Self { backend }
    }

    pub fn enabled(self) -> SettingPmid {
        SettingPmid::from_segments(["launch", "backends", self.backend, "enabled"])
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LaunchProfilePmids {
    pub server: ServerLaunchProfilePmids,
    pub desktop: DesktopLaunchProfilePmids,
}

impl LaunchProfilePmids {
    pub const fn new() -> Self {
        Self { server: ServerLaunchProfilePmids::new(), desktop: DesktopLaunchProfilePmids::new() }
    }
}

impl Default for LaunchProfilePmids {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ServerLaunchProfilePmids;

impl ServerLaunchProfilePmids {
    pub const fn new() -> Self {
        Self
    }

    pub fn gateway_bind(self) -> SettingPmid {
        SettingPmid::from_segments(["launch", "profiles", "server", "gateway_bind"])
    }

    pub fn runtime_bind_host(self) -> SettingPmid {
        SettingPmid::from_segments(["launch", "profiles", "server", "runtime_bind_host"])
    }

    pub fn runtime_bind_base_port(self) -> SettingPmid {
        SettingPmid::from_segments(["launch", "profiles", "server", "runtime_bind_base_port"])
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DesktopLaunchProfilePmids;

impl DesktopLaunchProfilePmids {
    pub const fn new() -> Self {
        Self
    }

    pub fn runtime_bind_host(self) -> SettingPmid {
        SettingPmid::from_segments(["launch", "profiles", "desktop", "runtime_bind_host"])
    }

    pub fn runtime_bind_base_port(self) -> SettingPmid {
        SettingPmid::from_segments(["launch", "profiles", "desktop", "runtime_bind_base_port"])
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

// ── Diffusion PMIDs ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct DiffusionPmids {
    pub paths: DiffusionPathPmids,
    pub performance: DiffusionPerformancePmids,
}

impl DiffusionPmids {
    pub const fn new() -> Self {
        Self { paths: DiffusionPathPmids::new(), performance: DiffusionPerformancePmids::new() }
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

    pub fn vae_device(self) -> SettingPmid {
        SettingPmid::from_segments(["diffusion", "performance", "vae_device"])
    }

    pub fn clip_device(self) -> SettingPmid {
        SettingPmid::from_segments(["diffusion", "performance", "clip_device"])
    }

    pub fn offload_params_to_cpu(self) -> SettingPmid {
        SettingPmid::from_segments(["diffusion", "performance", "offload_params_to_cpu"])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_builder_generates_expected_pmid() {
        assert_eq!(PMID.setup.backends.ggml_llama.tag().as_str(), "setup.backends.ggml.llama.tag");
        assert_eq!(
            PMID.runtime.model_auto_unload.idle_minutes().as_str(),
            "runtime.model_auto_unload.idle_minutes"
        );
        assert_eq!(
            PMID.launch.profiles.desktop.runtime_bind_base_port().as_str(),
            "launch.profiles.desktop.runtime_bind_base_port"
        );
    }

    #[test]
    fn all_pmids_are_unique() {
        use std::collections::HashSet;
        let all = PMID.all();
        let count = all.len();
        let unique: HashSet<_> = all.iter().map(|p| p.as_str()).collect();
        assert_eq!(unique.len(), count, "duplicate PMIDs detected");
    }
}
