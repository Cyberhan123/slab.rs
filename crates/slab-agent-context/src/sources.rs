//! Host-supplied context sources.
//!
//! The agent context hook needs values that change at runtime (the workspace
//! root moves on workspace migrate; the instruction template changes on model
//! swap). Rather than baking them into the hook constructor — which would go
//! stale — the hook holds an [`AgentContextSources`] port and re-reads the
//! current values on every agent start. `slab-app-core` supplies the real
//! implementation; tests supply a mock.

use std::path::PathBuf;

use async_trait::async_trait;

use crate::snapshots::{EnvironmentSnapshot, MemoryContext, PermissionSnapshot};

/// Resolves the dynamic inputs the context hook needs at agent-start time.
///
/// The path accessors are cheap and non-blocking; filesystem scanning itself
/// happens in [`crate::skill_manager`] and [`crate::agent_md_manager`], not
/// here. [`AgentContextSources::instruction_template_for`] is async because
/// resolving a model to its pack template may touch async model state.
#[async_trait]
pub trait AgentContextSources: Send + Sync {
    /// Current workspace root, if any. `None` disables workspace skill/AGENTS.md
    /// discovery (global app-home discovery still runs).
    fn workspace_root(&self) -> Option<PathBuf>;

    /// Absolute directory holding global skills (`<app_home>/skills`).
    fn app_home_skills_dir(&self) -> PathBuf;

    /// Absolute path to the global `AGENTS.md` (`<app_home>/AGENTS.md`).
    fn app_home_agents_md(&self) -> PathBuf;

    /// The model-provided instruction template source (raw jinja text) for the
    /// given model id, if any. `None` selects the bundled default template.
    async fn instruction_template_for(&self, model_id: &str) -> Option<String>;

    /// Whether the backing model can carry a real `developer` role end-to-end.
    ///
    /// Today this is always `false`: genai 0.6.5 exposes a closed
    /// `ChatRole { System, User, Assistant, Tool }` with no `Developer` variant,
    /// so `developer` is preserved internally and flattened to `system` at the
    /// provider boundary regardless of provider family.
    fn supports_developer_role(&self, _model_id: &str) -> bool {
        false
    }

    /// Environment facts (cwd / shell / os / start-time) for the
    /// `<environment_context>` fragment. Cheap and side-effect free; the host
    /// computes the timestamp so this crate stays clock-free.
    fn environment_snapshot(&self) -> EnvironmentSnapshot;

    /// Effective permission state for the thread, for the
    /// `<permissions_instructions>` fragment. The host bridges from its exec
    /// policy so this crate stays free of `slab-exec-policy`.
    fn permission_snapshot(&self, thread_id: &str) -> PermissionSnapshot;

    /// Whether the host registered the `apply_patch` tool. The default mirrors
    /// the registration gate (the workspace-bound tools register only when a
    /// workspace root exists); hosts with additional registration conditions
    /// override this. Combined with the permission snapshot and the tool
    /// whitelist in the hook, this decides whether the system prompt may
    /// tell the model to prefer `apply_patch`.
    fn apply_patch_registered(&self) -> bool {
        self.workspace_root().is_some()
    }

    /// Folded read-side memory context, if memory is enabled and a v1 summary
    /// exists. `None` skips the memory fragment. The host bridges from
    /// `slab-agent-memories` so this crate stays free of it.
    ///
    /// Async + parameterized because the recall path (when enabled) runs a
    /// small model side query against the thread's first user message; the
    /// host bounds that call with a timeout and degrades to `relevant_body:
    /// None` instead of failing agent start.
    async fn memory_context(
        &self,
        thread_id: &str,
        model_id: &str,
        input_message: Option<&str>,
    ) -> Option<MemoryContext>;

    /// Drop per-thread recall state when the thread ends. Default no-op;
    /// hosts with a recall cache override this to release entries.
    fn evict_thread(&self, _thread_id: &str) {}
}
