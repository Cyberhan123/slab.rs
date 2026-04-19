use slab_runtime_macros::backend_handler;

extern crate self as slab_runtime_core;

pub mod backend {
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

    #[derive(Clone)]
    pub struct BackendRequest;

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

    #[async_trait::async_trait]
    pub trait RuntimeWorkerHandler: Send + 'static {
        fn request_routes() -> &'static [RequestRouteMatcher<Self>]
        where
            Self: Sized,
        {
            &[]
        }

        fn runtime_control_routes() -> &'static [RuntimeRoute<Self>]
        where
            Self: Sized,
        {
            &[]
        }

        fn peer_control_routes() -> &'static [PeerRoute<Self>]
        where
            Self: Sized,
        {
            &[]
        }

        fn peer_control_fallback() -> Option<PeerDispatchFn<Self>>
        where
            Self: Sized,
        {
            None
        }

        fn control_lagged_route() -> Option<LaggedDispatchFn<Self>>
        where
            Self: Sized,
        {
            None
        }

        async fn handle_request(&mut self, req: BackendRequest)
        where
            Self: Sized,
        {
            dispatch_backend_request(self, req, Self::request_routes()).await;
        }

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

        async fn handle_runtime_control(&mut self, signal: RuntimeControlSignal)
        where
            Self: Sized,
        {
            dispatch_runtime_control(self, signal, Self::runtime_control_routes()).await;
        }

        async fn handle_control_lagged(&mut self)
        where
            Self: Sized,
        {
            dispatch_control_lagged(self, Self::control_lagged_route()).await;
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

use crate::backend::{BackendRequest, PeerWorkerCommand, RuntimeControlSignal};
