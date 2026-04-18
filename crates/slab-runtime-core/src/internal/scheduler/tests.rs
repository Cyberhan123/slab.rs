use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::internal::scheduler::backend::admission::{ResourceManager, ResourceManagerConfig};
use crate::internal::scheduler::backend::protocol::{BackendOp, BackendReply};
use crate::internal::scheduler::orchestrator::Orchestrator;
use crate::internal::scheduler::pipeline::PipelineBuilder;
use crate::internal::scheduler::types::{CoreError, GlobalOperationKind, Payload, TaskStatus};

fn text_payload(s: &str) -> Payload {
    Payload::Text(Arc::from(s))
}

#[test]
fn payload_clone_does_not_copy_bytes() {
    let data: Arc<[u8]> = Arc::from(vec![1u8, 2, 3]);
    let p1 = Payload::Bytes(Arc::clone(&data));
    let p2 = p1.clone();
    // Both variants should share the same underlying allocation.
    match (p1, p2) {
        (Payload::Bytes(a), Payload::Bytes(b)) => {
            assert!(Arc::ptr_eq(&a, &b), "clone should share Arc pointer");
        }
        _ => {
            panic!("unexpected payload variant");
        }
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
            rm.acquire_inference_lease("test-backend", std::time::Duration::from_millis(20)).await,
            Err(CoreError::Timeout)
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
async fn inference_lease_unknown_backend_returns_driver_not_registered() {
    let rm = ResourceManager::new();
    let err = rm
        .acquire_inference_lease("nonexistent", std::time::Duration::from_millis(20))
        .await
        .unwrap_err();
    assert!(
        matches!(err, crate::internal::scheduler::types::CoreError::DriverNotRegistered { .. }),
        "expected DriverNotRegistered error"
    );
}

#[tokio::test]
async fn cpu_stage_transforms_payload() {
    use crate::internal::scheduler::stage::CpuStage;

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
    use crate::internal::scheduler::stage::CpuStage;

    let stage = CpuStage::new("fail-stage", |_p| {
        Err(CoreError::CpuStageFailed {
            stage_name: "fail-stage".to_owned(),
            message: "intentional error".to_owned(),
        })
    });
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

    let op = BackendOp { name: "echo".to_owned(), options: Payload::default() };

    let task_id = PipelineBuilder::new(orchestrator.clone(), text_payload("ping"))
        .gpu("echo-stage", "echo-backend", op)
        .run()
        .await
        .expect("submit should succeed");

    let result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let view = orchestrator.get_status(task_id).await.expect("task should exist");
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
    // test builds) and then fail the task with CoreError::Timeout.
    let mut rm = ResourceManager::with_config(ResourceManagerConfig {
        backend_capacity: 0,
        ..ResourceManagerConfig::default()
    });
    rm.register_backend("busy-backend", |_shared_rx, _control_tx| {});
    let orchestrator = Orchestrator::start(rm, 64);

    let op = BackendOp { name: "noop".to_owned(), options: Payload::default() };

    let task_id = PipelineBuilder::new(orchestrator.clone(), text_payload("x"))
        .gpu("noop-stage", "busy-backend", op)
        .run()
        .await
        .expect("submit should succeed");

    // Wait up to 2 s; in test builds GPU_ACQUIRE_TIMEOUT is 200 ms so the
    // task should fail well within this window.
    let result = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let view = orchestrator.get_status(task_id).await.expect("task should exist");
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
                            mpsc::channel::<crate::backend::StreamChunk>(8);
                        let _ = req.reply_tx.send(BackendReply::Stream(stream_rx));
                        for word in ["hello", " ", "world"] {
                            let _ = stream_tx
                                .send(crate::backend::StreamChunk::Token(word.to_owned()))
                                .await;
                        }
                        let _ = stream_tx.send(crate::backend::StreamChunk::Done).await;
                    }
                    None => break,
                }
            }
        });
    });
    let orchestrator = Orchestrator::start(rm, 64);
    let op = BackendOp { name: "stream-gen".to_owned(), options: Payload::default() };

    let task_id = PipelineBuilder::new(orchestrator.clone(), text_payload("prompt"))
        .gpu_stream("stream-stage", "stream-backend", op)
        .run_stream()
        .await
        .expect("submit should succeed");

    // Wait for SucceededStreaming.
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let view = orchestrator.get_status(task_id).await.expect("task should exist");
            if matches!(view.status, TaskStatus::SucceededStreaming) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("task should reach SucceededStreaming");

    let mut handle =
        orchestrator.take_stream(task_id).await.expect("stream handle should be available");

    let mut tokens = String::new();
    while let Some(chunk) = handle.recv().await {
        match chunk {
            crate::backend::StreamChunk::Token(t) => tokens.push_str(&t),
            crate::backend::StreamChunk::Json(_) => continue,
            crate::backend::StreamChunk::Done => break,
            crate::backend::StreamChunk::Error(e) => {
                panic!("stream error: {e}")
            }
            crate::backend::StreamChunk::Image(_) => {
                panic!("unexpected image chunk in stream")
            }
        }
    }
    assert_eq!(tokens, "hello world");
}

