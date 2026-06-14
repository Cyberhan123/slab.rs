use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{ConfigError, SettingValue, SettingsDocument};

pub(crate) struct SettingDescriptor {
    pub pmid: &'static str,
    pub get: fn(&SettingsDocument) -> SettingValue,
    pub set: fn(&mut SettingsDocument, SettingValue) -> Result<(), ConfigError>,
}

macro_rules! descriptor {
    ($pmid:literal, $($field:ident).+) => {
        SettingDescriptor {
            pmid: $pmid,
            get: |document| setting_value_from(&document.$($field).+),
            set: |document, value| set_setting_value(&mut document.$($field).+, value, $pmid),
        }
    };
}

pub(crate) fn setting_descriptor(pmid: &str) -> Option<SettingDescriptor> {
    Some(match pmid {
        "general.language" => descriptor!("general.language", general.language),
        "database.url" => descriptor!("database.url", database.url),
        "logging.level" => descriptor!("logging.level", logging.level),
        "logging.json" => descriptor!("logging.json", logging.json),
        "logging.path" => descriptor!("logging.path", logging.path),
        "telemetry.enabled" => descriptor!("telemetry.enabled", telemetry.enabled),
        "telemetry.environment" => descriptor!("telemetry.environment", telemetry.environment),
        "telemetry.service_name" => descriptor!("telemetry.service_name", telemetry.service_name),
        "telemetry.service_version" => {
            descriptor!("telemetry.service_version", telemetry.service_version)
        }
        "telemetry.metrics_exporter" => {
            descriptor!("telemetry.metrics_exporter", telemetry.metrics_exporter)
        }
        "telemetry.capture_content" => {
            descriptor!("telemetry.capture_content", telemetry.capture_content)
        }
        "telemetry.span_attributes" => {
            descriptor!("telemetry.span_attributes", telemetry.span_attributes)
        }
        "telemetry.tracestate" => descriptor!("telemetry.tracestate", telemetry.tracestate),
        "tools.ffmpeg.enabled" => descriptor!("tools.ffmpeg.enabled", tools.ffmpeg.enabled),
        "tools.ffmpeg.auto_download" => {
            descriptor!("tools.ffmpeg.auto_download", tools.ffmpeg.auto_download)
        }
        "tools.ffmpeg.install_dir" => {
            descriptor!("tools.ffmpeg.install_dir", tools.ffmpeg.install_dir)
        }
        "tools.ffmpeg.source.version" => {
            descriptor!("tools.ffmpeg.source.version", tools.ffmpeg.source.version)
        }
        "tools.ffmpeg.source.artifact" => {
            descriptor!("tools.ffmpeg.source.artifact", tools.ffmpeg.source.artifact)
        }
        "agent.debug" => descriptor!("agent.debug", agent.debug),
        "agent.hooks.enabled" => descriptor!("agent.hooks.enabled", agent.hooks.enabled),
        "agent.hooks.scripts" => descriptor!("agent.hooks.scripts", agent.hooks.scripts),
        "agent.memories.enabled" => {
            descriptor!("agent.memories.enabled", agent.memories.enabled)
        }
        "agent.memories.model" => descriptor!("agent.memories.model", agent.memories.model),
        "agent.memories.memory_root" => {
            descriptor!("agent.memories.memory_root", agent.memories.memory_root)
        }
        "agent.memories.phase1_scan_limit" => {
            descriptor!("agent.memories.phase1_scan_limit", agent.memories.phase1_scan_limit)
        }
        "agent.memories.phase1_concurrency" => {
            descriptor!("agent.memories.phase1_concurrency", agent.memories.phase1_concurrency)
        }
        "agent.memories.phase1_idle_seconds" => {
            descriptor!("agent.memories.phase1_idle_seconds", agent.memories.phase1_idle_seconds)
        }
        "agent.memories.phase1_lease_seconds" => {
            descriptor!("agent.memories.phase1_lease_seconds", agent.memories.phase1_lease_seconds)
        }
        "agent.memories.phase1_retry_seconds" => {
            descriptor!("agent.memories.phase1_retry_seconds", agent.memories.phase1_retry_seconds)
        }
        "agent.memories.phase1_max_age_days" => {
            descriptor!("agent.memories.phase1_max_age_days", agent.memories.phase1_max_age_days)
        }
        "agent.memories.phase2_limit" => {
            descriptor!("agent.memories.phase2_limit", agent.memories.phase2_limit)
        }
        "agent.memories.phase2_lease_seconds" => {
            descriptor!("agent.memories.phase2_lease_seconds", agent.memories.phase2_lease_seconds)
        }
        "agent.memories.max_unused_days" => {
            descriptor!("agent.memories.max_unused_days", agent.memories.max_unused_days)
        }
        "agent.memories.extension_retention_days" => descriptor!(
            "agent.memories.extension_retention_days",
            agent.memories.extension_retention_days
        ),
        "agent.tools.mcp.enabled" => {
            descriptor!("agent.tools.mcp.enabled", agent.tools.mcp.enabled)
        }
        "agent.tools.mcp.servers" => {
            descriptor!("agent.tools.mcp.servers", agent.tools.mcp.servers)
        }
        "agent.tools.websearch.default_provider" => descriptor!(
            "agent.tools.websearch.default_provider",
            agent.tools.websearch.default_provider
        ),
        "agent.tools.websearch.providers" => {
            descriptor!("agent.tools.websearch.providers", agent.tools.websearch.providers)
        }
        "runtime.mode" => descriptor!("runtime.mode", runtime.mode),
        "runtime.transport" => descriptor!("runtime.transport", runtime.transport),
        "runtime.sessions.state_dir" => {
            descriptor!("runtime.sessions.state_dir", runtime.sessions.state_dir)
        }
        "runtime.logging.level" => descriptor!("runtime.logging.level", runtime.logging.level),
        "runtime.logging.json" => descriptor!("runtime.logging.json", runtime.logging.json),
        "runtime.logging.path" => descriptor!("runtime.logging.path", runtime.logging.path),
        "runtime.capacity.queue" => {
            descriptor!("runtime.capacity.queue", runtime.capacity.queue)
        }
        "runtime.capacity.concurrent_requests" => descriptor!(
            "runtime.capacity.concurrent_requests",
            runtime.capacity.concurrent_requests
        ),
        "runtime.endpoint.http.address" => {
            descriptor!("runtime.endpoint.http.address", runtime.endpoint.http.address)
        }
        "runtime.endpoint.ipc.path" => {
            descriptor!("runtime.endpoint.ipc.path", runtime.endpoint.ipc.path)
        }
        "runtime.ggml.install_dir" => {
            descriptor!("runtime.ggml.install_dir", runtime.ggml.install_dir)
        }
        "runtime.ggml.source.version" => {
            descriptor!("runtime.ggml.source.version", runtime.ggml.source.version)
        }
        "runtime.ggml.source.artifact" => {
            descriptor!("runtime.ggml.source.artifact", runtime.ggml.source.artifact)
        }
        "runtime.ggml.logging.level" => {
            descriptor!("runtime.ggml.logging.level", runtime.ggml.logging.level)
        }
        "runtime.ggml.logging.json" => {
            descriptor!("runtime.ggml.logging.json", runtime.ggml.logging.json)
        }
        "runtime.ggml.logging.path" => {
            descriptor!("runtime.ggml.logging.path", runtime.ggml.logging.path)
        }
        "runtime.ggml.capacity.queue" => {
            descriptor!("runtime.ggml.capacity.queue", runtime.ggml.capacity.queue)
        }
        "runtime.ggml.capacity.concurrent_requests" => descriptor!(
            "runtime.ggml.capacity.concurrent_requests",
            runtime.ggml.capacity.concurrent_requests
        ),
        "runtime.ggml.endpoint.http.address" => {
            descriptor!("runtime.ggml.endpoint.http.address", runtime.ggml.endpoint.http.address)
        }
        "runtime.ggml.endpoint.ipc.path" => {
            descriptor!("runtime.ggml.endpoint.ipc.path", runtime.ggml.endpoint.ipc.path)
        }
        "runtime.ggml.backends.llama.enabled" => {
            descriptor!("runtime.ggml.backends.llama.enabled", runtime.ggml.backends.llama.enabled)
        }
        "runtime.ggml.backends.llama.context_length" => descriptor!(
            "runtime.ggml.backends.llama.context_length",
            runtime.ggml.backends.llama.context_length
        ),
        "runtime.ggml.backends.llama.flash_attn" => descriptor!(
            "runtime.ggml.backends.llama.flash_attn",
            runtime.ggml.backends.llama.flash_attn
        ),
        "runtime.ggml.backends.llama.source.version" => descriptor!(
            "runtime.ggml.backends.llama.source.version",
            runtime.ggml.backends.llama.source.version
        ),
        "runtime.ggml.backends.llama.source.artifact" => descriptor!(
            "runtime.ggml.backends.llama.source.artifact",
            runtime.ggml.backends.llama.source.artifact
        ),
        "runtime.ggml.backends.llama.logging.level" => descriptor!(
            "runtime.ggml.backends.llama.logging.level",
            runtime.ggml.backends.llama.logging.level
        ),
        "runtime.ggml.backends.llama.logging.json" => descriptor!(
            "runtime.ggml.backends.llama.logging.json",
            runtime.ggml.backends.llama.logging.json
        ),
        "runtime.ggml.backends.llama.logging.path" => descriptor!(
            "runtime.ggml.backends.llama.logging.path",
            runtime.ggml.backends.llama.logging.path
        ),
        "runtime.ggml.backends.llama.capacity.queue" => descriptor!(
            "runtime.ggml.backends.llama.capacity.queue",
            runtime.ggml.backends.llama.capacity.queue
        ),
        "runtime.ggml.backends.llama.capacity.concurrent_requests" => descriptor!(
            "runtime.ggml.backends.llama.capacity.concurrent_requests",
            runtime.ggml.backends.llama.capacity.concurrent_requests
        ),
        "runtime.ggml.backends.llama.endpoint.http.address" => descriptor!(
            "runtime.ggml.backends.llama.endpoint.http.address",
            runtime.ggml.backends.llama.endpoint.http.address
        ),
        "runtime.ggml.backends.llama.endpoint.ipc.path" => descriptor!(
            "runtime.ggml.backends.llama.endpoint.ipc.path",
            runtime.ggml.backends.llama.endpoint.ipc.path
        ),
        "runtime.ggml.backends.whisper.enabled" => descriptor!(
            "runtime.ggml.backends.whisper.enabled",
            runtime.ggml.backends.whisper.enabled
        ),
        "runtime.ggml.backends.whisper.flash_attn" => descriptor!(
            "runtime.ggml.backends.whisper.flash_attn",
            runtime.ggml.backends.whisper.flash_attn
        ),
        "runtime.ggml.backends.whisper.source.version" => descriptor!(
            "runtime.ggml.backends.whisper.source.version",
            runtime.ggml.backends.whisper.source.version
        ),
        "runtime.ggml.backends.whisper.source.artifact" => descriptor!(
            "runtime.ggml.backends.whisper.source.artifact",
            runtime.ggml.backends.whisper.source.artifact
        ),
        "runtime.ggml.backends.whisper.logging.level" => descriptor!(
            "runtime.ggml.backends.whisper.logging.level",
            runtime.ggml.backends.whisper.logging.level
        ),
        "runtime.ggml.backends.whisper.logging.json" => descriptor!(
            "runtime.ggml.backends.whisper.logging.json",
            runtime.ggml.backends.whisper.logging.json
        ),
        "runtime.ggml.backends.whisper.logging.path" => descriptor!(
            "runtime.ggml.backends.whisper.logging.path",
            runtime.ggml.backends.whisper.logging.path
        ),
        "runtime.ggml.backends.whisper.capacity.queue" => descriptor!(
            "runtime.ggml.backends.whisper.capacity.queue",
            runtime.ggml.backends.whisper.capacity.queue
        ),
        "runtime.ggml.backends.whisper.capacity.concurrent_requests" => descriptor!(
            "runtime.ggml.backends.whisper.capacity.concurrent_requests",
            runtime.ggml.backends.whisper.capacity.concurrent_requests
        ),
        "runtime.ggml.backends.whisper.endpoint.http.address" => descriptor!(
            "runtime.ggml.backends.whisper.endpoint.http.address",
            runtime.ggml.backends.whisper.endpoint.http.address
        ),
        "runtime.ggml.backends.whisper.endpoint.ipc.path" => descriptor!(
            "runtime.ggml.backends.whisper.endpoint.ipc.path",
            runtime.ggml.backends.whisper.endpoint.ipc.path
        ),
        "runtime.ggml.backends.diffusion.enabled" => descriptor!(
            "runtime.ggml.backends.diffusion.enabled",
            runtime.ggml.backends.diffusion.enabled
        ),
        "runtime.ggml.backends.diffusion.flash_attn" => descriptor!(
            "runtime.ggml.backends.diffusion.flash_attn",
            runtime.ggml.backends.diffusion.flash_attn
        ),
        "runtime.ggml.backends.diffusion.source.version" => descriptor!(
            "runtime.ggml.backends.diffusion.source.version",
            runtime.ggml.backends.diffusion.source.version
        ),
        "runtime.ggml.backends.diffusion.source.artifact" => descriptor!(
            "runtime.ggml.backends.diffusion.source.artifact",
            runtime.ggml.backends.diffusion.source.artifact
        ),
        "runtime.ggml.backends.diffusion.logging.level" => descriptor!(
            "runtime.ggml.backends.diffusion.logging.level",
            runtime.ggml.backends.diffusion.logging.level
        ),
        "runtime.ggml.backends.diffusion.logging.json" => descriptor!(
            "runtime.ggml.backends.diffusion.logging.json",
            runtime.ggml.backends.diffusion.logging.json
        ),
        "runtime.ggml.backends.diffusion.logging.path" => descriptor!(
            "runtime.ggml.backends.diffusion.logging.path",
            runtime.ggml.backends.diffusion.logging.path
        ),
        "runtime.ggml.backends.diffusion.capacity.queue" => descriptor!(
            "runtime.ggml.backends.diffusion.capacity.queue",
            runtime.ggml.backends.diffusion.capacity.queue
        ),
        "runtime.ggml.backends.diffusion.capacity.concurrent_requests" => descriptor!(
            "runtime.ggml.backends.diffusion.capacity.concurrent_requests",
            runtime.ggml.backends.diffusion.capacity.concurrent_requests
        ),
        "runtime.ggml.backends.diffusion.endpoint.http.address" => descriptor!(
            "runtime.ggml.backends.diffusion.endpoint.http.address",
            runtime.ggml.backends.diffusion.endpoint.http.address
        ),
        "runtime.ggml.backends.diffusion.endpoint.ipc.path" => descriptor!(
            "runtime.ggml.backends.diffusion.endpoint.ipc.path",
            runtime.ggml.backends.diffusion.endpoint.ipc.path
        ),
        "runtime.candle.enabled" => descriptor!("runtime.candle.enabled", runtime.candle.enabled),
        "runtime.candle.install_dir" => {
            descriptor!("runtime.candle.install_dir", runtime.candle.install_dir)
        }
        "runtime.candle.source.version" => {
            descriptor!("runtime.candle.source.version", runtime.candle.source.version)
        }
        "runtime.candle.source.artifact" => {
            descriptor!("runtime.candle.source.artifact", runtime.candle.source.artifact)
        }
        "runtime.candle.logging.level" => {
            descriptor!("runtime.candle.logging.level", runtime.candle.logging.level)
        }
        "runtime.candle.logging.json" => {
            descriptor!("runtime.candle.logging.json", runtime.candle.logging.json)
        }
        "runtime.candle.logging.path" => {
            descriptor!("runtime.candle.logging.path", runtime.candle.logging.path)
        }
        "runtime.candle.capacity.queue" => {
            descriptor!("runtime.candle.capacity.queue", runtime.candle.capacity.queue)
        }
        "runtime.candle.capacity.concurrent_requests" => descriptor!(
            "runtime.candle.capacity.concurrent_requests",
            runtime.candle.capacity.concurrent_requests
        ),
        "runtime.candle.endpoint.http.address" => descriptor!(
            "runtime.candle.endpoint.http.address",
            runtime.candle.endpoint.http.address
        ),
        "runtime.candle.endpoint.ipc.path" => {
            descriptor!("runtime.candle.endpoint.ipc.path", runtime.candle.endpoint.ipc.path)
        }
        "runtime.onnx.enabled" => descriptor!("runtime.onnx.enabled", runtime.onnx.enabled),
        "runtime.onnx.install_dir" => {
            descriptor!("runtime.onnx.install_dir", runtime.onnx.install_dir)
        }
        "runtime.onnx.source.version" => {
            descriptor!("runtime.onnx.source.version", runtime.onnx.source.version)
        }
        "runtime.onnx.source.artifact" => {
            descriptor!("runtime.onnx.source.artifact", runtime.onnx.source.artifact)
        }
        "runtime.onnx.logging.level" => {
            descriptor!("runtime.onnx.logging.level", runtime.onnx.logging.level)
        }
        "runtime.onnx.logging.json" => {
            descriptor!("runtime.onnx.logging.json", runtime.onnx.logging.json)
        }
        "runtime.onnx.logging.path" => {
            descriptor!("runtime.onnx.logging.path", runtime.onnx.logging.path)
        }
        "runtime.onnx.capacity.queue" => {
            descriptor!("runtime.onnx.capacity.queue", runtime.onnx.capacity.queue)
        }
        "runtime.onnx.capacity.concurrent_requests" => descriptor!(
            "runtime.onnx.capacity.concurrent_requests",
            runtime.onnx.capacity.concurrent_requests
        ),
        "runtime.onnx.endpoint.http.address" => {
            descriptor!("runtime.onnx.endpoint.http.address", runtime.onnx.endpoint.http.address)
        }
        "runtime.onnx.endpoint.ipc.path" => {
            descriptor!("runtime.onnx.endpoint.ipc.path", runtime.onnx.endpoint.ipc.path)
        }
        "providers.registry" => descriptor!("providers.registry", providers.registry),
        "models.cache_dir" => descriptor!("models.cache_dir", models.cache_dir),
        "models.config_dir" => descriptor!("models.config_dir", models.config_dir),
        "models.download_source" => {
            descriptor!("models.download_source", models.download_source)
        }
        "models.auto_unload.enabled" => {
            descriptor!("models.auto_unload.enabled", models.auto_unload.enabled)
        }
        "models.auto_unload.idle_minutes" => {
            descriptor!("models.auto_unload.idle_minutes", models.auto_unload.idle_minutes)
        }
        "models.auto_unload.min_free_system_memory_bytes" => descriptor!(
            "models.auto_unload.min_free_system_memory_bytes",
            models.auto_unload.min_free_system_memory_bytes
        ),
        "models.auto_unload.min_free_gpu_memory_bytes" => descriptor!(
            "models.auto_unload.min_free_gpu_memory_bytes",
            models.auto_unload.min_free_gpu_memory_bytes
        ),
        "models.auto_unload.max_pressure_evictions_per_load" => descriptor!(
            "models.auto_unload.max_pressure_evictions_per_load",
            models.auto_unload.max_pressure_evictions_per_load
        ),
        "plugin.install_dir" => descriptor!("plugin.install_dir", plugin.install_dir),
        "plugin.js_runtime_transport" => {
            descriptor!("plugin.js_runtime_transport", plugin.js_runtime_transport)
        }
        "plugin.python_runtime_transport" => {
            descriptor!("plugin.python_runtime_transport", plugin.python_runtime_transport)
        }
        "server.address" => descriptor!("server.address", server.address),
        "server.logging.level" => descriptor!("server.logging.level", server.logging.level),
        "server.logging.json" => descriptor!("server.logging.json", server.logging.json),
        "server.logging.path" => descriptor!("server.logging.path", server.logging.path),
        "server.cors.allowed_origins" => {
            descriptor!("server.cors.allowed_origins", server.cors.allowed_origins)
        }
        "server.admin.token" => descriptor!("server.admin.token", server.admin.token),
        "server.swagger.enabled" => {
            descriptor!("server.swagger.enabled", server.swagger.enabled)
        }
        "server.cloud_http_trace" => {
            descriptor!("server.cloud_http_trace", server.cloud_http_trace)
        }
        _ => return None,
    })
}

