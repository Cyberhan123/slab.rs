//! Claude-style command registry: the load-bearing substrate for user-facing
//! `/`-commands.
//!
//! Adding a command is registering one [`CommandSpec`] via
//! [`build_command_registry`]; it then appears in the `/` menu, is parsed by the
//! client, and dispatches by [`CommandKind`]. This mirrors the spirit of the
//! Slice 0 tool registry (`ToolRouter`) and the host-side registration pattern in
//! `plugin_capability.rs`, but as a pure projection rebuilt per use — commands
//! carry no interior mutability because they are a pure function of the scanned
//! skill sources plus the static built-ins.
//!
//! HTTP-free per AGENTS.md (`slab-app-core` stays off HTTP).

use serde::{Deserialize, Serialize};

use slab_agent_context::skill_manager::SkillRecord;

/// How a command dispatches on the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandKind {
    /// Intercepts submission and runs a host action (e.g. `/compact`, `/fork`);
    /// never reaches the model.
    Control,
    /// Expands into prompt text that is sent to the model (e.g. skills, `/plan`).
    Prompt,
}

/// Where a command was declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandSource {
    /// Registered in code by the host.
    Builtin,
    /// Derived from a discovered `SKILL.md`.
    Skill,
}

/// A user-facing `/`-command. The single registration unit: a new command is one
/// [`CommandSpec`] handed to [`CommandRegistryBuilder::register`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    /// Primary trigger name, without the leading `/`.
    pub name: String,
    /// Alternate trigger names (without `/`); resolved by
    /// [`CommandRegistry::resolve`]. Empty for built-ins today.
    pub aliases: Vec<String>,
    pub description: String,
    pub kind: CommandKind,
    pub source: CommandSource,
    /// `Control`-kind action key the client maps to a host callback (e.g.
    /// `"compact"`, `"fork"`). `None` for non-`Control` kinds.
    pub control_action: Option<String>,
}

/// A resolved, queryable set of commands. Built via [`CommandRegistryBuilder`]
/// (see [`build_command_registry`]).
#[derive(Debug, Default)]
pub struct CommandRegistry {
    /// Registered commands in registration order.
    specs: Vec<CommandSpec>,
}

impl CommandRegistry {
    pub fn builder() -> CommandRegistryBuilder {
        CommandRegistryBuilder::default()
    }

    /// All registered commands, in registration order.
    pub fn list(&self) -> Vec<&CommandSpec> {
        self.specs.iter().collect()
    }

    /// Resolve a command by primary name or alias (the token after the `/`).
    pub fn resolve(&self, name: &str) -> Option<&CommandSpec> {
        self.specs.iter().find(|s| s.name == name || s.aliases.iter().any(|a| a == name))
    }
}

/// Collects commands from multiple sources, applying custom-overrides-builtin
/// merge semantics: registering a command whose primary name matches an existing
/// one replaces the earlier entry in place (so the `/` menu never shows two
/// commands under the same trigger). Sources are registered in priority order —
/// later sources win.
#[derive(Debug, Default)]
pub struct CommandRegistryBuilder {
    specs: Vec<CommandSpec>,
}

impl CommandRegistryBuilder {
    /// Register a command. If `spec.name` matches an already-registered command,
    /// the new spec replaces it in place (custom-overrides-builtin).
    pub fn register(&mut self, spec: CommandSpec) -> &mut Self {
        if let Some(existing) = self.specs.iter_mut().find(|s| s.name == spec.name) {
            *existing = spec;
        } else {
            self.specs.push(spec);
        }
        self
    }

    pub fn build(self) -> CommandRegistry {
        CommandRegistry { specs: self.specs }
    }
}

/// Build the command registry: built-in `Control`/`Prompt` commands first, then
/// each scanned skill as a `Prompt` command. A skill whose `name` matches a
/// built-in overrides it.
pub fn build_command_registry(skills: &[SkillRecord]) -> CommandRegistry {
    let mut builder = CommandRegistry::builder();
    register_builtins(&mut builder);
    for skill in skills {
        builder.register(skill_command(skill));
    }
    builder.build()
}

