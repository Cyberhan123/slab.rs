use minijinja::{Environment, context};

use crate::{MemoryError, Result};

pub const PHASE1_SYSTEM_TEMPLATE: &str = include_str!("../templates/memories/system.md");
pub const PHASE1_INPUT_TEMPLATE: &str = include_str!("../templates/memories/input.md");
pub const MEMORY_READ_TEMPLATE: &str = include_str!("../templates/memories/read.md");
pub const PHASE2_CONSOLIDATION_TEMPLATE: &str =
    include_str!("../templates/memories/consolidation.md");
pub const HOOK_INSTRUCTIONS_TEMPLATE: &str = include_str!("../templates/hooks/instructions.md");

pub fn render_phase1_input(
    rollout_path: &str,
    rollout_cwd: &str,
    rollout_contents: &str,
) -> Result<String> {
    render(
        PHASE1_INPUT_TEMPLATE,
        context! {
            rollout_path => rollout_path,
            rollout_cwd => rollout_cwd,
            rollout_contents => rollout_contents,
        },
    )
}

pub fn render_memory_read(base_path: &str, memory_summary: &str) -> Result<String> {
    render(
        MEMORY_READ_TEMPLATE,
        context! {
            base_path => base_path,
            memory_summary => memory_summary,
        },
    )
}

pub fn render_phase2_consolidation(
    memory_root: &str,
    phase2_workspace_diff_file: &str,
    memory_extensions_folder_structure: &str,
    memory_extensions_primary_inputs: &str,
) -> Result<String> {
    render(
        PHASE2_CONSOLIDATION_TEMPLATE,
        context! {
            memory_root => memory_root,
            phase2_workspace_diff_file => phase2_workspace_diff_file,
            memory_extensions_folder_structure => memory_extensions_folder_structure,
            memory_extensions_primary_inputs => memory_extensions_primary_inputs,
        },
    )
}

pub fn render_hook_instructions() -> String {
    HOOK_INSTRUCTIONS_TEMPLATE.to_owned()
}

fn render(template: &str, context: minijinja::Value) -> Result<String> {
    let env = Environment::new();
    env.render_str(template, context).map_err(|error| MemoryError::Template(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_read_template() {
        let rendered = render_memory_read("C:/memories", "v1\nsummary").expect("rendered");

        assert!(rendered.contains("C:/memories/MEMORY.md"));
        assert!(rendered.contains("v1\nsummary"));
    }

    #[test]
    fn renders_phase1_input_template() {
        let rendered =
            render_phase1_input("rollout.jsonl", "C:/repo", "user: hi").expect("rendered");

        assert!(rendered.contains("rollout_path: rollout.jsonl"));
        assert!(rendered.contains("rollout_cwd: C:/repo"));
        assert!(rendered.contains("user: hi"));
    }
}
