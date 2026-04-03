#![cfg(windows)]

use slab_ggml::{GGML, GGMLError};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

type DllDirectoryCookie = *mut std::ffi::c_void;

unsafe extern "system" {
    fn AddDllDirectory(new_directory: *const u16) -> DllDirectoryCookie;
}

static DLL_DIRS_INIT: OnceLock<Result<(), String>> = OnceLock::new();
static GGML_TEST_LOCK: Mutex<()> = Mutex::new(());

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root should resolve")
}

fn vendored_runtime_dir() -> PathBuf {
    workspace_root().join("vendor").join("ggml").join("bin")
}

fn vendored_cpu_backend_library_path() -> PathBuf {
    vendored_runtime_dir().join("ggml-cpu-x64.dll")
}

fn add_dll_directory(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;

    if !path.is_dir() {
        return Err(format!("runtime directory does not exist: {}", path.display()));
    }

    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);

    let cookie = unsafe { AddDllDirectory(wide.as_ptr()) };
    if cookie.is_null() {
        return Err(format!("AddDllDirectory failed for {}", path.display()));
    }

    Ok(())
}

fn ensure_vendored_runtime_dir_registered() {
    let init = DLL_DIRS_INIT.get_or_init(|| add_dll_directory(&vendored_runtime_dir()));

    if let Err(error) = init {
        panic!("failed to register vendored ggml runtime directory: {error}");
    }
}

fn load_vendored_ggml() -> GGML {
    ensure_vendored_runtime_dir_registered();

    GGML::from_dir(vendored_runtime_dir())
        .unwrap_or_else(|error| panic!("failed to load vendored ggml runtime: {error}"))
}

#[test]
fn vendored_ggml_loads_and_reports_version() {
    let _guard = GGML_TEST_LOCK.lock().unwrap();
    let ggml = load_vendored_ggml();

    match ggml.version() {
        Ok(version) => assert!(!version.trim().is_empty()),
        Err(GGMLError::MissingSymbol { symbol, .. }) => assert_eq!(symbol, "ggml_version"),
        Err(error) => panic!("unexpected ggml version result: {error}"),
    }
}

#[test]
fn vendored_ggml_loads_all_backends_from_vendor_directory() {
    let _guard = GGML_TEST_LOCK.lock().unwrap();
    let ggml = load_vendored_ggml();

    ggml.load_all_backend_from_path(&vendored_runtime_dir().to_string_lossy())
        .expect("loading ggml backends from vendored directory should succeed");
}

#[test]
fn vendored_ggml_can_load_cpu_backend_directly() {
    let _guard = GGML_TEST_LOCK.lock().unwrap();
    let ggml = load_vendored_ggml();

    let reg = ggml
        .ggml_backend_load(&vendored_cpu_backend_library_path().to_string_lossy())
        .expect("loading vendored cpu backend should succeed");

    assert_eq!(format!("{reg:?}"), "GGMLBackendReg");
}

#[test]
fn ggml_list_backends() {
    // let _guard = GGML_TEST_LOCK.lock().unwrap();
    let ggml = load_vendored_ggml();

    ggml.load_all_backend_from_path(&vendored_runtime_dir().to_string_lossy())
        .expect("loading ggml backends from vendored directory should succeed");

    let backend_count = ggml.ggml_backend_dev_count();
    assert!(backend_count >= 1, "should have at least one backend device");

    for index in 0..backend_count {
        let device = ggml.ggml_backend_dev_get(index).expect("should get backend device");
        let name = ggml.ggml_backend_dev_name(device).expect("should get backend device name");
        println!("Backend device {index}: {name}");
    }
}
