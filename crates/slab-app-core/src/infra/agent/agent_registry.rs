//! Built-in agent registry — host adapter for [`slab_agent::AgentRegistry`].
//!
//! Holds claude-style fixed agent definitions registered at startup: the
//! `plan` agent (read-only architect — plan mode runs a turn as this agent
//! via `TurnStartParams.agent_type = "plan"`) and the `memory` agent (the
//! phase2 consolidation writer, launched by the memory pipeline as a
//! self-owned system subagent). Adding a new built-in agent = pushing another
//! [`AgentDefinition`]` here. The constraint each definition carries is
//! tool-shaping only — the exec_policy/approval port remains the security
//! boundary.

use slab_agent::{AgentDefinition, AgentRegistry, ModelPolicy, ToolConstraint};

/// The built-in `memory` agent type: the phase2 consolidation writer.
pub(crate) const MEMORY_AGENT_TYPE: &str = "memory";

/// Registry populated with the built-in agent definitions.
pub struct BuiltinAgentRegistry {
    agents: Vec<AgentDefinition>,
}

impl BuiltinAgentRegistry {
    /// Registry pre-loaded with all built-in agents.
    pub fn with_builtins() -> Self {
        Self { agents: vec![plan_agent_definition(), memory_agent_definition()] }
    }
}

impl AgentRegistry for BuiltinAgentRegistry {
    fn get(&self, agent_type: &str) -> Option<AgentDefinition> {
        self.agents.iter().find(|def| def.agent_type == agent_type).cloned()
    }

    fn list(&self) -> Vec<AgentDefinition> {
        self.agents.clone()
    }
}

/// The built-in `plan` agent: a read-only architect that researches, inspects,
/// and proposes a plan but cannot execute mutations. Plan mode runs a turn as
/// this agent (`TurnStartParams.agent_type = "plan"`); the denylist below is the
/// read-only enforcement. It is precise (not a `git_*` glob) so read-only
/// `git_status`/`git_diff` remain available to the architect, and it denies
/// `delegate_subagent` so the agent cannot recurse.
fn plan_agent_definition() -> AgentDefinition {
    AgentDefinition {
        agent_type: "plan".to_owned(),
        description: "Read-only architect: researches, inspects, and proposes a plan; cannot execute mutations."
            .to_owned(),
        tools: ToolConstraint::Denylist(vec![
            "shell".to_owned(),
            "write_file".to_owned(),
            "apply_patch".to_owned(),
            "git_commit".to_owned(),
            "delegate_subagent".to_owned(),
        ]),
        system_prompt: slab_agent_context::render_plan_agent_prompt()
            .expect("plan_agent system prompt template renders"),
        model: ModelPolicy::Inherit,
    }
}

