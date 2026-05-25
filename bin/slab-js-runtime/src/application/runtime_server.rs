use std::sync::Arc;

use serde_json::Value;

use crate::application::PluginExecutor;

/// Application-layer runtime server that owns plugin request dispatch.
///
/// Transport adapters should deserialize incoming messages, forward the method
/// and params here, and serialize the returned payload back to the caller.
pub struct PluginRuntimeServer {
    executor: Arc<dyn PluginExecutor>,
}

impl PluginRuntimeServer {
    /// Creates a new runtime server backed by the provided executor.
    pub fn new(executor: Arc<dyn PluginExecutor>) -> Self {
        Self { executor }
    }

    /// Returns the startup payload emitted once a transport session is ready.
    #[must_use]
    pub fn ready_payload(&self) -> Value {
        serde_json::json!({
            "runtime": "slab-js-runtime",
            "engine": "deno"
        })
    }

    /// Handles a runtime request and returns the JSON payload for the response.
    pub async fn handle_request(&self, method: &str, params: Value) -> Result<Value, String> {
        match method {
            "plugin.call" => {
                let request = serde_json::from_value(params)
                    .map_err(|error| format!("invalid plugin.call params: {error}"))?;
                self.executor
                    .execute(request)
                    .await
                    .and_then(|response| serde_json::to_value(response).map_err(Into::into))
                    .map_err(|error| error.to_string())
            }
            _ => Err(format!("unknown runtime method `{method}`")),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use slab_types::{
        PluginPermissionsManifest, PluginRuntimeCallRequest, PluginRuntimeCallResponse,
    };

    use super::PluginRuntimeServer;
    use crate::application::PluginExecutor;

    struct StubExecutor;

    #[async_trait::async_trait]
    impl PluginExecutor for StubExecutor {
        async fn execute(
            &self,
            request: PluginRuntimeCallRequest,
        ) -> Result<PluginRuntimeCallResponse, anyhow::Error> {
            Ok(PluginRuntimeCallResponse {
                result: serde_json::json!({
                    "pluginId": request.plugin_id,
                    "exportName": request.export_name,
                }),
            })
        }
    }

    #[tokio::test]
    async fn handles_plugin_call_requests() {
        let server = PluginRuntimeServer::new(Arc::new(StubExecutor));
        let payload = server
            .handle_request(
                "plugin.call",
                serde_json::to_value(PluginRuntimeCallRequest {
                    call_id: "call-1".to_owned(),
                    plugin_id: "plugin-1".to_owned(),
                    root_dir: ".".to_owned(),
                    entry: "main.ts".into(),
                    bundle: None,
                    export_name: "run".to_owned(),
                    params: serde_json::json!([]),
                    permissions: PluginPermissionsManifest::default(),
                    file_grants: Vec::new(),
                    blocked_fetch_origins: Vec::new(),
                })
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            payload,
            serde_json::json!({
                "result": {
                    "pluginId": "plugin-1",
                    "exportName": "run"
                }
            })
        );
    }

    #[tokio::test]
    async fn rejects_unknown_methods() {
        let server = PluginRuntimeServer::new(Arc::new(StubExecutor));

        let error = server.handle_request("runtime.nope", Value::Null).await.unwrap_err();

        assert_eq!(error, "unknown runtime method `runtime.nope`");
    }
}