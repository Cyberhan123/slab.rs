use std::path::PathBuf;

use futures::StreamExt;
use tokio::sync::mpsc;

use crate::base::error::CoreError;
use crate::base::types::{Payload, StreamChunk};
use crate::dispatch::{
    BackendDriverDescriptor, DispatchPlanner, DriverLoadStyle, ModelSourceKind,
};
use crate::runtime::{BuiltinDriversConfig, Runtime};
use crate::scheduler::backend::admission::{ResourceManager, ResourceManagerConfig};
use crate::scheduler::backend::protocol::{
    BackendReply, DriverRequestKind, ManagementEvent, RequestRoute,
};
use crate::scheduler::orchestrator::Orchestrator;
use crate::spec::{
    Capability, ModelFamily, ModelSource, ModelSpec, TaskKind, TextGenerationRequest,
};
use crate::task::TaskState;

fn text_spec() -> ModelSpec {
    ModelSpec::new(
        ModelFamily::Llama,
        Capability::TextGeneration,
        ModelSource::LocalPath {
            path: PathBuf::from("fixtures/fake-model.gguf"),
        },
    )
}

fn test_runtime() -> Runtime {
    let mut rm = ResourceManager::with_config(ResourceManagerConfig {
        backend_capacity: 2,
        ..ResourceManagerConfig::default()
    });
    rm.register_backend("test-llama", |shared_rx, _control_tx| {
        tokio::spawn(async move {
            let mut loaded = false;

            loop {
                let req = {
                    let mut lock = shared_rx.lock().await;
                    lock.recv().await
                };

                let Some(req) = req else {
                    break;
                };

                match req.driver_kind().expect("backend request should be typed") {
                    DriverRequestKind::Management { event, .. } => {
                        match event {
                            ManagementEvent::Initialize => {}
                            ManagementEvent::LoadModel => loaded = true,
                            ManagementEvent::UnloadModel => loaded = false,
                        }
                        let _ = req.reply_tx.send(BackendReply::Value(Payload::None));
                    }
                    DriverRequestKind::Inference(invocation) => {
                        if !loaded {
                            let _ = req
                                .reply_tx
                                .send(BackendReply::Error("model not loaded".to_owned()));
                            continue;
                        }

                        match invocation.route {
                            RequestRoute::Inference => {
                                let text = req
                                    .input
                                    .to_text_string()
                                    .expect("text generation input should be text");

                                if text == "__wait__" {
                                    let mut cancel_rx = req.cancel_rx.clone();
                                    let _ = cancel_rx.wait_for(|cancelled| *cancelled).await;
                                    let _ =
                                        req.reply_tx.send(BackendReply::Error("cancelled".into()));
                                } else {
                                    let _ = req.reply_tx.send(BackendReply::Value(Payload::text(
                                        text,
                                    )));
                                }
                            }
                            RequestRoute::InferenceStream => {
                                let text = req
                                    .input
                                    .to_text_string()
                                    .expect("streaming input should be text");
                                let (stream_tx, stream_rx) = mpsc::channel(4);
                                let _ = req.reply_tx.send(BackendReply::Stream(stream_rx));

                                tokio::spawn(async move {
                                    let _ = stream_tx.send(StreamChunk::Token(text)).await;
                                    let _ = stream_tx.send(StreamChunk::Done).await;
                                });
                            }
                            other => {
                                let _ = req.reply_tx.send(BackendReply::Error(format!(
                                    "unsupported route for test backend: {other:?}"
                                )));
                            }
                        }
                    }
                }
            }
        });
    });

    let planner = DispatchPlanner::new(vec![BackendDriverDescriptor {
        driver_id: "candle.llama".to_owned(),
        backend_id: "test-llama".to_owned(),
        family: ModelFamily::Llama,
        capability: Capability::TextGeneration,
        supported_sources: vec![
            ModelSourceKind::LocalPath,
            ModelSourceKind::LocalArtifacts,
            ModelSourceKind::HuggingFace,
        ],
        supports_streaming: true,
        load_style: DriverLoadStyle::ModelOnly,
        priority: 0,
    }]);

    Runtime::new(
        Orchestrator::start(rm, 32),
        planner,
        BuiltinDriversConfig::default(),
    )
}

