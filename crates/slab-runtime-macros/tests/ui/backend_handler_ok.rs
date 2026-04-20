#![allow(dead_code, unused_imports, unused_variables)]

include!("support/backend_fixture.rs");

struct Worker {
    peer_bus: PeerControlBus,
}

#[derive(Debug)]
struct WorkerError;

impl std::fmt::Display for WorkerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("worker error")
    }
}

impl std::error::Error for WorkerError {}

#[backend_handler(peer_bus = peer_bus)]
impl Worker {
    fn new() -> Self {
        Self { peer_bus: PeerControlBus }
    }

    #[on_event(LoadModel)]
    async fn on_load_model(
        &mut self,
        _input: Input<String>,
        _seq: BroadcastSeq,
    ) -> Result<(), WorkerError> {
        Ok(())
    }

    #[on_event(Inference)]
    async fn on_inference(
        &mut self,
        _text: String,
        _options: Options<String>,
    ) -> Result<Json<String>, WorkerError> {
        Ok(Json("ok".to_owned()))
    }

    #[on_event(UnloadModel)]
    async fn on_unload_model(&mut self) -> Result<(), WorkerError> {
        Ok(())
    }

    #[on_event(InferenceImage)]
    async fn on_inference_image(
        &mut self,
        _input: Input<String>,
    ) -> Result<Typed<String>, WorkerError> {
        Ok(Typed("typed".to_owned()))
    }

    #[on_runtime_control(GlobalLoad)]
    async fn on_global_load(
        &mut self,
        _op_id: ControlOpId,
        _input: Input<String>,
    ) -> Result<(), WorkerError> {
        Ok(())
    }

    #[on_peer_control(LoadModel)]
    async fn on_peer_load(
        &mut self,
        _seq: BroadcastSeq,
        _input: Input<String>,
    ) -> Result<(), WorkerError> {
        Ok(())
    }

    #[on_peer_control(Unload)]
    async fn on_peer_unload(&mut self, _seq: BroadcastSeq) -> Result<(), WorkerError> {
        Ok(())
    }

    #[on_peer_control]
    async fn on_peer_any(&mut self, _cmd: PeerWorkerCommand) -> Result<(), WorkerError> {
        Ok(())
    }

    #[on_control_lagged]
    async fn on_lagged(&mut self) -> Result<(), WorkerError> {
        Ok(())
    }
}

fn main() {
    let worker = Worker::new();
    worker.emit_peer_load_model_deployment(1, "typed".to_owned());
    worker.emit_peer_load_model_deployment_payload(2, Payload::None);
    worker.emit_peer_unload_generation(3);
    let _ = worker.peer_sender_id();
    let _ = Worker::route_table();
}
