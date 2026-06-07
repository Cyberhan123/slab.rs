use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub grpc_bind: String,
    pub log_level: String,
    pub log_json: bool,
    pub queue_capacity: usize,
    pub backend_capacity: usize,
    pub base_lib_path: PathBuf,
    pub log_file: Option<PathBuf>,
    pub enabled_backends: CliEnabledBackends,
    pub shutdown_on_stdin_close: bool,
    pub llama_lib_dir: Option<PathBuf>,
    pub whisper_lib_dir: Option<PathBuf>,
    pub diffusion_lib_dir: Option<PathBuf>,
    pub enable_candle_llama: bool,
    pub enable_candle_whisper: bool,
    pub enable_candle_diffusion: bool,
    pub onnx_enabled: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct CliEnabledBackends {
    pub llama: bool,
    pub whisper: bool,
    pub diffusion: bool,
}

impl CliEnabledBackends {
    pub fn all() -> Self {
        Self { llama: true, whisper: true, diffusion: true }
    }
}

impl std::fmt::Display for CliEnabledBackends {
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

pub(crate) fn resolve_base_lib_path(path: PathBuf, current_dir: &Path) -> PathBuf {
    let absolute = if path.is_absolute() { path } else { current_dir.join(path) };
    absolute.canonicalize().unwrap_or(absolute)
}

#[cfg(test)]
mod tests {
    use super::resolve_base_lib_path;
    use std::path::{Path, PathBuf};

    #[test]
    fn resolve_base_lib_path_makes_relative_paths_absolute() {
        let cwd = Path::new("C:/slab/bin/slab-app/src-tauri");
        let resolved = resolve_base_lib_path(PathBuf::from("./resources/libs"), cwd);

        assert!(resolved.is_absolute());
        assert_eq!(resolved, cwd.join("resources").join("libs"));
    }

    #[test]
    fn resolve_base_lib_path_keeps_absolute_paths_stable() {
        let cwd = Path::new("C:/slab/bin/slab-app/src-tauri");
        let input = PathBuf::from("C:/slab/vendor/runtime/libs");
        let resolved = resolve_base_lib_path(input.clone(), cwd);

        assert_eq!(resolved, input);
    }
}
