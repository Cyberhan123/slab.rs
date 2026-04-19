use std::{future::Future, pin::Pin};

use async_trait::async_trait;
use flume::Receiver;
use tokio::sync::broadcast;

use crate::internal::scheduler::backend::protocol::BackendReply;
use crate::internal::scheduler::backend::protocol::{
    BackendRequest, PeerWorkerCommand, RequestRoute, RuntimeControlSignal, WorkerCommand,
};

/// Shared ingress receiver consumed competitively by multiple worker runners.
pub type SharedIngressRx = Receiver<BackendRequest>;

/// Wrap an ingress receiver for competitive multi-worker consumption.
pub fn shared_ingress(rx: Receiver<BackendRequest>) -> SharedIngressRx {
    rx
}

/// Boxed future used by macro-generated thin dispatch shims.
pub type HandlerFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
/// Request-dispatch function pointer (`&mut handler`, `BackendRequest`) -> async unit.
pub type RequestDispatchFn<T> = for<'a> fn(&'a mut T, BackendRequest) -> HandlerFuture<'a>;
/// Runtime-control-dispatch function pointer.
pub type RuntimeDispatchFn<T> = for<'a> fn(&'a mut T, RuntimeControlSignal) -> HandlerFuture<'a>;
/// Peer-control-dispatch function pointer.
pub type PeerDispatchFn<T> = for<'a> fn(&'a mut T, PeerWorkerCommand) -> HandlerFuture<'a>;
/// Control-lagged-dispatch function pointer.
pub type LaggedDispatchFn<T> = for<'a> fn(&'a mut T) -> HandlerFuture<'a>;

/// Thin request route entry used by macro-generated `#[backend_handler]` code.
pub struct RequestRouteMatcher<T> {
    pub matches: fn(RequestRoute) -> bool,
    pub handle: RequestDispatchFn<T>,
}

/// Thin runtime-control route entry used by macro-generated `#[backend_handler]` code.
pub struct RuntimeRoute<T> {
    pub matches: fn(&RuntimeControlSignal) -> bool,
    pub handle: RuntimeDispatchFn<T>,
}

/// Thin peer-control route entry used by macro-generated `#[backend_handler]` code.
pub struct PeerRoute<T> {
    pub matches: fn(&PeerWorkerCommand) -> bool,
    pub handle: PeerDispatchFn<T>,
}

/// Backend implementation-facing contract for runtime-managed worker loops.
#[async_trait]
pub trait RuntimeWorkerHandler: Send + 'static {
    /// Return typed request routes for this worker.
    ///
    /// Macro-based workers usually expose these via generated `routes()` accessors and
    /// delegate this method to them. Hand-written workers can override directly.
    fn request_routes() -> &'static [RequestRouteMatcher<Self>]
    where
        Self: Sized,
    {
        &[]
    }

    /// Return runtime-control routes for this worker.
    fn runtime_control_routes() -> &'static [RuntimeRoute<Self>]
    where
        Self: Sized,
    {
        &[]
    }

    /// Return peer-control routes for this worker.
    fn peer_control_routes() -> &'static [PeerRoute<Self>]
    where
        Self: Sized,
    {
        &[]
    }

    /// Return an optional peer-control fallback route.
    fn peer_control_fallback() -> Option<PeerDispatchFn<Self>>
    where
        Self: Sized,
    {
        None
    }

    /// Return an optional lagged-control cleanup route.
    fn control_lagged_route() -> Option<LaggedDispatchFn<Self>>
    where
        Self: Sized,
    {
        None
    }

    /// Handle a request consumed from backend ingress queue.
    async fn handle_request(&mut self, req: BackendRequest)
    where
        Self: Sized,
    {
        dispatch_backend_request(self, req, Self::request_routes()).await;
    }

    /// Handle a peer synchronization command after runtime filtering.
    ///
    /// Runtime runner guarantees this callback is invoked only for commands that:
    /// - have strictly increasing `seq_id` per worker, and
    /// - are not self-echoed (`sender_id != worker_id`).
    async fn handle_peer_control(&mut self, cmd: PeerWorkerCommand)
    where
        Self: Sized,
    {
        dispatch_peer_control(
            self,
            cmd,
            Self::peer_control_fallback(),
            Self::peer_control_routes(),
        )
        .await;
    }

    /// Handle a runtime-issued global control signal.
    async fn handle_runtime_control(&mut self, signal: RuntimeControlSignal)
    where
        Self: Sized,
    {
        dispatch_runtime_control(self, signal, Self::runtime_control_routes()).await;
    }

    /// Handle control-bus lag (`broadcast::RecvError::Lagged`).
    ///
    /// Default dispatches to the optional lagged route.
    async fn handle_control_lagged(&mut self)
    where
        Self: Sized,
    {
        dispatch_control_lagged(self, Self::control_lagged_route()).await;
    }
}

