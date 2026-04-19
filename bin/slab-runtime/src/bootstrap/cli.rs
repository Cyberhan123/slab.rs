use std::path::{Path, PathBuf};

use anyhow::Context;
use clap::Parser;

use crate::infra::config::{RuntimeConfig, parse_enabled_backends, resolve_base_lib_path};

#[derive(Parser, Debug, Clone)]
#[command(name = "slab-runtime", version, about = "Slab gRPC runtime worker")]
pub struct Cli {
    #[arg(long = "grpc-bind", default_value = "127.0.0.1:50051")]
    pub grpc_bind: String,
    #[arg(long = "log")]
    pub log_level: Option<String>,
    #[arg(long = "log-json", action = clap::ArgAction::SetTrue)]
    pub log_json: bool,
    #[arg(long = "queue-capacity")]
    pub queue_capacity: Option<usize>,
    #[arg(long = "backend-capacity")]
    pub backend_capacity: Option<usize>,
    #[arg(long = "lib-dir")]
    pub lib_dir: Option<PathBuf>,
    #[arg(long = "log-file")]
    pub log_file: Option<PathBuf>,
    #[arg(long = "enabled-backends")]
    pub enabled_backends: Option<String>,
    #[arg(long, default_value_t = false)]
    pub shutdown_on_stdin_close: bool,
}

impl Cli {
    pub fn parse() -> Self {
        <Self as Parser>::parse()
    }

    pub fn into_runtime_config(self) -> anyhow::Result<RuntimeConfig> {
        let enabled_backends = parse_enabled_backends(self.enabled_backends.as_deref())?;
        let current_dir =
            std::env::current_dir().context("failed to resolve slab-runtime current directory")?;
        let base_lib_path = resolve_base_lib_path(
            self.lib_dir.unwrap_or_else(|| Path::new("./resources/libs").to_path_buf()),
            &current_dir,
        );
        let llama_lib_dir = enabled_backends.llama.then(|| base_lib_path.clone());
        let whisper_lib_dir = enabled_backends.whisper.then(|| base_lib_path.clone());
        let diffusion_lib_dir = enabled_backends.diffusion.then(|| base_lib_path.clone());

        Ok(RuntimeConfig {
            grpc_bind: self.grpc_bind,
            log_level: self.log_level.unwrap_or_else(|| "info".to_owned()),
            log_json: self.log_json,
            queue_capacity: self.queue_capacity.unwrap_or(64),
            backend_capacity: self.backend_capacity.unwrap_or(4),
            base_lib_path,
            log_file: self.log_file,
            enabled_backends,
            shutdown_on_stdin_close: self.shutdown_on_stdin_close,
            llama_lib_dir,
            whisper_lib_dir,
            diffusion_lib_dir,
            enable_candle_llama: false,
            enable_candle_whisper: false,
            enable_candle_diffusion: false,
            onnx_enabled: false,
        })
    }
}
