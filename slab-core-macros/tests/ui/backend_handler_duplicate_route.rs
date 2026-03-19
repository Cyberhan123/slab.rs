#![allow(dead_code, unused_imports, unused_variables)]

include!("support/backend_fixture.rs");

struct Worker;

#[backend_handler]
impl Worker {
    #[on_event(Inference)]
    async fn on_inference_a(&mut self, _req: BackendRequest) {}

    #[on_event(Inference)]
    async fn on_inference_b(&mut self, _req: BackendRequest) {}
}

fn main() {}