fn register_builtins(builder: &mut CommandRegistryBuilder) {
    builder
        .register(CommandSpec {
            name: "compact".into(),
            aliases: vec![],
            description: "Summarize the conversation history to reclaim context.".into(),
            kind: CommandKind::Control,
            source: CommandSource::Builtin,
            control_action: Some("compact".into()),
        })
        .register(CommandSpec {
            name: "fork".into(),
            aliases: vec![],
            description: "Branch the current thread into a new child thread.".into(),
            kind: CommandKind::Control,
            source: CommandSource::Builtin,
            control_action: Some("fork".into()),
        })
        .register(CommandSpec {
            name: "plan".into(),
            aliases: vec![],
            description: "Seed a planning prompt for the model.".into(),
            kind: CommandKind::Prompt,
            source: CommandSource::Builtin,
            control_action: None,
        });
}

/// Project a discovered skill into a `Prompt` command. Skill expansion still
/// flows through the existing server-side `join_user_text` path; the registry
/// only needs the skill's surface (name + description) for menu/resolve.
fn skill_command(skill: &SkillRecord) -> CommandSpec {
    CommandSpec {
        name: skill.name.clone(),
        aliases: vec![],
        description: skill.description.clone(),
        kind: CommandKind::Prompt,
        source: CommandSource::Skill,
        control_action: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slab_agent_context::skill_manager::{SkillRecord, SkillSource};
    use std::path::PathBuf;

    fn skill(name: &str, description: &str) -> SkillRecord {
        SkillRecord {
            name: name.into(),
            description: description.into(),
            path: PathBuf::new(),
            source: SkillSource::Workspace,
        }
    }

    #[test]
    fn registry_lists_builtins_and_skills() {
        let registry = build_command_registry(&[skill("rust", "Rust tips")]);
        let names: Vec<&str> = registry.list().iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"compact"));
        assert!(names.contains(&"fork"));
        assert!(names.contains(&"plan"));
        assert!(names.contains(&"rust"));
    }

    #[test]
    fn resolve_by_name() {
        let registry = build_command_registry(&[]);
        let cmd = registry.resolve("compact").expect("compact builtin");
        assert_eq!(cmd.kind, CommandKind::Control);
        assert_eq!(cmd.source, CommandSource::Builtin);
        assert_eq!(cmd.control_action.as_deref(), Some("compact"));
    }

    #[test]
    fn resolve_by_alias() {
        let mut builder = CommandRegistry::builder();
        builder.register(CommandSpec {
            name: "compact".into(),
            aliases: vec!["c".into()],
            description: String::new(),
            kind: CommandKind::Control,
            source: CommandSource::Builtin,
            control_action: Some("compact".into()),
        });
        let registry = builder.build();
        assert!(registry.resolve("c").is_some(), "alias resolves");
        assert!(registry.resolve("compact").is_some(), "primary resolves");
    }

    #[test]
    fn skill_overrides_builtin() {
        // A skill literally named `compact` overrides the built-in (same slot,
        // becomes a Prompt command sourced from the skill).
        let registry = build_command_registry(&[skill("compact", "my compact")]);
        let cmd = registry.resolve("compact").expect("compact resolves");
        assert_eq!(cmd.source, CommandSource::Skill);
        assert_eq!(cmd.kind, CommandKind::Prompt);
        assert_eq!(cmd.description, "my compact");
        // Exactly one entry for the name — no duplicate in the menu.
        let count = registry.list().iter().filter(|s| s.name == "compact").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn plan_is_prompt_kind() {
        let registry = build_command_registry(&[]);
        let cmd = registry.resolve("plan").expect("plan builtin");
        assert_eq!(cmd.kind, CommandKind::Prompt);
        assert!(cmd.control_action.is_none());
    }

    #[test]
    fn resolve_unknown_returns_none() {
        let registry = build_command_registry(&[]);
        assert!(registry.resolve("nope").is_none());
    }
}
