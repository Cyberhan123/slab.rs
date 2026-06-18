use chrono::Utc;
use slab_types::{RuntimeBackendId, RuntimeBackendLoadSpec};
use std::path::{Path, PathBuf};

use crate::domain::models::{
    ManagedModelBackendId, ModelLoadCommand, ModelSpec, UnifiedModel, UnifiedModelKind,
    UnifiedModelStatus,
};
use crate::error::{AppCoreError, AppCoreErrorData};
use crate::infra::model_packs;
use crate::test_support::{TestAppCore, ready_local_llama_command};

use super::super::runtime::*;

fn local_llama_model(id: &str, local_path: &str) -> UnifiedModel {
    UnifiedModel {
        id: id.to_owned(),
        display_name: id.to_owned(),
        kind: UnifiedModelKind::Local,
        backend_id: Some(ManagedModelBackendId::GgmlLlama),
        capabilities: Vec::new(),
        status: UnifiedModelStatus::Ready,
        spec: ModelSpec { local_path: Some(local_path.to_owned()), ..ModelSpec::default() },
        runtime_presets: None,
        materialized_artifacts: std::collections::BTreeMap::new(),
        selected_download_source: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

#[test]
fn catalog_model_pack_path_uses_imported_pack_for_downloaded_model_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let model = local_llama_model("Qwen3.5-9B", "C:/models/Qwen3.5-9B-Q8_0.gguf");
    let pack_path = model_packs::model_pack_file_path(dir.path(), &model.id);
    std::fs::write(&pack_path, []).expect("pack marker");

    assert_eq!(
        catalog_model_pack_path(dir.path(), &model, model.spec.local_path.as_deref().unwrap()),
        Some(pack_path)
    );
}

#[test]
fn catalog_model_pack_path_keeps_explicit_pack_path() {
    let model = local_llama_model("Qwen3.5-9B", "C:/models/Qwen3.5-9B.slab");

    assert_eq!(
        catalog_model_pack_path(
            Path::new("C:/other-models"),
            &model,
            model.spec.local_path.as_deref().unwrap()
        ),
        Some(PathBuf::from("C:/models/Qwen3.5-9B.slab"))
    );
}

#[tokio::test]
async fn model_runtime_loads_catalog_llama_by_model_id() {
    let app = TestAppCore::new().await;
    let model_path = app.write_model_file("runtime-ready.gguf");
    let model = app
        .model
        .create_model(ready_local_llama_command("runtime-load", &model_path))
        .await
        .expect("create runtime model");
    app.runtime.allow_backend(RuntimeBackendId::GgmlLlama);

    let status = app
        .model
        .load_model(ModelLoadCommand {
            model_id: Some(model.id.clone()),
            backend_id: None,
            model_path: None,
            num_workers: Some(2),
        })
        .await
        .expect("load catalog model");

    assert_eq!(status.backend, RuntimeBackendId::GgmlLlama.to_string());
    assert_eq!(status.status, "ready");
    let loads = app.runtime.loads();
    assert_eq!(loads.len(), 1);
    match &loads[0] {
        RuntimeBackendLoadSpec::GgmlLlama(config) => {
            assert_eq!(config.model_path, model_path);
            assert_eq!(config.num_workers, 2);
            assert!(config.chat_template.is_none());
            assert!(config.gbnf.is_none());
        }
        other => panic!("unexpected load spec: {other:?}"),
    }
}

#[tokio::test]
async fn model_runtime_state_tracks_loaded_and_active_catalog_model() {
    let app = TestAppCore::new().await;
    let model_path = app.write_model_file("runtime-state.gguf");
    let model = app
        .model
        .create_model(ready_local_llama_command("runtime-state", &model_path))
        .await
        .expect("create runtime model");
    app.runtime.allow_backend(RuntimeBackendId::GgmlLlama);

    let initial = app.model.runtime_state_for_model(&model).await.expect("runtime state");
    assert_eq!(initial.backend_id, RuntimeBackendId::GgmlLlama);
    assert!(!initial.loaded);
    assert!(!initial.active);
    assert_eq!(initial.active_refs, 0);

    app.model
        .load_model(ModelLoadCommand {
            model_id: Some(model.id.clone()),
            backend_id: None,
            model_path: None,
            num_workers: None,
        })
        .await
        .expect("load catalog model");

    let loaded = app.model.runtime_state_for_model(&model).await.expect("runtime state");
    assert!(loaded.loaded);
    assert!(!loaded.active);
    assert_eq!(loaded.active_refs, 0);

    let _guard = app.auto_unload.acquire(RuntimeBackendId::GgmlLlama).await;
    let active = app.model.runtime_state_for_model(&model).await.expect("runtime state");
    assert!(active.loaded);
    assert!(active.active);
    assert_eq!(active.active_refs, 1);
}

#[tokio::test]
async fn model_runtime_loads_catalog_candle_llama_by_model_id() {
    let app = TestAppCore::new().await;
    let model_path = app.write_model_file("runtime-ready-candle.gguf");
    let mut command = ready_local_llama_command("runtime-load-candle", &model_path);
    command.backend_id = Some(ManagedModelBackendId::CandleLlama);
    let model = app.model.create_model(command).await.expect("create candle runtime model");
    app.runtime.allow_backend(RuntimeBackendId::CandleLlama);

    let status = app
        .model
        .load_model(ModelLoadCommand {
            model_id: Some(model.id.clone()),
            backend_id: None,
            model_path: None,
            num_workers: None,
        })
        .await
        .expect("load candle catalog model");

    assert_eq!(status.backend, RuntimeBackendId::CandleLlama.to_string());
    assert_eq!(status.status, "ready");
    let loads = app.runtime.loads();
    assert_eq!(loads.len(), 1);
    match &loads[0] {
        RuntimeBackendLoadSpec::CandleLlama(config) => {
            assert_eq!(config.model_path, model_path);
            assert!(config.tokenizer_path.is_none());
            assert!(config.device.is_none());
        }
        other => panic!("unexpected load spec: {other:?}"),
    }
}

#[tokio::test]
async fn model_runtime_unavailable_backend_rejects_before_gateway_call() {
    let app = TestAppCore::new().await;
    let model_path = app.write_model_file("runtime-unavailable.gguf");
    let model = app
        .model
        .create_model(ready_local_llama_command("runtime-unavailable", &model_path))
        .await
        .expect("create runtime model");

    let error = app
        .model
        .load_model(ModelLoadCommand {
            model_id: Some(model.id),
            backend_id: None,
            model_path: None,
            num_workers: None,
        })
        .await
        .expect_err("unavailable backend should reject");

    let AppCoreError::BadRequestData { data, .. } = &error else {
        panic!("unexpected error: {error}");
    };
    let AppCoreErrorData::RuntimeEngineExhausted { attempts, .. } = data.as_ref() else {
        panic!("unexpected error data: {data:?}");
    };
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].engine, RuntimeBackendId::GgmlLlama.to_string());
    assert!(attempts[0].message.contains("gRPC endpoint is not configured"));
    assert!(app.runtime.loads().is_empty());
}

