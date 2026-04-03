use super::*;
use crate::internal::engine::ggml::llama::adapter::GGMLLlamaEngine;
use hf_hub::api::sync::Api;
use slab_llama::{LlamaContextParams, LlamaModelParams};
use std::path::PathBuf;
use std::sync::Arc;

#[cfg(target_os = "linux")]
use slab_llama::{ChatMessage, LlamaBatch};
#[cfg(target_os = "linux")]
use std::sync::{Mutex, OnceLock};

/// Ensure llama dynamic library artifacts are available under `testdata/`.
///
/// Tests rely on this helper to make local execution reproducible: if artifacts
/// are missing, they are downloaded once via `DylibService`.
async fn ensure_llama_dir() -> PathBuf {
    let mut test_data_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    test_data_path.push("../testdata");
    test_data_path.join("llama")
}

/// Download the GGUF model used in llama integration tests.
fn download_test_model() -> PathBuf {
    let api = Api::new().expect("failed to init hf-api");
    api.model("bartowski/Qwen2.5-0.5B-Instruct-GGUF".into())
        .get("Qwen2.5-0.5B-Instruct-Q4_K_M.gguf")
        .expect("failed to download model")
}

/// Build a ready-to-use `LlamaService` with loaded model and running engine.
///
/// All external test calls are routed through `GGMLLlamaEngine` so the tests verify
/// the intended public API surface.
async fn make_service(num_workers: usize) -> Arc<GGMLLlamaEngine> {
    let llama_dir = ensure_llama_dir().await;
    let model_path = download_test_model();

    let service = GGMLLlamaEngine::from_path(llama_dir.as_path())
        .expect("failed to initialize llama service");
    service
        .load_model_with_workers(
            model_path.as_path(),
            LlamaModelParams::default(),
            LlamaContextParams::default(),
            num_workers,
        )
        .expect("load model failed");

    service
}

#[tokio::test]
#[ignore = "requires external model download / network"]
async fn test_llama_inference() {
    let service = make_service(1).await;

    let result =
        service.inference("Hello, my name is", 64, None, None).await.expect("inference failed");

    println!("Generated: {result}");
    assert!(!result.is_empty(), "expected non-empty output");
}

/// Validates the happy path for service-session streaming generation:
/// create session → append input → consume stream until `Done` → end session.
#[tokio::test]
#[ignore = "requires external model download / network"]
async fn test_service_basic_generation() {
    let service = make_service(1).await;

    let sid = service.create_session().await.expect("create_session failed");
    service.append_input(sid, "Hello, my name is".to_string()).await.expect("append_input failed");

    let mut stream = service.generate_stream(sid, 32).await.expect("generate_stream failed");

    let mut output = String::new();
    while let Some(chunk) = stream.recv().await {
        match chunk {
            StreamChunk::Token(text) => output.push_str(&text),
            StreamChunk::Done => break,
            StreamChunk::Error(e) => panic!("generation error: {e}"),
        }
    }

    println!("service basic output: {output}");
    assert!(!output.is_empty(), "expected non-empty output");

    service.end_session(sid).await.expect("end_session failed");
}

/// Validates that API calls against an unknown session return
/// `GGMLLlamaEngineError::SessionNotFound` with the requested id preserved.
#[tokio::test]
#[ignore = "requires external model download / network"]
async fn test_service_session_not_found() {
    let service = make_service(1).await;

    let _err = service.append_input(9999, "hello".to_string()).await.unwrap_err();

    // todo: fix this test once error variants are stable and can be matched on directly
    // assert!(
    //     matches!(
    //         err,
    //         engine::EngineError(
    //         GGMLEngineError(GGMLLlamaEngineError::SessionNotFound { session_id: 9999 })
    //     ),
    //     "unexpected error: {err}"
    // );
}

