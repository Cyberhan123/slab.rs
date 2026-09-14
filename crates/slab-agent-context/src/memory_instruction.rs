//! The memory fragment: read-side memory instructions wrapping the injected
//! `MEMORY_SUMMARY`, rendered through the bundled `memory` template into a
//! `developer` message.
//!
//! The structured fields arrive through
//! [`crate::sources::AgentContextSources::memory_context`]; the host loads
//! them from `slab-agent-memories` so this crate stays free of it. The
//! `<oai-mem-citation>` contract in the template is load-bearing: the host
//! parses that block out of final replies to track memory usage.

use crate::fragment::ContextFragment;
use crate::helper::MEMORY_TEMPLATE_NAME;

/// Renders the `## Memory` developer instruction: how to use the injected
/// summary, the memory-workspace layout, the `<oai-mem-citation>` contract,
/// and the explicit-request-only update path.
#[derive(Debug, Clone)]
pub struct MemoryInstructionFragment {
    /// Absolute per-project memory root (`<memory_root>/projects/<key>/`).
    pub base_path: String,
    /// The loaded (v1-gated, token-truncated) `memory_summary.md` body.
    pub memory_summary: String,
}

impl ContextFragment for MemoryInstructionFragment {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn template_name(&self) -> &'static str {
        MEMORY_TEMPLATE_NAME
    }

    fn render_context(&self) -> serde_json::Value {
        serde_json::json!({
            "base_path": self.base_path,
            "memory_summary": self.memory_summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helper::build_environment;

    #[test]
    fn renders_memory_instruction_with_base_path_and_summary() {
        let env = build_environment();
        let body = MemoryInstructionFragment {
            base_path: "C:/memories/projects/slab-rs".to_owned(),
            memory_summary: "v1\n# Summary\nprefers minimal diffs".to_owned(),
        }
        .render_body(&env)
        .expect("renders");

        // The memory-workspace routes interpolate the base path.
        assert!(body.contains("C:/memories/projects/slab-rs/MEMORY.md"));
        assert!(body.contains("C:/memories/projects/slab-rs/rollout_summaries/"));
        // The summary is wrapped verbatim.
        assert!(body.contains("========= MEMORY_SUMMARY BEGINS ========="));
        assert!(body.contains("prefers minimal diffs"));
        assert!(body.contains("========= MEMORY_SUMMARY ENDS ========="));
    }

    #[test]
    fn keeps_the_parseable_citation_and_update_contracts() {
        let env = build_environment();
        let body = MemoryInstructionFragment {
            base_path: "/memories/projects/p".to_owned(),
            memory_summary: "v1\ns".to_owned(),
        }
        .render_body(&env)
        .expect("renders");

        // The citation block must stay parseable by the host's
        // `parse_memory_citations` regexes.
        assert!(body.contains("<oai-mem-citation>"));
        assert!(body.contains("<citation_entries>"));
        assert!(
            body.contains("MEMORY.md:234-236|note=[responsesapi citation extraction code pointer]")
        );
        assert!(body.contains("<rollout_ids>"));
        assert!(body.contains("VERY LAST content"));
        assert!(body.contains("Do not cite `memory_summary.md`"));
        // The write path routes through the memory_note tool.
        assert!(body.contains("`memory_note` tool"));
        assert!(body.contains("consolidation applies"));
    }
}