#[tokio::test]
async fn model_runtime_unloads_catalog_model_by_model_id() {
    let app = TestAppCore::new().await;
    let model_path = app.write_model_file("runtime-unload.gguf");
    let model = app
        .model
        .create_model(ready_local_llama_command("runtime-unload", &model_path))
        .await
        .expect("create runtime model");
    app.runtime.allow_backend(RuntimeBackendId::GgmlLlama);

    let status = app
        .model
        .unload_model(ModelLoadCommand {
            model_id: Some(model.id),
            backend_id: None,
            model_path: None,
            num_workers: None,
        })
        .await
        .expect("unload catalog model");

    assert_eq!(status.backend, RuntimeBackendId::GgmlLlama.to_string());
    assert_eq!(status.status, "unloaded");
    assert_eq!(app.runtime.unloads(), vec![RuntimeBackendId::GgmlLlama]);
}

#[tokio::test]
async fn model_runtime_rejects_manual_unload_while_inference_active() {
    let app = TestAppCore::new().await;
    let model_path = app.write_model_file("runtime-unload-busy.gguf");
    let model = app
        .model
        .create_model(ready_local_llama_command("runtime-unload-busy", &model_path))
        .await
        .expect("create runtime model");
    app.runtime.allow_backend(RuntimeBackendId::GgmlLlama);
    let _guard = app.auto_unload.acquire(RuntimeBackendId::GgmlLlama).await;

    let error = app
        .model
        .unload_model(ModelLoadCommand {
            model_id: Some(model.id),
            backend_id: None,
            model_path: None,
            num_workers: None,
        })
        .await
        .expect_err("active inference should block manual unload");

    assert!(
        matches!(&error, AppCoreError::Conflict(message) if message.contains("active inference")),
        "unexpected error: {error}"
    );
    assert!(app.runtime.unloads().is_empty());
}
