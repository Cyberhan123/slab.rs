#![allow(dead_code, unused_imports, unused_variables)]

include!("support/backend_fixture.rs");

struct Worker;

#[backend_handler]
impl Worker {
    fn new() -> Self {
        Self
    }

    #[on_event(LoadModel)]
    async fn on_load_model(
        &mut self,
        _input: Input<String>,
        _seq: BroadcastSeq,
    ) -> Result<(), String> {
        Ok(())
    }

    #[on_event(Inference)]
    async fn on_inference(
        &mut self,
        _text: String,
        _options: Options<String>,
    ) -> Result<Json<String>, String> {
        Ok(Json("ok".to_owned()))
    }

    #[on_event(UnloadModel)]
    async fn on_unload_model(&mut self) -> Result<(), String> {
        Ok(())
    }

    #[on_event(InferenceImage)]
    async fn on_inference_image(&mut self, _input: Input<String>) -> Result<Typed<String>, String> {
        Ok(Typed("typed".to_owned()))
    }

    #[on_runtime_control(GlobalLoad)]
    async fn on_global_load(
        &mut self,
        _op_id: ControlOpId,
        _input: Input<String>,
    ) -> Result<(), String> {
        Ok(())
    }

    #[on_peer_control(LoadModel)]
    async fn on_peer_load(
        &mut self,
        _seq: BroadcastSeq,
        _input: Input<String>,
    ) -> Result<(), String> {
        Ok(())
    }

    #[on_peer_control]
    async fn on_peer_any(&mut self, _cmd: PeerWorkerCommand) -> Result<(), String> {
        Ok(())
    }

    #[on_control_lagged]
    async fn on_lagged(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn main() {
    let _ = Worker::route_table();
}
