#![allow(dead_code, unused_imports, unused_variables)]

include!("support/backend_fixture.rs");

struct Worker;

#[backend_handler]
impl Worker {
    fn new() -> Self {
        Self
    }

    #[on_peer_control]
    async fn on_peer_any_a(&mut self, _cmd: PeerWorkerCommand) {}

    #[on_peer_control]
    async fn on_peer_any_b(&mut self, _cmd: PeerWorkerCommand) {}
}

fn main() {}
