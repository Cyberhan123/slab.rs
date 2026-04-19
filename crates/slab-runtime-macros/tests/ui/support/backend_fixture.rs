use slab_runtime_macros::backend_handler;

extern crate self as slab_runtime_core;

pub mod backend {
    use std::fmt;
    use std::future::Future;
    use std::pin::Pin;

    #[derive(Clone, Copy)]
    pub enum RequestRoute {
        LoadModel,
        UnloadModel,
        Inference,
        InferenceStream,
        InferenceImage,
    }

    #[derive(Clone, Default)]
    pub enum Payload {
        #[default]
        None,
    }

    #[derive(Clone)]
    pub struct BackendOp {
        pub options: Payload,
    }

    #[derive(Clone, Default)]
    pub struct WatchRx;

    #[derive(Clone, Default)]
    pub struct ReplyTx;

    impl ReplyTx {
        pub fn send(self, _reply: BackendReply) -> Result<(), ()> {
            Ok(())
        }
    }

    pub struct BackendRequest {
        pub op: BackendOp,
        pub input: Payload,
        pub cancel_rx: WatchRx,
        pub reply_tx: ReplyTx,
    }

    #[derive(Clone)]
    pub struct Input<T>(pub T);

    #[derive(Clone)]
    pub struct Options<T>(pub T);

    #[derive(Clone)]
    pub struct CancelRx(pub WatchRx);

    #[derive(Clone, Copy, Default)]
    pub struct BroadcastSeq(pub u64);

    #[derive(Clone, Copy, Default)]
    pub struct ControlOpId(pub u64);

    #[derive(Clone)]
    pub struct Json<T>(pub T);

    #[derive(Clone)]
    pub struct Typed<T>(pub T);

    pub enum BackendReply {
        Ack,
    }

    impl BackendReply {
        pub fn error(_message: impl Into<String>) -> Self {
            Self::Ack
        }
    }

    pub trait IntoBackendReply {
        fn into_backend_reply(self) -> Result<BackendReply, String>;
    }

    impl IntoBackendReply for () {
        fn into_backend_reply(self) -> Result<BackendReply, String> {
            Ok(BackendReply::Ack)
        }
    }

    impl<T> IntoBackendReply for Json<T> {
        fn into_backend_reply(self) -> Result<BackendReply, String> {
            Ok(BackendReply::Ack)
        }
    }

    impl<T> IntoBackendReply for Typed<T> {
        fn into_backend_reply(self) -> Result<BackendReply, String> {
            Ok(BackendReply::Ack)
        }
    }

    pub fn backend_reply_from_event_result<T, E>(_result: Result<T, E>) -> BackendReply
    where
        T: IntoBackendReply,
        E: fmt::Display,
    {
        BackendReply::Ack
    }

    pub fn extract_event_text(_req: &BackendRequest) -> Result<String, String> {
        Ok(String::new())
    }

    pub fn extract_event_payload(req: &BackendRequest) -> Result<Payload, String> {
        Ok(req.input.clone())
    }

    pub fn extract_event_input<T>(_req: &BackendRequest) -> Result<Input<T>, String> {
        Err("unsupported in fixture".to_owned())
    }

    pub fn extract_event_options<T>(_req: &BackendRequest) -> Result<Options<T>, String> {
        Err("unsupported in fixture".to_owned())
    }

    pub fn extract_event_cancel_rx(req: &BackendRequest) -> Result<CancelRx, String> {
        Ok(CancelRx(req.cancel_rx.clone()))
    }

    pub fn extract_event_broadcast_seq(_req: &BackendRequest) -> Result<BroadcastSeq, String> {
        Ok(BroadcastSeq(0))
    }

    pub fn extract_runtime_control_op_id(
        _signal: &RuntimeControlSignal,
    ) -> Result<ControlOpId, String> {
        Ok(ControlOpId(0))
    }

    pub fn extract_runtime_control_payload(
        _signal: &RuntimeControlSignal,
    ) -> Result<Payload, String> {
        Ok(Payload::None)
    }

    pub fn extract_runtime_control_input<T>(
        _signal: &RuntimeControlSignal,
    ) -> Result<Input<T>, String> {
        Err("unsupported in fixture".to_owned())
    }

    pub fn extract_peer_control_payload(_cmd: &PeerWorkerCommand) -> Result<Payload, String> {
        Ok(Payload::None)
    }

    pub fn extract_peer_control_input<T>(_cmd: &PeerWorkerCommand) -> Result<Input<T>, String> {
        Err("unsupported in fixture".to_owned())
    }

