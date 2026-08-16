//! Shared helpers: the minijinja environment factory, size truncation, path
//! aliasing, and the skill-roots table builder.

use std::path::Path;

use minijinja::{Environment, UndefinedBehavior};
use serde::Serialize;

use crate::agent_md_manager::AgentMdRecord;
use crate::skill_manager::SkillRecord;

pub const SYSTEM_TEMPLATE: &str = include_str!("../templates/system.jinja");
/// The bundled default instruction template, used when a model does not ship
/// its own `instruction_template.jinja` (cloud models, or local packs without
/// one). Model-provided templates override it under the `instruction` name.
pub const DEFAULT_DEVELOPER_TEMPLATE: &str = include_str!("../templates/developer.jinja");
pub const SKILL_TEMPLATE: &str = include_str!("../templates/skill.jinja");
pub const AGENTS_MD_TEMPLATE: &str = include_str!("../templates/agents_md.jinja");
pub const ENVIRONMENT_TEMPLATE: &str = include_str!("../templates/environment.jinja");
pub const PERMISSIONS_TEMPLATE: &str = include_str!("../templates/permissions.jinja");
pub const REASONING_EFFORT_TEMPLATE: &str = include_str!("../templates/reasoning_effort.jinja");
/// The read-only `plan` built-in agent's system prompt (Slice 4 Phase F).
pub const PLAN_AGENT_TEMPLATE: &str = include_str!("../templates/plan_agent.jinja");

/// Template names registered in every environment built by [`build_environment`].
pub const SYSTEM_TEMPLATE_NAME: &str = "system";
pub const INSTRUCTION_TEMPLATE_NAME: &str = "instruction";
pub const SKILL_TEMPLATE_NAME: &str = "skill";
pub const AGENTS_MD_TEMPLATE_NAME: &str = "agents_md";
pub const ENVIRONMENT_TEMPLATE_NAME: &str = "environment";
pub const PERMISSIONS_TEMPLATE_NAME: &str = "permissions";
pub const REASONING_EFFORT_TEMPLATE_NAME: &str = "reasoning_effort";
pub const PLAN_AGENT_TEMPLATE_NAME: &str = "plan_agent";

/// A short-path alias mapping, emitted in the developer instruction so prompts
/// can reference `skill://<name>` / `instruction://agent.md` instead of long
/// absolute paths.
#[derive(Debug, Clone, Serialize)]
pub struct SkillRoot {
    pub alias: String,
    pub target: String,
}

/// Build the rendering environment for the static bundled templates.
///
/// The developer instruction template is intentionally NOT registered here:
/// it may be a model-provided runtime string (not `'static`), so the developer
/// fragment owns its resolved source and renders via `Environment::render_str`
/// instead. The caller resolves that source (model template or
/// [`DEFAULT_DEVELOPER_TEMPLATE`]).
pub fn build_environment() -> Environment<'static> {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    // Bundled templates are compile-time valid; registration cannot fail.
    let _ = env.add_template(SYSTEM_TEMPLATE_NAME, SYSTEM_TEMPLATE);
    let _ = env.add_template(SKILL_TEMPLATE_NAME, SKILL_TEMPLATE);
    let _ = env.add_template(AGENTS_MD_TEMPLATE_NAME, AGENTS_MD_TEMPLATE);
    let _ = env.add_template(ENVIRONMENT_TEMPLATE_NAME, ENVIRONMENT_TEMPLATE);
    let _ = env.add_template(PERMISSIONS_TEMPLATE_NAME, PERMISSIONS_TEMPLATE);
    let _ = env.add_template(REASONING_EFFORT_TEMPLATE_NAME, REASONING_EFFORT_TEMPLATE);
    let _ = env.add_template(PLAN_AGENT_TEMPLATE_NAME, PLAN_AGENT_TEMPLATE);
    env
}

/// Truncate `value` to at most `max` chars, appending `…` when truncated. The
/// returned string never exceeds `max` chars (counting `…` as one char).
pub fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_owned();
    }
    if max == 0 {
        return String::new();
    }
    let mut truncated: String = value.chars().take(max - 1).collect();
    truncated.push('…');
    truncated
}

/// Render a skill expansion block (the `<skill>` body) as plain text, for
/// injection into a user message by the host when a skill is invoked. Mirrors
/// the bundled `skill` template.
pub fn render_skill_block(name: &str, path: &str, contents: &str) -> String {
    format!("<skill>\n<name>{name}</name>\n<path>{path}</path>\n{contents}\n</skill>")
}

/// Render an absolute path as a `file://` URL with forward slashes.
///
/// Paths are expected to already be absolute (they come from scanning an
/// absolute workspace/app-home root); no canonicalization is performed so the
/// locator is stable across platforms and the file need not be re-resolved.
pub fn file_url(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let with_leading_slash =
        if normalized.starts_with('/') { normalized } else { format!("/{normalized}") };
    format!("file://{with_leading_slash}")
}

/// Build the alias table: one `skill://<name>` entry per skill plus an
/// `instruction://agent.md` entry per discovered `AGENTS.md`.
pub fn build_skill_roots(skills: &[SkillRecord], agents: &[AgentMdRecord]) -> Vec<SkillRoot> {
    let mut roots: Vec<SkillRoot> = skills
        .iter()
        .map(|skill| SkillRoot { alias: skill.alias(), target: file_url(&skill.path) })
        .collect();
    for agent_md in agents {
        roots.push(SkillRoot {
            alias: "instruction://agent.md".to_owned(),
            target: file_url(&agent_md.path),
        });
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_short_value_unchanged() {
        assert_eq!(truncate("abc", 10), "abc");
    }

    #[test]
    fn truncate_long_value_gets_ellipsis() {
        // Stays within the limit: 2 chars + ellipsis = 3 total.
        let out = truncate("abcdef", 3);
        assert_eq!(out, "ab…");
        assert_eq!(out.chars().count(), 3);
    }

    #[test]
    fn file_url_normalizes_windows_path() {
        let url = file_url(Path::new("C:/Users/someone/skills/rust/SKILL.md"));
        assert_eq!(url, "file:///C:/Users/someone/skills/rust/SKILL.md");
    }

    #[test]
    fn file_url_keeps_posix_path() {
        let url = file_url(Path::new("/home/u/skills/rust/SKILL.md"));
        assert_eq!(url, "file:///home/u/skills/rust/SKILL.md");
    }

    #[test]
    fn bundled_default_template_compiles() {
        let env = build_environment();
        let rendered = env
            .render_str(
                DEFAULT_DEVELOPER_TEMPLATE,
                serde_json::json!({
                    "skills": [{
                        "name": "rust-code-style",
                        "description": "Use when working on Rust code.",
                        "path": "/x/SKILL.md",
                        "source": "workspace"
                    }],
                    "skill_roots": [{
                        "alias": "skill://rust-code-style",
                        "target": "file:///x/SKILL.md"
                    }],
                }),
            )
            .expect("renders");
        assert!(rendered.contains("skill://rust-code-style"));
        assert!(rendered.contains("name: rust-code-style"));
        assert!(rendered.contains("file:///x/SKILL.md"));
    }
}
