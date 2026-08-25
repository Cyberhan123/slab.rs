use std::path::{Path, PathBuf};
use std::sync::Arc;

use slab_agent::{AgentHook, AgentRuntime, ToolRouter};
use slab_agent_rollout::RolloutFileStore;
use slab_config::AgentMemoriesConfig;

use crate::context::ModelState;
use crate::domain::services::{PluginService, workspace_root_from_config};
use crate::error::AppCoreError;

use super::rollout_store::RolloutBackedAgentStore;

#[derive(Clone)]
pub(crate) struct AgentRuntimeReloader {
    state: ModelState,
    runtime: AgentRuntime,
    tool_router: Arc<ToolRouter>,
    /// Shared rollout true source so a reloaded memory pipeline reads
    /// the SAME rollout files the live agent runtime writes. Retained for
    /// `rollout_path` stamping; the conversation itself is read via
    /// `rollout_store` (the production read path).
    rollout: Arc<RolloutFileStore>,
    /// Shared rollout-backed store (the production read path). A reloaded memory
    /// pipeline reads the conversation through this — the SAME
    /// `read_thread_messages` path the runtime uses — so the memory model and
    /// the runtime never diverge (closes the G5 orphan window for memory).
    rollout_store: Arc<RolloutBackedAgentStore>,
}

impl AgentRuntimeReloader {
    pub(crate) fn new(
        state: ModelState,
        runtime: AgentRuntime,
        rollout: Arc<RolloutFileStore>,
        rollout_store: Arc<RolloutBackedAgentStore>,
    ) -> Self {
        let tool_router = runtime.tool_router();
        Self { state, runtime, tool_router, rollout, rollout_store }
    }

    pub(crate) async fn reload(&self) -> Result<(), AppCoreError> {
        let settings = self.state.pmid().config();
        let memory_config = settings.agent.memories.clone();
        let memory_root = memory_root(&memory_config);
        self.refresh_memory_tools(&memory_config, &memory_root);

        let plugin_service = PluginService::new(self.state.clone());

        let mut hooks = self.internal_memory_hooks(memory_config, memory_root);
        if settings.agent.hooks.enabled {
            let mut scripts =
                crate::infra::agent::hooks::legacy_hook_scripts(&settings.agent.hooks);
            // Hooks need the synced plugin state (scan_and_sync), gated on
            // hooks.enabled so a background reload never scans when hooks are off.
            let plugins = plugin_service.enabled_agent_hook_plugins().await?;
            scripts.extend(crate::infra::agent::hooks::plugin_hook_scripts(&plugins));
            if let Some(script_hook) = crate::infra::agent::hooks::registered_hook_from_scripts(
                scripts,
                self.state.config(),
            ) {
                hooks.push(script_hook);
            }
        }
        self.runtime.replace_hooks(hooks);

        // B-7: register a `plugin__<id>__<cap>` proxy for every Tool-kind
        // capability of enabled plugins. Uses a READ-ONLY manifest scan (no
        // state upsert) so the background reload cannot race a host/test-seeded
        // plugin state. Re-registering picks up installs / enables / disables.
        let capability_sources = plugin_service.enabled_capability_sources_readonly().await?;
        let capability_port: Arc<dyn slab_agent::PluginToolPort> =
            Arc::new(crate::infra::agent::plugin_capability::PluginServiceCapabilityPort::new(
                plugin_service,
            ));
        crate::infra::agent::plugin_capability::register_plugin_capability_tools(
            &self.tool_router,
            capability_port,
            &capability_sources,
        );
        Ok(())
    }

    fn refresh_memory_tools(&self, config: &AgentMemoriesConfig, memory_root: &Path) {
        let workspace_root = workspace_root_from_config(self.state.config());
        let extra_roots = if config.enabled { vec![memory_root.to_path_buf()] } else { Vec::new() };
        self.tool_router.register(Box::new(slab_agent_tools::ReadFileTool::new_with_extra_roots(
            workspace_root.clone(),
            extra_roots.clone(),
        )));
        self.tool_router.register(Box::new(slab_agent_tools::WriteFileTool::new_with_extra_roots(
            workspace_root.clone(),
            extra_roots.clone(),
        )));
        self.tool_router.register(Box::new(slab_agent_tools::ListDirTool::new_with_extra_roots(
            workspace_root.clone(),
            extra_roots.clone(),
        )));
        self.tool_router.register(Box::new(slab_agent_tools::FileGlobTool::new_with_extra_roots(
            workspace_root.clone(),
            extra_roots.clone(),
        )));
        self.tool_router.register(Box::new(slab_agent_tools::GrepTool::new_with_extra_roots(
            workspace_root,
            extra_roots,
        )));
    }

    fn internal_memory_hooks(
        &self,
        memory_config: AgentMemoriesConfig,
        memory_root: PathBuf,
    ) -> Vec<Arc<dyn AgentHook>> {
        let memory_pipeline = crate::infra::agent::memory::AgentMemoryPipeline::new(
            Arc::clone(self.state.store()),
            Arc::clone(&self.rollout),
            Arc::clone(&self.rollout_store),
            Arc::new(self.state.clone()),
            memory_config.clone(),
            memory_root.clone(),
        );
        memory_pipeline.set_control(self.runtime.control());
        let shell = crate::infra::agent::context::shell_kind(
            self.state.pmid().config().agent.tools.shell.launcher,
        );
        let exec_policy = self.runtime.control().exec_policy();
        // The memory read-side instruction is folded into the context hook
        // (AppContextSources::memory_context); only the write-side pipeline stays.
        vec![
            Arc::new(crate::infra::agent::memory::AgentMemoryStartupHook::new(memory_pipeline)),
            Arc::new(slab_agent_context::ContextInstructionHook::new(Arc::new(
                crate::infra::agent::context::AppContextSources::new(
                    self.state.clone(),
                    shell,
                    exec_policy,
                    memory_config,
                    memory_root,
                ),
            ))),
        ]
    }
}

fn memory_root(config: &AgentMemoriesConfig) -> PathBuf {
    config
        .memory_root
        .as_deref()
        .and_then(normalize_non_empty_path)
        .unwrap_or_else(|| slab_utils::app_home::app_home_dir().join("memories"))
}

fn normalize_non_empty_path(value: &str) -> Option<PathBuf> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}
