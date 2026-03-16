use libloading::Library;
use std::fmt;
use std::path::Path;
use std::sync::Arc;

mod common_logging;
mod error;
mod ggml_logging_hook;
mod standalone;
mod utilities;
mod whisper_ctx;
mod whisper_ctx_wrapper;
mod whisper_grammar;
mod whisper_logging_hook;
mod whisper_params;
mod whisper_state;
mod whisper_vad;

pub use common_logging::GGMLLogLevel;
pub use error::WhisperError;
pub use utilities::*;
pub use whisper_ctx::DtwMode;
pub use whisper_ctx::DtwModelPreset;
pub use whisper_ctx::DtwParameters;
pub use whisper_ctx::WhisperContextParameters;
use whisper_ctx::WhisperInnerContext;
pub use whisper_ctx_wrapper::WhisperContext;
pub use whisper_grammar::{WhisperGrammarElement, WhisperGrammarElementType};
pub use whisper_params::{FullParams, SamplingStrategy, SegmentCallbackData};

pub use whisper_state::{WhisperSegment, WhisperState, WhisperStateSegmentIterator, WhisperToken};
pub use whisper_vad::*;

pub type WhisperSysContext = slab_whisper_sys::whisper_context;
pub type WhisperSysState = slab_whisper_sys::whisper_state;

pub type WhisperTokenData = slab_whisper_sys::whisper_token_data;
pub type WhisperTokenId = slab_whisper_sys::whisper_token;
pub type WhisperNewSegmentCallback = slab_whisper_sys::whisper_new_segment_callback;
pub type WhisperStartEncoderCallback = slab_whisper_sys::whisper_encoder_begin_callback;
pub type WhisperProgressCallback = slab_whisper_sys::whisper_progress_callback;
pub type WhisperLogitsFilterCallback = slab_whisper_sys::whisper_logits_filter_callback;
pub type WhisperAbortCallback = slab_whisper_sys::ggml_abort_callback;
pub type WhisperLogCallback = slab_whisper_sys::ggml_log_callback;
pub type DtwAhead = slab_whisper_sys::whisper_ahead;

#[derive(Clone)]
pub struct Whisper {
    lib: Arc<slab_whisper_sys::WhisperLib>,
    // Keep ggml.dll loaded when backend symbols are resolved from it.
    _ggml_lib: Option<Arc<Library>>,
}

fn load_ggml_backend(
    path: &Path,
    whisper_lib: &slab_whisper_sys::WhisperLib,
) -> Result<Option<Arc<Library>>, WhisperError> {
    let whisper_dir_path = path
        .parent()
        .ok_or(WhisperError::InitBackendError("Invalid path".to_string()))?;
    if !whisper_dir_path.is_dir() {
        return Err(WhisperError::InitBackendError(format!(
            "Whisper directory does not exist: {}",
            whisper_dir_path.display()
        )));
    }
    use std::ffi::CString;
    let c_str = CString::new(
        whisper_dir_path
            .to_str()
            .ok_or(WhisperError::InitBackendError("Invalid path".to_string()))?,
    )?;

    if let Ok(ggml_backend_load_all_from_path) =
        whisper_lib.ggml_backend_load_all_from_path.as_ref()
    {
        unsafe { ggml_backend_load_all_from_path(c_str.as_ptr()) };

        let reg_count = whisper_lib
            .ggml_backend_reg_count
            .as_ref()
            .ok()
            .map(|f| unsafe { f() });
        let dev_count = whisper_lib
            .ggml_backend_dev_count
            .as_ref()
            .ok()
            .map(|f| unsafe { f() });

        if matches!((reg_count, dev_count), (Some(0), Some(0))) {
            return Err(WhisperError::InitBackendError(format!(
                "No GGML backends/devices were registered from directory: {}. Ensure ggml-*.dll files match whisper.dll version.",
                whisper_dir_path.display()
            )));
        }
        return Ok(None);
    }

    #[cfg(windows)]
    {
        use libloading::os::windows::{
            Library as WinLibrary, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
        };
        use std::env::consts::{DLL_PREFIX, DLL_SUFFIX};
        let ggml_path = whisper_dir_path.join(format!("{}ggml{}", DLL_PREFIX, DLL_SUFFIX));
        let ggml_lib: libloading::Library = unsafe {
            WinLibrary::load_with_flags(
                ggml_path.as_path(),
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
            )?
            .into()
        };

        let ggml_backend_load_all_from_path: unsafe extern "C" fn(
            dir_path: *const ::std::os::raw::c_char,
        ) = unsafe {
            *ggml_lib
                .get(b"ggml_backend_load_all_from_path\0")
                .map_err(|e| {
                    WhisperError::InitBackendError(format!(
                        "Missing ggml symbol ggml_backend_load_all_from_path: {}",
                        e
                    ))
                })?
        };
        let ggml_backend_reg_count: unsafe extern "C" fn() -> usize = unsafe {
            *ggml_lib.get(b"ggml_backend_reg_count\0").map_err(|e| {
                WhisperError::InitBackendError(format!(
                    "Missing ggml symbol ggml_backend_reg_count: {}",
                    e
                ))
            })?
        };
        let ggml_backend_dev_count: unsafe extern "C" fn() -> usize = unsafe {
            *ggml_lib.get(b"ggml_backend_dev_count\0").map_err(|e| {
                WhisperError::InitBackendError(format!(
                    "Missing ggml symbol ggml_backend_dev_count: {}",
                    e
                ))
            })?
        };

        unsafe { ggml_backend_load_all_from_path(c_str.as_ptr()) };
        let reg_count = unsafe { ggml_backend_reg_count() };
        let dev_count = unsafe { ggml_backend_dev_count() };
        if reg_count == 0 && dev_count == 0 {
            return Err(WhisperError::InitBackendError(format!(
                "No GGML backends/devices were registered from directory: {}. Ensure ggml-*.dll files match whisper.dll version.",
                whisper_dir_path.display()
            )));
        }
        Ok(Some(Arc::new(ggml_lib)))
    }

    #[cfg(not(windows))]
    {
        Err(WhisperError::InitBackendError(
            "Missing ggml_backend_load_all_from_path in whisper library and ggml fallback is only implemented on Windows.".to_string(),
        ))
    }
}

