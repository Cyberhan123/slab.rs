//! `memory_note` — the explicit-request memory-update tool.
//!
//! The read-side instruction (rendered by `slab-agent-context`) tells the
//! model to update memories ONLY when the user explicitly asks; this tool is
//! the single sanctioned write path for that contract. It owns the ad-hoc
//! note invariants the prompt used to ask the model to uphold by hand:
//! deterministic `<timestamp>-<slug>.md` naming, containment inside the
//! project's `extensions/ad_hoc/notes/` directory (the dir is baked in at
//! registration — there is no path argument), and the same secret redaction
//! phase 1 applies to extracted memories. Phase 2 consolidation folds the
//! notes into the memory files; the notes themselves are never edited here.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::Deserialize;
use slab_agent::{
    AgentError, OperationCategory, OperationDescriptor, ToolContext, ToolOutput, TypedTool,
};

use crate::phase1::sanitize_slug;
use crate::redaction::redact_secrets;

/// The `memory_note` tool name; the host registers/unregisters it with the
/// memory enable state (bootstrap + workspace refresh).
pub const MEMORY_NOTE_TOOL_NAME: &str = "memory_note";

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MemoryNoteArgs {
    /// The addition, deletion, or correction to apply to memories, exactly as
    /// the user requested. One note per request.
    pub content: String,
    /// Short slug for the note filename (sanitized; optional).
    #[serde(default)]
    pub slug: Option<String>,
}

pub struct MemoryNoteTool {
    /// ABSOLUTE per-project notes dir (`<project_root>/extensions/ad_hoc/notes`),
    /// resolved by the host at registration so the tool can write nowhere else.
    notes_dir: PathBuf,
}

impl MemoryNoteTool {
    pub fn new(notes_dir: PathBuf) -> Self {
        Self { notes_dir }
    }
}

#[async_trait::async_trait]
impl TypedTool for MemoryNoteTool {
    type Input = MemoryNoteArgs;

    fn name(&self) -> &str {
        MEMORY_NOTE_TOOL_NAME
    }

    fn description(&self) -> &str {
        "Append one memory update note (a remember, forget, or correction). \
         Use ONLY when the user explicitly asks to update memories."
    }

    fn category(&self) -> OperationCategory {
        OperationCategory::FileEdit
    }

    fn describe_operation(&self, _arguments: &serde_json::Value) -> Option<OperationDescriptor> {
        // The concrete filename is generated at execute time; describing the
        // notes dir with its shape keeps the policy layer seeing a file edit
        // on the memory workspace.
        Some(OperationDescriptor::file_edit(
            self.notes_dir.join("<timestamp>-<slug>.md").to_string_lossy().into_owned(),
        ))
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        args: MemoryNoteArgs,
    ) -> Result<ToolOutput, AgentError> {
        let content = redact_secrets(args.content.trim());
        if content.is_empty() {
            return Err(AgentError::ToolExecution(
                "memory note content is empty after trimming".to_owned(),
            ));
        }
        let slug = args
            .slug
            .as_deref()
            .map(sanitize_slug)
            .filter(|slug| !slug.is_empty())
            .unwrap_or_else(|| "note".to_owned());
        // Subsecond precision keeps two same-slug notes in one burst from
        // colliding onto one file (notes are append-only by contract).
        let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%.6fZ");
        let path = self.notes_dir.join(format!("{timestamp}-{slug}.md"));
        std::fs::create_dir_all(&self.notes_dir).map_err(|error| {
            AgentError::ToolExecution(format!("create memory notes dir failed: {error}"))
        })?;
        std::fs::write(&path, format!("{content}\n")).map_err(|error| {
            AgentError::ToolExecution(format!("write memory note failed: {error}"))
        })?;
        Ok(ToolOutput {
            content: serde_json::json!({ "path": path.to_string_lossy() }).to_string(),
            metadata: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use slab_agent::ToolHandler;

    use super::*;

    fn ctx() -> ToolContext {
        ToolContext::for_thread("t").build()
    }

    #[tokio::test]
    async fn writes_timestamped_note_and_creates_the_notes_dir() {
        let root = tempfile::tempdir().expect("tempdir");
        let notes_dir = root.path().join("extensions").join("ad_hoc").join("notes");
        let tool = MemoryNoteTool::new(notes_dir.clone());

        let output = ToolHandler::execute(
            &tool,
            &ctx(),
            &serde_json::json!({"content": "prefer trailing commas", "slug": "Rust Style!!"}),
        )
        .await
        .expect("note written");

        let rendered = serde_json::from_str::<serde_json::Value>(&output.content).expect("json");
        let path = PathBuf::from(rendered["path"].as_str().expect("path"));
        assert!(path.starts_with(&notes_dir), "{path:?}");
        assert!(
            path.file_name().expect("name").to_string_lossy().ends_with("-rust-style.md"),
            "slug sanitized into the filename: {path:?}"
        );
        let body = std::fs::read_to_string(&path).expect("note body");
        assert_eq!(body, "prefer trailing commas\n");
    }

    #[tokio::test]
    async fn redacts_secrets_like_phase1_ingestion() {
        let root = tempfile::tempdir().expect("tempdir");
        let tool = MemoryNoteTool::new(root.path().join("notes"));

        ToolHandler::execute(
            &tool,
            &ctx(),
            &serde_json::json!({"content": "token: sk-ant-0123456789abcdef012345"}),
        )
        .await
        .expect("note written");

        let note = std::fs::read_dir(root.path().join("notes"))
            .expect("notes dir")
            .flatten()
            .next()
            .expect("one note")
            .path();
        let body = std::fs::read_to_string(&note).expect("body");
        assert!(body.contains("[REDACTED_SECRET]"), "{body}");
        assert!(!body.contains("sk-ant-0123456789"), "{body}");
    }

    #[tokio::test]
    async fn rejects_empty_content_and_falls_back_to_note_slug() {
        let root = tempfile::tempdir().expect("tempdir");
        let notes_dir = root.path().join("notes");
        let tool = MemoryNoteTool::new(notes_dir.clone());

        let error = ToolHandler::execute(&tool, &ctx(), &serde_json::json!({"content": "   "}))
            .await
            .expect_err("empty content");
        assert!(error.to_string().contains("empty"), "{error}");

        // A slug that sanitizes to nothing falls back to `note`.
        ToolHandler::execute(&tool, &ctx(), &serde_json::json!({"content": "x", "slug": "///"}))
            .await
            .expect("note written");
        let name = std::fs::read_dir(&notes_dir)
            .expect("notes dir")
            .flatten()
            .next()
            .expect("one note")
            .path()
            .file_name()
            .expect("name")
            .to_string_lossy()
            .into_owned();
        assert!(name.ends_with("-note.md"), "{name}");
    }

    #[test]
    fn schema_requires_content_and_optional_slug() {
        let schema = ToolHandler::parameters_schema(&MemoryNoteTool::new(PathBuf::from("/n")));
        assert_eq!(schema["required"], serde_json::json!(["content"]));
        assert!(schema["properties"]["slug"].is_object());
    }
}
