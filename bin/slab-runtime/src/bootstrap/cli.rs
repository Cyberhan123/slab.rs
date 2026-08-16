use std::path::{Path, PathBuf};

use anyhow::Context;
use clap::{Parser, ValueEnum};

use crate::infra::config::{CliEnabledBackends, RuntimeConfig, resolve_base_lib_path};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum EnabledBackendArg {
    #[value(alias = "ggml.llama")]
    Llama,
    #[value(alias = "ggml.whisper")]
    Whisper,
    #[value(alias = "ggml.parakeet")]
    Parakeet,
    #[value(alias = "ggml.diffusion")]
    Diffusion,
    #[value(name = "candle.llama", alias = "candle-llama")]
    CandleLlama,
    #[value(name = "candle.whisper", alias = "candle-whisper")]
    CandleWhisper,
    #[value(name = "candle.diffusion", alias = "candle-diffusion")]
    CandleDiffusion,
}

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
    /// Root directory for the on-disk ggml.llama kv-cache. Defaults to
    /// `<runtime-home>/kv-cache` (two levels above the lib dir). Pass a path to
    /// override; there is no explicit disable flag (point it at a read-only path
    /// and persistence degrades best-effort to in-process caching).
    #[arg(long = "kv-cache-dir")]
    pub kv_cache_dir: Option<PathBuf>,
    #[arg(long = "enabled-backends", value_enum, value_delimiter = ',', ignore_case = true)]
    enabled_backends: Vec<EnabledBackendArg>,
    #[arg(long, default_value_t = false)]
    pub shutdown_on_stdin_close: bool,
}

impl Cli {
    pub fn parse() -> Self {
        <Self as Parser>::parse()
    }

    pub fn into_runtime_config(self) -> anyhow::Result<RuntimeConfig> {
        let enabled_backends = if self.enabled_backends.is_empty() {
            CliEnabledBackends::all()
        } else {
            let mut enabled = CliEnabledBackends {
                llama: false,
                whisper: false,
                parakeet: false,
                diffusion: false,
                candle_llama: false,
                candle_whisper: false,
                candle_diffusion: false,
            };
            for backend in self.enabled_backends {
                match backend {
                    EnabledBackendArg::Llama => enabled.llama = true,
                    EnabledBackendArg::Whisper => enabled.whisper = true,
                    EnabledBackendArg::Parakeet => enabled.parakeet = true,
                    EnabledBackendArg::Diffusion => enabled.diffusion = true,
                    EnabledBackendArg::CandleLlama => enabled.candle_llama = true,
                    EnabledBackendArg::CandleWhisper => enabled.candle_whisper = true,
                    EnabledBackendArg::CandleDiffusion => enabled.candle_diffusion = true,
                }
            }
            enabled
        };
        let current_dir =
            std::env::current_dir().context("failed to resolve slab-runtime current directory")?;
        let base_lib_path = resolve_base_lib_path(
            self.lib_dir.unwrap_or_else(|| Path::new("./resources/libs").to_path_buf()),
            &current_dir,
        );
        let llama_lib_dir = enabled_backends.llama.then(|| base_lib_path.clone());
        let whisper_lib_dir = enabled_backends.whisper.then(|| base_lib_path.clone());
        let parakeet_lib_dir = enabled_backends.parakeet.then(|| base_lib_path.clone());
        let diffusion_lib_dir = enabled_backends.diffusion.then(|| base_lib_path.clone());
        // Default kv-cache root = the canonical app home's `kv-cache` dir
        // (slab-utils app_home), co-located with the DB/settings/models/logs.
        // Overridable via --kv-cache-dir. (slab-utils is a ggml-gated dep, so the
        // default only resolves when the llama backend is available.)
        let kv_cache_dir = self.kv_cache_dir.or_else(default_kv_cache_dir);

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
            parakeet_lib_dir,
            diffusion_lib_dir,
            kv_cache_dir,
            enable_candle_llama: enabled_backends.candle_llama,
            enable_candle_whisper: enabled_backends.candle_whisper,
            enable_candle_diffusion: enabled_backends.candle_diffusion,
            onnx_enabled: false,
        })
    }
}

/// Default kv-cache root, sourced from the canonical app home (slab-utils).
/// Lives under `sessions/kv-cache` because kv-cache snapshots are keyed per
/// thread/session (`agent:{thread_id}`). Only resolves when the ggml/llama
/// backend is built in (slab-utils is a ggml-gated dep); otherwise off.
#[cfg(feature = "ggml")]
fn default_kv_cache_dir() -> Option<PathBuf> {
    Some(slab_utils::app_home::sessions_dir().join("kv-cache"))
}

#[cfg(not(feature = "ggml"))]
fn default_kv_cache_dir() -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::Parser;

    #[test]
    fn runtime_config_defaults_to_all_backends() {
        let cli = <Cli as Parser>::try_parse_from(["slab-runtime"]).expect("parse cli");
        let config = cli.into_runtime_config().expect("build runtime config");

        assert!(config.enabled_backends.llama);
        assert!(config.enabled_backends.whisper);
        assert!(config.enabled_backends.diffusion);
        assert!(!config.enabled_backends.candle_llama);
    }

    #[test]
    fn runtime_config_accepts_legacy_backend_aliases() {
        let cli = <Cli as Parser>::try_parse_from([
            "slab-runtime",
            "--enabled-backends",
            "ggml.llama,whisper",
        ])
        .expect("parse cli");
        let config = cli.into_runtime_config().expect("build runtime config");

        assert!(config.enabled_backends.llama);
        assert!(config.enabled_backends.whisper);
        assert!(!config.enabled_backends.diffusion);
    }

    #[test]
    fn runtime_config_accepts_candle_backend_ids() {
        let cli = <Cli as Parser>::try_parse_from([
            "slab-runtime",
            "--enabled-backends",
            "candle.llama,candle.whisper,candle.diffusion",
        ])
        .expect("parse cli");
        let config = cli.into_runtime_config().expect("build runtime config");

        assert!(!config.enabled_backends.llama);
        assert!(config.enabled_backends.candle_llama);
        assert!(config.enabled_backends.candle_whisper);
        assert!(config.enabled_backends.candle_diffusion);
        assert!(config.enable_candle_llama);
        assert!(config.enable_candle_whisper);
        assert!(config.enable_candle_diffusion);
    }
}