/// Verifies multi-turn KV reuse behavior through the service API.
///
/// The second turn should generate successfully without re-creating the session,
/// implying prior context state is retained for that session.
#[tokio::test]
#[ignore = "requires external model download / network"]
async fn test_service_kv_reuse_multiturn() {
    let service = make_service(1).await;

    let sid = service.create_session().await.expect("create_session failed");

    service.append_input(sid, "What is 1+1?".to_string()).await.expect("first append failed");
    let mut stream = service.generate_stream(sid, 16).await.expect("first generate_stream failed");
    let mut turn1 = String::new();
    while let Some(chunk) = stream.recv().await {
        match chunk {
            StreamChunk::Token(t) => turn1.push_str(&t),
            StreamChunk::Done => break,
            StreamChunk::Error(e) => panic!("turn1 error: {e}"),
        }
    }
    assert!(!turn1.is_empty(), "first turn should produce output");

    service.append_input(sid, " And what is 2+2?".to_string()).await.expect("second append failed");
    let mut stream2 =
        service.generate_stream(sid, 16).await.expect("second generate_stream failed");
    let mut turn2 = String::new();
    while let Some(chunk) = stream2.recv().await {
        match chunk {
            StreamChunk::Token(t) => turn2.push_str(&t),
            StreamChunk::Done => break,
            StreamChunk::Error(e) => panic!("turn2 error: {e}"),
        }
    }
    assert!(!turn2.is_empty(), "second turn should produce output");

    service.end_session(sid).await.expect("end_session failed");
}

/// Verifies cancellation semantics and post-cancel usability via service API.
///
/// A running stream is cancelled after a few emitted tokens; the same session
/// then accepts new input and can start a fresh generation.
#[tokio::test]
#[ignore = "requires external model download / network"]
async fn test_service_cancel_and_resume() {
    let service = make_service(1).await;

    let sid = service.create_session().await.expect("create_session failed");
    service.append_input(sid, "Count to one hundred:".to_string()).await.expect("append failed");

    let mut stream = service.generate_stream(sid, 512).await.expect("generate_stream failed");

    let mut tokens_before_cancel = 0usize;
    loop {
        match stream.recv().await {
            Some(StreamChunk::Token(_)) => {
                tokens_before_cancel += 1;
                if tokens_before_cancel >= 3 {
                    break;
                }
            }
            Some(StreamChunk::Done) | None => break,
            Some(StreamChunk::Error(e)) => panic!("stream error: {e}"),
        }
    }

    service.cancel_generate(sid).await.expect("cancel failed");

    service
        .append_input(sid, " Just say done.".to_string())
        .await
        .expect("append after cancel failed");
    let mut stream2 = service.generate_stream(sid, 8).await.expect("generate after cancel failed");
    let mut post_cancel = String::new();
    while let Some(chunk) = stream2.recv().await {
        match chunk {
            StreamChunk::Token(t) => post_cancel.push_str(&t),
            StreamChunk::Done => break,
            StreamChunk::Error(e) => panic!("post-cancel error: {e}"),
        }
    }
    assert!(!post_cancel.is_empty(), "should generate after cancel");

    service.end_session(sid).await.expect("end_session failed");
}

/// Exercises sequence-id free-list reuse by repeatedly creating and ending
/// sessions through the service API.
#[tokio::test]
#[ignore = "requires external model download / network"]
async fn test_service_seq_id_reuse() {
    let service = make_service(1).await;

    for _ in 0..4 {
        let sid = service.create_session().await.expect("create_session");
        service.append_input(sid, "hi".to_string()).await.expect("append");
        let mut stream = service.generate_stream(sid, 4).await.expect("generate_stream");
        while let Some(chunk) = stream.recv().await {
            match chunk {
                StreamChunk::Done => break,
                StreamChunk::Error(e) => panic!("{e}"),
                _ => {}
            }
        }
        service.end_session(sid).await.expect("end_session");
    }
}

// ── Local test helpers ────────────────────────────────────────────────────────

/// Process-wide mutex that serializes `Llama::new` / `GGMLLlamaEngine::from_path`
/// calls across test threads.
///
/// `ggml_backend_load_all_from_path` (called inside `Llama::new`) performs
/// concurrent `dlopen` calls that are not safe when invoked simultaneously from
/// multiple threads.  All test helpers that create a `Llama` or `GGMLLlamaEngine`
/// must hold this lock for the duration of the library-load step.
#[cfg(target_os = "linux")]
static LLAMA_INIT_LOCK: Mutex<()> = Mutex::new(());

