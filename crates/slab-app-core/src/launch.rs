use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use slab_types::RuntimeBackendId;
use slab_types::settings::{
    LoggingOverrideConfig, PmidConfig, RuntimeMode, RuntimeTransportMode, SettingsDocumentV2,
};

use crate::config::Config;
use crate::error::AppCoreError;

const RUNTIME_BACKEND_SLOTS: [(RuntimeBackendId, &str, u32); 3] = [
    (RuntimeBackendId::GgmlWhisper, "whisper", 0),
    (RuntimeBackendId::GgmlLlama, "llama", 1),
    (RuntimeBackendId::GgmlDiffusion, "diffusion", 2),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchProfile {
    Server,
    Desktop,
}

impl LaunchProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Server => "server",
            Self::Desktop => "desktop",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchHostPaths {
    pub runtime_lib_dir_fallback: Option<PathBuf>,
    pub runtime_log_dir_fallback: PathBuf,
    pub runtime_ipc_dir_fallback: PathBuf,
    pub shutdown_on_stdin_close: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRuntimeChildSpec {
    pub backend: RuntimeBackendId,
    pub grpc_bind_address: String,
    pub transport: RuntimeTransportMode,
    pub queue_capacity: usize,
    pub backend_capacity: usize,
    pub lib_dir: Option<PathBuf>,
    pub log_level: Option<String>,
    pub log_json: Option<bool>,
    pub log_file: PathBuf,
    pub shutdown_on_stdin_close: bool,
}

impl ResolvedRuntimeChildSpec {
    pub fn command_args(&self, log_level: Option<&str>, log_json: bool) -> Vec<String> {
        let effective_log_level = self.log_level.as_deref().or(log_level);
        let effective_log_json = self.log_json.unwrap_or(log_json);
        let mut args = vec![
            "--enabled-backends".to_owned(),
            self.backend.canonical_id().to_owned(),
            "--grpc-bind".to_owned(),
            self.grpc_bind_address.clone(),
            "--queue-capacity".to_owned(),
            self.queue_capacity.to_string(),
            "--backend-capacity".to_owned(),
            self.backend_capacity.to_string(),
            "--log-file".to_owned(),
            self.log_file.to_string_lossy().into_owned(),
        ];

        if let Some(lib_dir) = &self.lib_dir {
            args.push("--lib-dir".to_owned());
            args.push(lib_dir.to_string_lossy().into_owned());
        }
        if let Some(log_level) = effective_log_level.filter(|value| !value.trim().is_empty()) {
            args.push("--log".to_owned());
            args.push(log_level.trim().to_owned());
        }
        if effective_log_json {
            args.push("--log-json".to_owned());
        }
        if self.shutdown_on_stdin_close {
            args.push("--shutdown-on-stdin-close".to_owned());
        }

        args
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedRuntimeEndpoints {
    pub whisper: Option<String>,
    pub llama: Option<String>,
    pub diffusion: Option<String>,
}

impl ResolvedRuntimeEndpoints {
    pub fn backend_endpoint(&self, backend: RuntimeBackendId) -> Option<&str> {
        match backend {
            RuntimeBackendId::GgmlWhisper => self.whisper.as_deref(),
            RuntimeBackendId::GgmlLlama => self.llama.as_deref(),
            RuntimeBackendId::GgmlDiffusion => self.diffusion.as_deref(),
            _ => None,
        }
    }

    pub fn apply_to_config(&self, config: &mut Config, transport: RuntimeTransportMode) {
        config.transport_mode = transport.as_str().to_owned();
        config.whisper_grpc_endpoint = self.whisper.clone();
        config.llama_grpc_endpoint = self.llama.clone();
        config.diffusion_grpc_endpoint = self.diffusion.clone();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedGatewaySpec {
    pub bind_address: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLaunchSpec {
    pub profile: LaunchProfile,
    pub transport: RuntimeTransportMode,
    pub runtime_log_dir: PathBuf,
    pub runtime_ipc_dir: Option<PathBuf>,
    pub extra_dirs: Vec<PathBuf>,
    pub children: Vec<ResolvedRuntimeChildSpec>,
    pub endpoints: ResolvedRuntimeEndpoints,
    pub gateway: Option<ResolvedGatewaySpec>,
}

impl ResolvedLaunchSpec {
    pub fn prepare_filesystem(&self) -> Result<(), AppCoreError> {
        fs::create_dir_all(&self.runtime_log_dir).map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to create runtime log directory '{}': {error}",
                self.runtime_log_dir.display()
            ))
        })?;

        if let Some(ipc_dir) = &self.runtime_ipc_dir {
            fs::create_dir_all(ipc_dir).map_err(|error| {
                AppCoreError::Internal(format!(
                    "failed to create runtime IPC directory '{}': {error}",
                    ipc_dir.display()
                ))
            })?;
        }

        for dir in &self.extra_dirs {
            fs::create_dir_all(dir).map_err(|error| {
                AppCoreError::Internal(format!(
                    "failed to create runtime support directory '{}': {error}",
                    dir.display()
                ))
            })?;
        }

        Ok(())
    }

    pub fn apply_to_config(&self, config: &mut Config) {
        self.endpoints.apply_to_config(config, self.transport);
        if let Some(gateway) = &self.gateway {
            config.bind_address = gateway.bind_address.clone();
        }
    }
}

pub fn resolve_launch_spec(
    settings: &PmidConfig,
    profile: LaunchProfile,
    host_paths: &LaunchHostPaths,
) -> Result<ResolvedLaunchSpec, AppCoreError> {
    let launch = &settings.launch;
    let transport = launch.transport;
    let runtime_log_dir = launch
        .runtime_log_dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| host_paths.runtime_log_dir_fallback.clone());
    let runtime_ipc_dir = if transport == RuntimeTransportMode::Ipc {
        Some(
            launch
                .runtime_ipc_dir
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| host_paths.runtime_ipc_dir_fallback.clone()),
        )
    } else {
        None
    };
    let lib_dir = settings
        .setup
        .backends
        .dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| host_paths.runtime_lib_dir_fallback.clone());

    let queue_capacity = usize::try_from(launch.queue_capacity).map_err(|_| {
        AppCoreError::Internal("launch.queue_capacity does not fit into usize".to_owned())
    })?;
    let backend_capacity = usize::try_from(launch.backend_capacity).map_err(|_| {
        AppCoreError::Internal("launch.backend_capacity does not fit into usize".to_owned())
    })?;

    let mut children = Vec::new();
    let mut endpoints = ResolvedRuntimeEndpoints::default();
    let pid = std::process::id();

    for (backend, alias, slot) in RUNTIME_BACKEND_SLOTS {
        if !backend_enabled(settings, backend) {
            continue;
        }

        let endpoint = resolve_backend_endpoint(
            settings,
            profile,
            transport,
            runtime_ipc_dir.as_deref(),
            alias,
            slot,
            pid,
        )?;

        match backend {
            RuntimeBackendId::GgmlWhisper => endpoints.whisper = Some(endpoint.clone()),
            RuntimeBackendId::GgmlLlama => endpoints.llama = Some(endpoint.clone()),
            RuntimeBackendId::GgmlDiffusion => endpoints.diffusion = Some(endpoint.clone()),
            _ => {}
        }

        children.push(ResolvedRuntimeChildSpec {
            backend,
            grpc_bind_address: endpoint,
            transport,
            queue_capacity,
            backend_capacity,
            lib_dir: lib_dir.clone(),
            log_level: None,
            log_json: None,
            log_file: runtime_log_dir.join(format!(
                "slab-runtime-{}-{}-{}.log",
                pid,
                profile.as_str(),
                alias
            )),
            shutdown_on_stdin_close: host_paths.shutdown_on_stdin_close,
        });
    }

    if children.is_empty() {
        return Err(AppCoreError::Internal(
            "launch settings disable every runtime backend; enable at least one launch.backends.*.enabled setting"
                .to_owned(),
        ));
    }

    let gateway = match profile {
        LaunchProfile::Server => Some(ResolvedGatewaySpec {
            bind_address: settings.launch.profiles.server.gateway_bind.trim().to_owned(),
        }),
        LaunchProfile::Desktop => None,
    };

    Ok(ResolvedLaunchSpec {
        profile,
        transport,
        runtime_log_dir,
        runtime_ipc_dir,
        extra_dirs: Vec::new(),
        children,
        endpoints,
        gateway,
    })
}

