use std::fmt;

/// A dot-separated Property-Management ID that uniquely identifies a setting.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SettingPmid(String);

impl SettingPmid {
    pub fn from_path(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

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

/// The complete settings PMID catalog.
#[derive(Debug, Clone, Copy)]
pub struct SettingsPmidCatalog {
    pub general: GeneralPmids,
    pub database: DatabasePmids,
    pub logging: LoggingPmids,
    pub telemetry: TelemetryPmids,
    pub tools: ToolsPmids,
    pub agent: AgentPmids,
    pub runtime: RuntimePmids,
    pub providers: ProvidersPmids,
    pub models: ModelsPmids,
    pub plugin: PluginPmids,
    pub server: ServerPmids,
}

impl SettingsPmidCatalog {
    pub const fn new() -> Self {
        Self {
            general: GeneralPmids,
            database: DatabasePmids,
            logging: LoggingPmids::new("logging"),
            telemetry: TelemetryPmids,
            tools: ToolsPmids::new(),
            agent: AgentPmids::new(),
            runtime: RuntimePmids::new(),
            providers: ProvidersPmids,
            models: ModelsPmids::new(),
            plugin: PluginPmids,
            server: ServerPmids::new(),
        }
    }

    pub fn all(self) -> Vec<SettingPmid> {
        vec![
            self.general.language(),
            self.database.url(),
            self.logging.level(),
            self.logging.json(),
            self.logging.path(),
            self.telemetry.enabled(),
            self.telemetry.environment(),
            self.telemetry.service_name(),
            self.telemetry.service_version(),
            self.telemetry.metrics_exporter(),
            self.telemetry.capture_content(),
            self.telemetry.span_attributes(),
            self.telemetry.tracestate(),
            self.tools.ffmpeg.enabled(),
            self.tools.ffmpeg.auto_download(),
            self.tools.ffmpeg.install_dir(),
            self.tools.ffmpeg.source.version(),
            self.tools.ffmpeg.source.artifact(),
            self.agent.debug(),
            self.agent.hooks.enabled(),
            self.agent.hooks.scripts(),
            self.agent.memories.enabled(),
            self.agent.memories.model(),
            self.agent.memories.memory_root(),
            self.agent.memories.phase1_scan_limit(),
            self.agent.memories.phase1_concurrency(),
            self.agent.memories.phase1_idle_seconds(),
            self.agent.memories.phase1_lease_seconds(),
            self.agent.memories.phase1_retry_seconds(),
            self.agent.memories.phase1_max_age_days(),
            self.agent.memories.phase2_limit(),
            self.agent.memories.phase2_lease_seconds(),
            self.agent.memories.max_unused_days(),
            self.agent.memories.extension_retention_days(),
            self.agent.tools.mcp.enabled(),
            self.agent.tools.mcp.servers(),
            self.agent.tools.websearch.default_provider(),
            self.agent.tools.websearch.providers(),
            self.runtime.mode(),
            self.runtime.transport(),
            self.runtime.sessions.state_dir(),
            self.runtime.launch.server.bind_host(),
            self.runtime.launch.server.base_port(),
            self.runtime.launch.desktop.bind_host(),
            self.runtime.launch.desktop.base_port(),
            self.runtime.logging.level(),
            self.runtime.logging.json(),
            self.runtime.logging.path(),
            self.runtime.capacity.queue(),
            self.runtime.capacity.concurrent_requests(),
            self.runtime.endpoint.http_address(),
            self.runtime.endpoint.ipc_path(),
            self.runtime.ggml.install_dir(),
            self.runtime.ggml.source.version(),
            self.runtime.ggml.source.artifact(),
            self.runtime.ggml.logging.level(),
            self.runtime.ggml.logging.json(),
            self.runtime.ggml.logging.path(),
            self.runtime.ggml.capacity.queue(),
            self.runtime.ggml.capacity.concurrent_requests(),
            self.runtime.ggml.endpoint.http_address(),
            self.runtime.ggml.endpoint.ipc_path(),
            self.runtime.ggml.backends.llama.enabled(),
            self.runtime.ggml.backends.llama.context_length(),
            self.runtime.ggml.backends.llama.flash_attn(),
            self.runtime.ggml.backends.llama.source.version(),
            self.runtime.ggml.backends.llama.source.artifact(),
            self.runtime.ggml.backends.llama.logging.level(),
            self.runtime.ggml.backends.llama.logging.json(),
            self.runtime.ggml.backends.llama.logging.path(),
            self.runtime.ggml.backends.llama.capacity.queue(),
            self.runtime.ggml.backends.llama.capacity.concurrent_requests(),
            self.runtime.ggml.backends.llama.endpoint.http_address(),
            self.runtime.ggml.backends.llama.endpoint.ipc_path(),
            self.runtime.ggml.backends.whisper.enabled(),
            self.runtime.ggml.backends.whisper.flash_attn(),
            self.runtime.ggml.backends.whisper.source.version(),
            self.runtime.ggml.backends.whisper.source.artifact(),
            self.runtime.ggml.backends.whisper.logging.level(),
            self.runtime.ggml.backends.whisper.logging.json(),
            self.runtime.ggml.backends.whisper.logging.path(),
            self.runtime.ggml.backends.whisper.capacity.queue(),
            self.runtime.ggml.backends.whisper.capacity.concurrent_requests(),
            self.runtime.ggml.backends.whisper.endpoint.http_address(),
            self.runtime.ggml.backends.whisper.endpoint.ipc_path(),
            self.runtime.ggml.backends.diffusion.enabled(),
            self.runtime.ggml.backends.diffusion.flash_attn(),
            self.runtime.ggml.backends.diffusion.source.version(),
            self.runtime.ggml.backends.diffusion.source.artifact(),
            self.runtime.ggml.backends.diffusion.logging.level(),
            self.runtime.ggml.backends.diffusion.logging.json(),
            self.runtime.ggml.backends.diffusion.logging.path(),
            self.runtime.ggml.backends.diffusion.capacity.queue(),
            self.runtime.ggml.backends.diffusion.capacity.concurrent_requests(),
            self.runtime.ggml.backends.diffusion.endpoint.http_address(),
            self.runtime.ggml.backends.diffusion.endpoint.ipc_path(),
            self.runtime.candle.enabled(),
            self.runtime.candle.install_dir(),
            self.runtime.candle.source.version(),
            self.runtime.candle.source.artifact(),
            self.runtime.candle.logging.level(),
            self.runtime.candle.logging.json(),
            self.runtime.candle.logging.path(),
            self.runtime.candle.capacity.queue(),
            self.runtime.candle.capacity.concurrent_requests(),
            self.runtime.candle.endpoint.http_address(),
            self.runtime.candle.endpoint.ipc_path(),
            self.runtime.onnx.enabled(),
            self.runtime.onnx.install_dir(),
            self.runtime.onnx.source.version(),
            self.runtime.onnx.source.artifact(),
            self.runtime.onnx.logging.level(),
            self.runtime.onnx.logging.json(),
            self.runtime.onnx.logging.path(),
            self.runtime.onnx.capacity.queue(),
            self.runtime.onnx.capacity.concurrent_requests(),
            self.runtime.onnx.endpoint.http_address(),
            self.runtime.onnx.endpoint.ipc_path(),
            self.providers.registry(),
            self.models.cache_dir(),
            self.models.config_dir(),
            self.models.download_source(),
            self.models.auto_unload.enabled(),
            self.models.auto_unload.idle_minutes(),
            self.models.auto_unload.min_free_system_memory_bytes(),
            self.models.auto_unload.min_free_gpu_memory_bytes(),
            self.models.auto_unload.max_pressure_evictions_per_load(),
            self.plugin.install_dir(),
            self.plugin.js_runtime_transport(),
            self.plugin.python_runtime_transport(),
            self.server.address(),
            self.server.logging.level(),
            self.server.logging.json(),
            self.server.logging.path(),
            self.server.cors.allowed_origins(),
            self.server.admin.token(),
            self.server.swagger.enabled(),
            self.server.cloud_http_trace(),
        ]
    }
}

impl Default for SettingsPmidCatalog {
    fn default() -> Self {
        Self::new()
    }
}

pub const PMID: SettingsPmidCatalog = SettingsPmidCatalog::new();

#[derive(Debug, Clone, Copy, Default)]
pub struct GeneralPmids;

impl GeneralPmids {
    pub fn language(self) -> SettingPmid {
        SettingPmid::from_path("general.language")
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DatabasePmids;

impl DatabasePmids {
    pub fn url(self) -> SettingPmid {
        SettingPmid::from_path("database.url")
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LoggingPmids {
    prefix: &'static str,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TelemetryPmids;

impl TelemetryPmids {
    pub fn enabled(self) -> SettingPmid {
        SettingPmid::from_path("telemetry.enabled")
    }

    pub fn environment(self) -> SettingPmid {
        SettingPmid::from_path("telemetry.environment")
    }

    pub fn service_name(self) -> SettingPmid {
        SettingPmid::from_path("telemetry.service_name")
    }

    pub fn service_version(self) -> SettingPmid {
        SettingPmid::from_path("telemetry.service_version")
    }

    pub fn metrics_exporter(self) -> SettingPmid {
        SettingPmid::from_path("telemetry.metrics_exporter")
    }

    pub fn capture_content(self) -> SettingPmid {
        SettingPmid::from_path("telemetry.capture_content")
    }

    pub fn span_attributes(self) -> SettingPmid {
        SettingPmid::from_path("telemetry.span_attributes")
    }

    pub fn tracestate(self) -> SettingPmid {
        SettingPmid::from_path("telemetry.tracestate")
    }
}

impl LoggingPmids {
    pub const fn new(prefix: &'static str) -> Self {
        Self { prefix }
    }

    pub fn level(self) -> SettingPmid {
        SettingPmid::from_path(format!("{}.level", self.prefix))
    }

    pub fn json(self) -> SettingPmid {
        SettingPmid::from_path(format!("{}.json", self.prefix))
    }

    pub fn path(self) -> SettingPmid {
        SettingPmid::from_path(format!("{}.path", self.prefix))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CapacityPmids {
    prefix: &'static str,
}

impl CapacityPmids {
    pub const fn new(prefix: &'static str) -> Self {
        Self { prefix }
    }

    pub fn queue(self) -> SettingPmid {
        SettingPmid::from_path(format!("{}.queue", self.prefix))
    }

    pub fn concurrent_requests(self) -> SettingPmid {
        SettingPmid::from_path(format!("{}.concurrent_requests", self.prefix))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EndpointPmids {
    prefix: &'static str,
}

impl EndpointPmids {
    pub const fn new(prefix: &'static str) -> Self {
        Self { prefix }
    }

    pub fn http_address(self) -> SettingPmid {
        SettingPmid::from_path(format!("{}.http.address", self.prefix))
    }

    pub fn ipc_path(self) -> SettingPmid {
        SettingPmid::from_path(format!("{}.ipc.path", self.prefix))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SourcePmids {
    prefix: &'static str,
}

impl SourcePmids {
    pub const fn new(prefix: &'static str) -> Self {
        Self { prefix }
    }

    pub fn version(self) -> SettingPmid {
        SettingPmid::from_path(format!("{}.version", self.prefix))
    }

    pub fn artifact(self) -> SettingPmid {
        SettingPmid::from_path(format!("{}.artifact", self.prefix))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ToolsPmids {
    pub ffmpeg: FfmpegToolPmids,
}

impl ToolsPmids {
    pub const fn new() -> Self {
        Self { ffmpeg: FfmpegToolPmids::new() }
    }
}

impl Default for ToolsPmids {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FfmpegToolPmids {
    pub source: SourcePmids,
}

impl FfmpegToolPmids {
    pub const fn new() -> Self {
        Self { source: SourcePmids::new("tools.ffmpeg.source") }
    }

    pub fn enabled(self) -> SettingPmid {
        SettingPmid::from_path("tools.ffmpeg.enabled")
    }

    pub fn auto_download(self) -> SettingPmid {
        SettingPmid::from_path("tools.ffmpeg.auto_download")
    }

    pub fn install_dir(self) -> SettingPmid {
        SettingPmid::from_path("tools.ffmpeg.install_dir")
    }
}

impl Default for FfmpegToolPmids {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AgentPmids {
    pub tools: AgentToolsPmids,
    pub hooks: AgentHooksPmids,
    pub memories: AgentMemoriesPmids,
}

impl AgentPmids {
    pub const fn new() -> Self {
        Self { tools: AgentToolsPmids::new(), hooks: AgentHooksPmids, memories: AgentMemoriesPmids }
    }

    pub fn debug(self) -> SettingPmid {
        SettingPmid::from_path("agent.debug")
    }
}

impl Default for AgentPmids {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AgentHooksPmids;

impl AgentHooksPmids {
    pub fn enabled(self) -> SettingPmid {
        SettingPmid::from_path("agent.hooks.enabled")
    }

    pub fn scripts(self) -> SettingPmid {
        SettingPmid::from_path("agent.hooks.scripts")
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AgentMemoriesPmids;

impl AgentMemoriesPmids {
    pub fn enabled(self) -> SettingPmid {
        SettingPmid::from_path("agent.memories.enabled")
    }

    pub fn model(self) -> SettingPmid {
        SettingPmid::from_path("agent.memories.model")
    }

    pub fn memory_root(self) -> SettingPmid {
        SettingPmid::from_path("agent.memories.memory_root")
    }

    pub fn phase1_scan_limit(self) -> SettingPmid {
        SettingPmid::from_path("agent.memories.phase1_scan_limit")
    }

    pub fn phase1_concurrency(self) -> SettingPmid {
        SettingPmid::from_path("agent.memories.phase1_concurrency")
    }

    pub fn phase1_idle_seconds(self) -> SettingPmid {
        SettingPmid::from_path("agent.memories.phase1_idle_seconds")
    }

    pub fn phase1_lease_seconds(self) -> SettingPmid {
        SettingPmid::from_path("agent.memories.phase1_lease_seconds")
    }

    pub fn phase1_retry_seconds(self) -> SettingPmid {
        SettingPmid::from_path("agent.memories.phase1_retry_seconds")
    }

    pub fn phase1_max_age_days(self) -> SettingPmid {
        SettingPmid::from_path("agent.memories.phase1_max_age_days")
    }

    pub fn phase2_limit(self) -> SettingPmid {
        SettingPmid::from_path("agent.memories.phase2_limit")
    }

    pub fn phase2_lease_seconds(self) -> SettingPmid {
        SettingPmid::from_path("agent.memories.phase2_lease_seconds")
    }

    pub fn max_unused_days(self) -> SettingPmid {
        SettingPmid::from_path("agent.memories.max_unused_days")
    }

    pub fn extension_retention_days(self) -> SettingPmid {
        SettingPmid::from_path("agent.memories.extension_retention_days")
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AgentToolsPmids {
    pub mcp: AgentMcpPmids,
    pub websearch: AgentWebSearchPmids,
}

impl AgentToolsPmids {
    pub const fn new() -> Self {
        Self { mcp: AgentMcpPmids, websearch: AgentWebSearchPmids }
    }
}

impl Default for AgentToolsPmids {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AgentMcpPmids;

impl AgentMcpPmids {
    pub fn enabled(self) -> SettingPmid {
        SettingPmid::from_path("agent.tools.mcp.enabled")
    }

    pub fn servers(self) -> SettingPmid {
        SettingPmid::from_path("agent.tools.mcp.servers")
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AgentWebSearchPmids;

impl AgentWebSearchPmids {
    pub fn default_provider(self) -> SettingPmid {
        SettingPmid::from_path("agent.tools.websearch.default_provider")
    }

    pub fn providers(self) -> SettingPmid {
        SettingPmid::from_path("agent.tools.websearch.providers")
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimePmids {
    pub sessions: RuntimeSessionsPmids,
    pub launch: RuntimeLaunchPmids,
    pub logging: LoggingPmids,
    pub capacity: CapacityPmids,
    pub endpoint: EndpointPmids,
    pub ggml: GgmlRuntimePmids,
    pub candle: SingleRuntimeFamilyPmids,
    pub onnx: SingleRuntimeFamilyPmids,
}

impl RuntimePmids {
    pub const fn new() -> Self {
        Self {
            sessions: RuntimeSessionsPmids,
            launch: RuntimeLaunchPmids::new(),
            logging: LoggingPmids::new("runtime.logging"),
            capacity: CapacityPmids::new("runtime.capacity"),
            endpoint: EndpointPmids::new("runtime.endpoint"),
            ggml: GgmlRuntimePmids::new(),
            candle: SingleRuntimeFamilyPmids::candle(),
            onnx: SingleRuntimeFamilyPmids::onnx(),
        }
    }

    pub fn mode(self) -> SettingPmid {
        SettingPmid::from_path("runtime.mode")
    }

    pub fn transport(self) -> SettingPmid {
        SettingPmid::from_path("runtime.transport")
    }
}

impl Default for RuntimePmids {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RuntimeSessionsPmids;

impl RuntimeSessionsPmids {
    pub fn state_dir(self) -> SettingPmid {
        SettingPmid::from_path("runtime.sessions.state_dir")
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeLaunchPmids {
    pub server: RuntimeLaunchProfilePmids,
    pub desktop: RuntimeLaunchProfilePmids,
}

impl RuntimeLaunchPmids {
    pub const fn new() -> Self {
        Self {
            server: RuntimeLaunchProfilePmids::new("runtime.launch.server"),
            desktop: RuntimeLaunchProfilePmids::new("runtime.launch.desktop"),
        }
    }
}

impl Default for RuntimeLaunchPmids {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeLaunchProfilePmids {
    prefix: &'static str,
}

impl RuntimeLaunchProfilePmids {
    pub const fn new(prefix: &'static str) -> Self {
        Self { prefix }
    }

    pub fn bind_host(self) -> SettingPmid {
        SettingPmid::from_path(format!("{}.bind_host", self.prefix))
    }

    pub fn base_port(self) -> SettingPmid {
        SettingPmid::from_path(format!("{}.base_port", self.prefix))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GgmlRuntimePmids {
    pub source: SourcePmids,
    pub logging: LoggingPmids,
    pub capacity: CapacityPmids,
    pub endpoint: EndpointPmids,
    pub backends: GgmlBackendPmids,
}

impl GgmlRuntimePmids {
    pub const fn new() -> Self {
        Self {
            source: SourcePmids::new("runtime.ggml.source"),
            logging: LoggingPmids::new("runtime.ggml.logging"),
            capacity: CapacityPmids::new("runtime.ggml.capacity"),
            endpoint: EndpointPmids::new("runtime.ggml.endpoint"),
            backends: GgmlBackendPmids::new(),
        }
    }

    pub fn install_dir(self) -> SettingPmid {
        SettingPmid::from_path("runtime.ggml.install_dir")
    }
}

impl Default for GgmlRuntimePmids {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GgmlBackendPmids {
    pub llama: LlamaRuntimePmids,
    pub whisper: RuntimeBackendLeafPmids,
    pub diffusion: RuntimeBackendLeafPmids,
}

impl GgmlBackendPmids {
    pub const fn new() -> Self {
        Self {
            llama: LlamaRuntimePmids::new(),
            whisper: RuntimeBackendLeafPmids::whisper(),
            diffusion: RuntimeBackendLeafPmids::diffusion(),
        }
    }
}

impl Default for GgmlBackendPmids {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeBackendLeafPmids {
    prefix: &'static str,
    pub source: SourcePmids,
    pub logging: LoggingPmids,
    pub capacity: CapacityPmids,
    pub endpoint: EndpointPmids,
}

impl RuntimeBackendLeafPmids {
    const fn new(
        prefix: &'static str,
        source_prefix: &'static str,
        logging_prefix: &'static str,
        capacity_prefix: &'static str,
        endpoint_prefix: &'static str,
    ) -> Self {
        Self {
            prefix,
            source: SourcePmids::new(source_prefix),
            logging: LoggingPmids::new(logging_prefix),
            capacity: CapacityPmids::new(capacity_prefix),
            endpoint: EndpointPmids::new(endpoint_prefix),
        }
    }

    pub const fn whisper() -> Self {
        Self::new(
            "runtime.ggml.backends.whisper",
            "runtime.ggml.backends.whisper.source",
            "runtime.ggml.backends.whisper.logging",
            "runtime.ggml.backends.whisper.capacity",
            "runtime.ggml.backends.whisper.endpoint",
        )
    }

    pub const fn diffusion() -> Self {
        Self::new(
            "runtime.ggml.backends.diffusion",
            "runtime.ggml.backends.diffusion.source",
            "runtime.ggml.backends.diffusion.logging",
            "runtime.ggml.backends.diffusion.capacity",
            "runtime.ggml.backends.diffusion.endpoint",
        )
    }

    pub fn enabled(self) -> SettingPmid {
        SettingPmid::from_path(format!("{}.enabled", self.prefix))
    }

    pub fn flash_attn(self) -> SettingPmid {
        SettingPmid::from_path(format!("{}.flash_attn", self.prefix))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LlamaRuntimePmids {
    prefix: &'static str,
    pub source: SourcePmids,
    pub logging: LoggingPmids,
    pub capacity: CapacityPmids,
    pub endpoint: EndpointPmids,
}

impl LlamaRuntimePmids {
    pub const fn new() -> Self {
        Self {
            prefix: "runtime.ggml.backends.llama",
            source: SourcePmids::new("runtime.ggml.backends.llama.source"),
            logging: LoggingPmids::new("runtime.ggml.backends.llama.logging"),
            capacity: CapacityPmids::new("runtime.ggml.backends.llama.capacity"),
            endpoint: EndpointPmids::new("runtime.ggml.backends.llama.endpoint"),
        }
    }

    pub fn enabled(self) -> SettingPmid {
        SettingPmid::from_path(format!("{}.enabled", self.prefix))
    }

    pub fn context_length(self) -> SettingPmid {
        SettingPmid::from_path(format!("{}.context_length", self.prefix))
    }

    pub fn flash_attn(self) -> SettingPmid {
        SettingPmid::from_path(format!("{}.flash_attn", self.prefix))
    }
}

impl Default for LlamaRuntimePmids {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SingleRuntimeFamilyPmids {
    prefix: &'static str,
    pub source: SourcePmids,
    pub logging: LoggingPmids,
    pub capacity: CapacityPmids,
    pub endpoint: EndpointPmids,
}

impl SingleRuntimeFamilyPmids {
    const fn new(
        prefix: &'static str,
        source_prefix: &'static str,
        logging_prefix: &'static str,
        capacity_prefix: &'static str,
        endpoint_prefix: &'static str,
    ) -> Self {
        Self {
            prefix,
            source: SourcePmids::new(source_prefix),
            logging: LoggingPmids::new(logging_prefix),
            capacity: CapacityPmids::new(capacity_prefix),
            endpoint: EndpointPmids::new(endpoint_prefix),
        }
    }

    pub const fn candle() -> Self {
        Self::new(
            "runtime.candle",
            "runtime.candle.source",
            "runtime.candle.logging",
            "runtime.candle.capacity",
            "runtime.candle.endpoint",
        )
    }

    pub const fn onnx() -> Self {
        Self::new(
            "runtime.onnx",
            "runtime.onnx.source",
            "runtime.onnx.logging",
            "runtime.onnx.capacity",
            "runtime.onnx.endpoint",
        )
    }

    pub fn enabled(self) -> SettingPmid {
        SettingPmid::from_path(format!("{}.enabled", self.prefix))
    }

    pub fn install_dir(self) -> SettingPmid {
        SettingPmid::from_path(format!("{}.install_dir", self.prefix))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ProvidersPmids;

impl ProvidersPmids {
    pub fn registry(self) -> SettingPmid {
        SettingPmid::from_path("providers.registry")
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ModelsPmids {
    pub auto_unload: AutoUnloadPmids,
}

impl ModelsPmids {
    pub const fn new() -> Self {
        Self { auto_unload: AutoUnloadPmids }
    }

    pub fn cache_dir(self) -> SettingPmid {
        SettingPmid::from_path("models.cache_dir")
    }

    pub fn config_dir(self) -> SettingPmid {
        SettingPmid::from_path("models.config_dir")
    }

    pub fn download_source(self) -> SettingPmid {
        SettingPmid::from_path("models.download_source")
    }
}

impl Default for ModelsPmids {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AutoUnloadPmids;

impl AutoUnloadPmids {
    pub fn enabled(self) -> SettingPmid {
        SettingPmid::from_path("models.auto_unload.enabled")
    }

    pub fn idle_minutes(self) -> SettingPmid {
        SettingPmid::from_path("models.auto_unload.idle_minutes")
    }

    pub fn min_free_system_memory_bytes(self) -> SettingPmid {
        SettingPmid::from_path("models.auto_unload.min_free_system_memory_bytes")
    }

    pub fn min_free_gpu_memory_bytes(self) -> SettingPmid {
        SettingPmid::from_path("models.auto_unload.min_free_gpu_memory_bytes")
    }

    pub fn max_pressure_evictions_per_load(self) -> SettingPmid {
        SettingPmid::from_path("models.auto_unload.max_pressure_evictions_per_load")
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PluginPmids;

impl PluginPmids {
    pub fn install_dir(self) -> SettingPmid {
        SettingPmid::from_path("plugin.install_dir")
    }

    pub fn js_runtime_transport(self) -> SettingPmid {
        SettingPmid::from_path("plugin.js_runtime_transport")
    }

    pub fn python_runtime_transport(self) -> SettingPmid {
        SettingPmid::from_path("plugin.python_runtime_transport")
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ServerPmids {
    pub logging: LoggingPmids,
    pub cors: CorsPmids,
    pub admin: AdminPmids,
    pub swagger: SwaggerPmids,
}

impl ServerPmids {
    pub const fn new() -> Self {
        Self {
            logging: LoggingPmids::new("server.logging"),
            cors: CorsPmids,
            admin: AdminPmids,
            swagger: SwaggerPmids,
        }
    }

    pub fn address(self) -> SettingPmid {
        SettingPmid::from_path("server.address")
    }

    pub fn cloud_http_trace(self) -> SettingPmid {
        SettingPmid::from_path("server.cloud_http_trace")
    }
}

impl Default for ServerPmids {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CorsPmids;

impl CorsPmids {
    pub fn allowed_origins(self) -> SettingPmid {
        SettingPmid::from_path("server.cors.allowed_origins")
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AdminPmids;

impl AdminPmids {
    pub fn token(self) -> SettingPmid {
        SettingPmid::from_path("server.admin.token")
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SwaggerPmids;

impl SwaggerPmids {
    pub fn enabled(self) -> SettingPmid {
        SettingPmid::from_path("server.swagger.enabled")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::PMID;

    #[test]
    fn settings_pmids_are_unique() {
        let all = PMID.all();
        let unique: BTreeSet<String> = all.iter().map(|pmid| pmid.as_str().to_owned()).collect();

        assert_eq!(all.len(), unique.len());
        assert!(unique.contains("general.language"));
        assert!(unique.contains("runtime.ggml.backends.llama.context_length"));
        assert!(unique.contains("runtime.launch.server.bind_host"));
        assert!(unique.contains("runtime.launch.server.base_port"));
        assert!(unique.contains("runtime.launch.desktop.bind_host"));
        assert!(unique.contains("runtime.launch.desktop.base_port"));
        assert!(unique.contains("runtime.ggml.backends.llama.flash_attn"));
        assert!(unique.contains("runtime.ggml.backends.whisper.flash_attn"));
        assert!(unique.contains("runtime.ggml.backends.diffusion.flash_attn"));
        assert!(unique.contains("agent.hooks.enabled"));
        assert!(unique.contains("agent.hooks.scripts"));
        assert!(unique.contains("agent.memories.enabled"));
        assert!(unique.contains("agent.memories.memory_root"));
        assert!(unique.contains("agent.memories.phase1_scan_limit"));
        assert!(unique.contains("agent.memories.phase2_limit"));
        assert!(unique.contains("agent.tools.mcp.enabled"));
        assert!(unique.contains("agent.tools.mcp.servers"));
        assert!(unique.contains("agent.tools.websearch.default_provider"));
        assert!(unique.contains("agent.tools.websearch.providers"));
        assert!(unique.contains("providers.registry"));
        assert!(unique.contains("telemetry.enabled"));
        assert!(unique.contains("telemetry.capture_content"));
        assert!(unique.contains("server.cloud_http_trace"));
    }
}
