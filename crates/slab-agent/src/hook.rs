//! Agent hook system
//!
//! Hooks let host code intercept tool calls before and after execution,
//! as well as session-start and stop events.

use async_trait::async_trait;
use serde_json::Value;

// ── Hook event types ─────────────────────────────────────────────────────────

/// An event dispatched to registered [`AgentHook`] implementations.
#[derive(Debug, Clone)]
pub enum HookEvent {
    /// Dispatched before a tool call is executed.  Handlers may block the
    /// call or modify its arguments.
    PreToolUse { tool_name: String, arguments: Value },
    /// Dispatched after a tool call has completed.
    PostToolUse { tool_name: String, arguments: Value, output: String },
    /// Dispatched when a new agent session (thread) starts.
    SessionStart { thread_id: String },
    /// Dispatched when an agent session terminates (success, error, or shutdown).
    Stop { thread_id: String },
}

// ── Hook outcome ──────────────────────────────────────────────────────────────

/// The decision returned by a hook handler.
#[derive(Debug, Clone)]
pub enum HookOutcome {
    /// Allow the operation to proceed unchanged.
    Continue,
    /// Block the tool call.  The supplied reason is returned to the LLM as the
    /// tool's output so it can decide how to recover.
    Block { reason: String },
    /// Allow the tool call but replace its arguments with the supplied value.
    ModifyArgs { arguments: Value },
}

// ── AgentHook trait ───────────────────────────────────────────────────────────

/// A hook that can intercept agent events.
///
/// Implementations are injected at controller construction time via
/// [`crate::AgentControl::new_with_hooks`].
#[async_trait]
pub trait AgentHook: Send + Sync {
    /// Handle an agent event and return a [`HookOutcome`].
    async fn on_event(&self, event: &HookEvent) -> HookOutcome;
}

// ── Registry helper ───────────────────────────────────────────────────────────

/// Run `event` through all hooks in `hooks`.
///
/// For `PreToolUse`, the first `Block` or `ModifyArgs` outcome wins.
/// For all other events, hooks are run in order and the first non-`Continue`
/// outcome is returned.  Returns `HookOutcome::Continue` if every hook
/// continues.
pub async fn dispatch_hooks(
    hooks: &[std::sync::Arc<dyn AgentHook>],
    event: &HookEvent,
) -> HookOutcome {
    for hook in hooks {
        let outcome = hook.on_event(event).await;
        match outcome {
            HookOutcome::Continue => continue,
            other => return other,
        }
    }
    HookOutcome::Continue
}
