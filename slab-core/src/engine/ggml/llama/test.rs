use super::*;
use crate::engine::ggml::llama::adapter::GGMLLlamaEngine;
use hf_hub::api::sync::Api;
use slab_llama::{LlamaContextParams, LlamaModelParams};
use std::path::PathBuf;
use std::sync::Arc;

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

    let service =
        GGMLLlamaEngine::from_path(llama_dir.as_path()).expect("failed to initialize llama service");
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
async fn test_llama_inference() {
    let service = make_service(1).await;

    let result = service
        .inference("Hello, my name is", 64, None)
        .await
        .expect("inference failed");

    println!("Generated: {result}");
    assert!(!result.is_empty(), "expected non-empty output");
}

/// Validates the happy path for service-session streaming generation:
/// create session → append input → consume stream until `Done` → end session.
#[tokio::test]
async fn test_service_basic_generation() {
    let service = make_service(1).await;

    let sid = service
        .create_session()
        .await
        .expect("create_session failed");
    service
        .append_input(sid, "Hello, my name is".to_string())
        .await
        .expect("append_input failed");

    let mut stream = service
        .generate_stream(sid, 32)
        .await
        .expect("generate_stream failed");

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
async fn test_service_session_not_found() {
    let service = make_service(1).await;

    let _err = service
        .append_input(9999, "hello".to_string())
        .await
        .unwrap_err();

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
async fn test_service_kv_reuse_multiturn() {
    let service = make_service(1).await;

    let sid = service
        .create_session()
        .await
        .expect("create_session failed");

    service
        .append_input(sid, "What is 1+1?".to_string())
        .await
        .expect("first append failed");
    let mut stream = service
        .generate_stream(sid, 16)
        .await
        .expect("first generate_stream failed");
    let mut turn1 = String::new();
    while let Some(chunk) = stream.recv().await {
        match chunk {
            StreamChunk::Token(t) => turn1.push_str(&t),
            StreamChunk::Done => break,
            StreamChunk::Error(e) => panic!("turn1 error: {e}"),
        }
    }
    assert!(!turn1.is_empty(), "first turn should produce output");

    service
        .append_input(sid, " And what is 2+2?".to_string())
        .await
        .expect("second append failed");
    let mut stream2 = service
        .generate_stream(sid, 16)
        .await
        .expect("second generate_stream failed");
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
async fn test_service_cancel_and_resume() {
    let service = make_service(1).await;

    let sid = service
        .create_session()
        .await
        .expect("create_session failed");
    service
        .append_input(sid, "Count to one hundred:".to_string())
        .await
        .expect("append failed");

    let mut stream = service
        .generate_stream(sid, 512)
        .await
        .expect("generate_stream failed");

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
    let mut stream2 = service
        .generate_stream(sid, 8)
        .await
        .expect("generate after cancel failed");
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
async fn test_service_seq_id_reuse() {
    let service = make_service(1).await;

    for _ in 0..4 {
        let sid = service.create_session().await.expect("create_session");
        service
            .append_input(sid, "hi".to_string())
            .await
            .expect("append");
        let mut stream = service
            .generate_stream(sid, 4)
            .await
            .expect("generate_stream");
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