pub fn resolve_launch_spec_v2(
    settings: &SettingsDocumentV2,
    profile: LaunchProfile,
    host_paths: &LaunchHostPaths,
) -> Result<ResolvedLaunchSpec, AppCoreError> {
    match settings.runtime.mode {
        RuntimeMode::ManagedChildren => {
            resolve_managed_launch_spec_v2(settings, profile, host_paths)
        }
        RuntimeMode::ExternalEndpoints => {
            resolve_external_launch_spec_v2(settings, profile, host_paths)
        }
    }
}

fn resolve_managed_launch_spec_v2(
    settings: &SettingsDocumentV2,
    profile: LaunchProfile,
    host_paths: &LaunchHostPaths,
) -> Result<ResolvedLaunchSpec, AppCoreError> {
    let transport = settings.runtime.transport;
    let runtime_log_dir = resolve_primary_runtime_log_dir_v2(settings, host_paths);
    let runtime_ipc_dir = (transport == RuntimeTransportMode::Ipc)
        .then(|| host_paths.runtime_ipc_dir_fallback.clone());
    let lib_dir = normalize_optional_text(settings.runtime.ggml.install_dir.as_deref())
        .map(PathBuf::from)
        .or_else(|| host_paths.runtime_lib_dir_fallback.clone());
    let enabled_backends = enabled_ggml_backends_v2(settings);

    let mut children = Vec::new();
    let mut endpoints = ResolvedRuntimeEndpoints::default();
    let mut extra_dirs = BTreeSet::new();
    let pid = std::process::id();
    let single_backend = enabled_backends.len() == 1;

    for (backend, alias, slot) in RUNTIME_BACKEND_SLOTS {
        if !v2_backend_enabled(settings, backend) {
            continue;
        }

        let endpoint = resolve_managed_backend_endpoint_v2(
            settings,
            profile,
            transport,
            host_paths,
            backend,
            alias,
            slot,
            pid,
            single_backend,
        )?;
        let (queue_capacity, backend_capacity) = resolve_backend_capacity_v2(settings, backend)?;
        let (log_level, log_json, log_dir) =
            resolve_backend_logging_v2(settings, backend, &runtime_log_dir);

        if log_dir != runtime_log_dir {
            extra_dirs.insert(log_dir.clone());
        }
        if transport == RuntimeTransportMode::Ipc {
            if let Some(dir) = directory_from_ipc_endpoint(&endpoint) {
                if runtime_ipc_dir.as_ref() != Some(&dir) {
                    extra_dirs.insert(dir);
                }
            }
        }

        match backend {
            RuntimeBackendId::GgmlWhisper => endpoints.whisper = Some(endpoint.clone()),
            RuntimeBackendId::GgmlLlama => endpoints.llama = Some(endpoint.clone()),
            RuntimeBackendId::GgmlDiffusion => endpoints.diffusion = Some(endpoint.clone()),
            _ => {}
        }

        children.push(ResolvedRuntimeChildSpec {
            backend,
            grpc_bind_address: endpoint,
            transport,
            queue_capacity,
            backend_capacity,
            lib_dir: lib_dir.clone(),
            log_level,
            log_json: Some(log_json),
            log_file: log_dir.join(format!(
                "slab-runtime-{}-{}-{}.log",
                pid,
                profile.as_str(),
                alias
            )),
            shutdown_on_stdin_close: host_paths.shutdown_on_stdin_close,
        });
    }

    let gateway = match profile {
        LaunchProfile::Server => {
            Some(ResolvedGatewaySpec { bind_address: settings.server.address.trim().to_owned() })
        }
        LaunchProfile::Desktop => None,
    };

    Ok(ResolvedLaunchSpec {
        profile,
        transport,
        runtime_log_dir,
        runtime_ipc_dir,
        extra_dirs: extra_dirs.into_iter().collect(),
        children,
        endpoints,
        gateway,
    })
}

