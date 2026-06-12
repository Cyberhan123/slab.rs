use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use slab_agent::AgentHook;
use slab_agent_memories::hooks::{
    HookScript, HookScriptLanguage, HookScriptOutput, HookScriptRunner, RegisteredScriptHook,
};
use slab_config::{
    AgentHookScriptConfig, AgentHookScriptLanguage, AgentHooksConfig, PluginJsRuntimeTransport,
    PluginPythonRuntimeTransport,
};
use slab_types::{PluginAgentHookRuntime, PluginPermissionsManifest, PluginRuntimeCallRequest};
use uuid::Uuid;

use crate::config::Config;
use crate::infra::endpoint::ensure_http_base_url;
use crate::infra::plugin_runtime::{
    PluginEventBus, PluginSidecarRuntimeClient, PluginSidecarRuntimeKind, PluginSidecarTransport,
};

#[derive(Clone)]
pub(crate) struct PluginHookSource {
    pub(crate) manifest: slab_types::PluginManifest,
    pub(crate) root_dir: std::path::PathBuf,
}

pub(crate) fn registered_script_hook(
    config: &AgentHooksConfig,
    app_config: &Config,
) -> Option<Arc<dyn AgentHook>> {
    if !config.enabled {
        return None;
    }
    registered_hook_from_scripts(legacy_hook_scripts(config), app_config)
}

pub(crate) fn registered_hook_from_scripts(
    scripts: Vec<HookScript>,
    app_config: &Config,
) -> Option<Arc<dyn AgentHook>> {
    if scripts.is_empty() {
        return None;
    }

    let event_bus = PluginEventBus::new();
    let api_base_url = ensure_http_base_url(app_config.bind_address.as_str())
        .unwrap_or_else(|_| slab_types::DESKTOP_API_ORIGIN.to_owned());
    let js_runtime = PluginSidecarRuntimeClient::for_current_server(
        PluginSidecarRuntimeKind::JavaScript,
        plugin_js_transport(app_config.plugin_js_runtime_transport),
        api_base_url.clone(),
        event_bus.clone(),
    );
    let python_runtime = PluginSidecarRuntimeClient::for_current_server(
        PluginSidecarRuntimeKind::Python,
        plugin_python_transport(app_config.plugin_python_runtime_transport),
        api_base_url,
        event_bus,
    );
    let runner = Arc::new(SidecarHookScriptRunner { js_runtime, python_runtime });
    Some(Arc::new(RegisteredScriptHook::new(scripts, runner)))
}

pub(crate) fn legacy_hook_scripts(config: &AgentHooksConfig) -> Vec<HookScript> {
    config.scripts.iter().filter(|script| script.enabled).filter_map(hook_script).collect()
}

pub(crate) fn plugin_hook_scripts(plugins: &[PluginHookSource]) -> Vec<HookScript> {
    plugins.iter().flat_map(plugin_hook_scripts_for_plugin).collect()
}

fn hook_script(config: &AgentHookScriptConfig) -> Option<HookScript> {
    if config.name.trim().is_empty()
        || config.root_dir.trim().is_empty()
        || config.entry.trim().is_empty()
        || config.export_name.trim().is_empty()
    {
        tracing::warn!(name = %config.name, "skipping incomplete agent hook script");
        return None;
    }
    Some(HookScript {
        name: config.name.clone(),
        plugin_id: None,
        language: match config.language {
            AgentHookScriptLanguage::JavaScript => HookScriptLanguage::JavaScript,
            AgentHookScriptLanguage::Python => HookScriptLanguage::Python,
        },
        root_dir: config.root_dir.clone(),
        entry: config.entry.clone(),
        bundle: None,
        export_name: config.export_name.clone(),
        events: config.events.clone(),
        permissions: PluginPermissionsManifest::default(),
    })
}

fn plugin_hook_scripts_for_plugin(plugin: &PluginHookSource) -> Vec<HookScript> {
    plugin
        .manifest
        .contributes
        .agent_hooks
        .iter()
        .filter_map(|hook| {
            if hook.id.trim().is_empty() || hook.transport.function.trim().is_empty() {
                tracing::warn!(
                    plugin_id = %plugin.manifest.id,
                    hook_id = %hook.id,
                    "skipping incomplete plugin agent hook"
                );
                return None;
            }
            let (language, entry, bundle) = match hook.transport.runtime {
                PluginAgentHookRuntime::JavaScript => {
                    let Some(js) = plugin.manifest.runtime.js.as_ref() else {
                        tracing::warn!(
                            plugin_id = %plugin.manifest.id,
                            hook_id = %hook.id,
                            "skipping plugin agent hook without runtime.js entry"
                        );
                        return None;
                    };
                    (HookScriptLanguage::JavaScript, js.entry.clone(), None)
                }
                PluginAgentHookRuntime::Python => {
                    let Some(python) = plugin.manifest.runtime.python.as_ref() else {
                        tracing::warn!(
                            plugin_id = %plugin.manifest.id,
                            hook_id = %hook.id,
                            "skipping plugin agent hook without runtime.python entry"
                        );
                        return None;
                    };
                    (HookScriptLanguage::Python, python.entry.clone(), python.bundle.clone())
                }
            };
            Some(HookScript {
                name: format!("{}.{}", plugin.manifest.id, hook.id),
                plugin_id: Some(plugin.manifest.id.clone()),
                language,
                root_dir: plugin.root_dir.to_string_lossy().into_owned(),
                entry,
                bundle,
                export_name: hook.transport.function.clone(),
                events: hook.events.iter().map(|event| event.as_str().to_owned()).collect(),
                permissions: plugin.manifest.permissions.clone(),
            })
        })
        .collect()
}