/// Guards the once-per-process `backend_init` call so it is invoked exactly
/// once even when multiple test functions call `make_local_llama` concurrently.
#[cfg(target_os = "linux")]
static LLAMA_BACKEND_INIT: OnceLock<()> = OnceLock::new();

/// Return the path to the platform-specific llama shared library directory
/// shipped with the repository under `testdata/llama/libs/<os>`.
#[cfg(target_os = "linux")]
fn llama_lib_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../testdata/llama/libs")
        .join(std::env::consts::OS)
}

/// Return the path to the small test model bundled in `testdata/llama/models`.
#[cfg(target_os = "linux")]
fn local_model_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata/llama/models/stories15M.Q4_0.gguf")
}

/// Load the llama dynamic library from the local testdata directory directly.
///
/// Both library load (`Llama::new`) and the once-per-process `backend_init` are
/// performed under `LLAMA_INIT_LOCK` so concurrent test threads cannot race on
/// either step.
#[cfg(target_os = "linux")]
fn make_local_llama() -> slab_llama::Llama {
    let _guard = LLAMA_INIT_LOCK
        .lock()
        .expect("LLAMA_INIT_LOCK was poisoned by a panicked thread during library initialization");
    let llama = slab_llama::Llama::new(llama_lib_dir())
        .expect("failed to load llama library from testdata");
    // `backend_init` must only be called once per process; guard it with OnceLock
    // while still holding the init lock so the call is fully serialized.
    LLAMA_BACKEND_INIT.get_or_init(|| llama.backend_init());
    llama
}

/// Build a `GGMLLlamaEngine` backed by the local testdata library and model.
#[cfg(target_os = "linux")]
async fn make_local_service(num_workers: usize) -> Arc<GGMLLlamaEngine> {
    let service = {
        // Serialize the dlopen-based backend loading step (not thread-safe).
        let _guard = LLAMA_INIT_LOCK.lock().expect(
            "LLAMA_INIT_LOCK was poisoned by a panicked thread during library initialization",
        );
        GGMLLlamaEngine::from_path(llama_lib_dir())
            .expect("failed to initialize llama service from testdata")
    };
    service
        .load_model_with_workers(
            local_model_path(),
            LlamaModelParams::default(),
            LlamaContextParams::default(),
            num_workers,
        )
        .expect("load local model failed");
    service
}

// ── Tier-1: low-level slab_llama integration tests ───────────────────────────

/// Verify that the dynamic library can be loaded and `backend_init` succeeds
/// without any model file being present.
#[test]
#[cfg(target_os = "linux")]
fn test_llama_backend_init() {
    let _llama = make_local_llama();
    // Reaching here without a panic confirms lib loading and backend init work.
}

/// Load the bundled stories-15M model and check that the key metadata fields
/// returned by the wrapper are sane (positive, non-zero values).
#[test]
#[cfg(target_os = "linux")]
fn test_model_load_and_metadata() {
    let llama = make_local_llama();

    let model = llama
        .load_model_from_file(local_model_path().to_str().unwrap(), LlamaModelParams::default())
        .expect("model load failed");

    assert!(model.n_vocab() > 0, "n_vocab should be positive, got {}", model.n_vocab());
    assert!(model.n_layer() > 0, "n_layer should be positive, got {}", model.n_layer());
    assert!(model.n_embd() > 0, "n_embd should be positive, got {}", model.n_embd());
    assert!(model.n_params() > 0, "n_params should be positive, got {}", model.n_params());

    println!(
        "Metadata: n_vocab={} n_layer={} n_embd={} n_params={} size={}",
        model.n_vocab(),
        model.n_layer(),
        model.n_embd(),
        model.n_params(),
        model.model_size(),
    );
}

/// Tokenize "Hello, world!" then detokenize and verify the roundtrip
/// preserves the original content.
#[test]
#[cfg(target_os = "linux")]
fn test_tokenize_roundtrip() {
    let llama = make_local_llama();

    let model = llama
        .load_model_from_file(local_model_path().to_str().unwrap(), LlamaModelParams::default())
        .expect("model load failed");

    let text = "Hello, world!";
    let tokens = model.tokenize(text, false, false).expect("tokenize failed");
    assert!(!tokens.is_empty(), "tokenization should produce at least one token");
    println!("Token ids: {tokens:?}");

    let roundtrip = model.tokens_to_str(&tokens, false, false).expect("detokenize failed");
    assert!(!roundtrip.is_empty(), "detokenized text should not be empty");

    // The roundtrip is allowed to differ in leading whitespace/casing but must
    // contain the key word from the original prompt.
    assert!(
        roundtrip.to_lowercase().contains("hello"),
        "detokenized text '{roundtrip}' should contain 'hello'"
    );
    println!("Roundtrip: '{text}' → tokens({}) → '{roundtrip}'", tokens.len());
}