/// Dispatch request by typed request route using a macro-provided route table.
pub async fn dispatch_backend_request<T>(
    handler: &mut T,
    req: BackendRequest,
    routes: &[RequestRouteMatcher<T>],
) {
    let cancelled = *req.cancel_rx.borrow();
    tracing::trace!(op = %req.op.name, kind = ?req.kind, cancelled, "dispatch backend request");
    match req.route() {
        Ok(route_key) => {
            for route in routes {
                if (route.matches)(route_key) {
                    (route.handle)(handler, req).await;
                    return;
                }
            }
            let op_name = req.op.name.clone();
            let _ = req.reply_tx.send(BackendReply::error(format!("unknown op: {}", op_name)));
        }
        Err(_) => {
            let op_name = req.op.name.clone();
            let _ = req.reply_tx.send(BackendReply::error(format!("unknown op: {}", op_name)));
        }
    }
}

/// Dispatch runtime control using a macro-provided route table.
pub async fn dispatch_runtime_control<T>(
    handler: &mut T,
    signal: RuntimeControlSignal,
    routes: &[RuntimeRoute<T>],
) {
    for route in routes {
        if (route.matches)(&signal) {
            (route.handle)(handler, signal).await;
            return;
        }
    }
}

/// Dispatch peer control if a peer-control handler is provided.
pub async fn dispatch_peer_control<T>(
    handler: &mut T,
    cmd: PeerWorkerCommand,
    route: Option<PeerDispatchFn<T>>,
    routes: &[PeerRoute<T>],
) {
    for peer_route in routes {
        if (peer_route.matches)(&cmd) {
            (peer_route.handle)(handler, cmd).await;
            return;
        }
    }
    if let Some(route) = route {
        route(handler, cmd).await;
    }
}

/// Dispatch control lagged callback if a handler is provided.
pub async fn dispatch_control_lagged<T>(handler: &mut T, route: Option<LaggedDispatchFn<T>>) {
    if let Some(route) = route {
        route(handler).await;
    }
}