struct SidecarHookScriptRunner {
    js_runtime: PluginSidecarRuntimeClient,
    python_runtime: PluginSidecarRuntimeClient,
}

#[async_trait]
impl HookScriptRunner for SidecarHookScriptRunner {
    async fn run(
        &self,
        script: &HookScript,
        event_name: &str,
        payload: serde_json::Value,
    ) -> std::result::Result<HookScriptOutput, String> {
        let request = hook_runtime_request(script, event_name, payload);
        let runtime = match script.language {
            HookScriptLanguage::JavaScript => &self.js_runtime,
            HookScriptLanguage::Python => &self.python_runtime,
        };
        let response = runtime.call(request).await.map_err(|error| error.to_string())?;
        serde_json::from_value(response.result)
            .map_err(|error| format!("invalid hook script output: {error}"))
    }
}

fn hook_runtime_request(
    script: &HookScript,
    event_name: &str,
    payload: serde_json::Value,
) -> PluginRuntimeCallRequest {
    let plugin_id = runtime_plugin_id(script);
    PluginRuntimeCallRequest {
        call_id: Uuid::new_v4().to_string(),
        plugin_id: plugin_id.clone(),
        root_dir: script.root_dir.clone(),
        entry: script.entry.clone(),
        bundle: script.bundle.clone(),
        export_name: script.export_name.clone(),
        params: serde_json::json!({
            "hookId": script.name,
            "pluginId": plugin_id,
            "event": event_name,
            "callType": hook_call_type(event_name),
            "sessionId": payload.get("session_id").cloned().unwrap_or(serde_json::Value::Null),
            "threadId": payload.get("thread_id").cloned().unwrap_or(serde_json::Value::Null),
            "payload": normalized_hook_payload(event_name, &payload),
        }),
        permissions: script.permissions.clone(),
        file_grants: Vec::new(),
        blocked_fetch_origins: Vec::new(),
    }
}

fn normalized_hook_payload(event_name: &str, payload: &Value) -> Value {
    let response = payload.get("response").cloned();
    let status = payload
        .get("status")
        .cloned()
        .or_else(|| response.as_ref().and_then(|value| value.get("finish_reason").cloned()))
        .unwrap_or(Value::Null);
    let mut normalized = serde_json::json!({
        "messages": payload.get("messages").cloned().unwrap_or_else(|| serde_json::json!([])),
        "tools": payload.get("tools").cloned().unwrap_or_else(|| serde_json::json!([])),
        "toolCall": normalized_tool_call(event_name, payload),
        "status": status,
    });
    if let Some(turn_index) = payload.get("turn_index").cloned() {
        normalized["turnIndex"] = turn_index;
    }
    if let Some(response) = response {
        normalized["response"] = response;
    }
    if let Some(config) = payload.get("config").cloned() {
        normalized["config"] = config;
    }
    if let Some(parent_id) = payload.get("parent_id").cloned() {
        normalized["parentId"] = parent_id;
    }
    if let Some(depth) = payload.get("depth").cloned() {
        normalized["depth"] = depth;
    }
    if let Some(error) = payload.get("error").cloned() {
        normalized["error"] = error;
    }
    normalized
}

fn normalized_tool_call(event_name: &str, payload: &Value) -> Value {
    if !matches!(event_name, "on_tool_start" | "on_tool_end") {
        return Value::Null;
    }
    serde_json::json!({
        "id": payload.get("call_id").cloned().unwrap_or(Value::Null),
        "name": payload.get("tool_name").cloned().unwrap_or(Value::Null),
        "arguments": payload.get("arguments").cloned().unwrap_or(Value::Null),
        "output": payload.get("output").cloned().unwrap_or(Value::Null),
    })
}

fn runtime_plugin_id(script: &HookScript) -> String {
    script.plugin_id.clone().unwrap_or_else(|| format!("agent-hook-{}", script.name))
}

