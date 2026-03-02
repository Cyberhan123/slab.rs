#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use tokio::sync::mpsc;

    use crate::runtime::backend::admission::ResourceManager;
    use crate::runtime::backend::protocol::{BackendOp, BackendReply, BackendRequest};
    use crate::runtime::orchestrator::Orchestrator;
    use crate::runtime::pipeline::PipelineBuilder;
    use crate::runtime::types::{Payload, TaskStatus};

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
        let mut rm = ResourceManager::new();
        rm.register_backend("test-backend", 2);

        let p1 = rm.try_acquire("test-backend").expect("first permit");
        let p2 = rm.try_acquire("test-backend").expect("second permit");
        assert!(
            rm.try_acquire("test-backend").is_err(),
            "third permit should be denied"
        );
        drop(p1);
        // After releasing one permit, a new acquisition should succeed.
        let _p3 = rm
            .try_acquire("test-backend")
            .expect("permit after release");
        drop(p2);
    }

    #[test]
    fn permit_unknown_backend_returns_busy() {
        let mut rm = ResourceManager::new();
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
    async fn gpu_stage_dispatches_and_receives_reply() {
        let orchestrator = {
            let mut rm = ResourceManager::new();
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
            options: Payload::default(),
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
        // Register backend with capacity 0 so that no permit is ever available.
        // The orchestrator will wait up to GPU_ACQUIRE_TIMEOUT (very short in
        // test builds) and then fail the task with RuntimeError::Timeout.
        let mut rm = ResourceManager::new();
        rm.register_backend("busy-backend", 0);
        let orchestrator = Orchestrator::start(rm, 64);

        let (ingress_tx, _ingress_rx) = mpsc::channel::<BackendRequest>(4);
        let op = BackendOp {
            name: "noop".to_owned(),
            options: Payload::default(),
        };

        let task_id = PipelineBuilder::new(orchestrator.clone(), text_payload("x"))
            .gpu("noop-stage", "busy-backend", op, ingress_tx)
            .run()
            .await
            .expect("submit should succeed");

        // Wait up to 2 s; in test builds GPU_ACQUIRE_TIMEOUT is 200 ms so the
        // task should fail well within this window.
        let result = tokio::time::timeout(std::time::Duration::from_secs(2), async {
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
        .expect("task should fail within 2 s (GPU_ACQUIRE_TIMEOUT is 200 ms in tests)");

        assert!(
            matches!(result, TaskStatus::Failed { .. }),
            "task should fail after permit timeout, got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn streaming_pipeline_returns_stream_handle() {
        let mut rm = ResourceManager::new();
        rm.register_backend("stream-backend", 4);
        let orchestrator = Orchestrator::start(rm, 64);

        let (ingress_tx, mut ingress_rx) = mpsc::channel::<BackendRequest>(16);
        let op = BackendOp {
            name: "stream-gen".to_owned(),
            options: Payload::default(),
        };

        // Backend worker that emits a few tokens then Done.
        tokio::spawn(async move {
            while let Some(req) = ingress_rx.recv().await {
                let (stream_tx, stream_rx) =
                    mpsc::channel::<crate::runtime::backend::protocol::StreamChunk>(8);
                let _ = req.reply_tx.send(BackendReply::Stream(stream_rx));
                for word in ["hello", " ", "world"] {
                    let _ = stream_tx
                        .send(crate::runtime::backend::protocol::StreamChunk::Token(
                            word.to_owned(),
                        ))
                        .await;
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
                crate::runtime::backend::protocol::StreamChunk::Image(_) => {
                    panic!("unexpected image chunk now stream")
                }
            }
        }
        assert_eq!(tokens, "hello world");
    }

    // ── Broadcast channel tests ───────────────────────────────────────────────

    /// Verify that `acquire_with_timeout` waits for a permit to become available
    /// instead of failing immediately.
    #[tokio::test]
    async fn acquire_with_timeout_waits_for_permit() {
        use crate::runtime::backend::admission::ResourceManager;

        let mut rm = ResourceManager::new();
        rm.register_backend("serial-backend", 1);

        // Grab the single permit.
        let permit = rm.try_acquire("serial-backend").expect("first permit");

        // Spawn a task that releases the permit after a short delay.
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            drop(permit);
        });

        // acquire_with_timeout should succeed once the permit is released.
        let result = rm
            .acquire_with_timeout("serial-backend", std::time::Duration::from_secs(2))
            .await;
        assert!(result.is_ok(), "should acquire permit after it is released");
    }

    /// Integration-style test that mirrors the production worker loop:
    /// spawns N tasks each running a `biased` `tokio::select!` over a broadcast
    /// arm (management commands) and an mpsc arm (inference work).  Verifies
    /// that after broadcasting `WorkerCommand::Unload`, every worker:
    ///   1. drops its in-memory "context" flag, and
    ///   2. returns a failure reply for any pending inference request
    ///      (because the context is now gone).
    #[tokio::test]
    async fn worker_broadcast_unload_clears_all_worker_contexts() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use tokio::sync::{broadcast, mpsc, oneshot};

        use crate::runtime::backend::protocol::WorkerCommand;

        const NUM_WORKERS: usize = 3;

        let (bc_tx, _) = broadcast::channel::<WorkerCommand>(16);

        // Per-worker mpsc queues for simulated inference requests.
        // Each request carries a oneshot reply sender; the worker replies with
        // whether the context is still loaded.
        let mut infer_txs: Vec<mpsc::Sender<oneshot::Sender<bool>>> = Vec::new();
        // Shared context flags accessible from the test for post-hoc assertions.
        let mut ctx_flags: Vec<Arc<AtomicBool>> = Vec::new();
        // Per-worker acknowledgement channels for when Unload is observed.
        let mut ack_rxs: Vec<oneshot::Receiver<()>> = Vec::new();

        for _ in 0..NUM_WORKERS {
            let mut bc_rx = bc_tx.subscribe();
            let (infer_tx, mut infer_rx) = mpsc::channel::<oneshot::Sender<bool>>(8);
            let ctx = Arc::new(AtomicBool::new(true)); // "model is loaded"
            let ctx_w = Arc::clone(&ctx);
            let (ack_tx, ack_rx) = oneshot::channel::<()>();

            infer_txs.push(infer_tx);
            ctx_flags.push(ctx);
            ack_rxs.push(ack_rx);

            tokio::spawn(async move {
                let mut ack_tx = Some(ack_tx);
                loop {
                    tokio::select! {
                        biased; // prioritize broadcast over inference

                        cmd = bc_rx.recv() => {
                            match cmd {
                                Ok(WorkerCommand::Unload { .. }) => {
                                    // Mirror the production behavior: drop context.
                                    ctx_w.store(false, Ordering::SeqCst);
                                    if let Some(tx) = ack_tx.take() {
                                        let _ = tx.send(());
                                    }
                                    // Exit after unload, matching production break.
                                    break;
                                }
                                // Other management commands (LoadLibrary, ReloadLibrary,
                                // LoadModel) are not relevant to this test scenario —
                                // ignore them and keep the loop running.
                                Ok(_) => {}
                                // Channel closed → exit.
                                Err(_) => break,
                            }
                        }

                        infer_req = infer_rx.recv() => {
                            match infer_req {
                                Some(reply_tx) => {
                                    // Reply with whether context is still loaded.
                                    let _ = reply_tx.send(ctx_w.load(Ordering::SeqCst));
                                }
                                None => break,
                            }
                        }
                    }
                }
            });
        }

        // Confirm all workers are ready before broadcasting (they are, since
        // the receivers were subscribed before spawning, but yield once to let
        // the spawned tasks enter their select! loops).
        tokio::task::yield_now().await;

        // Broadcast Unload to all workers.  Use sender_id=usize::MAX so that
        // no worker's self-echo guard fires and every worker processes it.
        bc_tx
            .send(WorkerCommand::Unload { sender_id: usize::MAX })
            .expect("broadcast should reach at least one subscriber");

        // Wait for each worker to acknowledge the Unload.
        for ack_rx in ack_rxs {
            tokio::time::timeout(std::time::Duration::from_secs(2), ack_rx)
                .await
                .expect("worker should ack Unload within 2 s")
                .expect("ack sender dropped without sending");
        }

        // All context flags must now be false.
        for (i, ctx) in ctx_flags.iter().enumerate() {
            assert!(
                !ctx.load(Ordering::SeqCst),
                "worker {i} context should be cleared after Unload"
            );
        }
    }

    // ── Storage tests ─────────────────────────────────────────────────────────
}