    pub fn extract_peer_control_broadcast_seq(
        _cmd: &PeerWorkerCommand,
    ) -> Result<BroadcastSeq, String> {
        Ok(BroadcastSeq(0))
    }

    #[derive(Clone)]
    pub enum RuntimeControlSignal {
        GlobalLoad {},
        GlobalUnload {},
    }

    #[derive(Clone)]
    pub enum PeerWorkerCommand {
        LoadModel {},
        Unload {},
    }

    pub type HandlerFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
    pub type RequestDispatchFn<T> = for<'a> fn(&'a mut T, BackendRequest) -> HandlerFuture<'a>;
    pub type RuntimeDispatchFn<T> =
        for<'a> fn(&'a mut T, RuntimeControlSignal) -> HandlerFuture<'a>;
    pub type PeerDispatchFn<T> = for<'a> fn(&'a mut T, PeerWorkerCommand) -> HandlerFuture<'a>;
    pub type LaggedDispatchFn<T> = for<'a> fn(&'a mut T) -> HandlerFuture<'a>;

    pub struct RequestRouteMatcher<T> {
        pub matches: fn(RequestRoute) -> bool,
        pub handle: RequestDispatchFn<T>,
    }

    pub struct RuntimeRoute<T> {
        pub matches: fn(&RuntimeControlSignal) -> bool,
        pub handle: RuntimeDispatchFn<T>,
    }

    pub struct PeerRoute<T> {
        pub matches: fn(&PeerWorkerCommand) -> bool,
        pub handle: PeerDispatchFn<T>,
    }

    #[derive(Clone, Copy)]
    pub struct WorkerRouteTable<T: 'static> {
        pub request_routes: &'static [RequestRouteMatcher<T>],
        pub runtime_control_routes: &'static [RuntimeRoute<T>],
        pub peer_control_routes: &'static [PeerRoute<T>],
        pub peer_control_fallback: Option<PeerDispatchFn<T>>,
        pub control_lagged_route: Option<LaggedDispatchFn<T>>,
    }

    impl<T: 'static> Default for WorkerRouteTable<T> {
        fn default() -> Self {
            Self {
                request_routes: &[],
                runtime_control_routes: &[],
                peer_control_routes: &[],
                peer_control_fallback: None,
                control_lagged_route: None,
            }
        }
    }

    #[async_trait::async_trait]
    pub trait RuntimeWorkerHandler: Send + 'static {
        fn route_table(&self) -> WorkerRouteTable<Self>
        where
            Self: Sized,
        {
            WorkerRouteTable::default()
        }

        async fn handle_request(&mut self, req: BackendRequest)
        where
            Self: Sized,
        {
            let route_table = self.route_table();
            dispatch_backend_request(self, req, route_table.request_routes).await;
        }

        async fn handle_peer_control(&mut self, cmd: PeerWorkerCommand)
        where
            Self: Sized,
        {
            let route_table = self.route_table();
            dispatch_peer_control(
                self,
                cmd,
                route_table.peer_control_fallback,
                route_table.peer_control_routes,
            )
            .await;
        }

        async fn handle_runtime_control(&mut self, signal: RuntimeControlSignal)
        where
            Self: Sized,
        {
            let route_table = self.route_table();
            dispatch_runtime_control(self, signal, route_table.runtime_control_routes).await;
        }

        async fn handle_control_lagged(&mut self)
        where
            Self: Sized,
        {
            let route_table = self.route_table();
            dispatch_control_lagged(self, route_table.control_lagged_route).await;
        }
    }

    pub async fn dispatch_backend_request<T>(
        _handler: &mut T,
        _req: BackendRequest,
        _routes: &[RequestRouteMatcher<T>],
    ) {
    }

    pub async fn dispatch_peer_control<T>(
        _handler: &mut T,
        _cmd: PeerWorkerCommand,
        _route: Option<PeerDispatchFn<T>>,
        _routes: &[PeerRoute<T>],
    ) {
    }

    pub async fn dispatch_runtime_control<T>(
        _handler: &mut T,
        _signal: RuntimeControlSignal,
        _routes: &[RuntimeRoute<T>],
    ) {
    }

    pub async fn dispatch_control_lagged<T>(
        _handler: &mut T,
        _route: Option<LaggedDispatchFn<T>>,
    ) {
    }
}

use crate::backend::{
    BackendRequest, BroadcastSeq, CancelRx, ControlOpId, Input, Json, Options, Payload,
    PeerWorkerCommand, RuntimeControlSignal, Typed,
};