impl Whisper {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, WhisperError> {
        #[cfg(windows)]
        {
            use libloading::os::windows::{
                Library, LOAD_LIBRARY_SEARCH_APPLICATION_DIR, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
            };
            let lib = unsafe {
                Library::load_with_flags(
                    path.as_ref(),
                    LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR
                        | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS
                        | LOAD_LIBRARY_SEARCH_APPLICATION_DIR,
                )?
            };

            let whisper_lib = unsafe { slab_whisper_sys::WhisperLib::from_library(lib)? };
            let ggml_lib = load_ggml_backend(path.as_ref(), &whisper_lib)?;

            Ok(Self {
                lib: Arc::new(whisper_lib),
                _ggml_lib: ggml_lib,
            })
        }

        #[cfg(not(windows))]
        {
            let raw_lib = unsafe { libloading::Library::new(path.as_ref())? };
            let lib = unsafe { slab_whisper_sys::WhisperLib::from_library(raw_lib)? };
            let ggml_lib = load_ggml_backend(path.as_ref(), &lib)?;
            Ok(Self {
                lib: Arc::new(lib),
                _ggml_lib: ggml_lib,
            })
        }
    }

    /// Redirect all whisper.cpp and GGML logs to logging hooks installed by whisper-rs.
    ///
    /// This will stop most logs from being output to stdout/stderr and will bring them into
    /// `log` or `tracing`, if the `log_backend` or `tracing_backend` features, respectively,
    /// are enabled. If neither is enabled, this will essentially disable logging, as they won't
    /// be output anywhere.
    ///
    /// Note whisper.cpp and GGML do not reliably follow Rust logging conventions.
    /// Use your logging crate's configuration to control how these logs will be output.
    /// whisper-rs does not currently output any logs, but this may change in the future.
    /// You should configure by module path and use `whisper_rs::ggml_logging_hook`,
    /// and/or `whisper_rs::whisper_logging_hook`, to avoid possibly ignoring useful
    /// `whisper-rs` logs in the future.
    ///
    /// Safe to call multiple times. Only has an effect the first time.
    /// (note this means installing your own logging handlers with unsafe functions after this call
    /// is permanent and cannot be undone)
    pub fn install_logging_hooks(&self) {
        self.install_whisper_logging_hook();
        self.install_ggml_logging_hook();
    }
}

impl fmt::Debug for Whisper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Whisper").finish()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::env;
    use std::env::consts::{DLL_PREFIX, DLL_SUFFIX};
    #[test]
    fn test_load_library() {
        match env::current_exe() {
            Ok(exe_path) => {
                let whisper_lib_name = format!("{}whisper{}", DLL_PREFIX, DLL_SUFFIX);
                let dir = exe_path
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join("resources\\lib\\whisper");
                let whisper_dll_path = dir.join(&whisper_lib_name);
                println!("可执行文件目录: {:?}", dir);
                println!("Whisper DLL 路径: {:?}", whisper_dll_path);
                match Whisper::new(whisper_dll_path) {
                    Ok(_) => println!("成功加载 Whisper 库！"),
                    Err(e) => println!("加载 Whisper 库失败: {}", e),
                }
            }
            Err(e) => println!("获取路径失败: {}", e),
        };
    }
}
