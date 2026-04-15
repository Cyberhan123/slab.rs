use super::pmid::SettingPmid;

/// The complete V2 settings PMID catalog.
#[derive(Debug, Clone, Copy)]
pub struct SettingsV2PmidCatalog {
    pub general: GeneralPmids,
    pub database: DatabasePmids,
    pub logging: LoggingPmids,
    pub tools: ToolsPmids,
    pub runtime: RuntimeV2Pmids,
    pub providers: ProvidersPmids,
    pub models: ModelsPmids,
    pub server: ServerPmids,
}

impl SettingsV2PmidCatalog {
    pub const fn new() -> Self {
        Self {
            general: GeneralPmids,
            database: DatabasePmids,
            logging: LoggingPmids::new("logging"),
            tools: ToolsPmids::new(),
            runtime: RuntimeV2Pmids::new(),
            providers: ProvidersPmids,
            models: ModelsPmids::new(),
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
            self.tools.ffmpeg.enabled(),
            self.tools.ffmpeg.auto_download(),
            self.tools.ffmpeg.install_dir(),
            self.tools.ffmpeg.source.version(),
            self.tools.ffmpeg.source.artifact(),
            self.runtime.mode(),
            self.runtime.transport(),
            self.runtime.sessions.state_dir(),
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

impl Default for SettingsV2PmidCatalog {
    fn default() -> Self {
        Self::new()
    }
}

pub const V2_PMID: SettingsV2PmidCatalog = SettingsV2PmidCatalog::new();

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

#[derive(Debug, Clone, Copy)]
pub struct RuntimeV2Pmids {
    pub sessions: RuntimeSessionsPmids,
    pub logging: LoggingPmids,
    pub capacity: CapacityPmids,
    pub endpoint: EndpointPmids,
    pub ggml: GgmlRuntimePmids,
    pub candle: SingleRuntimeFamilyPmids,
    pub onnx: SingleRuntimeFamilyPmids,
}

impl RuntimeV2Pmids {
    pub const fn new() -> Self {
        Self {
            sessions: RuntimeSessionsPmids,
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

impl Default for RuntimeV2Pmids {
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

    use super::V2_PMID;

    #[test]
    fn v2_pmids_are_unique() {
        let all = V2_PMID.all();
        let unique: BTreeSet<String> = all.iter().map(|pmid| pmid.as_str().to_owned()).collect();

        assert_eq!(all.len(), unique.len());
        assert!(unique.contains("general.language"));
        assert!(unique.contains("runtime.ggml.backends.llama.context_length"));
        assert!(unique.contains("providers.registry"));
        assert!(unique.contains("server.cloud_http_trace"));
    }
}
