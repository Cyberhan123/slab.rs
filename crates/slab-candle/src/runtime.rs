/// Runtime-facing Candle engine contract.
///
/// Implementations load caller-resolved local model assets and execute one
/// inference request without owning process-level CLI, download, or runtime
/// worker orchestration.
pub trait CandleRuntimeEngine {
    type LoadConfig;
    type InferenceRequest;
    type InferenceResponse;
    type Error: std::error::Error + Send + Sync + 'static;

    fn load_model(&mut self, config: Self::LoadConfig) -> Result<(), Self::Error>;
    fn unload_model(&mut self);
    fn is_model_loaded(&self) -> bool;
    fn infer(
        &mut self,
        request: Self::InferenceRequest,
    ) -> Result<Self::InferenceResponse, Self::Error>;
}
