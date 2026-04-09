use std::path::{Path, PathBuf};

use clap::Parser;

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

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub grpc_bind: String,
    pub log_level: String,
    pub log_json: bool,
    pub queue_capacity: usize,
    pub backend_capacity: usize,
    pub base_lib_path: PathBuf,
    pub log_file: Option<PathBuf>,
    pub enabled_backends: EnabledBackends,
    pub shutdown_on_stdin_close: bool,
    pub llama_lib_dir: Option<PathBuf>,
    pub whisper_lib_dir: Option<PathBuf>,
    pub diffusion_lib_dir: Option<PathBuf>,
    pub enable_candle_llama: bool,
    pub enable_candle_whisper: bool,
    pub enable_candle_diffusion: bool,
    pub onnx_enabled: bool,
}

impl Cli {
    pub fn into_runtime_config(self) -> anyhow::Result<RuntimeConfig> {
        let enabled_backends = parse_enabled_backends(self.enabled_backends.as_deref())?;
        let base_lib_path =
            self.lib_dir.unwrap_or_else(|| Path::new("./resources/libs").to_path_buf());
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

#[derive(Debug, Clone, Copy)]
pub struct EnabledBackends {
    pub llama: bool,
    pub whisper: bool,
    pub diffusion: bool,
}

impl EnabledBackends {
    pub fn all() -> Self {
        Self { llama: true, whisper: true, diffusion: true }
    }
}

impl std::fmt::Display for EnabledBackends {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for name in [
            self.llama.then_some("llama"),
            self.whisper.then_some("whisper"),
            self.diffusion.then_some("diffusion"),
        ]
        .into_iter()
        .flatten()
        {
            if !first {
                f.write_str(",")?;
            }
            f.write_str(name)?;
            first = false;
        }
        Ok(())
    }
}

pub fn parse_enabled_backends(raw: Option<&str>) -> anyhow::Result<EnabledBackends> {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(EnabledBackends::all());
    };

    let mut enabled = EnabledBackends { llama: false, whisper: false, diffusion: false };
    let mut unknown = Vec::new();

    for token in raw.split(',').map(str::trim).filter(|value| !value.is_empty()) {
        match token.to_ascii_lowercase().as_str() {
            "llama" | "ggml.llama" => enabled.llama = true,
            "whisper" | "ggml.whisper" => enabled.whisper = true,
            "diffusion" | "ggml.diffusion" => enabled.diffusion = true,
            other => unknown.push(other.to_string()),
        }
    }

    if !unknown.is_empty() {
        anyhow::bail!(
            "invalid enabled backends: {}. Supported: llama, whisper, diffusion",
            unknown.join(", ")
        );
    }
    if !enabled.llama && !enabled.whisper && !enabled.diffusion {
        anyhow::bail!("at least one backend must be enabled");
    }

    Ok(enabled)
}