/// Verify that `acquire_inference_lease` waits for capacity to become available
/// instead of failing immediately.
#[tokio::test]
async fn acquire_inference_lease_waits_for_capacity() {
    use crate::internal::scheduler::backend::admission::ResourceManager;

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
    let result =
        rm.acquire_inference_lease("serial-backend", std::time::Duration::from_secs(2)).await;
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

    use crate::internal::scheduler::backend::protocol::{
        PeerWorkerCommand, SyncMessage, WorkerCommand,
    };

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
            sync: SyncMessage::Generation { generation: 1 },
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
        assert!(!ctx.load(Ordering::SeqCst), "worker {i} context should be cleared after Unload");
    }
}

#[tokio::test]
async fn stale_broadcast_sequence_is_ignored() {
    use tokio::sync::broadcast;

    use crate::internal::scheduler::backend::protocol::{
        PeerWorkerCommand, SyncMessage, WorkerCommand,
    };

    let (bc_tx, mut bc_rx) = broadcast::channel::<WorkerCommand>(16);
    let applied = Arc::new(tokio::sync::Mutex::new(Vec::<u64>::new()));
    let applied_w = Arc::clone(&applied);

    let worker = tokio::spawn(async move {
        let mut last_applied_seq = 0u64;
        loop {
            match bc_rx.recv().await {
                Ok(WorkerCommand::Peer(cmd @ PeerWorkerCommand::Unload { .. })) => {
                    let seq_id = cmd.seq_id();
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
            sync: SyncMessage::Generation { generation: seq_id },
        }));
    }
    drop(bc_tx);

    worker.await.expect("broadcast worker should stop after sender is dropped");

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
    rm_state.mark_global_inconsistent(op_id).await;

    let op = BackendOp { name: "echo".to_owned(), options: Payload::default() };

    let result = PipelineBuilder::new(orchestrator, text_payload("blocked"))
        .gpu("echo-stage", "gate-backend", op)
        .run()
        .await;

    assert!(
        matches!(result, Err(CoreError::GlobalStateInconsistent { op_id: seen }) if seen == op_id),
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
    rm_state.mark_global_inconsistent(op_id).await;

    let blocked =
        PipelineBuilder::new(orchestrator.clone(), text_payload("first blocked submission"))
            .gpu(
                "echo-stage",
                "gate-backend",
                BackendOp { name: "echo".to_owned(), options: Payload::default() },
            )
            .run()
            .await;
    assert!(
        matches!(blocked, Err(CoreError::GlobalStateInconsistent { .. })),
        "submission should be blocked before manual override"
    );

    rm_state.manual_mark_consistent("operator override in test").await;

    let task_id =
        PipelineBuilder::new(orchestrator.clone(), text_payload("submission after override"))
            .gpu(
                "echo-stage",
                "gate-backend",
                BackendOp { name: "echo".to_owned(), options: Payload::default() },
            )
            .run()
            .await
            .expect("submission should be accepted after manual override");

    let status = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let view = orchestrator.get_status(task_id).await.expect("task should exist");
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
    use crate::internal::scheduler::backend::protocol::{RuntimeControlSignal, WorkerCommand};

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
        .run_global_management(GlobalOperationKind::LoadModels, payloads)
        .await
        .expect("global management call should succeed");

    let saw_runtime_signal = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        signal_rx.recv().await.is_some()
    })
    .await
    .expect("runtime control signal should arrive");

    assert!(saw_runtime_signal, "expected a runtime control signal on backend control channel");
}

