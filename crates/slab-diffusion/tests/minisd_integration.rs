use hf_hub::api::sync::Api;
use hf_hub::{Repo, RepoType};
use slab_diffusion::{Diffusion, SampleMethod};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const MINI_SD_REPO_ID: &str = "justinpinkney/miniSD";
const MINI_SD_FILENAME: &str = "miniSD.ckpt";

#[cfg(windows)]
type DllDirectoryCookie = *mut std::ffi::c_void;

#[cfg(windows)]
unsafe extern "system" {
    fn AddDllDirectory(new_directory: *const u16) -> DllDirectoryCookie;
}

#[cfg(windows)]
static DLL_DIRS_INIT: OnceLock<Result<(), String>> = OnceLock::new();

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root should resolve")
}

fn vendored_runtime_dir(artifact: &str) -> PathBuf {
    let subdir = if cfg!(windows) { "bin" } else { "lib" };
    workspace_root().join("vendor").join(artifact).join(subdir)
}

#[cfg(windows)]
fn add_dll_directory(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;

    if !path.is_dir() {
        return Err(format!("runtime directory does not exist: {}", path.display()));
    }

    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);

    print!("Adding DLL directory: {}... ", path.display());
    let cookie = unsafe { AddDllDirectory(wide.as_ptr()) };
    if cookie.is_null() {
        return Err(format!("AddDllDirectory failed for {}", path.display()));
    }
    println!("done.");
    Ok(())
}

#[cfg(windows)]
fn ensure_vendored_runtime_dirs_registered() {
    let init = DLL_DIRS_INIT.get_or_init(|| {
        add_dll_directory(&vendored_runtime_dir("diffusion"))?;
        add_dll_directory(&vendored_runtime_dir("ggml"))?;
        Ok(())
    });

    if let Err(error) = init {
        panic!("failed to register vendored runtime directories: {error}");
    }
}

#[cfg(not(windows))]
fn ensure_vendored_runtime_dirs_registered() {}

fn load_vendored_diffusion() -> Diffusion {
    ensure_vendored_runtime_dirs_registered();

    Diffusion::new(vendored_runtime_dir("diffusion"))
        .unwrap_or_else(|error| panic!("failed to load vendored diffusion runtime: {error}"))
}

fn resolve_minisd_model_path() -> PathBuf {
    let api = Api::new().expect("failed to init hf-hub api");
    let repo = Repo::with_revision(MINI_SD_REPO_ID.to_owned(), RepoType::Model, "main".to_owned());

    api.repo(repo)
        .get(MINI_SD_FILENAME)
        .unwrap_or_else(|error| panic!("failed to resolve miniSD model via hf-hub: {error}"))
}

#[test]
#[ignore = "requires vendored diffusion runtime and cached miniSD model"]
fn minisd_generates_small_image_from_hf_hub_model() {
    let diffusion = load_vendored_diffusion();
    let model_path = resolve_minisd_model_path();

    diffusion
        .backend_list_size()
        .unwrap_or_else(|error| panic!("failed to get diffusion backend list size: {error}"));
    let mut context_params = diffusion.new_context_params();
    context_params.set_model_path(&model_path.to_string_lossy());

    let ctx = diffusion
        .new_context(context_params)
        .unwrap_or_else(|error| panic!("failed to create miniSD context: {error}"));

    let mut sample_params = diffusion.new_sample_params();
    sample_params.set_sample_steps(2);
    sample_params.set_sample_method(SampleMethod::Euler);

    let mut image_params = diffusion.new_image_params();
    image_params.set_prompt("a tiny orange cat");
    image_params.set_width(256);
    image_params.set_height(256);
    image_params.set_seed(42);
    image_params.set_batch_count(1);
    image_params.set_sample_params(sample_params);

    let images = ctx
        .generate_image(image_params)
        .unwrap_or_else(|error| panic!("failed to generate miniSD test image: {error}"));

    assert_eq!(images.len(), 1);
    assert_eq!(images[0].width, 256);
    assert_eq!(images[0].height, 256);
    assert!(images[0].channel == 3);
    assert_eq!(
        images[0].data.len(),
        images[0].width as usize * images[0].height as usize * images[0].channel as usize,
    );
    assert!(images[0].data.iter().any(|value| *value != 0));
}