fn resolve_external_launch_spec_v2(
    settings: &SettingsDocumentV2,
    profile: LaunchProfile,
    host_paths: &LaunchHostPaths,
) -> Result<ResolvedLaunchSpec, AppCoreError> {
    let transport = settings.runtime.transport;
    let enabled_backends = enabled_ggml_backends_v2(settings);

    let mut endpoints = ResolvedRuntimeEndpoints::default();
    let single_backend = enabled_backends.len() == 1;

    for (backend, alias, _) in RUNTIME_BACKEND_SLOTS {
        if !v2_backend_enabled(settings, backend) {
            continue;
        }

        let endpoint = resolve_external_backend_endpoint_v2(
            settings,
            transport,
            backend,
            alias,
            single_backend,
        )?;

        match backend {
            RuntimeBackendId::GgmlWhisper => endpoints.whisper = Some(endpoint),
            RuntimeBackendId::GgmlLlama => endpoints.llama = Some(endpoint),
            RuntimeBackendId::GgmlDiffusion => endpoints.diffusion = Some(endpoint),
            _ => {}
        }
    }

    let gateway = match profile {
        LaunchProfile::Server => {
            Some(ResolvedGatewaySpec { bind_address: settings.server.address.trim().to_owned() })
        }
        LaunchProfile::Desktop => None,
    };

    Ok(ResolvedLaunchSpec {
        profile,
        transport,
        runtime_log_dir: resolve_primary_runtime_log_dir_v2(settings, host_paths),
        runtime_ipc_dir: None,
        extra_dirs: Vec::new(),
        children: Vec::new(),
        endpoints,
        gateway,
    })
}

fn backend_enabled(settings: &PmidConfig, backend: RuntimeBackendId) -> bool {
    match backend {
        RuntimeBackendId::GgmlLlama => settings.launch.backends.llama.enabled,
        RuntimeBackendId::GgmlWhisper => settings.launch.backends.whisper.enabled,
        RuntimeBackendId::GgmlDiffusion => settings.launch.backends.diffusion.enabled,
        _ => false,
    }
}

