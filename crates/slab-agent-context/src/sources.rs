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
}
