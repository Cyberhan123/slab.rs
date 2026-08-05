//! Live integration tests for `slab-llama` against the vendored llama.cpp runtime.
//!
//! These tests exercise the FFI wrapper surface that is otherwise only covered by
//! pure-Rust unit tests: GGUF `chat_template` read-back, the
//! `apply_chat_template` / `chat_builtin_templates` FFI, `model_quantize`,
//! the kv seq ops + per-sequence state round-trip, `perf_data` after prefill,
//! and incremental prefill via snapshot restore.
//!
//! Gated behind the `live` cargo feature **and** `#[ignore]` — they need network
//! (to fetch the fixture model into the global HF cache) and the vendored native
//! llama/ggml runtime. Run them explicitly:
//!
//! ```sh
//! cargo test -p slab-llama --features live -- --ignored
//! ```

#![cfg(feature = "live")]

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use slab_llama::{
    Llama, LlamaBatch, LlamaChatMessage, LlamaContextParams, LlamaFtype, LlamaModelParams,
    LlamaQuantizeParams,
};

#[cfg(windows)]
type DllDirectoryCookie = *mut std::ffi::c_void;

#[cfg(windows)]
unsafe extern "system" {
    fn AddDllDirectory(new_directory: *const u16) -> DllDirectoryCookie;
}

#[cfg(windows)]
static DLL_DIRS_INIT: OnceLock<Result<(), String>> = OnceLock::new();

/// Small fixture: Qwen2.5-0.5B-Instruct Q4_K_M (shares the global HF cache with
/// the e2e/app suite, so it is downloaded at most once per machine).
const FIXTURE_REPO_ID: &str = "Qwen/Qwen2.5-0.5B-Instruct-GGUF";
const FIXTURE_FILENAME: &str = "qwen2.5-0.5b-instruct-q4_k_m.gguf";

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

/// Register the vendored llama + ggml runtime directories once per process so the
/// dynamic loader can resolve `llama.dll` and its ggml sidecar. Mirrors
/// `crates/slab-diffusion/tests/minisd_integration.rs`.
#[cfg(windows)]
fn ensure_vendored_runtime_dirs_registered() {
    let init = DLL_DIRS_INIT.get_or_init(|| {
        let llama_dir = vendored_runtime_dir("llama");
        add_dll_directory(&llama_dir)?;
        let ggml_dir = vendored_runtime_dir("ggml");
        if ggml_dir != llama_dir {
            add_dll_directory(&ggml_dir)?;
        }
        Ok(())
    });

    if let Err(error) = init {
        panic!("failed to register vendored llama runtime directories: {error}");
    }
}

#[cfg(not(windows))]
fn ensure_vendored_runtime_dirs_registered() {}

/// Resolve the fixture GGUF via the hf-hub sync client into the global HF cache.
fn download_fixture_model() -> PathBuf {
    let client = hf_hub::HFClientSync::new().expect("failed to init hf-hub client");
    let (owner, name) = hf_hub::split_id(FIXTURE_REPO_ID);

    client
        .model(owner.to_owned(), name.to_owned())
        .download_file()
        .filename(FIXTURE_FILENAME.to_owned())
        .send()
        .unwrap_or_else(|error| panic!("failed to download fixture model via hf-hub: {error}"))
}

/// Load the vendored llama library + the fixture model. Returns `(llama, model)`.
fn load_fixture() -> (Llama, slab_llama::LlamaModel) {
    ensure_vendored_runtime_dirs_registered();

    let llama = Llama::new(vendored_runtime_dir("llama")).expect("load vendored llama library");
    llama.backend_init();

    let model_path = download_fixture_model();
    let path_str = model_path.to_str().expect("fixture model path is utf-8");
    let model = llama
        .load_model_from_file(path_str, LlamaModelParams::default())
        .expect("load fixture model");

    (llama, model)
}

/// Tokenize `text` and prefill it into `ctx` on sequence 0. Returns the token count.
fn prefill(
    model: &slab_llama::LlamaModel,
    ctx: &mut slab_llama::LlamaContext,
    text: &str,
) -> usize {
    let tokens = model.tokenize(text, true, true).expect("tokenize prefill text");
    let n = tokens.len();
    let mut batch = LlamaBatch::new(n);
    for (i, token) in tokens.iter().enumerate() {
        batch.add(*token, i as i32, &[0], i == n - 1).expect("batch add");
    }
    ctx.decode(&mut batch).expect("decode prefill batch");
    n
}

#[test]
#[ignore = "needs vendored llama runtime + network (downloads Qwen2.5-0.5B)"]
fn live_chat_template_from_gguf() {
    let (_llama, model) = load_fixture();

    // The GGUF `tokenizer.chat_template` must be readable.
    let template = model.chat_template().expect("read GGUF chat_template");
    assert!(
        template.as_ref().is_some_and(|text| !text.trim().is_empty()),
        "fixture model should embed a chat_template"
    );
    // Qwen2.5 uses ChatML.
    assert!(
        template.as_deref().is_some_and(|text| text.contains("im_start")),
        "expected ChatML marker in template, got: {template:?}"
    );
}

#[test]
#[ignore = "needs vendored llama runtime + network (downloads Qwen2.5-0.5B)"]
fn live_apply_chat_template_and_builtins() {
    let (_llama, model) = load_fixture();

    // The builtin-template FFI symbol is wired and returns a list.
    let builtins = model.chat_builtin_templates().expect("list builtin templates");
    assert!(!builtins.is_empty(), "llama.cpp should expose at least one builtin template");

    // Applying the default builtin template must yield a prompt containing the
    // user message. (Qwen ships a custom jinja template which the FFI does not
    // parse, so we let llama.cpp pick its builtin by passing `None`.)
    let messages = [
        LlamaChatMessage::new("system", "You are a concise assistant.").unwrap(),
        LlamaChatMessage::new("user", "Please reply with the single word: ping").unwrap(),
    ];
    let prompt = model
        .apply_chat_template(None, &messages, true)
        .expect("apply_chat_template (default builtin)");
    assert!(prompt.contains("ping"), "rendered prompt should contain the user message");
}