fn v2_backend_enabled(settings: &SettingsDocumentV2, backend: RuntimeBackendId) -> bool {
    match backend {
        RuntimeBackendId::GgmlLlama => settings.runtime.ggml.backends.llama.enabled,
        RuntimeBackendId::GgmlWhisper => settings.runtime.ggml.backends.whisper.enabled,
        RuntimeBackendId::GgmlDiffusion => settings.runtime.ggml.backends.diffusion.enabled,
        _ => false,
    }
}

fn enabled_ggml_backends_v2(settings: &SettingsDocumentV2) -> Vec<RuntimeBackendId> {
    RUNTIME_BACKEND_SLOTS
        .into_iter()
        .map(|(backend, _, _)| backend)
        .filter(|backend| v2_backend_enabled(settings, *backend))
        .collect()
}

fn resolve_backend_endpoint(
    settings: &PmidConfig,
    profile: LaunchProfile,
    transport: RuntimeTransportMode,
    runtime_ipc_dir: Option<&std::path::Path>,
    backend_alias: &str,
    slot: u32,
    pid: u32,
) -> Result<String, AppCoreError> {
    match transport {
        RuntimeTransportMode::Http => {
            let (host, base_port) = match profile {
                LaunchProfile::Server => (
                    settings.launch.profiles.server.runtime_bind_host.trim(),
                    settings.launch.profiles.server.runtime_bind_base_port,
                ),
                LaunchProfile::Desktop => (
                    settings.launch.profiles.desktop.runtime_bind_host.trim(),
                    settings.launch.profiles.desktop.runtime_bind_base_port,
                ),
            };

            if host.is_empty() {
                return Err(AppCoreError::Internal(format!(
                    "launch profile '{}' runtime bind host must not be empty",
                    profile.as_str()
                )));
            }

            let port = base_port.checked_add(slot).ok_or_else(|| {
                AppCoreError::Internal(format!(
                    "launch profile '{}' runtime port allocation overflowed",
                    profile.as_str()
                ))
            })?;
            if port == 0 || port > u32::from(u16::MAX) {
                return Err(AppCoreError::Internal(format!(
                    "launch profile '{}' resolved invalid runtime port '{}'",
                    profile.as_str(),
                    port
                )));
            }

            Ok(format!("{host}:{port}"))
        }
        RuntimeTransportMode::Ipc => {
            #[cfg(windows)]
            {
                let _ = runtime_ipc_dir;
                Ok(format!(
                    r"ipc://\\.\pipe\slab-runtime-{}-{}-{}",
                    pid,
                    profile.as_str(),
                    backend_alias
                ))
            }

            #[cfg(not(windows))]
            {
                let runtime_ipc_dir = runtime_ipc_dir.ok_or_else(|| {
                    AppCoreError::Internal("runtime IPC directory was not resolved".to_owned())
                })?;
                let path = runtime_ipc_dir.join(format!(
                    "slab-runtime-{}-{}-{}.sock",
                    pid,
                    profile.as_str(),
                    backend_alias
                ));
                Ok(format!("ipc://{}", path.to_string_lossy()))
            }
        }
    }
}

fn resolve_managed_backend_endpoint_v2(
    settings: &SettingsDocumentV2,
    profile: LaunchProfile,
    transport: RuntimeTransportMode,
    host_paths: &LaunchHostPaths,
    backend: RuntimeBackendId,
    backend_alias: &str,
    slot: u32,
    pid: u32,
    single_backend: bool,
) -> Result<String, AppCoreError> {
    if let Some(explicit) = explicit_leaf_endpoint_v2(settings, backend, transport) {
        return normalize_runtime_endpoint(explicit, transport);
    }

    if single_backend {
        if let Some(explicit) = shared_ggml_endpoint_v2(settings, transport) {
            return normalize_runtime_endpoint(explicit, transport);
        }
        if let Some(explicit) = shared_runtime_endpoint_v2(settings, transport) {
            return normalize_runtime_endpoint(explicit, transport);
        }
    }

    default_generated_backend_endpoint(profile, transport, host_paths, backend_alias, slot, pid)
}

fn resolve_external_backend_endpoint_v2(
    settings: &SettingsDocumentV2,
    transport: RuntimeTransportMode,
    backend: RuntimeBackendId,
    backend_alias: &str,
    single_backend: bool,
) -> Result<String, AppCoreError> {
    let explicit = explicit_leaf_endpoint_v2(settings, backend, transport)
        .or_else(|| single_backend.then(|| shared_ggml_endpoint_v2(settings, transport)).flatten())
        .or_else(|| {
            single_backend.then(|| shared_runtime_endpoint_v2(settings, transport)).flatten()
        })
        .ok_or_else(|| {
            AppCoreError::BadRequest(format!(
                "runtime.mode=external_endpoints requires an explicit endpoint for backend '{}'",
                backend_alias
            ))
        })?;

    normalize_runtime_endpoint(explicit, transport)
}