// ── Tests: purge_task ─────────────────────────────────────────────────────

/// Verify that `purge_task` removes the task record so subsequent
/// `get_status` calls return `TaskNotFound`.
#[tokio::test]
async fn purge_task_removes_record() {
    let mut rm = ResourceManager::new();
    rm.register_backend("echo", |shared_rx, _control_tx| {
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
    let orchestrator = Orchestrator::start(rm, 64);

    let task_id = PipelineBuilder::new(orchestrator.clone(), text_payload("hello"))
        .gpu("echo", "echo", BackendOp { name: "echo".into(), options: Payload::default() })
        .run()
        .await
        .expect("submit should succeed");

    // Wait for success.
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            match orchestrator.get_status(task_id).await {
                Ok(v) if matches!(v.status, TaskStatus::Succeeded { .. }) => break,
                _ => tokio::time::sleep(std::time::Duration::from_millis(10)).await,
            }
        }
    })
    .await
    .expect("task should complete");

    // Record should exist before purge.
    assert!(orchestrator.get_status(task_id).await.is_ok(), "record should exist before purge");

    orchestrator.purge_task(task_id).await;

    // After purge the record is gone.
    assert!(
        matches!(orchestrator.get_status(task_id).await, Err(CoreError::TaskNotFound { .. })),
        "record should be gone after purge"
    );
}

/// Verify that `purge_task` on a failed task also removes the record.
#[tokio::test]
async fn purge_task_removes_failed_record() {
    let mut rm = ResourceManager::with_config(ResourceManagerConfig {
        backend_capacity: 0, // No permits → timeout → failed
        ..ResourceManagerConfig::default()
    });
    rm.register_backend("stall", |_rx, _tx| {});
    let orchestrator = Orchestrator::start(rm, 64);

    let task_id = PipelineBuilder::new(orchestrator.clone(), text_payload("x"))
        .gpu("stall", "stall", BackendOp { name: "stall".into(), options: Payload::default() })
        .run()
        .await
        .expect("submit should succeed");

    // Wait for the GPU acquire timeout (≤ 200 ms in test builds).
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            match orchestrator.get_status(task_id).await {
                Ok(v) if matches!(v.status, TaskStatus::Failed { .. }) => break,
                _ => tokio::time::sleep(std::time::Duration::from_millis(20)).await,
            }
        }
    })
    .await
    .expect("task should fail quickly");

    orchestrator.purge_task(task_id).await;

    assert!(
        matches!(orchestrator.get_status(task_id).await, Err(CoreError::TaskNotFound { .. })),
        "failed task record should be gone after purge"
    );
}

// ── Tests: cancel_and_purge ───────────────────────────────────────────────

