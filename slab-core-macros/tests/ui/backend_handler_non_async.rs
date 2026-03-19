#![allow(dead_code, unused_imports, unused_variables)]

include!("support/backend_fixture.rs");

struct Worker;

#[backend_handler]
impl Worker {
    #[on_event(Inference)]
    fn on_inference(&mut self, _req: BackendRequest) {}
}

fn main() {}
