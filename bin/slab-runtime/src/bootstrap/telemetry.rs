use std::io::Write;
use std::path::Path;

use tracing::{error, info, warn};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::infra::config::RuntimeConfig;

pub(super) fn init_tracing(
    log_level: &str,
    log_json: bool,
    log_file: Option<&Path>,
) -> anyhow::Result<Option<slab_otel::OtelProvider>> {
    let mut bootstrap_warnings = Vec::new();
    let env_filter = match tracing_subscriber::EnvFilter::try_from_default_env() {
        Ok(filter) => filter,
        Err(_) => match log_level.parse::<tracing_subscriber::EnvFilter>() {
            Ok(filter) => filter,
            Err(error) => {
                bootstrap_warnings.push(format!(
                    "log level '{log_level}' is invalid ({error}); fallback to info"
                ));
                tracing_subscriber::EnvFilter::new("info")
            }
        },
    };

    slab_otel::provider::install_log_bridge();
    let settings = telemetry_settings(log_file);
    let provider = match slab_otel::OtelProvider::from(&settings) {
        Ok(provider) => provider,
        Err(error) => {
            bootstrap_warnings.push(format!(
                "failed to initialize slab-runtime OpenTelemetry provider: {error}; continuing with console logging"
            ));
            None
        }
    };

    if let Some(provider) = provider {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(provider.logger_layer())
            .with(provider.tracing_layer())
            .init();
        emit_bootstrap_warnings(&mut bootstrap_warnings, None::<String>);
        Ok(Some(provider))
    } else {
        init_console_tracing(env_filter, log_json);
        emit_bootstrap_warnings(&mut bootstrap_warnings, None::<String>);
        Ok(None)
    }
}

fn telemetry_settings(log_file: Option<&Path>) -> slab_otel::config::OtelSettings {
    let mut settings = slab_otel::config::OtelSettings::default_for_service("slab-runtime");
    settings.service_version = Some(env!("CARGO_PKG_VERSION").to_owned());
    if let Some(log_file) = log_file
        && let Some(parent) = log_file.parent()
    {
        settings.exporter =
            slab_otel::config::OtelExporter::LocalFile { directory: parent.to_path_buf() };
        settings.trace_exporter =
            slab_otel::config::OtelExporter::LocalFile { directory: parent.to_path_buf() };
    }
    settings
}

fn init_console_tracing(env_filter: tracing_subscriber::EnvFilter, log_json: bool) {
    if log_json {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().json().with_target(true).with_thread_ids(true))
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().with_target(true).with_thread_ids(true))
            .init();
    }
}

pub(super) fn install_panic_hook() {
    std::panic::set_hook(Box::new(|panic_info| {
        let location = panic_info
            .location()
            .map(|location| {
                format!("{}:{}:{}", location.file(), location.line(), location.column())
            })
            .unwrap_or_else(|| "<unknown>".to_string());
        let payload = if let Some(msg) = panic_info.payload().downcast_ref::<&str>() {
            (*msg).to_string()
        } else if let Some(msg) = panic_info.payload().downcast_ref::<String>() {
            msg.clone()
        } else {
            "non-string panic payload".to_string()
        };

        write_bootstrap_stderr(&format!("slab-runtime panic at {location}: {payload}"));
        error!(location = %location, payload = %payload, "slab-runtime panicked");
    }));
}

fn emit_bootstrap_warnings<T>(warnings: &mut Vec<String>, extra: Option<T>)
where
    T: Into<String>,
{
    if let Some(extra_warning) = extra {
        warnings.push(extra_warning.into());
    }

    for warning_message in warnings.drain(..) {
        warn!(warning = %warning_message, "slab-runtime bootstrap warning");
    }
}

fn write_bootstrap_stderr(message: &str) {
    let mut stderr = std::io::stderr().lock();
    let _ = writeln!(stderr, "{message}");
}

pub(super) fn log_startup(config: &RuntimeConfig) {
    info!(
        pid = std::process::id(),
        grpc_bind = %config.grpc_bind,
        enabled_backends = %config.enabled_backends,
        shutdown_on_stdin_close = config.shutdown_on_stdin_close,
        base_lib_path = %config.base_lib_path.display(),
        log_file = ?config.log_file.as_ref().map(|path| path.display().to_string()),
        current_dir = ?std::env::current_dir().ok(),
        current_exe = ?std::env::current_exe().ok(),
        "slab-runtime starting"
    );
    if let Some(path) = &config.llama_lib_dir {
        info!(backend = "llama", lib_dir = %path.display(), exists = path.exists(), "resolved runtime backend library directory");
    }
    if let Some(path) = &config.whisper_lib_dir {
        info!(backend = "whisper", lib_dir = %path.display(), exists = path.exists(), "resolved runtime backend library directory");
    }
    if let Some(path) = &config.diffusion_lib_dir {
        info!(backend = "diffusion", lib_dir = %path.display(), exists = path.exists(), "resolved runtime backend library directory");
    }
    info!(
        queue_capacity = config.queue_capacity,
        backend_capacity = config.backend_capacity,
        "initializing slab-core runtime"
    );
}