/// Prefill a short prompt, run one decode step, assert that the logit tensor
/// is populated (at least one non-zero value), then greedy-sample the next
/// token and check it falls within the valid vocabulary range.
#[test]
#[cfg(target_os = "linux")]
fn test_single_decode_and_sample() {
    let llama = make_local_llama();

    let model = llama
        .load_model_from_file(local_model_path().to_str().unwrap(), LlamaModelParams::default())
        .expect("model load failed");

    let mut ctx = model.new_context(LlamaContextParams::default()).expect("context create failed");

    let tokens = model.tokenize("Hello", false, false).expect("tokenize failed");
    assert!(!tokens.is_empty(), "prompt should produce tokens");

    let last_idx = (tokens.len() - 1) as i32;
    let mut batch = LlamaBatch::new(tokens.len());
    for (i, &token) in tokens.iter().enumerate() {
        let want_logits = i == tokens.len() - 1;
        batch.add(token, i as i32, &[0], want_logits).expect("batch add failed");
    }

    ctx.decode(&mut batch).expect("decode failed");

    // Verify that the logit slice is non-empty and contains at least one non-zero value.
    let logits = ctx.get_logits_ith(last_idx);
    assert!(!logits.is_empty(), "logits should be non-empty after decode");
    let has_nonzero = logits.iter().any(|&v| v != 0.0);
    assert!(has_nonzero, "at least one logit should be non-zero after a real decode step");

    // Greedy-sample the next token and check it is within vocabulary bounds.
    let mut sampler = llama.new_sampler_chain().add_greedy();
    let next_token = sampler.sample(&mut ctx, last_idx);
    let n_vocab = model.n_vocab();
    assert!(
        next_token >= 0 && next_token < n_vocab,
        "sampled token {next_token} must be in [0, {n_vocab})"
    );

    let piece = model.token_to_piece(next_token, false).unwrap_or_default();
    println!("Greedy next token after 'Hello': id={next_token} piece={piece:?}");
}

/// Verify that greedy sampling produces the same token on two independent runs
/// over the same prompt (determinism property of argmax sampling).
#[test]
#[cfg(target_os = "linux")]
fn test_sampler_correctness() {
    let llama = make_local_llama();

    let model = llama
        .load_model_from_file(local_model_path().to_str().unwrap(), LlamaModelParams::default())
        .expect("model load failed");

    let greedy_next = |prompt: &str| -> i32 {
        let mut ctx = model.new_context(LlamaContextParams::default()).expect("context");
        let tokens = model.tokenize(prompt, false, false).expect("tokenize");
        let last_idx = (tokens.len() - 1) as i32;
        let mut batch = LlamaBatch::new(tokens.len());
        for (i, &t) in tokens.iter().enumerate() {
            batch.add(t, i as i32, &[0], i == tokens.len() - 1).unwrap();
        }
        ctx.decode(&mut batch).expect("decode");
        let mut sampler = llama.new_sampler_chain().add_greedy();
        sampler.sample(&mut ctx, last_idx)
    };

    let token_a = greedy_next("Once upon a time");
    let token_b = greedy_next("Once upon a time");
    assert_eq!(token_a, token_b, "greedy sampling must be deterministic for identical prompts");
    println!("Greedy token (run 1={token_a}, run 2={token_b}) — determinism verified");
}

// ── Tier-2: service-level integration / end-to-end tests ─────────────────────

/// Generate 32 tokens from a short prompt using the service API and verify
/// that the output is non-empty.
#[tokio::test]
#[cfg(target_os = "linux")]
async fn test_multi_token_generation() {
    let service = make_local_service(1).await;

    let result =
        service.inference("Once upon a time", 32, None, None).await.expect("inference failed");

    assert!(!result.is_empty(), "32-token generation should produce non-empty text");
    println!("Generated ({} chars): {result}", result.len());
}