fn resolve_backend_capacity_v2(
    settings: &SettingsDocumentV2,
    backend: RuntimeBackendId,
) -> Result<(usize, usize), AppCoreError> {
    let root = &settings.runtime.capacity;
    let family = &settings.runtime.ggml.capacity;
    let leaf = match backend {
        RuntimeBackendId::GgmlLlama => &settings.runtime.ggml.backends.llama.capacity,
        RuntimeBackendId::GgmlWhisper => &settings.runtime.ggml.backends.whisper.capacity,
        RuntimeBackendId::GgmlDiffusion => &settings.runtime.ggml.backends.diffusion.capacity,
        _ => {
            return Err(AppCoreError::Internal(format!(
                "backend '{}' is not supported by resolve_backend_capacity_v2",
                backend.canonical_id()
            )));
        }
    };

    let queue = leaf.queue.or(family.queue).unwrap_or(root.queue);
    let concurrent =
        leaf.concurrent_requests.or(family.concurrent_requests).unwrap_or(root.concurrent_requests);

    let queue_capacity = usize::try_from(queue).map_err(|_| {
        AppCoreError::Internal(format!(
            "resolved queue capacity for '{}' does not fit into usize",
            backend.canonical_id()
        ))
    })?;
    let backend_capacity = usize::try_from(concurrent).map_err(|_| {
        AppCoreError::Internal(format!(
            "resolved concurrent request capacity for '{}' does not fit into usize",
            backend.canonical_id()
        ))
    })?;

    Ok((queue_capacity, backend_capacity))
}

fn resolve_backend_logging_v2(
    settings: &SettingsDocumentV2,
    backend: RuntimeBackendId,
    fallback_dir: &std::path::Path,
) -> (Option<String>, bool, PathBuf) {
    let global = &settings.logging;
    let runtime = &settings.runtime.logging;
    let family = &settings.runtime.ggml.logging;
    let leaf = match backend {
        RuntimeBackendId::GgmlLlama => &settings.runtime.ggml.backends.llama.logging,
        RuntimeBackendId::GgmlWhisper => &settings.runtime.ggml.backends.whisper.logging,
        RuntimeBackendId::GgmlDiffusion => &settings.runtime.ggml.backends.diffusion.logging,
        _ => &LoggingOverrideConfig::default(),
    };

    let level = leaf
        .level
        .as_deref()
        .and_then(normalize_text)
        .or_else(|| family.level.as_deref().and_then(normalize_text))
        .or_else(|| runtime.level.as_deref().and_then(normalize_text))
        .or_else(|| normalize_text(global.level.as_str()));
    let json = leaf.json.or(family.json).or(runtime.json).unwrap_or(global.json);
    let dir = leaf
        .path
        .as_deref()
        .and_then(normalize_text)
        .or_else(|| family.path.as_deref().and_then(normalize_text))
        .or_else(|| runtime.path.as_deref().and_then(normalize_text))
        .or_else(|| global.path.as_deref().and_then(normalize_text))
        .map(PathBuf::from)
        .unwrap_or_else(|| fallback_dir.to_path_buf());

    (level, json, dir)
}

fn resolve_primary_runtime_log_dir_v2(
    settings: &SettingsDocumentV2,
    host_paths: &LaunchHostPaths,
) -> PathBuf {
    settings
        .runtime
        .logging
        .path
        .as_deref()
        .and_then(normalize_text)
        .or_else(|| settings.logging.path.as_deref().and_then(normalize_text))
        .map(PathBuf::from)
        .unwrap_or_else(|| host_paths.runtime_log_dir_fallback.clone())
}

fn explicit_leaf_endpoint_v2(
    settings: &SettingsDocumentV2,
    backend: RuntimeBackendId,
    transport: RuntimeTransportMode,
) -> Option<&str> {
    match (backend, transport) {
        (RuntimeBackendId::GgmlLlama, RuntimeTransportMode::Http) => {
            settings.runtime.ggml.backends.llama.endpoint.http.address.as_deref()
        }
        (RuntimeBackendId::GgmlWhisper, RuntimeTransportMode::Http) => {
            settings.runtime.ggml.backends.whisper.endpoint.http.address.as_deref()
        }
        (RuntimeBackendId::GgmlDiffusion, RuntimeTransportMode::Http) => {
            settings.runtime.ggml.backends.diffusion.endpoint.http.address.as_deref()
        }
        (RuntimeBackendId::GgmlLlama, RuntimeTransportMode::Ipc) => {
            settings.runtime.ggml.backends.llama.endpoint.ipc.path.as_deref()
        }
        (RuntimeBackendId::GgmlWhisper, RuntimeTransportMode::Ipc) => {
            settings.runtime.ggml.backends.whisper.endpoint.ipc.path.as_deref()
        }
        (RuntimeBackendId::GgmlDiffusion, RuntimeTransportMode::Ipc) => {
            settings.runtime.ggml.backends.diffusion.endpoint.ipc.path.as_deref()
        }
        _ => None,
    }
}

