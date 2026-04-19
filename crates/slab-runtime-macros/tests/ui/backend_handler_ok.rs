#![allow(dead_code, unused_imports, unused_variables)]

include!("support/backend_fixture.rs");

struct Worker;

#[backend_handler]
impl Worker {
    fn new() -> Self {
        Self
    }

    #[on_event(Inference)]
    async fn on_inference(&mut self, _req: BackendRequest) {}

    #[on_runtime_control(GlobalLoad)]
    async fn on_global_load(&mut self, _signal: RuntimeControlSignal) {}

    #[on_peer_control(LoadModel)]
    async fn on_peer_load(&mut self, _cmd: PeerWorkerCommand) {}

    #[on_peer_control]
    async fn on_peer_any(&mut self, _cmd: PeerWorkerCommand) {}

    #[on_control_lagged]
    async fn on_lagged(&mut self) {}
}

fn main() {
    let _ = Worker::routes();
    let _ = Worker::runtime_routes();
    let _ = Worker::peer_routes();
    let _ = Worker::peer_fallback_route();
    let _ = Worker::lagged_route();
}
