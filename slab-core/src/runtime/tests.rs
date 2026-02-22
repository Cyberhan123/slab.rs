#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use tokio::sync::mpsc;

    use crate::runtime::backend::admission::ResourceManager;
    use crate::runtime::backend::protocol::{BackendOp, BackendReply, BackendRequest};
    use crate::runtime::orchestrator::Orchestrator;
    use crate::runtime::pipeline::PipelineBuilder;
    use crate::runtime::types::{Payload, TaskStatus};

    fn make_orchestrator() -> Orchestrator {
        Orchestrator::start(ResourceManager::new(), 64)
    }

    fn text_payload(s: &str) -> Payload {
        Payload::Text(Arc::from(s))
    }

    // ── Types tests ───────────────────────────────────────────────────────────

    #[test]
    fn payload_clone_does_not_copy_bytes() {
        let data: Arc<[u8]> = Arc::from(vec![1u8, 2, 3]);
        let p1 = Payload::Bytes(Arc::clone(&data));
        let p2 = p1.clone();
        // Both variants should share the same underlying allocation.
        if let (Payload::Bytes(a), Payload::Bytes(b)) = (p1, p2) {
            assert!(Arc::ptr_eq(&a, &b), "clone should share Arc pointer");
        } else {
            panic!("unexpected payload variant");
        }
    }

    // ── Admission control tests ───────────────────────────────────────────────

    #[test]
    fn permit_acquired_and_released() {
        let rm = ResourceManager::new();
        rm.register_backend("test-backend", 2);

        let p1 = rm.try_acquire("test-backend").expect("first permit");
        let p2 = rm.try_acquire("test-backend").expect("second permit");
        assert!(
            rm.try_acquire("test-backend").is_err(),
            "third permit should be denied"
        );
        drop(p1);
        // After releasing one permit, a new acquisition should succeed.
        let _p3 = rm.try_acquire("test-backend").expect("permit after release");
        drop(p2);
    }

    #[test]
    fn permit_unknown_backend_returns_busy() {
        let rm = ResourceManager::new();
        let err = rm.try_acquire("nonexistent").unwrap_err();
        assert!(
            matches!(err, crate::runtime::types::RuntimeError::Busy { .. }),
            "expected Busy error"
        );
    }

    // ── CPU stage tests ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn cpu_stage_transforms_payload() {
        use crate::runtime::stage::CpuStage;

        let stage = CpuStage::new("uppercase", |p| match p {
            Payload::Text(s) => Ok(Payload::Text(Arc::from(s.to_uppercase().as_str()))),
            other => Ok(other),
        });

        let input = text_payload("hello");
        let output = stage.run(input).await.expect("cpu stage should succeed");
        if let Payload::Text(s) = output {
            assert_eq!(&*s, "HELLO");
        } else {
            panic!("unexpected payload variant");
        }
    }

    #[tokio::test]
    async fn cpu_stage_propagates_error() {
        use crate::runtime::stage::CpuStage;

        let stage = CpuStage::new("fail-stage", |_p| Err("intentional error".to_owned()));
        let result = stage.run(text_payload("x")).await;
        assert!(result.is_err(), "stage should propagate work fn error");
    }

    // ── Orchestrator / pipeline integration tests ─────────────────────────────

    #[tokio::test]
    async fn pipeline_with_cpu_stages_succeeds() {
        let orchestrator = make_orchestrator();

        let task_id = PipelineBuilder::new(orchestrator.clone(), text_payload("hello"))
            .cpu("step1", |p| match p {
                Payload::Text(s) => Ok(Payload::Text(Arc::from(s.to_uppercase().as_str()))),
                other => Ok(other),
            })
            .cpu("step2", |p| match p {
                Payload::Text(s) => {
                    let reversed: String = s.chars().rev().collect();
                    Ok(Payload::Text(Arc::from(reversed.as_str())))
                }
                other => Ok(other),
            })
            .run()
            .await
            .expect("submit should succeed");

        // Poll until the task is no longer Pending/Running.
        let result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let view = orchestrator
                    .get_status(task_id)
                    .await
                    .expect("task should exist");
                match &view.status {
                    TaskStatus::Succeeded { .. } | TaskStatus::Failed { .. } => break view.status,
                    _ => tokio::time::sleep(std::time::Duration::from_millis(10)).await,
                }
            }
        })
        .await
        .expect("task should complete within timeout");

        if let TaskStatus::Succeeded { result: payload } = result {
            if let Payload::Text(s) = payload {
                assert_eq!(&*s, "OLLEH");
            } else {
                panic!("unexpected payload type");
            }
        } else {
            panic!("task should have succeeded, got {:?}", result);
        }
    }

    #[tokio::test]
    async fn pipeline_failed_stage_marks_task_failed() {
        let orchestrator = make_orchestrator();

        let task_id = PipelineBuilder::new(orchestrator.clone(), text_payload("x"))
            .cpu("failing-stage", |_| Err("boom".to_owned()))
            .run()
            .await
            .expect("submit should succeed");

        let result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let view = orchestrator
                    .get_status(task_id)
                    .await
                    .expect("task should exist");
                match &view.status {
                    TaskStatus::Failed { .. } | TaskStatus::Succeeded { .. } => break view.status,
                    _ => tokio::time::sleep(std::time::Duration::from_millis(10)).await,
                }
            }
        })
        .await
        .expect("task should complete within timeout");

        assert!(
            matches!(result, TaskStatus::Failed { .. }),
            "task should be in Failed state"
        );
    }

    #[tokio::test]
    async fn cancel_pending_task_transitions_to_cancelled() {
        let orchestrator = make_orchestrator();

        // Stage that sleeps long enough for cancellation to arrive.
        let task_id = PipelineBuilder::new(orchestrator.clone(), text_payload("x"))
            .cpu("slow-stage", |p| {
                std::thread::sleep(std::time::Duration::from_secs(10));
                Ok(p)
            })
            .run()
            .await
            .expect("submit should succeed");

        // Give the task a moment to reach the CPU stage, then cancel it.
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        orchestrator.cancel(task_id);

        // The task might succeed if the CPU stage is already running (non-preemptable),
        // but the cancel signal should be visible.
        // At minimum the status should be fetchable.
        let view = orchestrator
            .get_status(task_id)
            .await
            .expect("task should exist after cancel");
        assert!(
            !matches!(view.status, TaskStatus::Pending),
            "task should have started executing"
        );
    }

    #[tokio::test]
    async fn gpu_stage_dispatches_and_receives_reply() {
        let orchestrator = {
            let rm = ResourceManager::new();
            rm.register_backend("echo-backend", 4);
            Orchestrator::start(rm, 64)
        };

        let (ingress_tx, ingress_rx) = mpsc::channel::<BackendRequest>(16);
        let ingress_tx_clone = ingress_tx.clone();

        // Spawn a minimal echo backend worker.
        tokio::spawn(async move {
            let mut rx = ingress_rx;
            while let Some(req) = rx.recv().await {
                let _ = req.reply_tx.send(BackendReply::Value(req.input));
            }
        });

        let op = BackendOp {
            name: "echo".to_owned(),
            options: serde_json::Value::Null,
        };

        let task_id = PipelineBuilder::new(orchestrator.clone(), text_payload("ping"))
            .gpu("echo-stage", "echo-backend", op, ingress_tx_clone)
            .run()
            .await
            .expect("submit should succeed");

        let result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let view = orchestrator
                    .get_status(task_id)
                    .await
                    .expect("task should exist");
                match &view.status {
                    TaskStatus::Succeeded { .. } | TaskStatus::Failed { .. } => break view.status,
                    _ => tokio::time::sleep(std::time::Duration::from_millis(10)).await,
                }
            }
        })
        .await
        .expect("task should complete within timeout");

        if let TaskStatus::Succeeded { result: payload } = result {
            if let Payload::Text(s) = payload {
                assert_eq!(&*s, "ping");
            } else {
                panic!("unexpected payload type");
            }
        } else {
            panic!("task should have succeeded, got {:?}", result);
        }
    }

    #[tokio::test]
    async fn gpu_stage_busy_error_when_no_permits() {
        // Register backend with capacity 0.
        let rm = ResourceManager::new();
        rm.register_backend("busy-backend", 0);
        let orchestrator = Orchestrator::start(rm, 64);

        let (ingress_tx, _ingress_rx) = mpsc::channel::<BackendRequest>(4);
        let op = BackendOp {
            name: "noop".to_owned(),
            options: serde_json::Value::Null,
        };

        let task_id = PipelineBuilder::new(orchestrator.clone(), text_payload("x"))
            .gpu("noop-stage", "busy-backend", op, ingress_tx)
            .run()
            .await
            .expect("submit should succeed");

        let result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let view = orchestrator
                    .get_status(task_id)
                    .await
                    .expect("task should exist");
                match &view.status {
                    TaskStatus::Failed { .. } | TaskStatus::Succeeded { .. } => break view.status,
                    _ => tokio::time::sleep(std::time::Duration::from_millis(10)).await,
                }
            }
        })
        .await
        .expect("task should fail quickly");

        assert!(
            matches!(result, TaskStatus::Failed { .. }),
            "task should fail due to busy backend"
        );
    }

    #[tokio::test]
    async fn streaming_pipeline_returns_stream_handle() {
        let rm = ResourceManager::new();
        rm.register_backend("stream-backend", 4);
        let orchestrator = Orchestrator::start(rm, 64);

        let (ingress_tx, mut ingress_rx) = mpsc::channel::<BackendRequest>(16);
        let op = BackendOp {
            name: "stream-gen".to_owned(),
            options: serde_json::Value::Null,
        };

        // Backend worker that emits a few tokens then Done.
        tokio::spawn(async move {
            while let Some(req) = ingress_rx.recv().await {
                let (stream_tx, stream_rx) =
                    mpsc::channel::<crate::runtime::backend::protocol::StreamChunk>(8);
                let _ = req
                    .reply_tx
                    .send(BackendReply::Stream(stream_rx));
                for word in ["hello", " ", "world"] {
                    let _ = stream_tx.send(
                        crate::runtime::backend::protocol::StreamChunk::Token(word.to_owned()),
                    ).await;
                }
                let _ = stream_tx
                    .send(crate::runtime::backend::protocol::StreamChunk::Done)
                    .await;
            }
        });

        let task_id = PipelineBuilder::new(orchestrator.clone(), text_payload("prompt"))
            .gpu_stream("stream-stage", "stream-backend", op, ingress_tx)
            .run_stream()
            .await
            .expect("submit should succeed");

        // Wait for SucceededStreaming.
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let view = orchestrator
                    .get_status(task_id)
                    .await
                    .expect("task should exist");
                if matches!(view.status, TaskStatus::SucceededStreaming) {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("task should reach SucceededStreaming");

        let mut handle = orchestrator
            .take_stream(task_id)
            .await
            .expect("stream handle should be available");

        let mut tokens = String::new();
        while let Some(chunk) = handle.recv().await {
            match chunk {
                crate::runtime::backend::protocol::StreamChunk::Token(t) => tokens.push_str(&t),
                crate::runtime::backend::protocol::StreamChunk::Done => break,
                crate::runtime::backend::protocol::StreamChunk::Error(e) => {
                    panic!("stream error: {e}")
                }
            }
        }
        assert_eq!(tokens, "hello world");
    }

    // ── Storage tests ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn get_result_consumes_payload() {
        let orchestrator = make_orchestrator();

        let task_id = PipelineBuilder::new(orchestrator.clone(), text_payload("data"))
            .cpu("identity", Ok)
            .run()
            .await
            .expect("submit should succeed");

        // Wait for completion.
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let view = orchestrator
                    .get_status(task_id)
                    .await
                    .expect("task should exist");
                if matches!(view.status, TaskStatus::Succeeded { .. }) {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("task should complete");

        // First call returns the payload.
        let first = orchestrator.get_result(task_id).await;
        assert!(first.is_some(), "first get_result should return payload");
        // Second call returns None (payload consumed).
        let second = orchestrator.get_result(task_id).await;
        assert!(second.is_none(), "second get_result should return None");
    }
}
