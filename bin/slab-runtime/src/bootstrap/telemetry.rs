use std::fs::OpenOptions;
use std::path::Path;

use tracing::{error, info};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::infra::config::RuntimeConfig;

pub(super) fn init_tracing(
    log_level: &str,
    log_json: bool,
    log_file: Option<&Path>,
) -> anyhow::Result<Vec<WorkerGuard>> {
    use tracing_subscriber::Layer;

    let env_filter = match tracing_subscriber::EnvFilter::try_from_default_env() {
        Ok(filter) => filter,
        Err(_) => match log_level.parse::<tracing_subscriber::EnvFilter>() {
            Ok(filter) => filter,
            Err(error) => {
                eprintln!("WARN: log level '{log_level}' is invalid ({error}); fallback to info");
                tracing_subscriber::EnvFilter::new("info")
            }
        },
    };

    let stdout_layer: Box<dyn Layer<_> + Send + Sync> = if log_json {
        Box::new(tracing_subscriber::fmt::layer().json().with_target(true).with_thread_ids(true))
    } else {
        Box::new(tracing_subscriber::fmt::layer().with_target(true).with_thread_ids(true))
    };

    let mut guards = Vec::new();
    let registry = tracing_subscriber::registry().with(env_filter).with(stdout_layer);

    if let Some(path) = log_file {
        if let Some(parent) = path.parent()
            && let Err(error) = std::fs::create_dir_all(parent)
        {
            eprintln!(
                "WARN: failed to create slab-runtime log directory '{}': {error}; continuing without file logging",
                parent.display()
            );
            registry.init();
            return Ok(guards);
        }

        match OpenOptions::new().create(true).append(true).open(path) {
            Ok(file) => {
                let (file_writer, guard) = tracing_appender::non_blocking(file);
                guards.push(guard);

                let file_layer: Box<dyn Layer<_> + Send + Sync> = if log_json {
                    Box::new(
                        tracing_subscriber::fmt::layer()
                            .json()
                            .with_ansi(false)
                            .with_target(true)
                            .with_thread_ids(true)
                            .with_writer(file_writer),
                    )
                } else {
                    Box::new(
                        tracing_subscriber::fmt::layer()
                            .with_ansi(false)
                            .with_target(true)
                            .with_thread_ids(true)
                            .with_writer(file_writer),
                    )
                };

                registry.with(file_layer).init();
            }
            Err(error) => {
                eprintln!(
                    "WARN: failed to open slab-runtime log file '{}': {error}; continuing without file logging",
                    path.display()
                );
                registry.init();
                return Ok(guards);
            }
        }
    } else {
        registry.init();
    }

    Ok(guards)
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

        eprintln!("slab-runtime panic at {location}: {payload}");
        error!(location = %location, payload = %payload, "slab-runtime panicked");
    }));
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
