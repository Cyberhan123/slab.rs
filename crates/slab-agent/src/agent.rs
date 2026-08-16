//! Built-in agent registry and per-agent tool constraints (Slice 4, Pillar 4).
//!
//! Claude-style fixed agent definitions: each [`AgentDefinition`] carries a
//! [`ToolConstraint`] that narrows the model-facing tool list, plus a dedicated
//! system prompt. The constraint is **layered above** the existing
//! visibility/exposure projection — it is tool-shaping only. The
//! `exec_policy`/approval port remains the security boundary; the constraint
//! does not grant or deny anything the policy would otherwise forbid.
//!
//! The registry is process-level (static built-ins registered by the host at
//! startup) and owned by [`crate::control::AgentControl`], threaded into the
//! turn context so [`filter_tools_for_agent`] can run every turn. Per-agent
//! selection is driven by [`crate::config::AgentConfig::agent_type`], set by
//! `delegate_subagent` after a successful registry lookup.

use crate::port::ToolSpec;
use crate::tool::ToolName;

/// Per-agent tool constraint applied after the visibility/exposure projection.
///
/// Builtin tools are matched against their canonical wire name; MCP tools always
/// pass (operators cannot enumerate them ahead of time and the approval gate
/// still applies). Entries support a trailing `*` for prefix matching (e.g.
/// `git_*`); a bare `"*"` is rejected in favour of [`ToolConstraint::Wildcard`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ToolConstraint {
    /// No constraint — all tools pass. The default.
    #[default]
    Wildcard,
    /// Reject any builtin whose wire name matches an entry. An empty list is
    /// equivalent to [`ToolConstraint::Wildcard`] (deny nothing).
    Denylist(Vec<String>),
    /// Accept only builtins whose wire name matches an entry. An empty list
    /// denies every builtin (MCP tools still pass). A tool must *also* survive
    /// `AgentConfig::allowed_tools` — the two filters compose.
    Allowlist(Vec<String>),
}

/// Model selection policy for an agent definition.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ModelPolicy {
    /// Use whatever the caller/config specifies. The default.
    #[default]
    Inherit,
    /// Use this model unless the caller explicitly overrides it.
    Fixed(String),
}

/// A fixed, claude-style built-in agent definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentDefinition {
    /// Canonical agent type (e.g. `"plan"`). Looked up in the registry.
    pub agent_type: String,
    /// Human-readable summary of the agent's role.
    pub description: String,
    /// Tool constraint layered above visibility/exposure.
    pub tools: ToolConstraint,
    /// System prompt injected as the first message (path a — does not skip
    /// transient agents, so delegated subagents receive it).
    pub system_prompt: String,
    /// Model selection policy.
    pub model: ModelPolicy,
}

/// Registry of built-in agent definitions. Sync — in-memory lookups only.
///
/// The default [`NoopAgentRegistry`] resolves no types, so existing tests
/// (which never set `agent_type`) are unaffected.
pub trait AgentRegistry: Send + Sync {
    /// Look up a definition by agent type. Returns a fresh owned clone.
    fn get(&self, agent_type: &str) -> Option<AgentDefinition>;
    /// All registered definitions, in stable insertion order.
    fn list(&self) -> Vec<AgentDefinition>;
}

/// Registry that resolves no agent types — the default for `AgentControl`.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopAgentRegistry;

impl AgentRegistry for NoopAgentRegistry {
    fn get(&self, _agent_type: &str) -> Option<AgentDefinition> {
        None
    }
    fn list(&self) -> Vec<AgentDefinition> {
        Vec::new()
    }
}

/// Pure projection: narrow `specs` by an optional agent constraint.
///
/// Intended layering inside `allowed_tool_specs`:
/// ① visibility/exposure → **② this filter** → ③ `allowed_tools` retain →
/// ④ offline narrow → ⑤ `tool_choice`. A tool must survive both this filter and
/// `allowed_tools`. MCP tools (namespace `mcp`) always pass.
pub fn filter_tools_for_agent(
    specs: &[ToolSpec],
    constraint: Option<&ToolConstraint>,
) -> Vec<ToolSpec> {
    let Some(constraint) = constraint else {
        return specs.to_vec();
    };
    match constraint {
        ToolConstraint::Wildcard => specs.to_vec(),
        ToolConstraint::Denylist(denied) => specs
            .iter()
            .filter(|spec| is_mcp(&spec.name) || !matches_any(&spec.name, denied))
            .cloned()
            .collect(),
        ToolConstraint::Allowlist(allowed) => specs
            .iter()
            .filter(|spec| is_mcp(&spec.name) || matches_any(&spec.name, allowed))
            .cloned()
            .collect(),
    }
}

