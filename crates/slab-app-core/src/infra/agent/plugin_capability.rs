//! Plugin agent-capability proxy tools (B-7 / ADR-009).
//!
//! Each `Tool`-kind capability of an enabled plugin is exposed to the agent as
//! a `plugin__<plugin_id>__<capability_id>` tool, mirroring the MCP proxy
//! naming (`mcp__<server>__<tool>`). The proxy delegates execution to a
//! host-injected [`PluginToolPort`] adapter that routes the call to the
//! supervised plugin runtime via [`PluginService::dispatch_agent_capability`].
//!
//! Effect trust is **host-inferred** from the plugin's runtime kind
//! ([`infer_effect_trust`]); plugins cannot self-report it (red-team must_add).
//! The inferred tier is stamped on every tool output's metadata so the host
//! observability layer sees the isolation tier that was applied.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};

use slab_agent::{AgentError, PluginToolPort, ToolContext, ToolHandler, ToolOutput, ToolRouter};
use slab_types::{
    PluginCapabilityKind, PluginManifest,
    plugin_capability::{CapabilityEffectTrust, infer_effect_trust, plugin_agent_tool_name},
};

use crate::domain::services::PluginService;

/// Enabled plugin carrying its manifest, used to enumerate agent capabilities
/// (parallel to `hooks::PluginHookSource`).
#[derive(Clone)]
pub(crate) struct PluginCapabilitySource {
    pub(crate) manifest: PluginManifest,
    #[allow(dead_code)]
    pub(crate) root_dir: std::path::PathBuf,
}

/// One agent-visible plugin capability, ready to be wrapped as a proxy tool.
struct CapabilityDescriptor {
    plugin_id: String,
    capability_id: String,
    description: String,
    input_schema: Value,
    trust: Option<CapabilityEffectTrust>,
}

impl CapabilityDescriptor {
    /// Build one descriptor per `Tool`-kind capability declared by `source`.
    /// `Workflow` / `A2uSurface` capabilities are not callable agent tools and
    /// are skipped. Trust is derived once from the plugin's runtime kind.
    fn for_source(source: &PluginCapabilitySource) -> Vec<Self> {
        let plugin_id = source.manifest.id.clone();
        let has_js = source.manifest.runtime.js.is_some();
        let has_python = source.manifest.runtime.python.is_some();
        let has_wasm = source.manifest.runtime.wasm.is_some();
        let trust = infer_effect_trust(has_js, has_python, has_wasm);

        source
            .manifest
            .contributes
            .agent_capabilities
            .iter()
            .filter(|cap| cap.kind == PluginCapabilityKind::Tool)
            .map(|cap| CapabilityDescriptor {
                plugin_id: plugin_id.clone(),
                capability_id: cap.id.clone(),
                description: cap
                    .description
                    .clone()
                    .filter(|desc| !desc.trim().is_empty())
                    .unwrap_or_else(|| format!("Plugin capability `{plugin_id}`.")),
                input_schema: parse_input_schema(cap.input_schema.as_deref()),
                trust,
            })
            .collect()
    }
}

/// Proxy tool that forwards a `plugin__<id>__<cap>` call to the plugin runtime.
pub(crate) struct PluginCapabilityProxyTool {
    port: Arc<dyn PluginToolPort>,
    descriptor: CapabilityDescriptor,
    tool_name: String,
}

impl PluginCapabilityProxyTool {
    fn new(port: Arc<dyn PluginToolPort>, descriptor: CapabilityDescriptor) -> Self {
        let tool_name = plugin_agent_tool_name(&descriptor.plugin_id, &descriptor.capability_id);
        Self { port, descriptor, tool_name }
    }
}

#[async_trait]
impl ToolHandler for PluginCapabilityProxyTool {
    fn name(&self) -> &str {
        &self.tool_name
    }

    fn description(&self) -> &str {
        &self.descriptor.description
    }

    fn parameters_schema(&self) -> Value {
        if self.descriptor.input_schema.is_null() {
            return json!({ "type": "object", "properties": {} });
        }
        self.descriptor.input_schema.clone()
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let content = self
            .port
            .call_capability(&self.descriptor.plugin_id, &self.descriptor.capability_id, arguments)
            .await?;
        // Stamp the host-inferred trust tier (not plugin-self-reported) so the
        // host observability layer can audit which isolation tier was applied.
        let metadata = json!({
            "plugin_capability": {
                "plugin_id": self.descriptor.plugin_id,
                "capability_id": self.descriptor.capability_id,
                "trust": self.descriptor.trust.map(|trust| trust.as_str()),
            }
        });
        Ok(ToolOutput { content, metadata: Some(metadata) })
    }
}

/// Adapter bridging [`PluginToolPort`] to [`PluginService::dispatch_agent_capability`].
///
/// Lives in the composition root layer (app-core infra) so `slab-agent` never
/// depends on plugin/runtime machinery.
pub(crate) struct PluginServiceCapabilityPort {
    service: PluginService,
}

