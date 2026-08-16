//! Agent context management for slab.rs.
//!
//! This crate owns the discovery and rendering of agent context: skills
//! (workspace `.agents/skills` + global app-home `skills`), the workspace and
//! global `AGENTS.md`, and the system/developer/user instruction fragments.
//! Rendering uses minijinja; a model-provided `instruction_template.jinja`
//! (threaded from `slab-model-pack`) overrides the bundled default.
//!
//! Hosts wire the crate by registering [`ContextInstructionHook`] (an
//! `slab_agent::AgentHook`) and supplying an [`AgentContextSources`] impl.

pub mod agent_md_manager;
pub mod agent_prompt;
pub mod developer_instruction;
pub mod environment_instruction;
pub mod error;
pub mod fragment;
pub mod helper;
pub mod hooks;
pub mod permissions_instruction;
pub mod reasoning_effort;
pub mod skill_manager;
pub mod snapshots;
pub mod sources;
pub mod system_instruction;
pub mod user_instruction;

pub use agent_prompt::render_plan_agent_prompt;
pub use environment_instruction::EnvironmentContextFragment;
pub use error::{ContextError, Result};
pub use hooks::ContextInstructionHook;
pub use permissions_instruction::PermissionsInstructionFragment;
pub use reasoning_effort::ReasoningEffortFragment;
pub use snapshots::{
    EnvironmentSnapshot, MemoryContext, OsKind, PermissionBaselineLabel, PermissionModeLabel,
    PermissionSnapshot, ShellKind,
};
pub use sources::AgentContextSources;
