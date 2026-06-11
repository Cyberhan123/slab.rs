use std::sync::Arc;

use async_trait::async_trait;
use slab_agent::AgentHook;
use slab_agent_memories::hooks::{
    HookScript, HookScriptLanguage, HookScriptOutput, HookScriptRunner, RegisteredScriptHook,
};
use slab_config::{
    AgentHookScriptConfig, AgentHookScriptLanguage, AgentHooksConfig, PluginJsRuntimeTransport,
    PluginPythonRuntimeTransport,
};
use slab_types::{PluginPermissionsManifest, PluginRuntimeCallRequest};
use uuid::Uuid;

use crate::config::Config;
use crate::infra::endpoint::ensure_http_base_url;
use crate::infra::plugin_runtime::{
    PluginEventBus, PluginSidecarRuntimeClient, PluginSidecarRuntimeKind, PluginSidecarTransport,
};

pub(crate) fn registered_script_hook(
    config: &AgentHooksConfig,
    app_config: &Config,
) -> Option<Arc<dyn AgentHook>> {
    let scripts = config
        .scripts
        .iter()
        .filter(|script| script.enabled)
        .filter_map(hook_script)
        .collect::<Vec<_>>();
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
        language: match config.language {
            AgentHookScriptLanguage::JavaScript => HookScriptLanguage::JavaScript,
            AgentHookScriptLanguage::Python => HookScriptLanguage::Python,
        },
        root_dir: config.root_dir.clone(),
        entry: config.entry.clone(),
        export_name: config.export_name.clone(),
        events: config.events.clone(),
    })
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
    PluginRuntimeCallRequest {
        call_id: Uuid::new_v4().to_string(),
        plugin_id: format!("agent-hook-{}", script.name),
        root_dir: script.root_dir.clone(),
        entry: script.entry.clone(),
        bundle: None,
        export_name: script.export_name.clone(),
        params: serde_json::json!({
            "event": event_name,
            "payload": payload,
        }),
        permissions: PluginPermissionsManifest::default(),
        file_grants: Vec::new(),
        blocked_fetch_origins: Vec::new(),
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
                language: HookScriptLanguage::JavaScript,
                root_dir: "C:/hooks".to_owned(),
                entry: "hook.mjs".to_owned(),
                export_name: "run".to_owned(),
                events: Vec::new(),
            },
            "on_llm_start",
            serde_json::json!({"thread_id": "thread-1"}),
        );

        assert_eq!(request.plugin_id, "agent-hook-local");
        assert_eq!(request.root_dir, "C:/hooks");
        assert_eq!(request.entry, "hook.mjs");
        assert_eq!(request.export_name, "run");
        assert_eq!(request.params["event"], "on_llm_start");
        assert_eq!(request.params["payload"]["thread_id"], "thread-1");
        assert!(request.permissions.slab_api.is_empty());
        assert!(request.permissions.files.read.is_empty());
        assert_eq!(request.permissions.network.mode, PluginNetworkMode::Blocked);
        assert!(request.file_grants.is_empty());
    }
}