/// Spawn a framework-level runtime worker loop.
///
/// The loop owns:
/// - `tokio::select! { biased; ... }`,
/// - ingress/control listening, and
/// - peer command sequence/self-echo filtering.
async fn runtime_worker_loop<H>(
    shared_ingress: SharedIngressRx,
    mut control_rx: broadcast::Receiver<WorkerCommand>,
    worker_id: usize,
    mut handler: H,
) where
    H: RuntimeWorkerHandler,
{
    let mut last_applied_seq = 0u64;
    loop {
        tokio::select! {
            biased; // prioritize control traffic before ingress requests

            cmd = control_rx.recv() => {
                match cmd {
                    Ok(WorkerCommand::Peer(peer_cmd)) => {
                        let seq_id = peer_cmd.seq_id();
                        if seq_id <= last_applied_seq {
                            continue;
                        }
                        last_applied_seq = seq_id;
                        if peer_cmd.sender_id() == worker_id {
                            continue;
                        }
                        handler.handle_peer_control(peer_cmd).await;
                    }
                    Ok(WorkerCommand::Runtime(signal)) => {
                        handler.handle_runtime_control(signal).await;
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        handler.handle_control_lagged().await;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }

            req = shared_ingress.recv_async() => {
                match req {
                    Ok(req) => handler.handle_request(req).await,
                    Err(flume::RecvError::Disconnected) => break,
                }
            }
        }
    }
}

pub fn spawn_runtime_worker<H>(
    shared_ingress: SharedIngressRx,
    control_rx: broadcast::Receiver<WorkerCommand>,
    worker_id: usize,
    handler: H,
) -> tokio::task::JoinHandle<()>
where
    H: RuntimeWorkerHandler,
{
    tokio::spawn(runtime_worker_loop(shared_ingress, control_rx, worker_id, handler))
}

pub fn spawn_dedicated_runtime_worker<H>(
    shared_ingress: SharedIngressRx,
    control_tx: broadcast::Sender<WorkerCommand>,
    worker_id: usize,
    handler: H,
) where
    H: RuntimeWorkerHandler,
{
    let thread_name = format!("runtime-worker-{worker_id}");
    let thread_name_for_spawn = thread_name.clone();

    if let Err(error) = std::thread::Builder::new().name(thread_name.clone()).spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(runtime) => runtime,
            Err(error) => {
                tracing::error!(
                    worker_id,
                    thread = %thread_name_for_spawn,
                    error = %error,
                    "failed to build dedicated backend worker runtime"
                );
                return;
            }
        };

        runtime.block_on(runtime_worker_loop(
            shared_ingress,
            control_tx.subscribe(),
            worker_id,
            handler,
        ));
    }) {
        tracing::error!(
            worker_id,
            thread = %thread_name,
            error = %error,
            "failed to spawn dedicated backend worker thread"
        );
    }
}

/// Spawn N runtime workers from one shared ingress/control bus using a factory.
pub fn spawn_workers<H, F>(
    shared_ingress: SharedIngressRx,
    control_tx: broadcast::Sender<WorkerCommand>,
    worker_count: usize,
    mut make_handler: F,
) where
    H: RuntimeWorkerHandler,
    F: FnMut(usize, broadcast::Sender<WorkerCommand>) -> H,
{
    let worker_count = worker_count.max(1);
    for worker_id in 0..worker_count {
        let handler = make_handler(worker_id, control_tx.clone());
        spawn_runtime_worker(shared_ingress.clone(), control_tx.subscribe(), worker_id, handler);
    }
}

