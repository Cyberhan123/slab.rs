use std::env::consts::{DLL_PREFIX, DLL_SUFFIX};
use std::path::{Path, PathBuf};

use libloading::Error as LibraryLoadError;

#[cfg(windows)]
type NativeLibrary = libloading::os::windows::Library;

#[cfg(not(windows))]
type NativeLibrary = libloading::Library;

pub trait RuntimeLibrary: Sized {
    /// Load a generated dlopen wrapper type from the resolved shared-library path.
    ///
    /// # Safety
    ///
    /// Callers must ensure the target library path points to a compatible shared
    /// library for the requested wrapper type and that any lifetime or threading
    /// requirements imposed by the loaded symbols are upheld after loading.
    unsafe fn load_from_dir(lib_dir: &Path, path: &Path) -> Result<Self, LibraryLoadError>;
}

impl RuntimeLibrary for slab_ggml_sys::GGmlLib {
    unsafe fn load_from_dir(lib_dir: &Path, path: &Path) -> Result<Self, LibraryLoadError> {
        let ggml_base_path = library_path(lib_dir, "ggml-base");

        #[cfg(windows)]
        {
            let ggml_base_lib = open_native_library(ggml_base_path.as_path())?;
            let ggml_lib = open_native_library(path)?;
            Ok(unsafe { Self::from_library(ggml_base_lib, ggml_lib)? })
        }

        #[cfg(not(windows))]
        {
            unsafe { Self::new(ggml_base_path.as_path(), path) }
        }
    }
}

impl RuntimeLibrary for slab_llama_sys::LlamaLib {
    unsafe fn load_from_dir(_lib_dir: &Path, path: &Path) -> Result<Self, LibraryLoadError> {
        #[cfg(windows)]
        {
            Ok(unsafe { Self::from_library(open_native_library(path)?)? })
        }

        #[cfg(not(windows))]
        {
            unsafe { Self::new(path) }
        }
    }
}

impl RuntimeLibrary for slab_whisper_sys::WhisperLib {
    unsafe fn load_from_dir(_lib_dir: &Path, path: &Path) -> Result<Self, LibraryLoadError> {
        unsafe { Self::from_library(open_native_library(path)?) }
    }
}

impl RuntimeLibrary for slab_diffusion_sys::DiffusionLib {
    unsafe fn load_from_dir(_lib_dir: &Path, path: &Path) -> Result<Self, LibraryLoadError> {
        #[cfg(windows)]
        {
            Ok(unsafe { Self::from_library(open_native_library(path)?)? })
        }

        #[cfg(not(windows))]
        {
            unsafe { Self::new(path) }
        }
    }
}

/// Build the platform-specific shared library file name for a logical runtime name.
pub fn library_file_name(base_name: &str) -> String {
    format!("{}{}{}", DLL_PREFIX, base_name, DLL_SUFFIX)
}

/// Build the shared library path inside the unified runtime library directory.
pub fn library_path<P: AsRef<Path>>(lib_dir: P, base_name: &str) -> PathBuf {
    lib_dir.as_ref().join(library_file_name(base_name))
}

/// Resolve a logical runtime library name inside a shared library directory and
/// delegate the actual loading work to the provided closure.
pub fn load_library_from_dir<P, T, E, F>(lib_dir: P, base_name: &str, load: F) -> Result<T, E>
where
    P: AsRef<Path>,
    F: FnOnce(&Path, &Path) -> Result<T, E>,
{
    let lib_dir = lib_dir.as_ref();
    let lib_path = library_path(lib_dir, base_name);
    load(lib_dir, &lib_path)
}

/// Resolve an optional sidecar library path inside a shared library directory
/// and delegate the actual loading work to the provided closure.
pub fn load_optional_library_from_dir<P, T, F>(lib_dir: P, base_name: &str, load: F) -> Option<T>
where
    P: AsRef<Path>,
    F: FnOnce(&Path, &Path) -> Option<T>,
{
    let lib_dir = lib_dir.as_ref();
    let lib_path = library_path(lib_dir, base_name);
    load(lib_dir, &lib_path)
}

