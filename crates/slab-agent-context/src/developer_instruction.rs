//! The developer-role instruction.
//!
//! Renders the skills list (name + description + short `skill://` alias), the
//! skill-roots table that expands those aliases to `file://` paths, and the
//! "how to use skills" prose. Rendered through the model-provided instruction
//! template when present, else the bundled default. `developer` is preserved
//! as a first-class internal role here; provider boundaries flatten it to
//! `system` where the model cannot carry it (see `slab-app-core`).
//!
//! The template source may be a runtime string (a model-provided
//! `instruction_template.jinja`), so it is owned by the fragment and rendered
//! via `Environment::render_str` rather than registered on the static
//! environment.

use minijinja::Environment;

use crate::error::{ContextError, Result};
use crate::fragment::ContextFragment;
use crate::helper::{DEFAULT_DEVELOPER_TEMPLATE, SkillRoot};
use crate::skill_manager::SkillRecord;

#[derive(Debug, Clone)]
pub struct DeveloperInstructionFragment {
    pub skills: Vec<SkillRecord>,
    pub skill_roots: Vec<SkillRoot>,
    /// Resolved jinja source: the model's instruction template, or the bundled
    /// default when the model ships none.
    pub template_source: String,
}

impl DeveloperInstructionFragment {
    /// Build with the bundled default template source.
    pub fn new(skills: Vec<SkillRecord>, skill_roots: Vec<SkillRoot>) -> Self {
        Self { skills, skill_roots, template_source: DEFAULT_DEVELOPER_TEMPLATE.to_owned() }
    }
}

impl ContextFragment for DeveloperInstructionFragment {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn template_name(&self) -> &'static str {
        "instruction"
    }

    fn render_context(&self) -> serde_json::Value {
        serde_json::json!({
            "skills": self.skills,
            "skill_roots": self.skill_roots,
        })
    }

    fn render_body(&self, env: &Environment<'_>) -> Result<String> {
        env.render_str(&self.template_source, self.render_context())
            .map_err(|error| ContextError::Template(error.to_string()))
    }
}