fn shared_ggml_endpoint_v2(
    settings: &SettingsDocumentV2,
    transport: RuntimeTransportMode,
) -> Option<&str> {
    match transport {
        RuntimeTransportMode::Http => settings.runtime.ggml.endpoint.http.address.as_deref(),
        RuntimeTransportMode::Ipc => settings.runtime.ggml.endpoint.ipc.path.as_deref(),
    }
}

fn shared_runtime_endpoint_v2(
    settings: &SettingsDocumentV2,
    transport: RuntimeTransportMode,
) -> Option<&str> {
    match transport {
        RuntimeTransportMode::Http => settings.runtime.endpoint.http.address.as_deref(),
        RuntimeTransportMode::Ipc => settings.runtime.endpoint.ipc.path.as_deref(),
    }
}

fn normalize_runtime_endpoint(
    raw: &str,
    transport: RuntimeTransportMode,
) -> Result<String, AppCoreError> {
    let value = normalize_text(raw).ok_or_else(|| {
        AppCoreError::BadRequest("runtime endpoint value must not be blank".to_owned())
    })?;

    match transport {
        RuntimeTransportMode::Http => Ok(value),
        RuntimeTransportMode::Ipc => {
            if value.starts_with("ipc://") {
                Ok(value)
            } else {
                Ok(format!("ipc://{value}"))
            }
        }
    }
}

fn default_generated_backend_endpoint(
    profile: LaunchProfile,
    transport: RuntimeTransportMode,
    host_paths: &LaunchHostPaths,
    backend_alias: &str,
    slot: u32,
    pid: u32,
) -> Result<String, AppCoreError> {
    match transport {
        RuntimeTransportMode::Http => {
            let (host, base_port) = match profile {
                LaunchProfile::Server => ("127.0.0.1", 3001_u32),
                LaunchProfile::Desktop => ("127.0.0.1", 50051_u32),
            };
            let port = base_port.checked_add(slot).ok_or_else(|| {
                AppCoreError::Internal(format!(
                    "launch profile '{}' runtime port allocation overflowed",
                    profile.as_str()
                ))
            })?;
            if port == 0 || port > u32::from(u16::MAX) {
                return Err(AppCoreError::Internal(format!(
                    "launch profile '{}' resolved invalid runtime port '{}'",
                    profile.as_str(),
                    port
                )));
            }
            Ok(format!("{host}:{port}"))
        }
        RuntimeTransportMode::Ipc => {
            #[cfg(windows)]
            {
                let _ = host_paths;
                Ok(format!(
                    r"ipc://\\.\pipe\slab-runtime-{}-{}-{}",
                    pid,
                    profile.as_str(),
                    backend_alias
                ))
            }

            #[cfg(not(windows))]
            {
                let path = host_paths.runtime_ipc_dir_fallback.join(format!(
                    "slab-runtime-{}-{}-{}.sock",
                    pid,
                    profile.as_str(),
                    backend_alias
                ));
                Ok(format!("ipc://{}", path.to_string_lossy()))
            }
        }
    }
}

fn directory_from_ipc_endpoint(endpoint: &str) -> Option<PathBuf> {
    let raw = endpoint.strip_prefix("ipc://").unwrap_or(endpoint);
    if raw.starts_with(r"\\.\pipe\") {
        return None;
    }

    let path = PathBuf::from(raw);
    path.parent().map(|parent| parent.to_path_buf())
}

fn normalize_optional_text(raw: Option<&str>) -> Option<String> {
    raw.and_then(normalize_text)
}