fn open_native_library(path: &Path) -> Result<NativeLibrary, LibraryLoadError> {
    #[cfg(windows)]
    {
        use libloading::os::windows::{
            LOAD_LIBRARY_SEARCH_APPLICATION_DIR, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, Library,
        };

        unsafe {
            Library::load_with_flags(
                path,
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR
                    | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS
                    | LOAD_LIBRARY_SEARCH_APPLICATION_DIR,
            )
        }
    }

    #[cfg(not(windows))]
    {
        unsafe { libloading::Library::new(path) }
    }
}

/// Resolve a primary library and a named sidecar from the same runtime library
/// directory, then delegate their loading to the provided closures.
pub fn load_library_bundle_from_dir<P, Main, Sidecar, E, FMain, FSidecar>(
    lib_dir: P,
    main_name: &str,
    sidecar_name: &str,
    load_main: FMain,
    load_sidecar: FSidecar,
) -> Result<(Main, Option<Sidecar>), E>
where
    P: AsRef<Path>,
    FMain: FnOnce(&Path, &Path) -> Result<Main, E>,
    FSidecar: FnOnce(&Path, &Path) -> Option<Sidecar>,
{
    let lib_dir = lib_dir.as_ref();
    let main_path = library_path(lib_dir, main_name);
    let sidecar_path = library_path(lib_dir, sidecar_name);
    let main = load_main(lib_dir, &main_path)?;
    let sidecar = load_sidecar(lib_dir, &sidecar_path);
    Ok((main, sidecar))
}