/// The built-in `memory` agent: the phase2 consolidation writer. Launched by
/// the memory pipeline (`AgentMemoryPipeline::run_consolidation_agent`) as a
/// self-owned SYSTEM subagent — a root thread registered in the background
/// task registry (queryable/stoppable via `subagent_status` /
/// `subagent_stop`) with no parent to notify. The per-run consolidation
/// prompt (rendered by
/// `slab_agent_memories::templates::render_phase2_consolidation`) always
/// overrides the static stub below; the stub only keeps a stray
/// `agent_type="memory"` selection (turn start or a model delegation)
/// harmless. The allowlist is exactly the phase2 tool surface — read/search
/// plus the workspace writes consolidation needs, no shell, no delegation,
/// no memory_note (consolidation consumes notes, it does not create them).
fn memory_agent_definition() -> AgentDefinition {
    AgentDefinition {
        agent_type: MEMORY_AGENT_TYPE.to_owned(),
        description: "Memory consolidation agent: rewrites the per-project memory workspace (MEMORY.md, memory_summary.md, skills, rollout summaries) from the phase2 diff."
            .to_owned(),
        tools: ToolConstraint::Allowlist(vec![
            "read_file".to_owned(),
            "write_file".to_owned(),
            "list_dir".to_owned(),
            "grep".to_owned(),
        ]),
        system_prompt: slab_agent_memories::templates::render_memory_agent_prompt()
            .expect("memory agent stub prompt renders"),
        model: ModelPolicy::Inherit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slab_agent::{filter_tools_for_agent, port::ToolSpec};

    fn spec(name: &str) -> ToolSpec {
        ToolSpec {
            name: name.to_owned(),
            description: String::new(),
            parameters_schema: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    #[test]
    fn builtin_plan_agent_definition_shape() {
        let registry = BuiltinAgentRegistry::with_builtins();
        let def = registry.get("plan").expect("plan agent registered");
        assert_eq!(def.agent_type, "plan");
        assert!(!def.description.is_empty());
        let ToolConstraint::Denylist(denied) = &def.tools else {
            panic!("plan agent must use a denylist");
        };
        for expected in ["shell", "write_file", "apply_patch", "git_commit", "delegate_subagent"] {
            assert!(
                denied.contains(&expected.to_owned()),
                "denylist must contain {expected}: {denied:?}"
            );
        }
        // Read-only git tools are intentionally NOT denied.
        assert!(!denied.contains(&"git_status".to_owned()));
        assert!(!denied.contains(&"git_diff".to_owned()));
        assert!(!def.system_prompt.is_empty());
        // Confirms the prompt is sourced from the jinja template (Slice 4 Phase F),
        // not a stale const.
        assert!(
            def.system_prompt.contains("planning agent"),
            "system prompt should come from the jinja template: {}",
            def.system_prompt
        );
        assert_eq!(def.model, ModelPolicy::Inherit);
    }

    #[test]
    fn filter_removes_shell_for_plan_agent() {
        let registry = BuiltinAgentRegistry::with_builtins();
        let constraint = registry.get("plan").map(|def| def.tools);
        let specs = vec![
            spec("shell"),
            spec("write_file"),
            spec("read_file"),
            spec("grep"),
            spec("plan"),
            spec("git_status"),
        ];
        let out = filter_tools_for_agent(&specs, constraint.as_ref());
        let names: Vec<&str> = out.iter().map(|s| s.name.as_str()).collect();
        assert!(!names.contains(&"shell"));
        assert!(!names.contains(&"write_file"));
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"grep"));
        assert!(names.contains(&"plan"));
        // Read-only git tool survives the precise denylist.
        assert!(names.contains(&"git_status"));
    }

    #[test]
    fn unknown_agent_type_resolves_none() {
        let registry = BuiltinAgentRegistry::with_builtins();
        assert!(registry.get("does-not-exist").is_none());
    }

    #[test]
    fn list_returns_all_builtins() {
        let registry = BuiltinAgentRegistry::with_builtins();
        let list = registry.list();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].agent_type, "plan");
        assert_eq!(list[1].agent_type, "memory");
    }

    #[test]
    fn builtin_memory_agent_definition_shape() {
        let registry = BuiltinAgentRegistry::with_builtins();
        let def = registry.get(MEMORY_AGENT_TYPE).expect("memory agent registered");
        assert_eq!(def.agent_type, "memory");
        assert!(!def.description.is_empty());
        let ToolConstraint::Allowlist(allowed) = &def.tools else {
            panic!("memory agent must use an allowlist");
        };
        for expected in ["read_file", "write_file", "list_dir", "grep"] {
            assert!(
                allowed.contains(&expected.to_owned()),
                "allowlist must contain {expected}: {allowed:?}"
            );
        }
        // The static stub prompt is only a stray-selection guard — real runs
        // override it with the per-run consolidation prompt.
        assert!(!def.system_prompt.is_empty());
        assert!(
            def.system_prompt.contains("memory consolidation agent"),
            "stub prompt renders from the template: {}",
            def.system_prompt
        );
        assert_eq!(def.model, ModelPolicy::Inherit);
    }

    /// The allowlist is the phase2 tool surface: the four file tools survive,
    /// shell / delegation / memory_note do not.
    #[test]
    fn filter_keeps_only_file_tools_for_memory_agent() {
        let registry = BuiltinAgentRegistry::with_builtins();
        let constraint = registry.get(MEMORY_AGENT_TYPE).map(|def| def.tools);
        let specs = vec![
            spec("shell"),
            spec("read_file"),
            spec("write_file"),
            spec("list_dir"),
            spec("grep"),
            spec("delegate_subagent"),
            spec("memory_note"),
            spec("web_search"),
        ];
        let out = filter_tools_for_agent(&specs, constraint.as_ref());
        let names: Vec<&str> = out.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["read_file", "write_file", "list_dir", "grep"]);
    }
}