fn normalize_text(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() { None } else { Some(trimmed.to_owned()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slab_types::settings::{RuntimeMode, RuntimeTransportMode, SettingsDocumentV2};

    fn host_paths() -> LaunchHostPaths {
        LaunchHostPaths {
            runtime_lib_dir_fallback: Some(PathBuf::from("C:/runtime/libs")),
            runtime_log_dir_fallback: PathBuf::from("C:/runtime/logs"),
            runtime_ipc_dir_fallback: PathBuf::from("C:/runtime/ipc"),
            shutdown_on_stdin_close: true,
        }
    }

    #[test]
    fn planner_keeps_server_and_desktop_backend_shape_in_sync() {
        let settings = PmidConfig::default();

        let server = resolve_launch_spec(&settings, LaunchProfile::Server, &host_paths()).unwrap();
        let desktop =
            resolve_launch_spec(&settings, LaunchProfile::Desktop, &host_paths()).unwrap();

        assert_eq!(server.children.len(), desktop.children.len());
        assert_eq!(
            server.children.iter().map(|child| child.backend).collect::<Vec<_>>(),
            desktop.children.iter().map(|child| child.backend).collect::<Vec<_>>()
        );
        assert!(server.gateway.is_some());
        assert!(desktop.gateway.is_none());

        for (server_child, desktop_child) in server.children.iter().zip(desktop.children.iter()) {
            assert_eq!(server_child.backend, desktop_child.backend);
            assert_eq!(server_child.queue_capacity, desktop_child.queue_capacity);
            assert_eq!(server_child.backend_capacity, desktop_child.backend_capacity);
            assert_eq!(server_child.lib_dir, desktop_child.lib_dir);
            assert_ne!(server_child.grpc_bind_address, desktop_child.grpc_bind_address);
        }
    }

    #[test]
    fn planner_drops_disabled_backends() {
        let mut settings = PmidConfig::default();
        settings.launch.backends.diffusion.enabled = false;

        let spec = resolve_launch_spec(&settings, LaunchProfile::Server, &host_paths()).unwrap();

        assert_eq!(spec.children.len(), 2);
        assert!(spec.endpoints.diffusion.is_none());
        assert!(
            !spec.children.iter().any(|child| child.backend == RuntimeBackendId::GgmlDiffusion)
        );
    }

    #[test]
    fn planner_rejects_invalid_port_allocations() {
        let mut settings = PmidConfig::default();
        settings.launch.profiles.server.runtime_bind_base_port = u32::from(u16::MAX);

        let error =
            resolve_launch_spec(&settings, LaunchProfile::Server, &host_paths()).unwrap_err();
        assert!(error.to_string().contains("invalid runtime port"));
    }

    #[test]
    fn planner_prefers_settings_backend_dir_over_host_fallback() {
        let mut settings = PmidConfig::default();
        settings.setup.backends.dir = Some("D:/settings/backend-libs".to_owned());

        let spec = resolve_launch_spec(&settings, LaunchProfile::Desktop, &host_paths()).unwrap();

        assert!(
            spec.children
                .iter()
                .all(|child| child.lib_dir == Some(PathBuf::from("D:/settings/backend-libs")))
        );
    }

    #[test]
    fn launch_spec_applies_runtime_endpoints_and_gateway_bind_to_config() {
        let settings = PmidConfig::default();
        let spec = resolve_launch_spec(&settings, LaunchProfile::Server, &host_paths()).unwrap();
        let mut config = Config::from_env();

        config.bind_address = "127.0.0.1:1".to_owned();
        config.transport_mode = "unknown".to_owned();
        config.llama_grpc_endpoint = None;
        config.whisper_grpc_endpoint = None;
        config.diffusion_grpc_endpoint = None;

        spec.apply_to_config(&mut config);

        assert_eq!(config.bind_address, settings.launch.profiles.server.gateway_bind);
        assert_eq!(config.transport_mode, settings.launch.transport.as_str());
        assert_eq!(config.llama_grpc_endpoint, spec.endpoints.llama);
        assert_eq!(config.whisper_grpc_endpoint, spec.endpoints.whisper);
        assert_eq!(config.diffusion_grpc_endpoint, spec.endpoints.diffusion);
    }

    #[test]
    fn child_command_args_are_generated_from_resolved_spec() {
        let spec = ResolvedRuntimeChildSpec {
            backend: RuntimeBackendId::GgmlLlama,
            grpc_bind_address: "127.0.0.1:50052".to_owned(),
            transport: RuntimeTransportMode::Http,
            queue_capacity: 64,
            backend_capacity: 4,
            lib_dir: Some(PathBuf::from("C:/runtime/libs")),
            log_level: None,
            log_json: None,
            log_file: PathBuf::from("C:/runtime/logs/slab-runtime.log"),
            shutdown_on_stdin_close: true,
        };

        let args = spec.command_args(Some("debug"), true);

        assert!(args.windows(2).any(|pair| pair == ["--enabled-backends", "ggml.llama"]));
        assert!(args.windows(2).any(|pair| pair == ["--grpc-bind", "127.0.0.1:50052"]));
        assert!(args.contains(&"--shutdown-on-stdin-close".to_owned()));
        assert!(args.contains(&"--log-json".to_owned()));
    }

    #[test]
    fn v2_managed_planner_uses_leaf_overrides() {
        let mut settings = SettingsDocumentV2::default();
        settings.logging.level = "warn".to_owned();
        settings.runtime.logging.level = Some("debug".to_owned());
        settings.runtime.ggml.logging.level = Some("trace".to_owned());
        settings.runtime.ggml.backends.llama.logging.level = Some("error".to_owned());
        settings.runtime.ggml.backends.llama.capacity.concurrent_requests = Some(1);
        settings.runtime.ggml.backends.whisper.enabled = false;
        settings.runtime.ggml.backends.diffusion.enabled = false;
        settings.runtime.transport = RuntimeTransportMode::Http;
        settings.runtime.ggml.install_dir = Some("D:/settings/backend-libs".to_owned());
        settings.runtime.ggml.backends.llama.endpoint.http.address =
            Some("127.0.0.1:4100".to_owned());

        let spec = resolve_launch_spec_v2(&settings, LaunchProfile::Server, &host_paths()).unwrap();

        assert_eq!(spec.children.len(), 1);
        assert_eq!(spec.children[0].backend, RuntimeBackendId::GgmlLlama);
        assert_eq!(spec.children[0].grpc_bind_address, "127.0.0.1:4100");
        assert_eq!(spec.children[0].backend_capacity, 1);
        assert_eq!(spec.children[0].log_level.as_deref(), Some("error"));
        assert_eq!(spec.children[0].lib_dir, Some(PathBuf::from("D:/settings/backend-libs")));
        assert_eq!(
            spec.gateway.as_ref().map(|gateway| gateway.bind_address.as_str()),
            Some("127.0.0.1:3000")
        );
    }

    #[test]
    fn v2_managed_planner_allows_gateway_only_startup() {
        let mut settings = SettingsDocumentV2::default();
        settings.runtime.ggml.backends.llama.enabled = false;
        settings.runtime.ggml.backends.whisper.enabled = false;
        settings.runtime.ggml.backends.diffusion.enabled = false;

        let spec = resolve_launch_spec_v2(&settings, LaunchProfile::Server, &host_paths()).unwrap();

        assert!(spec.children.is_empty());
        assert!(spec.endpoints.llama.is_none());
        assert!(spec.endpoints.whisper.is_none());
        assert!(spec.endpoints.diffusion.is_none());
        assert_eq!(
            spec.gateway.as_ref().map(|gateway| gateway.bind_address.as_str()),
            Some("127.0.0.1:3000")
        );
    }

    #[test]
    fn v2_external_planner_requires_explicit_endpoints() {
        let mut settings = SettingsDocumentV2::default();
        settings.runtime.mode = RuntimeMode::ExternalEndpoints;
        settings.runtime.transport = RuntimeTransportMode::Http;

        let error =
            resolve_launch_spec_v2(&settings, LaunchProfile::Server, &host_paths()).unwrap_err();
        assert!(error.to_string().contains("requires an explicit endpoint"));
    }

    #[test]
    fn v2_external_planner_uses_explicit_endpoints_without_children() {
        let mut settings = SettingsDocumentV2::default();
        settings.runtime.mode = RuntimeMode::ExternalEndpoints;
        settings.runtime.transport = RuntimeTransportMode::Http;
        settings.runtime.ggml.backends.llama.endpoint.http.address =
            Some("127.0.0.1:9101".to_owned());
        settings.runtime.ggml.backends.whisper.endpoint.http.address =
            Some("127.0.0.1:9102".to_owned());
        settings.runtime.ggml.backends.diffusion.endpoint.http.address =
            Some("127.0.0.1:9103".to_owned());

        let spec = resolve_launch_spec_v2(&settings, LaunchProfile::Server, &host_paths()).unwrap();

        assert!(spec.children.is_empty());
        assert_eq!(spec.endpoints.llama.as_deref(), Some("127.0.0.1:9101"));
        assert_eq!(spec.endpoints.whisper.as_deref(), Some("127.0.0.1:9102"));
        assert_eq!(spec.endpoints.diffusion.as_deref(), Some("127.0.0.1:9103"));
    }

    #[test]
    fn v2_external_planner_allows_zero_enabled_backends() {
        let mut settings = SettingsDocumentV2::default();
        settings.runtime.mode = RuntimeMode::ExternalEndpoints;
        settings.runtime.ggml.backends.llama.enabled = false;
        settings.runtime.ggml.backends.whisper.enabled = false;
        settings.runtime.ggml.backends.diffusion.enabled = false;

        let spec = resolve_launch_spec_v2(&settings, LaunchProfile::Server, &host_paths()).unwrap();

        assert!(spec.children.is_empty());
        assert!(spec.endpoints.llama.is_none());
        assert!(spec.endpoints.whisper.is_none());
        assert!(spec.endpoints.diffusion.is_none());
    }
}
