#![allow(dead_code, unused_imports, unused_variables)]

include!("support/backend_fixture.rs");

struct Worker;

#[backend_handler]
impl Worker {
    fn new() -> Self {
        Self
    }

    #[on_event(Inference)]
    async fn on_inference(&mut self, _text: String) {}
}

fn main() {}
