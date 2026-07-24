//! `slab-agent-context` host adapter.
//!
//! [`AppContextSources`] is the app-core implementation of the
//! [`slab_agent_context::AgentContextSources`] port used by
//! [`slab_agent_context::ContextInstructionHook`]. It reads the workspace root
//! from the settings config (mirroring the other agent hooks/tools) and
//! resolves a model's instruction template through the model service.

use std::path::PathBuf;

use async_trait::async_trait;
use slab_agent_context::AgentContextSources;

use crate::context::ModelState;
use crate::domain::services::{resolve_local_chat_prompt_profile, workspace_root_from_config};

/// Backs the context hook with app-core model state.
#[derive(Clone)]
pub(crate) struct AppContextSources {
    model_state: ModelState,
}

impl AppContextSources {
    pub(crate) fn new(model_state: ModelState) -> Self {
        Self { model_state }
    }
}

#[async_trait]
impl AgentContextSources for AppContextSources {
    fn workspace_root(&self) -> Option<PathBuf> {
        workspace_root_from_config(self.model_state.config())
    }

    fn app_home_skills_dir(&self) -> PathBuf {
        slab_utils::app_home::skills_dir()
    }

    fn app_home_agents_md(&self) -> PathBuf {
        slab_utils::app_home::agents_md_path()
    }

    async fn instruction_template_for(&self, model_id: &str) -> Option<String> {
        resolve_local_chat_prompt_profile(&self.model_state, model_id)
            .await
            .ok()
            .and_then(|profile| profile.instruction_template_source)
    }
}