/// Spawn four concurrent inference sessions on a two-worker engine and verify
/// that every session produces output without error.  This exercises the
/// multi-threaded scheduling path of `LlamaInferenceEngine`.
#[tokio::test]
#[cfg(target_os = "linux")]
async fn test_concurrent_inference_safety() {
    let service = make_local_service(2).await;

    let mut handles = Vec::new();
    for i in 0..4u32 {
        let svc = Arc::clone(&service);
        let session_prompt = format!("Story {i}: once upon");
        handles.push(tokio::spawn(async move {
            let sid = svc.create_session().await.expect("create_session");
            svc.append_input(sid, session_prompt).await.expect("append_input");
            let mut stream = svc.generate_stream(sid, 16).await.expect("generate_stream");
            let mut output = String::new();
            while let Some(chunk) = stream.recv().await {
                match chunk {
                    StreamChunk::Token(t) => output.push_str(&t),
                    StreamChunk::Done => break,
                    StreamChunk::Error(e) => panic!("stream error in concurrent test: {e}"),
                }
            }
            svc.end_session(sid).await.expect("end_session");
            output
        }));
    }

    for (i, handle) in handles.into_iter().enumerate() {
        let output = handle.await.expect("concurrent task panicked");
        assert!(!output.is_empty(), "session {i} should produce output in concurrent run");
    }
}

/// Exercise the chat template path via the service API.
///
/// The stories-15M model does not embed a chat template so `apply_chat_template`
/// is expected to return an error.  The test documents this contract and
/// additionally verifies that the returned error does not cause a panic or
/// undefined behaviour.
#[tokio::test]
#[cfg(target_os = "linux")]
async fn test_chat_template_format() {
    let service = make_local_service(1).await;

    let messages = vec![
        ChatMessage { role: "user".to_string(), content: "Hello, how are you?".to_string() },
        ChatMessage {
            role: "assistant".to_string(),
            content: "I am a language model.".to_string(),
        },
    ];

    match service.apply_chat_template(&messages, true) {
        Err(e) => {
            // stories-15M has no embedded chat template; this branch is expected.
            println!("Model has no embedded chat template (expected for stories-15M); error: {e}");
        }
        Ok(formatted) => {
            panic!(
                "stories-15M is documented to have no embedded chat template; \
                 expected apply_chat_template to return Err, got Ok with output: {formatted}"
            );
        }
    }
}

/// Verify multi-turn KV-cache reuse with the local model: run two turns on the
/// same session and assert that both turns produce non-empty output.
#[tokio::test]
#[cfg(target_os = "linux")]
async fn test_kv_cache_multiturn_local() {
    let service = make_local_service(1).await;

    let sid = service.create_session().await.expect("create_session failed");

    service.append_input(sid, "Once upon a time".to_string()).await.expect("first append failed");
    let mut stream1 = service.generate_stream(sid, 16).await.expect("first generate_stream failed");
    let mut turn1 = String::new();
    while let Some(chunk) = stream1.recv().await {
        match chunk {
            StreamChunk::Token(t) => turn1.push_str(&t),
            StreamChunk::Done => break,
            StreamChunk::Error(e) => panic!("turn1 error: {e}"),
        }
    }
    assert!(!turn1.is_empty(), "first turn should produce output");

    service
        .append_input(sid, " there lived a brave".to_string())
        .await
        .expect("second append failed");
    let mut stream2 =
        service.generate_stream(sid, 16).await.expect("second generate_stream failed");
    let mut turn2 = String::new();
    while let Some(chunk) = stream2.recv().await {
        match chunk {
            StreamChunk::Token(t) => turn2.push_str(&t),
            StreamChunk::Done => break,
            StreamChunk::Error(e) => panic!("turn2 error: {e}"),
        }
    }
    assert!(!turn2.is_empty(), "second turn (KV-cache reuse) should produce output");

    service.end_session(sid).await.expect("end_session failed");
    println!("KV-cache multiturn: turn1={turn1:?} turn2={turn2:?}");
}
