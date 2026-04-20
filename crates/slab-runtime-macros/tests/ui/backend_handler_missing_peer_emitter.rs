#![allow(dead_code, unused_imports, unused_variables)]

include!("support/backend_fixture.rs");

struct Worker {
    peer_bus: PeerControlBus,
}

#[backend_handler(peer_bus = peer_bus)]
impl Worker {
    fn new() -> Self {
        Self { peer_bus: PeerControlBus }
    }

    #[on_peer_control(LoadModel)]
    async fn on_peer_load(
        &mut self,
        _seq: BroadcastSeq,
        _input: Input<String>,
    ) -> Result<(), String> {
        Ok(())
    }
}

fn main() {
    let worker = Worker::new();
    worker.emit_peer_unload_generation(7);
}
