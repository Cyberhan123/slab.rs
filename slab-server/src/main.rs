//! slab-server entry point.
//! Runs in supervisor mode by default.

mod api;
mod config;
mod context;
mod domain;
mod error;
mod infra;
mod model_auto_unload;

use std::{fs::OpenOptions, path::Path};

use clap::Parser;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::config::Config;
use crate::infra::supervisor::{SupervisorArgs, run as run_supervisor};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = SupervisorArgs::parse();
    let mut cfg = Config::from_env();
    args.apply_config_defaults(&mut cfg);

    let _log_guards = init_tracing(&cfg.log_level, cfg.log_json, cfg.log_file.as_deref())?;
    run_supervisor(args).await
}

fn init_tracing(
    log_level: &str,
    log_json: bool,
    log_file: Option<&Path>,
) -> anyhow::Result<Vec<WorkerGuard>> {
    let env_filter = match tracing_subscriber::EnvFilter::try_from_default_env() {
        Ok(f) => f,
        Err(_) => match log_level.parse::<tracing_subscriber::EnvFilter>() {
            Ok(f) => f,
            Err(e) => {
                eprintln!("WARN: log level '{}' is invalid ({}); fallback to info", log_level, e);
                tracing_subscriber::EnvFilter::new("info")
            }
        },
    };

    let mut guards = Vec::new();

    if let Some(path) = log_file {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                anyhow::anyhow!(
                    "failed to create slab-server log directory '{}': {error}",
                    parent.display()
                )
            })?;
        }

        let file = OpenOptions::new().create(true).append(true).open(path).map_err(|error| {
            anyhow::anyhow!("failed to open slab-server log file '{}': {error}", path.display())
        })?;
        let (file_writer, guard) = tracing_appender::non_blocking(file);
        guards.push(guard);

        if log_json {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    tracing_subscriber::fmt::layer().json().with_target(true).with_thread_ids(true),
                )
                .with(
                    tracing_subscriber::fmt::layer()
                        .json()
                        .with_ansi(false)
                        .with_target(true)
                        .with_thread_ids(true)
                        .with_writer(file_writer),
                )
                .init();
        } else {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(tracing_subscriber::fmt::layer().with_target(true).with_thread_ids(true))
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_ansi(false)
                        .with_target(true)
                        .with_thread_ids(true)
                        .with_writer(file_writer),
                )
                .init();
        }
    } else if log_json {
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

    Ok(guards)
}
