use std::path::PathBuf;

#[derive(Debug, Clone)]
pub(crate) struct FfmpegRuntimeProbe {
    pub installed: bool,
    pub version: Option<String>,
    pub binary: PathBuf,
}

pub(crate) fn resolve_ffmpeg_binary(configured_install_dir: Option<&str>) -> PathBuf {
    if let Some(path) = std::env::var_os("SLAB_FFMPEG_BIN") {
        let trimmed = path.to_string_lossy().trim().to_owned();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    if let Some(path) = resolve_from_install_dir(configured_install_dir) {
        return path;
    }

    if let Some(path) = resolve_from_env_dir() {
        return path;
    }

    if let Some(path) = resolve_from_ffmpeg_dir_env() {
        return path;
    }

    if let Some(path) = resolve_from_bundle_resources() {
        return path;
    }

    PathBuf::from("ffmpeg")
}

pub(crate) fn probe_ffmpeg_runtime(configured_install_dir: Option<&str>) -> FfmpegRuntimeProbe {
    let binary = resolve_ffmpeg_binary(configured_install_dir);

    let installed = ffmpeg_dynamic_runtime_ready();
    let version = installed.then_some("ffmpeg-next(static-runtime)".to_owned());

    FfmpegRuntimeProbe { installed, version, binary }
}

pub(crate) fn ensure_dynamic_runtime_ready() -> Result<(), String> {
    if ffmpeg_dynamic_runtime_ready() {
        return Ok(());
    }

    Err("ffmpeg-next static runtime initialization failed".to_owned())
}

fn resolve_from_env_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("SLAB_FFMPEG_DIR")?;
    let trimmed = dir.to_string_lossy().trim().to_owned();
    if trimmed.is_empty() {
        return None;
    }

    find_ffmpeg_binary(PathBuf::from(trimmed).as_path())
}

fn resolve_from_ffmpeg_dir_env() -> Option<PathBuf> {
    let dir = std::env::var_os("FFMPEG_DIR")?;
    let ffmpeg_dir = PathBuf::from(dir);
    if !ffmpeg_dir.exists() {
        return None;
    }

    find_ffmpeg_binary(ffmpeg_dir.as_path())
        .or_else(|| find_ffmpeg_binary(ffmpeg_dir.join("bin").as_path()))
}

fn resolve_from_install_dir(configured_install_dir: Option<&str>) -> Option<PathBuf> {
    let install_dir = configured_install_dir?.trim();
    if install_dir.is_empty() {
        return None;
    }

    find_ffmpeg_binary(PathBuf::from(install_dir).as_path())
}

fn find_ffmpeg_binary(dir: &std::path::Path) -> Option<PathBuf> {
    let candidates = if cfg!(target_os = "windows") {
        [dir.join("ffmpeg.exe"), dir.join("bin").join("ffmpeg.exe")]
    } else {
        [dir.join("ffmpeg"), dir.join("bin").join("ffmpeg")]
    };

    candidates.into_iter().find(|path| path.exists())
}

fn resolve_from_bundle_resources() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;
    let mut candidates = vec![exe_dir.join("resources").join("libs")];
    if let Some(parent) = exe_dir.parent() {
        candidates.push(parent.join("Resources").join("libs"));
    }

    candidates
        .iter()
        .find(|candidate| candidate.exists())
        .and_then(|candidate| find_ffmpeg_binary(candidate.as_path()))
}

#[cfg(feature = "ffmpeg-next-runtime")]
fn ffmpeg_dynamic_runtime_ready() -> bool {
    ffmpeg_next::init().is_ok()
}

#[cfg(not(feature = "ffmpeg-next-runtime"))]
fn ffmpeg_dynamic_runtime_ready() -> bool {
    true
}