/// Spawn N runtime workers pinned to dedicated OS threads.
pub fn spawn_dedicated_workers<H, F>(
    shared_ingress: SharedIngressRx,
    control_tx: broadcast::Sender<WorkerCommand>,
    worker_count: usize,
    mut make_handler: F,
) where
    H: RuntimeWorkerHandler,
    F: FnMut(usize, broadcast::Sender<WorkerCommand>) -> H,
{
    let worker_count = worker_count.max(1);
    for worker_id in 0..worker_count {
        let handler = make_handler(worker_id, control_tx.clone());
        spawn_dedicated_runtime_worker(
            shared_ingress.clone(),
            control_tx.clone(),
            worker_id,
            handler,
        );
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use tokio::sync::{Mutex, broadcast, watch};

    use crate::Payload;
    use crate::internal::scheduler::backend::protocol::{
        BackendOp, BackendReply, BackendRequest, PeerWorkerCommand, RuntimeControlSignal,
        SyncMessage, WorkerCommand,
    };
    use crate::internal::scheduler::backend::runner::{
        RuntimeWorkerHandler, shared_ingress, spawn_runtime_worker,
    };

    #[derive(Clone, Default)]
    struct Observed {
        peer_seqs: Arc<Mutex<Vec<u64>>>,
        runtime_ops: Arc<Mutex<Vec<u64>>>,
        request_count: Arc<AtomicUsize>,
    }

    struct TestHandler {
        observed: Observed,
    }

    #[async_trait]
    impl RuntimeWorkerHandler for TestHandler {
        async fn handle_request(&mut self, req: BackendRequest) {
            self.observed.request_count.fetch_add(1, Ordering::SeqCst);
            let _ = req.reply_tx.send(BackendReply::Value(Payload::default()));
        }

        async fn handle_peer_control(&mut self, cmd: PeerWorkerCommand) {
            self.observed.peer_seqs.lock().await.push(cmd.seq_id());
        }

        async fn handle_runtime_control(&mut self, signal: RuntimeControlSignal) {
            let op_id = match signal {
                RuntimeControlSignal::GlobalLoad { op_id, .. } => op_id,
                RuntimeControlSignal::GlobalUnload { op_id } => op_id,
            };
            self.observed.runtime_ops.lock().await.push(op_id);
        }
    }

    #[tokio::test]
    async fn runner_filters_self_echo_and_stale_peer_sequences() {
        let (ingress_tx, ingress_rx) = flume::bounded::<BackendRequest>(8);
        let (control_tx, control_rx) = broadcast::channel::<WorkerCommand>(8);
        let observed = Observed::default();
        let join = spawn_runtime_worker(
            shared_ingress(ingress_rx),
            control_rx,
            7,
            TestHandler { observed: observed.clone() },
        );

        let _ = control_tx.send(WorkerCommand::Peer(PeerWorkerCommand::Unload {
            sync: SyncMessage::Generation { generation: 1 },
            sender_id: 9,
        }));
        let _ = control_tx.send(WorkerCommand::Peer(PeerWorkerCommand::Unload {
            sync: SyncMessage::Generation { generation: 1 },
            sender_id: 9,
        }));
        let _ = control_tx.send(WorkerCommand::Peer(PeerWorkerCommand::Unload {
            sync: SyncMessage::Generation { generation: 2 },
            sender_id: 7,
        }));
        let _ = control_tx.send(WorkerCommand::Peer(PeerWorkerCommand::Unload {
            sync: SyncMessage::Generation { generation: 2 },
            sender_id: 9,
        }));
        let _ = control_tx.send(WorkerCommand::Peer(PeerWorkerCommand::Unload {
            sync: SyncMessage::Generation { generation: 3 },
            sender_id: 9,
        }));
        let _ = control_tx
            .send(WorkerCommand::Runtime(RuntimeControlSignal::GlobalUnload { op_id: 11 }));

        drop(control_tx);
        drop(ingress_tx);

        tokio::time::timeout(std::time::Duration::from_secs(2), join)
            .await
            .expect("runner should exit after channels are closed")
            .expect("runner task should not panic");

        let peer_seqs = observed.peer_seqs.lock().await.clone();
        let runtime_ops = observed.runtime_ops.lock().await.clone();
        assert_eq!(peer_seqs, vec![1, 3]);
        assert_eq!(runtime_ops, vec![11]);
    }

    #[tokio::test]
    async fn runner_dispatches_ingress_and_runtime_commands() {
        let (ingress_tx, ingress_rx) = flume::bounded::<BackendRequest>(8);
        let (control_tx, control_rx) = broadcast::channel::<WorkerCommand>(8);
        let observed = Observed::default();

        let join = spawn_runtime_worker(
            shared_ingress(ingress_rx),
            control_rx,
            0,
            TestHandler { observed: observed.clone() },
        );

        let (cancel_tx, cancel_rx) = watch::channel(false);
        drop(cancel_tx);
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        ingress_tx
            .send_async(BackendRequest::inference(
                BackendOp::new("test.op", Payload::default()),
                Payload::default(),
                cancel_rx,
                reply_tx,
            ))
            .await
            .expect("ingress send should succeed");

        let _ = control_tx.send(WorkerCommand::Runtime(RuntimeControlSignal::GlobalLoad {
            op_id: 22,
            payload: Payload::default(),
        }));

        let reply = tokio::time::timeout(std::time::Duration::from_secs(2), reply_rx)
            .await
            .expect("request should receive a reply")
            .expect("reply channel should not be dropped");
        assert!(matches!(reply, BackendReply::Value(_)));

        drop(control_tx);
        drop(ingress_tx);

        tokio::time::timeout(std::time::Duration::from_secs(2), join)
            .await
            .expect("runner should exit after channels are closed")
            .expect("runner task should not panic");

        assert_eq!(observed.request_count.load(Ordering::SeqCst), 1);
        let runtime_ops = observed.runtime_ops.lock().await.clone();
        assert_eq!(runtime_ops, vec![22]);
    }
}
