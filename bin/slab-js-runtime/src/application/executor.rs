use slab_types::PluginRuntimeCallRequest;

#[async_trait::async_trait]
pub trait PluginExecutor: Send + Sync {
    async fn execute(
        &self,
        request: PluginRuntimeCallRequest,
    ) -> Result<slab_types::PluginRuntimeCallResponse, anyhow::Error>;
}