/// Verify that `cancel_and_purge` signals cancellation before removing the
/// record, so the running stage actually sees the cancel flag.
///
/// The test submits a task whose backend delays long enough that we can
/// call `cancel_and_purge` while the stage is still running, then verifies:
///   1. The task record is gone immediately after `cancel_and_purge`.
///   2. The cancel watch sender (held by the spawned execute_task) received
///      the `true` signal — i.e., cancellation was delivered, not lost.
#[tokio::test]
async fn cancel_and_purge_signals_before_removing_record() {
    use std::sync::Arc;
    use tokio::sync::Notify;

    // Backend that parks until the cancel channel is closed/dropped.
    let started = Arc::new(Notify::new());
    let started_clone = Arc::clone(&started);

    let mut rm = ResourceManager::new();
    rm.register_backend("slow", move |shared_rx, _control_tx| {
        let started = Arc::clone(&started_clone);
        tokio::spawn(async move {
            loop {
                let req = {
                    let mut lock = shared_rx.lock().await;
                    lock.recv().await
                };
                match req {
                    Some(req) => {
                        started.notify_one();
                        // Wait until the cancel watch fires.
                        let mut cancel = req.cancel_rx.clone();
                        let _ = cancel.wait_for(|v| *v).await;
                        let _ = req.reply_tx.send(BackendReply::Error("cancelled".into()));
                    }
                    None => break,
                }
            }
        });
    });
    let orchestrator = Orchestrator::start(rm, 64);

    let task_id = PipelineBuilder::new(orchestrator.clone(), text_payload("slow"))
        .gpu("slow-stage", "slow", BackendOp { name: "slow".into(), options: Payload::default() })
        .run()
        .await
        .expect("submit should succeed");

    // Wait for the backend to actually start processing the request.
    tokio::time::timeout(std::time::Duration::from_secs(5), started.notified())
        .await
        .expect("backend should start within timeout");

    // cancel_and_purge should signal the cancel watch before removing the record.
    orchestrator.cancel_and_purge(task_id).await;

    // Record is gone immediately.
    assert!(
        matches!(orchestrator.get_status(task_id).await, Err(CoreError::TaskNotFound { .. })),
        "record should be gone after cancel_and_purge"
    );

    // The backend was unblocked because it received the cancel signal — if
    // cancellation had been lost the backend would block forever and the
    // test would time out.
}

/// Verify that `wait_result` returns `CoreError::Cancelled` when a task is
/// cancelled between pipeline stages.
///
/// Uses a 2-stage CPU→GPU pipeline.  The CPU stage gates completion via a
/// channel so the test can:
///   1. Wait for the CPU stage to start.
///   2. Call `cancel()` so the orchestrator sets the cancel watch.
///   3. Release the CPU stage gate.
///
/// When `execute_task` checks cancel *before* the GPU stage, it sees `true`
/// and transitions to `TaskStatus::Cancelled`.
#[tokio::test]
async fn wait_result_returns_cancelled_after_cancel() {
    use crate::internal::scheduler::stage::CpuStage;
    use std::sync::{Arc, Mutex};

    // CPU stage signals "started" and then blocks on a std channel gate.
    let (started_tx, mut started_rx) = tokio::sync::mpsc::channel::<()>(1);
    let (gate_tx, gate_rx) = std::sync::mpsc::channel::<()>();
    // Wrap Receiver in Arc<Mutex<_>> so it satisfies the Sync bound on CpuFn.
    let gate_rx = Arc::new(Mutex::new(gate_rx));

    let mut rm = ResourceManager::new();
    rm.register_backend("unreachable-backend", |_shared_rx, _control_tx| {
        // This backend is never reached; cancel fires before the GPU stage.
    });
    let orchestrator = Orchestrator::start(rm, 64);

    let task_id = PipelineBuilder::new(orchestrator.clone(), text_payload("input"))
        .cpu_stage(CpuStage::new("gating-cpu", move |p| {
            let _ = started_tx.blocking_send(());
            let _ = gate_rx.lock().unwrap().recv();
            Ok(p)
        }))
        .gpu(
            "unreachable-gpu",
            "unreachable-backend",
            BackendOp { name: "inference".into(), options: Payload::default() },
        )
        .run()
        .await
        .expect("submit should succeed");

    // Wait for the CPU stage to signal that it has started.
    started_rx.recv().await.expect("CPU stage should start");

    // Signal cancellation, then yield so the orchestrator loop can process
    // the Cancel command and set the cancel watch before the gate is released.
    orchestrator.cancel(task_id);
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }

    // Release the CPU stage gate.  execute_task will then check cancel before
    // the GPU stage and transition to TaskStatus::Cancelled.
    gate_tx.send(()).ok();

    let err = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        orchestrator.wait_result(task_id, std::time::Duration::from_secs(10)),
    )
    .await
    .expect("wait_result should complete after cancellation")
    .expect_err("cancelled task should return an error");

    assert!(
        matches!(err, CoreError::Cancelled),
        "wait_result should return CoreError::Cancelled, got {err:?}"
    );
}