#[test]
#[ignore = "needs vendored llama runtime + network (downloads Qwen2.5-0.5B)"]
fn live_quantize_roundtrip() {
    let (llama, _model) = load_fixture();
    let input = download_fixture_model();

    let output_dir = tempfile::tempdir().expect("create quantize output tempdir");
    let output = output_dir.path().join("fixture-q8_0.gguf");

    // Full FFI `llama_model_quantize` path (Q4_K_M -> Q8_0 requantize).
    let params = LlamaQuantizeParams {
        ftype: LlamaFtype::MostlyQ8_0,
        nthread: 4,
        allow_requantize: true,
        ..LlamaQuantizeParams::default()
    };

    let layers = llama
        .model_quantize(
            input.to_str().expect("input path utf-8"),
            output.to_str().expect("output path utf-8"),
            &params,
        )
        .expect("model_quantize should succeed");

    assert!(layers > 0, "model_quantize reported zero layers processed");
    assert!(output.is_file(), "quantized output gguf was not written");
    let output_size = std::fs::metadata(&output).expect("stat output gguf").len();
    assert!(output_size > 0, "quantized output gguf is empty");
}

#[test]
#[ignore = "needs vendored llama runtime + network (downloads Qwen2.5-0.5B)"]
fn live_kv_state_and_perf_after_prefill() {
    let (_llama, model) = load_fixture();
    let mut ctx =
        model.new_context(LlamaContextParams::default()).expect("create inference context");

    ctx.perf_reset();
    let token_count = prefill(&model, &mut ctx, "The quick brown fox jumps over the lazy dog.");
    assert!(token_count > 0);

    // Perf counters reflect the prefill.
    let perf = ctx.perf_data();
    assert!(perf.n_p_eval > 0, "perf n_p_eval should reflect prefilled tokens");

    // kv seq position + per-sequence state round-trip.
    let pos_max = ctx.kv_cache_seq_pos_max(0);
    assert!(pos_max > 0, "kv_cache_seq_pos_max should reflect the prefill");

    let size = ctx.state_seq_get_size(0);
    assert!(size > 0, "per-sequence state size should be non-zero");

    let mut blob = vec![0u8; size];
    let written = ctx.state_seq_get_data(&mut blob, 0).expect("state_seq_get_data");
    assert_eq!(written, size, "state_seq_get_data wrote the reported size");

    // Restoring the blob into a fresh context must reproduce the kv position.
    let mut restored =
        model.new_context(LlamaContextParams::default()).expect("create restore context");
    let consumed = restored.state_seq_set_data(&blob, 0).expect("state_seq_set_data");
    assert_eq!(consumed, size, "state_seq_set_data consumed the full blob");
    assert_eq!(
        restored.kv_cache_seq_pos_max(0),
        pos_max,
        "restored context should reach the same kv position"
    );
}

#[test]
#[ignore = "needs vendored llama runtime + network (downloads Qwen2.5-0.5B)"]
fn live_incremental_prefill_skips_reprefill() {
    let (_llama, model) = load_fixture();
    let p1 = "Once upon a time, in a quiet valley, there lived a small brass dragon.";
    let p2 = " Every evening it climbed the hill to count the sheep below.";

    // Turn 1: prefill p1, snapshot the kv state.
    let mut ctx1 = model.new_context(LlamaContextParams::default()).expect("create first context");
    ctx1.perf_reset();
    let n1 = prefill(&model, &mut ctx1, p1);
    let pos1 = ctx1.kv_cache_seq_pos_max(0);
    assert!(pos1 > 0);
    let size = ctx1.state_seq_get_size(0);
    let mut blob = vec![0u8; size];
    ctx1.state_seq_get_data(&mut blob, 0).expect("snapshot first-turn kv state");

    // Simulate a restart: fresh context, restore the snapshot, then prefill only
    // the delta (p2) starting at the restored position.
    let mut ctx2 = model.new_context(LlamaContextParams::default()).expect("create second context");
    ctx2.state_seq_set_data(&blob, 0).expect("restore first-turn kv state");
    ctx2.perf_reset();

    let full = format!("{p1}{p2}");
    let full_tokens = model.tokenize(&full, true, true).expect("tokenize full prompt");
    // Delta = the tokens beyond the first turn's prefix (approximate boundary).
    let split = n1.min(full_tokens.len());
    let delta_tokens = &full_tokens[split..];
    let start = (pos1 + 1) as usize;

    let mut batch = LlamaBatch::new(delta_tokens.len().max(1));
    for (i, token) in delta_tokens.iter().enumerate() {
        batch
            .add(*token, (start + i) as i32, &[0], i == delta_tokens.len() - 1)
            .expect("batch add delta token");
    }
    if !delta_tokens.is_empty() {
        ctx2.decode(&mut batch).expect("decode delta batch");
    }

    // Incremental prefill should process only the delta, not the
    // full prompt — turn-2 n_p_eval must be well below |full_tokens|.
    let perf2 = ctx2.perf_data();
    assert!(
        perf2.n_p_eval < full_tokens.len() as i32,
        "incremental prefill should process only the delta ({} tokens), but n_p_eval={} for a full prompt of {} tokens",
        delta_tokens.len(),
        perf2.n_p_eval,
        full_tokens.len()
    );
}