/// MCP tools are namespaced `mcp__*` and always survive agent constraints.
fn is_mcp(wire_name: &str) -> bool {
    ToolName::parse_wire(wire_name).namespace.as_str() == "mcp"
}

/// Match a wire name against a list of patterns. A trailing `*` is a non-empty
/// prefix match; anything else is exact equality. A bare `"*"` does not match
/// (use [`ToolConstraint::Wildcard`] instead).
fn matches_any(wire_name: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|pattern| match pattern.strip_suffix('*') {
        Some(prefix) => !prefix.is_empty() && wire_name.starts_with(prefix),
        None => wire_name == pattern,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::port::ToolSpec;

    fn spec(name: &str) -> ToolSpec {
        ToolSpec {
            name: name.to_owned(),
            description: String::new(),
            parameters_schema: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    #[test]
    fn filter_none_constraint_passes_all() {
        let specs = vec![spec("shell"), spec("read_file")];
        let out = filter_tools_for_agent(&specs, None);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn filter_wildcard_passes_all() {
        let specs = vec![spec("shell"), spec("read_file")];
        let out = filter_tools_for_agent(&specs, Some(&ToolConstraint::Wildcard));
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn filter_denylist_removes_exact() {
        let specs = vec![spec("shell"), spec("write_file"), spec("read_file")];
        let out = filter_tools_for_agent(
            &specs,
            Some(&ToolConstraint::Denylist(vec!["shell".to_owned(), "write_file".to_owned()])),
        );
        let names: Vec<&str> = out.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["read_file"]);
    }

    #[test]
    fn filter_denylist_glob_prefix() {
        let specs = vec![spec("git_commit"), spec("git_status"), spec("read_file")];
        let out = filter_tools_for_agent(
            &specs,
            Some(&ToolConstraint::Denylist(vec!["git_*".to_owned()])),
        );
        let names: Vec<&str> = out.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["read_file"]);
    }

    #[test]
    fn filter_denylist_glob_rejects_bare_star() {
        // A bare "*" is NOT a wildcard-as-glob — use ToolConstraint::Wildcard.
        let specs = vec![spec("shell"), spec("read_file")];
        let out =
            filter_tools_for_agent(&specs, Some(&ToolConstraint::Denylist(vec!["*".to_owned()])));
        assert_eq!(out.len(), 2, "bare star must not deny anything");
    }

    #[test]
    fn filter_denylist_empty_equivalent_to_wildcard() {
        let specs = vec![spec("shell"), spec("read_file")];
        let out = filter_tools_for_agent(&specs, Some(&ToolConstraint::Denylist(vec![])));
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn filter_allowlist_keeps_only_listed() {
        let specs = vec![spec("shell"), spec("read_file"), spec("grep")];
        let out = filter_tools_for_agent(
            &specs,
            Some(&ToolConstraint::Allowlist(vec!["read_file".to_owned(), "grep".to_owned()])),
        );
        let names: Vec<&str> = out.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["read_file", "grep"]);
    }

    #[test]
    fn filter_allowlist_empty_removes_all_builtins() {
        let specs = vec![spec("shell"), spec("read_file")];
        let out = filter_tools_for_agent(&specs, Some(&ToolConstraint::Allowlist(vec![])));
        assert!(out.is_empty());
    }

    #[test]
    fn filter_mcp_always_passes_under_denylist() {
        let specs = vec![spec("mcp__srv__write"), spec("shell"), spec("read_file")];
        let out = filter_tools_for_agent(
            &specs,
            Some(&ToolConstraint::Denylist(vec!["shell".to_owned(), "mcp__srv__write".to_owned()])),
        );
        let names: Vec<&str> = out.iter().map(|s| s.name.as_str()).collect();
        // MCP survives even though explicitly named; shell denied; read_file kept.
        assert_eq!(names, vec!["mcp__srv__write", "read_file"]);
    }

    #[test]
    fn filter_mcp_always_passes_under_allowlist() {
        let specs = vec![spec("mcp__srv__x"), spec("read_file"), spec("shell")];
        let out = filter_tools_for_agent(
            &specs,
            Some(&ToolConstraint::Allowlist(vec!["read_file".to_owned()])),
        );
        let names: Vec<&str> = out.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["mcp__srv__x", "read_file"]);
    }

    #[test]
    fn matches_exact_not_substring() {
        // Bare "git" must not match "git_commit" — only "git_*" does.
        let specs = vec![spec("git"), spec("git_commit")];
        let out =
            filter_tools_for_agent(&specs, Some(&ToolConstraint::Denylist(vec!["git".to_owned()])));
        let names: Vec<&str> = out.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["git_commit"]);
    }

    #[test]
    fn noop_registry_resolves_nothing() {
        let reg = NoopAgentRegistry;
        assert!(reg.get("plan").is_none());
        assert!(reg.list().is_empty());
    }
}