fn hook_call_type(event_name: &str) -> &'static str {
    match event_name {
        "on_agent_start" | "on_agent_end" => "agent_lifecycle",
        "on_llm_start" | "on_llm_end" => "llm",
        "on_tool_start" | "on_tool_end" => "tool",
        _ => "unknown",
    }
}

fn plugin_js_transport(value: PluginJsRuntimeTransport) -> PluginSidecarTransport {
    match value {
        PluginJsRuntimeTransport::Stdio => PluginSidecarTransport::Stdio,
        PluginJsRuntimeTransport::Uds => PluginSidecarTransport::Uds,
    }
}

fn plugin_python_transport(value: PluginPythonRuntimeTransport) -> PluginSidecarTransport {
    match value {
        PluginPythonRuntimeTransport::Stdio => PluginSidecarTransport::Stdio,
        PluginPythonRuntimeTransport::Uds => PluginSidecarTransport::Uds,
    }
}

#[cfg(test)]
mod tests {
    use slab_types::PluginNetworkMode;

    use super::*;

    #[test]
    fn hook_runtime_request_uses_local_script_shape_and_no_permissions() {
        let request = hook_runtime_request(
            &HookScript {
                name: "local".to_owned(),
                plugin_id: None,
                language: HookScriptLanguage::JavaScript,
                root_dir: "C:/hooks".to_owned(),
                entry: "hook.mjs".to_owned(),
                bundle: None,
                export_name: "run".to_owned(),
                events: Vec::new(),
                permissions: PluginPermissionsManifest::default(),
            },
            "on_llm_start",
            serde_json::json!({"thread_id": "thread-1", "session_id": "session-1"}),
        );

        assert_eq!(request.plugin_id, "agent-hook-local");
        assert_eq!(request.root_dir, "C:/hooks");
        assert_eq!(request.entry, "hook.mjs");
        assert_eq!(request.export_name, "run");
        assert_eq!(request.params["hookId"], "local");
        assert_eq!(request.params["pluginId"], "agent-hook-local");
        assert_eq!(request.params["event"], "on_llm_start");
        assert_eq!(request.params["callType"], "llm");
        assert_eq!(request.params["sessionId"], "session-1");
        assert_eq!(request.params["threadId"], "thread-1");
        assert_eq!(request.params["payload"]["messages"], serde_json::json!([]));
        assert_eq!(request.params["payload"]["tools"], serde_json::json!([]));
        assert_eq!(request.params["payload"]["toolCall"], serde_json::Value::Null);
        assert_eq!(request.params["payload"]["status"], serde_json::Value::Null);
        assert!(request.permissions.slab_api.is_empty());
        assert!(request.permissions.files.read.is_empty());
        assert_eq!(request.permissions.network.mode, PluginNetworkMode::Blocked);
        assert!(request.file_grants.is_empty());
    }

    #[test]
    fn hook_runtime_request_normalizes_tool_payload() {
        let request = hook_runtime_request(
            &HookScript {
                name: "plugin.hook".to_owned(),
                plugin_id: Some("plugin".to_owned()),
                language: HookScriptLanguage::Python,
                root_dir: "C:/plugins/plugin".to_owned(),
                entry: "hook.py".to_owned(),
                bundle: Some("bundle.slabpy".to_owned()),
                export_name: "run".to_owned(),
                events: Vec::new(),
                permissions: PluginPermissionsManifest::default(),
            },
            "on_tool_end",
            serde_json::json!({
                "thread_id": "thread-1",
                "session_id": "session-1",
                "turn_index": 3,
                "messages": [{"role": "user", "content": "hello"}],
                "call_id": "call-1",
                "tool_name": "read_file",
                "arguments": {"path": "README.md"},
                "output": "ok",
                "status": "completed"
            }),
        );

        assert_eq!(request.plugin_id, "plugin");
        assert_eq!(request.bundle.as_deref(), Some("bundle.slabpy"));
        assert_eq!(request.params["hookId"], "plugin.hook");
        assert_eq!(request.params["pluginId"], "plugin");
        assert_eq!(request.params["event"], "on_tool_end");
        assert_eq!(request.params["callType"], "tool");
        assert_eq!(request.params["sessionId"], "session-1");
        assert_eq!(request.params["threadId"], "thread-1");
        assert_eq!(request.params["payload"]["turnIndex"], 3);
        assert_eq!(request.params["payload"]["messages"][0]["role"], "user");
        assert_eq!(request.params["payload"]["tools"], serde_json::json!([]));
        assert_eq!(request.params["payload"]["toolCall"]["id"], "call-1");
        assert_eq!(request.params["payload"]["toolCall"]["name"], "read_file");
        assert_eq!(
            request.params["payload"]["toolCall"]["arguments"],
            serde_json::json!({"path": "README.md"})
        );
        assert_eq!(request.params["payload"]["toolCall"]["output"], "ok");
        assert_eq!(request.params["payload"]["status"], "completed");
    }
}