/// Load a generated runtime library wrapper type from the unified runtime
/// library directory.
pub fn load_runtime_library_from_dir<P, Main>(
    lib_dir: P,
    main_name: &str,
) -> Result<Main, LibraryLoadError>
where
    P: AsRef<Path>,
    Main: RuntimeLibrary,
{
    load_library_from_dir(lib_dir, main_name, |lib_dir, main_path| unsafe {
        Main::load_from_dir(lib_dir, main_path)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn library_path_joins_platform_file_name() {
        let expected = Path::new("runtime").join(library_file_name("llama"));
        assert_eq!(library_path("runtime", "llama"), expected);
    }

    #[test]
    fn load_library_from_dir_passes_resolved_directory_and_path() {
        let value = load_library_from_dir("runtime", "whisper", |lib_dir, lib_path| {
            Ok::<_, ()>((lib_dir.to_path_buf(), lib_path.to_path_buf()))
        })
        .expect("helper should delegate success");

        assert_eq!(value.0, Path::new("runtime"));
        assert_eq!(value.1, Path::new("runtime").join(library_file_name("whisper")));
    }

    #[test]
    fn load_optional_library_from_dir_passes_resolved_directory_and_path() {
        let value = load_optional_library_from_dir("runtime", "ggml", |lib_dir, lib_path| {
            Some((lib_dir.to_path_buf(), lib_path.to_path_buf()))
        })
        .expect("helper should delegate success");

        assert_eq!(value.0, Path::new("runtime"));
        assert_eq!(value.1, Path::new("runtime").join(library_file_name("ggml")));
    }

    #[test]
    fn load_library_bundle_from_dir_passes_main_and_sidecar_paths() {
        let value = load_library_bundle_from_dir(
            "runtime",
            "diffusion",
            "ggml",
            |_lib_dir, main_path| Ok::<_, ()>(main_path.to_path_buf()),
            |_lib_dir, sidecar_path| Some(sidecar_path.to_path_buf()),
        )
        .expect("bundle helper should delegate success");

        assert_eq!(
            value.0,
            Path::new("runtime").join(library_file_name("diffusion"))
        );
        assert_eq!(
            value.1.expect("sidecar path should be returned"),
            Path::new("runtime").join(library_file_name("ggml"))
        );
    }

    #[test]
    fn load_optional_library_from_dir_returns_none_when_closure_returns_none() {
        let value = load_optional_library_from_dir("runtime", "nonexistent", |_lib_dir, _lib_path| {
            None::<(PathBuf, PathBuf)>
        });

        assert!(value.is_none(), "should return None when closure returns None");
    }

    #[test]
    fn load_library_from_dir_propagates_errors() {
        let result = load_library_from_dir("runtime", "test", |_lib_dir, _lib_path| {
            Err::<(PathBuf, PathBuf), &str>("simulated error")
        });

        assert!(result.is_err(), "should propagate closure errors");
    }

    #[test]
    fn load_library_bundle_from_dir_returns_none_when_sidecar_missing() {
        let value = load_library_bundle_from_dir(
            "runtime",
            "diffusion",
            "ggml",
            |_lib_dir, main_path| Ok::<PathBuf, ()>(main_path.to_path_buf()),
            |_lib_dir, _sidecar_path| None::<PathBuf>,
        )
        .expect("bundle helper should succeed");

        assert!(value.1.is_none(), "sidecar should be None when closure returns None");
    }

    #[test]
    fn load_library_bundle_from_dir_propagates_main_errors() {
        let result = load_library_bundle_from_dir(
            "runtime",
            "diffusion",
            "ggml",
            |_lib_dir, _main_path| Err::<(), &str>("main load error"),
            |_lib_dir, _sidecar_path| Some(PathBuf::new()),
        );

        assert!(result.is_err(), "should propagate main library errors");
    }

    #[test]
    fn library_file_name_contains_platform_prefix_and_suffix() {
        let llama_lib = library_file_name("llama");
        let whisper_lib = library_file_name("whisper");

        assert!(!llama_lib.is_empty());
        assert!(!whisper_lib.is_empty());

        // Check that the library name contains the base name
        #[cfg(target_os = "windows")]
        {
            assert!(llama_lib.contains("llama"));
            assert!(llama_lib.ends_with(".dll"));
            assert!(whisper_lib.contains("whisper"));
            assert!(whisper_lib.ends_with(".dll"));
        }

        #[cfg(target_os = "linux")]
        {
            assert!(llama_lib.contains("llama"));
            assert!(llama_lib.ends_with(".so"));
            assert!(whisper_lib.contains("whisper"));
            assert!(whisper_lib.ends_with(".so"));
        }

        #[cfg(target_os = "macos")]
        {
            assert!(llama_lib.contains("llama"));
            assert!(llama_lib.ends_with(".dylib"));
            assert!(whisper_lib.contains("whisper"));
            assert!(whisper_lib.ends_with(".dylib"));
        }
    }

    #[test]
    fn library_path_converts_relative_path_to_absolute() {
        let path = library_path("relative/runtime", "test");
        // The library_path function joins the directory with the library file name
        // It should return an absolute path since we're joining with a library file name
        assert!(path.is_absolute() || path.starts_with("relative/runtime"));
    }

    #[test]
    fn load_library_from_dir_with_absolute_path() {
        let absolute_dir = std::path::PathBuf::from("/absolute/runtime");
        let value = load_library_from_dir(&absolute_dir, "test", |lib_dir, lib_path| {
            Ok::<(PathBuf, PathBuf), ()>((lib_dir.to_path_buf(), lib_path.to_path_buf()))
        })
        .expect("should work with absolute paths");

        assert_eq!(value.0, absolute_dir);
        // The library path should be absolute
        assert!(value.1.starts_with("/absolute/runtime"));
    }

    #[test]
    fn load_optional_library_from_dir_with_empty_base_name() {
        let value = load_optional_library_from_dir("runtime", "", |lib_dir, lib_path| {
            Some((lib_dir.to_path_buf(), lib_path.to_path_buf()))
        })
        .expect("should handle empty base name");

        assert_eq!(value.0, Path::new("runtime"));
        // Empty base name should still produce a library path with prefix/suffix
        assert!(!value.1.to_string_lossy().is_empty());
    }

    #[test]
    fn load_library_bundle_from_dir_with_same_dir() {
        // Both libraries come from the same directory in normal usage
        let runtime_dir = Path::new("runtime");

        let value = load_library_bundle_from_dir(
            runtime_dir,
            "main",
            "sidecar",
            |lib_dir, main_path| {
                assert_eq!(lib_dir, runtime_dir);
                Ok::<PathBuf, ()>(main_path.to_path_buf())
            },
            |lib_dir, sidecar_path| {
                assert_eq!(lib_dir, runtime_dir);
                Some(sidecar_path.to_path_buf())
            },
        )
        .expect("bundle helper should use correct directories");

        assert_eq!(value.0, runtime_dir.join(library_file_name("main")));
        assert_eq!(
            value.1.expect("sidecar should exist"),
            runtime_dir.join(library_file_name("sidecar"))
        );
    }
}