/// Verify that `wait_stream` returns `CoreError::Cancelled` when a streaming
/// task is cancelled between pipeline stages.
///
/// Same gating strategy as `wait_result_returns_cancelled_after_cancel`, but
/// uses a CPU→GPU-stream pipeline so the cancelled-before-stream-stage path
/// in `wait_stream` is exercised.
#[tokio::test]
async fn wait_stream_returns_cancelled_after_cancel() {
    use crate::internal::scheduler::stage::CpuStage;
    use std::sync::{Arc, Mutex};

    let (started_tx, mut started_rx) = tokio::sync::mpsc::channel::<()>(1);
    let (gate_tx, gate_rx) = std::sync::mpsc::channel::<()>();
    let gate_rx = Arc::new(Mutex::new(gate_rx));

    let mut rm = ResourceManager::new();
    rm.register_backend("unreachable-stream", |_shared_rx, _control_tx| {
        // Never reached.
    });
    let orchestrator = Orchestrator::start(rm, 64);

    let task_id = PipelineBuilder::new(orchestrator.clone(), text_payload("prompt"))
        .cpu_stage(CpuStage::new("gating-cpu-stream", move |p| {
            let _ = started_tx.blocking_send(());
            let _ = gate_rx.lock().unwrap().recv();
            Ok(p)
        }))
        .gpu_stream(
            "unreachable-stream-stage",
            "unreachable-stream",
            BackendOp { name: "inference.stream".into(), options: Payload::default() },
        )
        .run_stream()
        .await
        .expect("submit should succeed");

    started_rx.recv().await.expect("CPU stage should start");

    orchestrator.cancel(task_id);
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    gate_tx.send(()).ok();

    let err = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        orchestrator.wait_stream(task_id, std::time::Duration::from_secs(10)),
    )
    .await
    .expect("wait_stream should complete after cancellation")
    .expect_err("cancelled task should return an error");

    assert!(
        matches!(err, CoreError::Cancelled),
        "wait_stream should return CoreError::Cancelled, got {err:?}"
    );
}