pub(crate) fn setting_value(
    document: &SettingsDocument,
    pmid: &str,
) -> Result<SettingValue, ConfigError> {
    let descriptor = setting_descriptor(pmid)
        .ok_or_else(|| ConfigError::NotFound(format!("setting pmid '{pmid}' not found")))?;
    debug_assert_eq!(descriptor.pmid, pmid);
    Ok((descriptor.get)(document))
}

pub(crate) fn set_document_value(
    document: &mut SettingsDocument,
    pmid: &str,
    value: SettingValue,
) -> Result<(), ConfigError> {
    let descriptor = setting_descriptor(pmid)
        .ok_or_else(|| ConfigError::NotFound(format!("setting pmid '{pmid}' not found")))?;
    debug_assert_eq!(descriptor.pmid, pmid);
    (descriptor.set)(document, value)
}

fn setting_value_from<T: Serialize>(value: &T) -> SettingValue {
    serde_json::to_value(value).map(SettingValue::from).unwrap_or_default()
}

fn set_setting_value<T: DeserializeOwned>(
    target: &mut T,
    value: SettingValue,
    pmid: &str,
) -> Result<(), ConfigError> {
    *target = serde_json::from_value(value.into_json_value()).map_err(|error| {
        ConfigError::BadRequest(format!("setting '{pmid}' value has invalid shape: {error}"))
    })?;
    Ok(())
}
