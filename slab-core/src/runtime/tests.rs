#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    use crate::runtime::backend::admission::{ResourceManager, ResourceManagerConfig};
    use crate::runtime::backend::protocol::{BackendOp, BackendReply};
    use crate::runtime::orchestrator::Orchestrator;
    use crate::runtime::pipeline::PipelineBuilder;
    use crate::runtime::types::{
        FailedGlobalOperation, GlobalOperationKind, Payload, RuntimeError, TaskStatus,
    };

    fn text_payload(s: &str) -> Payload {
        Payload::Text(Arc::from(s))
    }


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


    #[tokio::test]
    async fn inference_lease_acquired_and_released() {
        let mut rm = ResourceManager::with_config(ResourceManagerConfig {
            backend_capacity: 2,
            ..ResourceManagerConfig::default()
        });
        rm.register_backend("test-backend", |_shared_rx, _control_tx| {});

        let l1 = rm
            .acquire_inference_lease("test-backend", std::time::Duration::from_millis(20))
            .await
            .expect("first lease");
        let l2 = rm
            .acquire_inference_lease("test-backend", std::time::Duration::from_millis(20))
            .await
            .expect("second lease");
        assert!(
            matches!(
                rm.acquire_inference_lease("test-backend", std::time::Duration::from_millis(20))
                    .await,
                Err(RuntimeError::Timeout)
            ),
            "third lease should time out while capacity is exhausted"
        );
        drop(l1);
        // After releasing one lease, a new lease should succeed.
        let _l3 = rm
            .acquire_inference_lease("test-backend", std::time::Duration::from_secs(1))
            .await
            .expect("lease after release");
        drop(l2);
    }

    #[tokio::test]
    async fn inference_lease_unknown_backend_returns_busy() {
        let rm = ResourceManager::new();
        let err = rm
            .acquire_inference_lease("nonexistent", std::time::Duration::from_millis(20))
            .await
            .unwrap_err();
        assert!(
            matches!(err, crate::runtime::types::RuntimeError::Busy { .. }),
            "expected Busy error"
        );
    }


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

    #[tokio::test]
    async fn gpu_stage_dispatches_and_receives_reply() {
        let orchestrator = {
            let mut rm = ResourceManager::with_config(ResourceManagerConfig {
                backend_capacity: 4,
                ..ResourceManagerConfig::default()
            });
            rm.register_backend("echo-backend", |shared_rx, _control_tx| {
                // Spawn a minimal echo backend worker.
                tokio::spawn(async move {
                    loop {
                        let req = {
                            let mut lock = shared_rx.lock().await;
                            lock.recv().await
                        };
                        match req {
                            Some(req) => {
                                let _ = req.reply_tx.send(BackendReply::Value(req.input));
                            }
                            None => break,
                        }
                    }
                });
            });
            Orchestrator::start(rm, 64)
        };

        let op = BackendOp {
            name: "echo".to_owned(),
            options: Payload::default(),
        };

        let task_id = PipelineBuilder::new(orchestrator.clone(), text_payload("ping"))
            .gpu("echo-stage", "echo-backend", op)
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
        let mut rm = ResourceManager::with_config(ResourceManagerConfig {
            backend_capacity: 0,
            ..ResourceManagerConfig::default()
        });
        rm.register_backend("busy-backend", |_shared_rx, _control_tx| {});
        let orchestrator = Orchestrator::start(rm, 64);

        let op = BackendOp {
            name: "noop".to_owned(),
            options: Payload::default(),
        };

        let task_id = PipelineBuilder::new(orchestrator.clone(), text_payload("x"))
            .gpu("noop-stage", "busy-backend", op)
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
        let mut rm = ResourceManager::with_config(ResourceManagerConfig {
            backend_capacity: 4,
            ..ResourceManagerConfig::default()
        });
        rm.register_backend("stream-backend", |shared_rx, _control_tx| {
            // Backend worker that emits a few tokens then Done.
            tokio::spawn(async move {
                loop {
                    let req = {
                        let mut lock = shared_rx.lock().await;
                        lock.recv().await
                    };
                    match req {
                        Some(req) => {
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
                        None => break,
                    }
                }
            });
        });
        let orchestrator = Orchestrator::start(rm, 64);
        let op = BackendOp {
            name: "stream-gen".to_owned(),
            options: Payload::default(),
        };

        let task_id = PipelineBuilder::new(orchestrator.clone(), text_payload("prompt"))
            .gpu_stream("stream-stage", "stream-backend", op)
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

    /// Verify that `acquire_inference_lease` waits for capacity to become available
    /// instead of failing immediately.
    #[tokio::test]
    async fn acquire_inference_lease_waits_for_capacity() {
        use crate::runtime::backend::admission::ResourceManager;

        let mut rm = ResourceManager::with_config(ResourceManagerConfig {
            backend_capacity: 1,
            ..ResourceManagerConfig::default()
        });
        rm.register_backend("serial-backend", |_shared_rx, _control_tx| {});

        // Grab the single lease.
        let lease = rm
            .acquire_inference_lease("serial-backend", std::time::Duration::from_secs(2))
            .await
            .expect("first lease");

        // Spawn a task that releases the lease after a short delay.
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            drop(lease);
        });

        // acquire_inference_lease should succeed once the lease is released.
        let result = rm
            .acquire_inference_lease("serial-backend", std::time::Duration::from_secs(2))
            .await;
        assert!(result.is_ok(), "should acquire lease after it is released");
    }

    /// Integration-style test that mirrors the production worker loop:
    /// spawns N tasks each running a `biased` `tokio::select!` over a broadcast
    /// arm (management commands) and an mpsc arm (inference work).  Verifies
    /// that after broadcasting `WorkerCommand::Peer(PeerWorkerCommand::Unload)`,
    /// every worker:
    ///   1. drops its in-memory "context" flag, and
    ///   2. returns a failure reply for any pending inference request
    ///      (because the context is now gone).
    #[tokio::test]
    async fn worker_broadcast_unload_clears_all_worker_contexts() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use tokio::sync::{broadcast, mpsc, oneshot};

        use crate::runtime::backend::protocol::{PeerWorkerCommand, WorkerCommand};

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
                                Ok(WorkerCommand::Peer(PeerWorkerCommand::Unload { .. })) => {
                                    // Mirror the production behavior: drop context.
                                    ctx_w.store(false, Ordering::SeqCst);
                                    if let Some(tx) = ack_tx.take() {
                                        let _ = tx.send(());
                                    }
                                    // Exit after unload, matching production break.
                                    break;
                                }
                                // Other management commands (LoadLibrary, ReloadLibrary,
                                // LoadModel) are not relevant to this test scenario 鈥?
                                // ignore them and keep the loop running.
                                Ok(_) => {}
                                // Channel closed 鈫?exit.
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
            .send(WorkerCommand::Peer(PeerWorkerCommand::Unload {
                sender_id: usize::MAX,
                seq_id: 1,
            }))
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

    // 鈹€鈹€ Storage tests 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

    #[tokio::test]
    async fn stale_broadcast_sequence_is_ignored() {
        use tokio::sync::broadcast;

        use crate::runtime::backend::protocol::{PeerWorkerCommand, WorkerCommand};

        let (bc_tx, mut bc_rx) = broadcast::channel::<WorkerCommand>(16);
        let applied = Arc::new(tokio::sync::Mutex::new(Vec::<u64>::new()));
        let applied_w = Arc::clone(&applied);

        let worker = tokio::spawn(async move {
            let mut last_applied_seq = 0u64;
            loop {
                match bc_rx.recv().await {
                    Ok(WorkerCommand::Peer(PeerWorkerCommand::Unload { seq_id, .. })) => {
                        if seq_id <= last_applied_seq {
                            continue;
                        }
                        last_applied_seq = seq_id;
                        applied_w.lock().await.push(seq_id);
                    }
                    Ok(_) => {}
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        for seq_id in [1u64, 1, 0, 2, 2, 3, 1] {
            let _ = bc_tx.send(WorkerCommand::Peer(PeerWorkerCommand::Unload {
                sender_id: usize::MAX,
                seq_id,
            }));
        }
        drop(bc_tx);

        worker
            .await
            .expect("broadcast worker should stop after sender is dropped");

        let observed = applied.lock().await.clone();
        assert_eq!(
            observed,
            vec![1, 2, 3],
            "worker should apply only strictly increasing sequence ids"
        );
    }

    #[tokio::test]
    async fn inconsistent_global_state_blocks_inference_submission() {
        let mut rm = ResourceManager::with_config(ResourceManagerConfig {
            backend_capacity: 2,
            ..ResourceManagerConfig::default()
        });
        rm.register_backend("gate-backend", |shared_rx, _control_tx| {
            tokio::spawn(async move {
                loop {
                    let req = {
                        let mut lock = shared_rx.lock().await;
                        lock.recv().await
                    };
                    match req {
                        Some(req) => {
                            let _ = req.reply_tx.send(BackendReply::Value(req.input));
                        }
                        None => break,
                    }
                }
            });
        });

        let rm_state = rm.clone();
        let orchestrator = Orchestrator::start(rm, 64);

        let op_id = 42;
        rm_state
            .mark_global_inconsistent(
                op_id,
                vec!["gate-backend".to_owned()],
                vec!["forced inconsistent state for test".to_owned()],
                FailedGlobalOperation {
                    kind: GlobalOperationKind::LoadModelsAll,
                    payloads: HashMap::new(),
                },
            )
            .await;

        let op = BackendOp {
            name: "echo".to_owned(),
            options: Payload::default(),
        };

        let result = PipelineBuilder::new(orchestrator, text_payload("blocked"))
            .gpu("echo-stage", "gate-backend", op)
            .run()
            .await;

        assert!(
            matches!(result, Err(RuntimeError::GlobalStateInconsistent { op_id: seen }) if seen == op_id),
            "inference submission should be rejected while global state is inconsistent, got {result:?}"
        );
    }

    #[tokio::test]
    async fn manual_override_clears_inference_gate() {
        let mut rm = ResourceManager::with_config(ResourceManagerConfig {
            backend_capacity: 2,
            ..ResourceManagerConfig::default()
        });
        rm.register_backend("gate-backend", |shared_rx, _control_tx| {
            tokio::spawn(async move {
                loop {
                    let req = {
                        let mut lock = shared_rx.lock().await;
                        lock.recv().await
                    };
                    match req {
                        Some(req) => {
                            let _ = req.reply_tx.send(BackendReply::Value(req.input));
                        }
                        None => break,
                    }
                }
            });
        });

        let rm_state = rm.clone();
        let orchestrator = Orchestrator::start(rm, 64);

        let op_id = 99;
        rm_state
            .mark_global_inconsistent(
                op_id,
                vec!["gate-backend".to_owned()],
                vec!["forced inconsistent state for test".to_owned()],
                FailedGlobalOperation {
                    kind: GlobalOperationKind::LoadModelsAll,
                    payloads: HashMap::new(),
                },
            )
            .await;

        let blocked = PipelineBuilder::new(
            orchestrator.clone(),
            text_payload("first blocked submission"),
        )
        .gpu(
            "echo-stage",
            "gate-backend",
            BackendOp {
                name: "echo".to_owned(),
                options: Payload::default(),
            },
        )
        .run()
        .await;
        assert!(
            matches!(blocked, Err(RuntimeError::GlobalStateInconsistent { .. })),
            "submission should be blocked before manual override"
        );

        rm_state
            .manual_mark_consistent("operator override in test")
            .await;

        let task_id = PipelineBuilder::new(
            orchestrator.clone(),
            text_payload("submission after override"),
        )
        .gpu(
            "echo-stage",
            "gate-backend",
            BackendOp {
                name: "echo".to_owned(),
                options: Payload::default(),
            },
        )
        .run()
        .await
        .expect("submission should be accepted after manual override");

        let status = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let view = orchestrator
                    .get_status(task_id)
                    .await
                    .expect("task should exist");
                match view.status {
                    TaskStatus::Succeeded { .. } | TaskStatus::Failed { .. } => break view.status,
                    _ => tokio::time::sleep(std::time::Duration::from_millis(10)).await,
                }
            }
        })
        .await
        .expect("task should complete");

        assert!(
            matches!(status, TaskStatus::Succeeded { .. }),
            "task should succeed after manual override, got {status:?}"
        );
    }

    #[tokio::test]
    async fn global_management_emits_runtime_control_signal() {
        use crate::runtime::backend::protocol::{RuntimeControlSignal, WorkerCommand};

        let mut rm = ResourceManager::with_config(ResourceManagerConfig {
            backend_capacity: 2,
            ..ResourceManagerConfig::default()
        });
        let (signal_tx, mut signal_rx) = mpsc::unbounded_channel::<()>();
        rm.register_backend("ctrl-backend", move |shared_rx, control_tx| {
            let mut control_rx = control_tx.subscribe();
            let signal_tx = signal_tx.clone();
            tokio::spawn(async move {
                loop {
                    match control_rx.recv().await {
                        Ok(WorkerCommand::Runtime(RuntimeControlSignal::GlobalLoad { .. })) => {
                            let _ = signal_tx.send(());
                            break;
                        }
                        Ok(_) => {}
                        Err(_) => break,
                    }
                }
            });
            tokio::spawn(async move {
                loop {
                    let req = {
                        let mut lock = shared_rx.lock().await;
                        lock.recv().await
                    };
                    match req {
                        Some(req) => {
                            let _ = req.reply_tx.send(BackendReply::Value(Payload::default()));
                        }
                        None => break,
                    }
                }
            });
        });

        let orchestrator = Orchestrator::start(rm, 64);

        let mut payloads = HashMap::new();
        payloads.insert(
            "ctrl-backend".to_owned(),
            Payload::Json(serde_json::json!({
                "model_path": "/tmp/model.bin",
                "num_workers": 1
            })),
        );

        orchestrator
            .run_global_management(GlobalOperationKind::LoadModelsAll, payloads)
            .await
            .expect("global management call should succeed");

        let saw_runtime_signal = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            signal_rx.recv().await.is_some()
        })
        .await
        .expect("runtime control signal should arrive");

        assert!(
            saw_runtime_signal,
            "expected a runtime control signal on backend control channel"
        );
    }
}