impl PluginServiceCapabilityPort {
    pub(crate) fn new(service: PluginService) -> Self {
        Self { service }
    }
}

#[async_trait]
impl PluginToolPort for PluginServiceCapabilityPort {
    async fn call_capability(
        &self,
        plugin_id: &str,
        capability_id: &str,
        arguments: &Value,
    ) -> Result<String, AgentError> {
        let value = self
            .service
            .dispatch_agent_capability(plugin_id, capability_id, arguments.clone())
            .await
            .map_err(|error| AgentError::ToolExecution(error.to_string()))?;
        serde_json::to_string(&value).map_err(|error| {
            AgentError::ToolExecution(format!("plugin capability result is not JSON: {error}"))
        })
    }
}

/// Register a `plugin__<id>__<cap>` proxy for every `Tool`-kind capability of
/// every enabled plugin in `sources`, all dispatching through `port`. A proxy
/// replaces any previously registered handler with the same tool name.
pub(crate) fn register_plugin_capability_tools(
    router: &ToolRouter,
    port: Arc<dyn PluginToolPort>,
    sources: &[PluginCapabilitySource],
) {
    for source in sources {
        for descriptor in CapabilityDescriptor::for_source(source) {
            router
                .register(Box::new(PluginCapabilityProxyTool::new(Arc::clone(&port), descriptor)));
        }
    }
}

