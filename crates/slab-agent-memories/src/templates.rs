use minijinja::{Environment, context};

use crate::{MemoryError, Result};

pub const PHASE1_SYSTEM_TEMPLATE: &str = include_str!("../templates/memories/system.md");
pub const PHASE1_INPUT_TEMPLATE: &str = include_str!("../templates/memories/input.md");
pub const PHASE2_CONSOLIDATION_TEMPLATE: &str =
    include_str!("../templates/memories/consolidation.md");
pub const RECALL_TEMPLATE: &str = include_str!("../templates/memories/recall.md");
pub const RECALL_SELECT_TEMPLATE: &str = include_str!("../templates/memories/recall-select.md");
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

pub fn render_phase2_consolidation(
    memory_root: &str,
    phase2_workspace_diff_file: &str,
    memory_extensions_folder_structure: &str,
    memory_extensions_primary_inputs: &str,
) -> Result<String> {
    let mut rendered = render(
        PHASE2_CONSOLIDATION_TEMPLATE,
        context! {
            memory_root => memory_root,
            phase2_workspace_diff_file => phase2_workspace_diff_file,
            memory_extensions_folder_structure => memory_extensions_folder_structure,
            memory_extensions_primary_inputs => memory_extensions_primary_inputs,
        },
    )?;
    // The ad-hoc notes extension instructions ride with the consolidation
    // prompt: the phase2 sub-agent is transient and never receives the
    // read-side memory fragment, so this is their only consumer.
    rendered.push_str("\n\n");
    rendered.push_str(HOOK_INSTRUCTIONS_TEMPLATE);
    Ok(rendered)
}

/// Wrap the recall-selected summaries as the `slab_memory_relevant` body.
pub fn render_memory_relevant(base_path: &str, body: &str) -> Result<String> {
    render(
        RECALL_TEMPLATE,
        context! {
            base_path => base_path,
            body => body,
        },
    )
}

/// The side-query system prompt for recall selection.
pub fn render_recall_select(top_k: usize) -> Result<String> {
    render(
        RECALL_SELECT_TEMPLATE,
        context! {
            top_k => top_k,
        },
    )
}

fn render(template: &str, context: minijinja::Value) -> Result<String> {
    let env = Environment::new();
    env.render_str(template, context).map_err(|error| MemoryError::Template(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_phase1_input_template() {
        let rendered =
            render_phase1_input("rollout.jsonl", "C:/repo", "user: hi").expect("rendered");

        assert!(rendered.contains("rollout_path: rollout.jsonl"));
        assert!(rendered.contains("rollout_cwd: C:/repo"));
        assert!(rendered.contains("user: hi"));
    }

    #[test]
    fn renders_consolidation_with_ad_hoc_note_instructions() {
        let rendered =
            render_phase2_consolidation("C:/memories/projects/p", "diff.md", "", "").expect("r");

        assert!(rendered.contains("Memory Writing Agent"));
        // The ad-hoc notes extension instructions must reach the (transient)
        // consolidation agent — it never sees the read-side memory fragment.
        assert!(rendered.contains("# Ad-hoc notes"));
        assert!(rendered.contains("Never delete a note file."));
        assert!(rendered.contains("[ad-hoc note]"));
    }
}
