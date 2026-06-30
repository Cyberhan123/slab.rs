use std::path::{Path, PathBuf};
use std::sync::Arc;

use slab_agent::{AgentControl, AgentHook, ToolRouter};
use slab_config::AgentMemoriesConfig;

use crate::context::ModelState;
use crate::domain::services::{PluginService, workspace_root_from_config};
use crate::error::AppCoreError;

#[derive(Clone)]
pub(crate) struct AgentRuntimeReloader {
    state: ModelState,
    control: Arc<AgentControl>,
    tool_router: Arc<ToolRouter>,
}

impl AgentRuntimeReloader {
    pub(crate) fn new(state: ModelState, control: Arc<AgentControl>) -> Self {
        let tool_router = control.tool_router();
        Self { state, control, tool_router }
    }

    pub(crate) async fn reload(&self) -> Result<(), AppCoreError> {
        let settings = self.state.pmid().config();
        let memory_config = settings.agent.memories.clone();
        let memory_root = memory_root(&memory_config);
        self.refresh_memory_tools(&memory_config, &memory_root);

        let mut hooks = self.internal_memory_hooks(memory_config, memory_root);
        if settings.agent.hooks.enabled {
            let mut scripts =
                crate::infra::agent::hooks::legacy_hook_scripts(&settings.agent.hooks);
            let plugins =
                PluginService::new(self.state.clone()).enabled_agent_hook_plugins().await?;
            scripts.extend(crate::infra::agent::hooks::plugin_hook_scripts(&plugins));
            if let Some(script_hook) = crate::infra::agent::hooks::registered_hook_from_scripts(
                scripts,
                self.state.config(),
            ) {
                hooks.push(script_hook);
            }
        }
        self.control.replace_hooks(hooks);
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
        super::a2u_tools::register_builtin_a2u_tools(&self.tool_router);
    }

    fn internal_memory_hooks(
        &self,
        memory_config: AgentMemoriesConfig,
        memory_root: PathBuf,
    ) -> Vec<Arc<dyn AgentHook>> {
        let memory_pipeline = crate::infra::agent::memory::AgentMemoryPipeline::new(
            Arc::clone(self.state.store()),
            Arc::new(self.state.clone()),
            memory_config.clone(),
            memory_root.clone(),
        );
        memory_pipeline.set_control(Arc::clone(&self.control));
        vec![
            Arc::new(slab_agent_memories::hooks::MemoryInstructionHook::new(
                memory_config.enabled,
                memory_root,
            )),
            Arc::new(crate::infra::agent::memory::AgentMemoryStartupHook::new(memory_pipeline)),
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