/// Parse a capability `inputSchema` string into a JSON Schema value. An absent,
/// empty, or invalid schema becomes [`Value::Null`] (the proxy surfaces an
/// empty object schema in that case).
fn parse_input_schema(raw: Option<&str>) -> Value {
    let Some(raw) = raw.map(str::trim).filter(|raw| !raw.is_empty()) else {
        return Value::Null;
    };
    serde_json::from_str::<Value>(raw).unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use serde_json::{Value, json};
    use slab_agent::{PluginToolPort, ToolContext, ToolHandler, ToolRouter};
    use slab_types::PluginManifest;

    use super::{
        CapabilityDescriptor, PluginCapabilityProxyTool, PluginCapabilitySource, parse_input_schema,
    };

    /// Test double that records the calls it receives.
    struct CapturingPort {
        calls: Mutex<Vec<(String, String, Value)>>,
        result: Value,
    }

    impl CapturingPort {
        fn new(result: Value) -> Self {
            Self { calls: Mutex::new(Vec::new()), result }
        }
    }

    #[async_trait]
    impl PluginToolPort for CapturingPort {
        async fn call_capability(
            &self,
            plugin_id: &str,
            capability_id: &str,
            arguments: &Value,
        ) -> Result<String, slab_agent::AgentError> {
            self.calls.lock().unwrap().push((
                plugin_id.to_owned(),
                capability_id.to_owned(),
                arguments.clone(),
            ));
            Ok(serde_json::to_string(&self.result).unwrap())
        }
    }

    /// Deserialize a v1 manifest with the given `agentCapabilities` JSON array.
    /// `with_js = true` adds a JS runtime (⇒ host-inferred TauriSandbox trust).
    fn manifest_with_caps(id: &str, capabilities: Vec<Value>, with_js: bool) -> PluginManifest {
        let mut runtime = json!({ "ui": { "entry": "ui/index.html" } });
        if with_js {
            runtime["js"] = json!({ "entry": "main.js" });
        }
        serde_json::from_value(json!({
            "manifestVersion": 1,
            "id": id,
            "name": id,
            "version": "0.1.0",
            "runtime": runtime,
            "contributes": { "agentCapabilities": capabilities }
        }))
        .expect("test manifest deserializes")
    }

    fn tool_cap(id: &str, input_schema: Option<&str>) -> Value {
        let mut cap = json!({
            "id": id,
            "kind": "tool",
            "description": format!("cap {id}"),
            "transport": { "type": "pluginCall", "function": format!("do_{id}") }
        });
        if let Some(schema) = input_schema {
            cap["inputSchema"] = json!(schema);
        }
        cap
    }

    fn source(id: &str, capabilities: Vec<Value>, with_js: bool) -> PluginCapabilitySource {
        PluginCapabilitySource {
            manifest: manifest_with_caps(id, capabilities, with_js),
            root_dir: ".".into(),
        }
    }

    fn ctx() -> ToolContext {
        ToolContext::for_thread("thread").build()
    }

    #[test]
    fn descriptors_skip_non_tool_capabilities() {
        let capabilities = vec![
            tool_cap("t1", None),
            json!({ "id": "w1", "kind": "workflow", "transport": { "type": "pluginCall", "function": "do_w1" } }),
        ];
        let descriptors =
            CapabilityDescriptor::for_source(&source("team-plugin", capabilities, false));

        // Only the `tool`-kind capability is exposed; `workflow` is skipped.
        assert_eq!(descriptors.len(), 1);
        assert_eq!(descriptors[0].capability_id, "t1");
        assert_eq!(descriptors[0].plugin_id, "team-plugin");
    }

    #[test]
    fn proxy_name_uses_stable_plugin_capability_naming() {
        let descriptor = CapabilityDescriptor {
            plugin_id: "video-subtitle-translator".to_owned(),
            capability_id: "translate".to_owned(),
            description: "translate".to_owned(),
            input_schema: Value::Null,
            trust: None,
        };
        let port: Arc<dyn PluginToolPort> = Arc::new(CapturingPort::new(json!({})));
        let proxy = PluginCapabilityProxyTool::new(Arc::clone(&port), descriptor);

        // Hyphens are sanitized to `_`, mirroring the MCP proxy naming.
        assert_eq!(proxy.name(), "plugin__video_subtitle_translator__translate");
    }

    #[test]
    fn proxy_surface_empty_object_schema_when_input_schema_absent() {
        let descriptor = CapabilityDescriptor {
            plugin_id: "p".to_owned(),
            capability_id: "c".to_owned(),
            description: "c".to_owned(),
            input_schema: Value::Null,
            trust: None,
        };
        let proxy =
            PluginCapabilityProxyTool::new(Arc::new(CapturingPort::new(json!({}))), descriptor);

        assert_eq!(proxy.parameters_schema(), json!({ "type": "object", "properties": {} }));
    }

    #[test]
    fn parse_input_schema_handles_absent_and_invalid() {
        assert!(parse_input_schema(None).is_null());
        assert!(parse_input_schema(Some("   ")).is_null());
        assert!(parse_input_schema(Some("{not json")).is_null());
        let parsed =
            parse_input_schema(Some(r#"{"type":"object","properties":{"q":{"type":"string"}}}"#));
        assert_eq!(parsed["properties"]["q"]["type"], "string");
    }

    #[tokio::test]
    async fn proxy_execute_forwards_args_and_stamps_host_inferred_trust() {
        let port = Arc::new(CapturingPort::new(json!({ "translated": "hola" })));
        // JS runtime present ⇒ host-inferred TauriSandbox trust tier.
        let descriptor =
            CapabilityDescriptor::for_source(&source("p", vec![tool_cap("c", None)], true))
                .pop()
                .unwrap();
        let port_for_proxy: Arc<CapturingPort> = Arc::clone(&port);
        let proxy = PluginCapabilityProxyTool::new(port_for_proxy, descriptor);

        let output = proxy.execute(&ctx(), &json!({ "text": "hello" })).await.expect("execute");

        assert_eq!(output.content, r#"{"translated":"hola"}"#);
        // The port received (plugin_id, capability_id, arguments) verbatim.
        let calls = port.calls.lock().unwrap();
        assert_eq!(calls[0].0, "p");
        assert_eq!(calls[0].1, "c");
        assert_eq!(calls[0].2["text"], "hello");
        // Host-inferred trust (JS ⇒ tauri_sandbox) is stamped on the metadata.
        assert_eq!(output.metadata.unwrap()["plugin_capability"]["trust"], "tauri_sandbox");
    }

    #[tokio::test]
    async fn proxy_execute_surfaces_port_error_as_tool_execution() {
        struct ErrorPort;
        #[async_trait]
        impl PluginToolPort for ErrorPort {
            async fn call_capability(
                &self,
                _plugin_id: &str,
                _capability_id: &str,
                _arguments: &Value,
            ) -> Result<String, slab_agent::AgentError> {
                Err(slab_agent::AgentError::ToolExecution("plugin missing".to_owned()))
            }
        }
        let descriptor = CapabilityDescriptor {
            plugin_id: "p".to_owned(),
            capability_id: "c".to_owned(),
            description: "c".to_owned(),
            input_schema: Value::Null,
            trust: None,
        };
        let proxy = PluginCapabilityProxyTool::new(Arc::new(ErrorPort), descriptor);

        let error = proxy.execute(&ctx(), &json!({})).await.expect_err("should error");
        assert!(matches!(error, slab_agent::AgentError::ToolExecution(_)));
    }

    #[test]
    fn register_plugin_capability_tools_registers_one_proxy_per_tool_capability() {
        let router = ToolRouter::new();
        let port: Arc<dyn PluginToolPort> = Arc::new(CapturingPort::new(json!({})));
        let src = source(
            "team-plugin",
            vec![
                tool_cap("search", None),
                tool_cap("render", Some(r#"{"type":"object"}"#)),
                json!({ "id": "surf", "kind": "a2u_surface", "transport": { "type": "pluginCall", "function": "do_surf" } }),
            ],
            false,
        );
        super::register_plugin_capability_tools(&router, port, std::slice::from_ref(&src));

        assert!(router.get("plugin__team_plugin__search").is_some());
        assert!(router.get("plugin__team_plugin__render").is_some());
        // a2u_surface (non-tool) and unknown capabilities get no proxy.
        assert!(router.get("plugin__team_plugin__surf").is_none());
        assert!(router.get("plugin__team_plugin__missing").is_none());
    }
}
