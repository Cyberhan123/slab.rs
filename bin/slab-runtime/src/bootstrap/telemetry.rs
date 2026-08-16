use std::io::Write;
use std::path::{Path, PathBuf};

use slab_otel::config::{OtelExporter, OtelSettings};
use tracing::{error, info, warn};
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{Layer, fmt};

use crate::infra::config::RuntimeConfig;

/// Holds the tracing guards that must outlive the process so the non-blocking
/// file writer flushes on shutdown and the OTel provider stays installed.
#[derive(Debug)]
pub(super) struct RuntimeTracing {
    _otel: Option<slab_otel::OtelProvider>,
    _file_guard: Option<WorkerGuard>,
}

struct FileLogging {
    writer: NonBlocking,
    guard: WorkerGuard,
}

pub(super) fn init_tracing(
    log_level: &str,
    log_json: bool,
    log_file: Option<&Path>,
) -> anyhow::Result<RuntimeTracing> {
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

    let mut settings = telemetry_settings(log_file);
    let file_log_path = effective_file_log_path(log_file, &settings);
    if file_log_path.is_some() {
        // The rotating redacting fmt layer replaces the OTel log exporter; keep
        // `trace_exporter` (spans) untouched so traces still land on disk.
        settings.exporter = OtelExporter::None;
    }
    let file_logging = file_log_path.map(init_file_logging).transpose()?;

    let provider = match slab_otel::OtelProvider::from(&settings) {
        Ok(provider) => provider,
        Err(error) => {
            bootstrap_warnings.push(format!(
                "failed to initialize slab-runtime OpenTelemetry provider: {error}; continuing with console logging"
            ));
            None
        }
    };

    match (provider.as_ref(), file_logging.as_ref()) {
        (Some(provider), Some(file_logging)) => tracing_subscriber::registry()
            .with(env_filter)
            .with(provider.tracing_layer())
            .with(redacted_fmt_layer(file_logging.writer.clone(), log_json))
            .init(),
        (Some(provider), None) => tracing_subscriber::registry()
            .with(env_filter)
            .with(provider.logger_layer())
            .with(provider.tracing_layer())
            .init(),
        (None, Some(file_logging)) => tracing_subscriber::registry()
            .with(env_filter)
            .with(console_fmt_layer(log_json))
            .with(redacted_fmt_layer(file_logging.writer.clone(), log_json))
            .init(),
        (None, None) => init_console_tracing(env_filter, log_json),
    }

    emit_bootstrap_warnings(&mut bootstrap_warnings, None::<String>);
    Ok(RuntimeTracing {
        _otel: provider,
        _file_guard: file_logging.map(|file_logging| file_logging.guard),
    })
}

fn telemetry_settings(log_file: Option<&Path>) -> OtelSettings {
    let mut settings = OtelSettings::default_for_service("slab-runtime");
    settings.service_version = Some(env!("CARGO_PKG_VERSION").to_owned());
    if let Some(log_file) = log_file
        && let Some(parent) = log_file.parent()
    {
        settings.exporter = OtelExporter::LocalFile { directory: parent.to_path_buf() };
        settings.trace_exporter = OtelExporter::LocalFile { directory: parent.to_path_buf() };
    }
    settings
}

fn effective_file_log_path(log_file: Option<&Path>, settings: &OtelSettings) -> Option<PathBuf> {
    log_file.map(Path::to_path_buf).or_else(|| {
        settings.exporter.local_directory().map(|directory| directory.join("slab-runtime.log"))
    })
}

fn init_file_logging(path: PathBuf) -> anyhow::Result<FileLogging> {
    let writer = slab_utils::log::RedactingSizeRotatingWriter::new(
        path,
        slab_utils::log::DEFAULT_MAX_LOG_BYTES,
        slab_utils::log::DEFAULT_MAX_LOG_FILES,
    )?;
    let (writer, guard) = tracing_appender::non_blocking(writer);
    Ok(FileLogging { writer, guard })
}

fn console_fmt_layer<S>(log_json: bool) -> Box<dyn Layer<S> + Send + Sync + 'static>
where
    S: tracing::Subscriber + for<'span> LookupSpan<'span> + Send + Sync + 'static,
{
    if log_json {
        Box::new(fmt::layer().json().with_target(true).with_thread_ids(true))
    } else {
        Box::new(fmt::layer().with_target(true).with_thread_ids(true))
    }
}

fn redacted_fmt_layer<S>(
    writer: NonBlocking,
    log_json: bool,
) -> Box<dyn Layer<S> + Send + Sync + 'static>
where
    S: tracing::Subscriber + for<'span> LookupSpan<'span> + Send + Sync + 'static,
{
    if log_json {
        Box::new(
            fmt::layer()
                .json()
                .with_ansi(false)
                .with_target(true)
                .with_thread_ids(true)
                .with_writer(writer),
        )
    } else {
        Box::new(
            fmt::layer()
                .with_ansi(false)
                .with_target(true)
                .with_thread_ids(true)
                .with_writer(writer),
        )
    }
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