#[tokio::test]
async fn auto_model_typed_handle_runs_and_streams() {
    let runtime = test_runtime();
    let model = runtime.model(text_spec());
    let text_model = model
        .text_generation()
        .expect("text model handle should be available");

    let deployment = text_model.load().await.expect("model should load");
    assert_eq!(deployment.resolved.driver_id, "candle.llama");

    let response = text_model
        .run(TextGenerationRequest {
            prompt: "hello runtime".to_owned(),
            ..TextGenerationRequest::default()
        })
        .await
        .expect("run should succeed");
    assert_eq!(response.text, "hello runtime");

    let chunks = text_model
        .stream(TextGenerationRequest {
            prompt: "hello stream".to_owned(),
            ..TextGenerationRequest::default()
        })
        .await
        .expect("stream should start")
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .expect("stream chunks should decode");

    let combined = chunks
        .into_iter()
        .map(|chunk| chunk.delta)
        .collect::<Vec<_>>()
        .join("");
    assert_eq!(combined, "hello stream");
}

#[tokio::test]
async fn pipeline_submit_returns_task_handle_with_lifecycle_controls() {
    let runtime = test_runtime();
    let model = runtime.model(text_spec());
    let typed = model
        .text_generation()
        .expect("text model handle should be available");

    let pipeline = crate::pipeline(&runtime, TaskKind::TextGeneration, &typed)
        .expect("pipeline creation should succeed")
        .into_text_generation()
        .expect("pipeline should be typed");

    let handle = pipeline
        .submit(TextGenerationRequest {
            prompt: "hello task".to_owned(),
            ..TextGenerationRequest::default()
        })
        .await
        .expect("task submission should succeed");

    let snapshot = handle.status().await.expect("status should be readable");
    assert_eq!(snapshot.task_kind, TaskKind::TextGeneration);

    let response = handle.result().await.expect("task should succeed");
    assert_eq!(response.text, "hello task");

    let snapshot = handle
        .status()
        .await
        .expect("status should still be readable after result");
    assert!(matches!(snapshot.status, TaskState::ResultConsumed));

    handle.purge().await;

    let error = handle
        .status()
        .await
        .expect_err("purged task should be gone");
    assert!(matches!(error, CoreError::TaskNotFound { .. }));
}

#[tokio::test]
async fn pipeline_submit_exposes_stream_and_cancel_and_purge() {
    let runtime = test_runtime();
    let pipeline = crate::pipeline(&runtime, TaskKind::TextGeneration, text_spec())
        .expect("pipeline creation should succeed")
        .into_text_generation()
        .expect("pipeline should be typed");

    let stream_handle = pipeline
        .submit(TextGenerationRequest {
            prompt: "stream via task handle".to_owned(),
            stream: true,
            ..TextGenerationRequest::default()
        })
        .await
        .expect("streaming task should submit");

    let stream_chunks = stream_handle
        .take_stream()
        .await
        .expect("stream handle should be available")
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .expect("stream chunks should decode");

    let combined = stream_chunks
        .into_iter()
        .map(|chunk| chunk.delta)
        .collect::<Vec<_>>()
        .join("");
    assert_eq!(combined, "stream via task handle");

    let slow_handle = pipeline
        .submit(TextGenerationRequest {
            prompt: "__wait__".to_owned(),
            ..TextGenerationRequest::default()
        })
        .await
        .expect("slow task should submit");

    slow_handle.cancel();
    slow_handle.cancel_and_purge().await;

    let error = slow_handle
        .status()
        .await
        .expect_err("cancelled and purged task should be gone");
    assert!(matches!(error, CoreError::TaskNotFound { .. }));
}