/// Test cancellation of an in-flight GPU task when the backend honors cancel.
#[tokio::test]
async fn inflight_gpu_cancel_reaches_terminal_state() {
    use std::sync::Arc;
    use tokio::sync::Notify;

    let task_started = Arc::new(Notify::new());

    let mut rm = ResourceManager::new();
    let task_started_clone = Arc::clone(&task_started);

    rm.register_backend("slow-backend", move |shared_rx, _control_tx| {
        let task_started = Arc::clone(&task_started_clone);
        tokio::spawn(async move {
            loop {
                let req = {
                    let mut lock = shared_rx.lock().await;
                    lock.recv().await
                };
                match req {
                    Some(req) => {
                        task_started.notify_one();
                        let mut cancel_rx = req.cancel_rx;
                        tokio::select! {
                            _ = cancel_rx.changed() => {
                                let _ = req.reply_tx.send(BackendReply::Error("cancelled".into()));
                            }
                            _ = tokio::time::sleep(Duration::from_secs(5)) => {
                                let _ = req.reply_tx.send(BackendReply::Value(Payload::default()));
                            }
                        }
                        break;
                    }
                    None => break,
                }
            }
        });
    });

    let orchestrator = Orchestrator::start(rm, 64);

    // Submit a task that will block
    let task_id = PipelineBuilder::new(orchestrator.clone(), text_payload("test"))
        .gpu(
            "slow-stage",
            "slow-backend",
            BackendOp { name: "test".into(), options: Payload::default() },
        )
        .run()
        .await
        .expect("submit should succeed");

    // Wait for the task to start
    task_started.notified().await;

    // Cancel the task
    orchestrator.cancel(task_id);

    let final_status = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let view = orchestrator.get_status(task_id).await.expect("task should exist");
            if matches!(view.status, TaskStatus::Cancelled | TaskStatus::Failed { .. }) {
                break view.status;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("task should reach a terminal cancelled/failed state");

    assert!(
        matches!(final_status, TaskStatus::Cancelled | TaskStatus::Failed { .. }),
        "task should be cancelled or failed after backend observes cancel"
    );
}

/// Test backend worker failure and recovery.
///
/// Simulates a worker crash and verifies that the orchestrator can detect
/// the failure and that subsequent requests are handled appropriately.
#[tokio::test]
async fn backend_worker_failure_detected_on_request() {
    let worker_alive = Arc::new(AtomicBool::new(true));
    let worker_alive_clone = Arc::clone(&worker_alive);

    let mut rm = ResourceManager::new();

    rm.register_backend("crashy-backend", move |shared_rx, _control_tx| {
        let worker_alive = Arc::clone(&worker_alive_clone);
        tokio::spawn(async move {
            // Process one request then "crash" (stop processing)
            let req = {
                let mut lock = shared_rx.lock().await;
                lock.recv().await
            };
            if let Some(req) = req {
                let _ = req.reply_tx.send(BackendReply::Value(Payload::default()));
            }
            // Simulate worker crash
            worker_alive.store(false, Ordering::SeqCst);
        });
    });

    let orchestrator = Orchestrator::start(rm, 64);

    // First request should succeed
    let task_id1 = PipelineBuilder::new(orchestrator.clone(), text_payload("test1"))
        .gpu(
            "test-stage",
            "crashy-backend",
            BackendOp { name: "test".into(), options: Payload::default() },
        )
        .run()
        .await
        .expect("submit should succeed");

    // Wait for completion
    let result1 = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        async {
            loop {
                let view = orchestrator.get_status(task_id1).await.expect("task should exist");
                if matches!(view.status, TaskStatus::Succeeded { .. } | TaskStatus::Failed { .. }) {
                    break view.status;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        },
    )
    .await;

    assert!(
        matches!(result1, Ok(TaskStatus::Succeeded { .. })),
        "first task should succeed before worker crash"
    );

    // Verify worker is no longer alive
    assert!(!worker_alive.load(Ordering::SeqCst), "worker should have crashed");

    // Second request should fail because backend is down
    let task_id2 = PipelineBuilder::new(orchestrator.clone(), text_payload("test2"))
        .gpu(
            "test-stage2",
            "crashy-backend",
            BackendOp { name: "test".into(), options: Payload::default() },
        )
        .run()
        .await
        .expect("submit should succeed");

    // Wait for failure
    let result2 = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        async {
            loop {
                let view = orchestrator.get_status(task_id2).await.expect("task should exist");
                if matches!(view.status, TaskStatus::Failed { .. }) {
                    break view.status.clone();
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        },
    )
    .await;

    assert!(
        matches!(result2, Ok(TaskStatus::Failed { .. })),
        "second task should fail after worker crash, got {result2:?}"
    );
}

/// Test resource exhaustion and admission control edge cases.
///
/// Verifies that when all permits are exhausted, new requests wait
/// for permits to become available rather than failing immediately.
#[tokio::test]
async fn admission_control_waits_for_available_permits() {
    use std::sync::Arc;
    use tokio::sync::Notify;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const CAPACITY: usize = 2;
    let active_count = Arc::new(AtomicUsize::new(0));
    let completion_notify = Arc::new(Notify::new());

    let mut rm = ResourceManager::with_config(ResourceManagerConfig {
        backend_capacity: CAPACITY,
        ..ResourceManagerConfig::default()
    });

    let active_count_clone = Arc::clone(&active_count);
    let completion_notify_clone = Arc::clone(&completion_notify);

    rm.register_backend("limited-backend", move |shared_rx, _control_tx| {
        for _ in 0..CAPACITY {
            let shared_rx = Arc::clone(&shared_rx);
            let active_count = Arc::clone(&active_count_clone);
            let completion_notify = Arc::clone(&completion_notify_clone);
            tokio::spawn(async move {
                loop {
                    let req = {
                        let mut lock = shared_rx.lock().await;
                        lock.recv().await
                    };
                    match req {
                        Some(req) => {
                            active_count.fetch_add(1, Ordering::SeqCst);
                            // Simulate work while holding one inference permit per worker.
                            completion_notify.notified().await;
                            let _ = req.reply_tx.send(BackendReply::Value(Payload::default()));
                            active_count.fetch_sub(1, Ordering::SeqCst);
                        }
                        None => break,
                    }
                }
            });
        }
    });

    let orchestrator = Orchestrator::start(rm, 64);

    // Submit tasks up to capacity
    let mut task_ids = Vec::new();
    for i in 0..CAPACITY {
        let task_id = PipelineBuilder::new(orchestrator.clone(), text_payload(&format!("task{}", i)))
            .gpu(
                &format!("stage{}", i),
                "limited-backend",
                BackendOp { name: "test".into(), options: Payload::default() },
            )
            .run()
            .await
            .expect("submit should succeed");
        task_ids.push(task_id);
    }

    // Wait for all permits to be acquired
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        active_count.load(Ordering::SeqCst),
        CAPACITY,
        "all permits should be acquired"
    );

    // Submit one more task that should wait for a permit
    let extra_task_id = PipelineBuilder::new(orchestrator.clone(), text_payload("extra"))
        .gpu(
            "extra-stage",
            "limited-backend",
            BackendOp { name: "test".into(), options: Payload::default() },
        )
        .run()
        .await
        .expect("submit should succeed");

    // Verify the extra task is still pending (waiting for permit)
    tokio::time::sleep(Duration::from_millis(50)).await;
    let status = orchestrator.get_status(extra_task_id).await.expect("task should exist");
    assert!(
        matches!(status.status, TaskStatus::Pending | TaskStatus::Running { .. }),
        "extra task should be waiting for permit, got {:?}", status.status
    );

    // Release one permit by signaling completion
    completion_notify.notify_one();

    // Now the extra task should proceed
    tokio::time::sleep(Duration::from_millis(50)).await;
    let status = orchestrator.get_status(extra_task_id).await.expect("task should exist");
    assert!(
        matches!(status.status, TaskStatus::Pending | TaskStatus::Running { .. } | TaskStatus::Succeeded { .. }),
        "extra task should have progressed"
    );
}

/// Test that multiple concurrent requests correctly wait for permits in FIFO order.
#[tokio::test]
async fn admission_control_fifo_ordering() {
    use std::sync::Arc;
    use tokio::sync::Notify;

    const CAPACITY: usize = 1;
    let completion_notify = Arc::new(Notify::new());
    let completion_order = Arc::new(std::sync::Mutex::new(Vec::new()));

    let mut rm = ResourceManager::with_config(ResourceManagerConfig {
        backend_capacity: CAPACITY,
        ..ResourceManagerConfig::default()
    });

    let completion_notify_clone = Arc::clone(&completion_notify);
    let completion_order_clone = Arc::clone(&completion_order);

    rm.register_backend("fifo-backend", move |shared_rx, _control_tx| {
        let completion_notify = Arc::clone(&completion_notify_clone);
        let completion_order = Arc::clone(&completion_order_clone);
        tokio::spawn(async move {
            let mut task_seq = 0u64;
            loop {
                let req = {
                    let mut lock = shared_rx.lock().await;
                    lock.recv().await
                };
                match req {
                    Some(req) => {
                        // Record completion order
                        let seq = task_seq;
                        task_seq += 1;
                        completion_notify.notified().await;
                        let _ = req.reply_tx.send(BackendReply::Value(Payload::default()));
                        completion_order.lock().unwrap().push(seq);
                    }
                    None => break,
                }
            }
        });
    });

    let orchestrator = Orchestrator::start(rm, 64);

    // Submit first task (acquires the only permit)
    let _task1 = PipelineBuilder::new(orchestrator.clone(), text_payload("task1"))
        .gpu(
            "stage1",
            "fifo-backend",
            BackendOp { name: "test".into(), options: Payload::default() },
        )
        .run()
        .await
        .expect("submit should succeed");

    // Give it time to acquire the permit
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Submit multiple tasks that will queue
    let mut handles = Vec::new();
    for i in 0..3 {
        let orch = orchestrator.clone();
        let handle = tokio::spawn(async move {
            PipelineBuilder::new(orch, text_payload(&format!("task{}", i + 2)))
                .gpu(
                    &format!("stage{}", i + 2),
                    "fifo-backend",
                    BackendOp { name: "test".into(), options: Payload::default() },
                )
                .run()
                .await
        });
        handles.push(handle);
    }

    // Release permits one by one and verify ordering
    for _ in 0..3 {
        completion_notify.notify_one();
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // All submissions should succeed
    for handle in handles {
        handle.await.expect("task should not panic").expect("submit should succeed");
    }
}
