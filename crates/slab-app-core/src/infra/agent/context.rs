//! `slab-agent-context` host adapter.
//!
//! [`AppContextSources`] is the app-core implementation of the
//! [`slab_agent_context::AgentContextSources`] port used by
//! [`slab_agent_context::ContextInstructionHook`]. It reads the workspace root
//! from the settings config (mirroring the other agent hooks/tools), resolves a
//! model's instruction template through the model service, snapshots the
//! environment + permission state, and bridges the folded read-side memory
//! context — keeping `slab-agent-context` free of `slab-exec-policy` and
//! `slab-agent-memories`.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use slab_agent_context::{
    AgentContextSources, EnvironmentSnapshot, MemoryContext, OsKind, PermissionBaselineLabel,
    PermissionModeLabel, PermissionSnapshot, ShellKind,
};

use crate::context::ModelState;
use crate::domain::services::{resolve_local_chat_prompt_profile, workspace_root_from_config};

/// Map the configured shell launcher to the context crate's shell family. `Auto`
/// follows the same fallback `slab-shell-command` uses (PowerShell on Windows,
/// bash elsewhere) — the model does not need the resolved bash path.
pub(crate) fn shell_kind(launcher: slab_config::ShellLauncherKind) -> ShellKind {
    match launcher {
        slab_config::ShellLauncherKind::Bash => ShellKind::Bash,
        slab_config::ShellLauncherKind::PowerShell => ShellKind::PowerShell,
        slab_config::ShellLauncherKind::Cmd => ShellKind::Cmd,
        slab_config::ShellLauncherKind::Auto => {
            if cfg!(windows) {
                ShellKind::PowerShell
            } else {
                ShellKind::Bash
            }
        }
    }
}

/// Backs the context hook with app-core model state + the shared exec policy.
#[derive(Clone)]
pub(crate) struct AppContextSources {
    model_state: ModelState,
    shell: ShellKind,
    exec_policy: Arc<dyn slab_exec_policy::ExecPolicyPort>,
    memory_enabled: bool,
    memory_root: PathBuf,
}

impl AppContextSources {
    pub(crate) fn new(
        model_state: ModelState,
        shell: ShellKind,
        exec_policy: Arc<dyn slab_exec_policy::ExecPolicyPort>,
        memory_enabled: bool,
        memory_root: PathBuf,
    ) -> Self {
        Self { model_state, shell, exec_policy, memory_enabled, memory_root }
    }

    fn os_kind() -> OsKind {
        if cfg!(windows) {
            OsKind::Windows
        } else if cfg!(target_os = "macos") {
            OsKind::MacOS
        } else if cfg!(target_os = "linux") {
            OsKind::Linux
        } else {
            OsKind::Unknown
        }
    }
}

fn map_mode(mode: slab_exec_policy::PermissionMode) -> PermissionModeLabel {
    match mode {
        slab_exec_policy::PermissionMode::RequestApproval => PermissionModeLabel::RequestApproval,
        slab_exec_policy::PermissionMode::ApproveForMe => PermissionModeLabel::ApproveForMe,
        slab_exec_policy::PermissionMode::FullControl => PermissionModeLabel::FullControl,
        slab_exec_policy::PermissionMode::Custom => PermissionModeLabel::Custom,
    }
}

fn map_baseline(baseline: slab_exec_policy::PermissionBaseline) -> PermissionBaselineLabel {
    match baseline {
        slab_exec_policy::PermissionBaseline::ReadOnly => PermissionBaselineLabel::ReadOnly,
        slab_exec_policy::PermissionBaseline::WorkspaceWrite => {
            PermissionBaselineLabel::WorkspaceWrite
        }
        slab_exec_policy::PermissionBaseline::FullAccess => PermissionBaselineLabel::FullAccess,
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

    fn environment_snapshot(&self) -> EnvironmentSnapshot {
        EnvironmentSnapshot {
            cwd: self.workspace_root().map(|root| root.to_string_lossy().into_owned()),
            shell: self.shell,
            os: Self::os_kind(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    fn permission_snapshot(&self, thread_id: &str) -> PermissionSnapshot {
        let state = self.exec_policy.permission_state_for(thread_id);
        let shell_allowed = state.exposure.contains(slab_exec_policy::OperationCategory::Shell);
        let file_write_allowed =
            state.exposure.contains(slab_exec_policy::OperationCategory::FileEdit);
        let network_allowed = state.exposure.contains(slab_exec_policy::OperationCategory::Network);
        PermissionSnapshot {
            mode: map_mode(state.mode),
            baseline: map_baseline(state.baseline),
            read_only: !shell_allowed && !file_write_allowed && !network_allowed,
            shell_allowed,
            file_write_allowed,
            network_allowed,
        }
    }

    fn memory_context(&self) -> Option<MemoryContext> {
        if !self.memory_enabled {
            return None;
        }
        let config = slab_agent_memories::read::MemoryReadConfig {
            memory_root: self.memory_root.clone(),
            inject_hook_instructions: true,
        };
        match slab_agent_memories::read::render_read_developer_message(&config) {
            Ok(Some(body)) => Some(MemoryContext { body }),
            Ok(None) => None,
            Err(error) => {
                tracing::warn!(%error, "memory context render skipped");
                None
            }
        }
    }
}
