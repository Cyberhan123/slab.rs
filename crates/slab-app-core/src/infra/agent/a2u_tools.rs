use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use slab_agent::{AgentError, ToolContext, ToolHandler, ToolOutput};

pub(crate) struct WorkspaceOpenTool;

impl WorkspaceOpenTool {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[derive(Debug, Deserialize)]
struct WorkspaceOpenArgs {
    #[serde(default)]
    path: Option<String>,
    #[serde(default, alias = "revealPath")]
    reveal_path: Option<String>,
    #[serde(default)]
    file: Option<String>,
    #[serde(default, alias = "relativePath")]
    relative_path: Option<String>,
}

#[async_trait]
impl ToolHandler for WorkspaceOpenTool {
    fn name(&self) -> &str {
        "workspace.open"
    }

    fn description(&self) -> &str {
        "Open the trusted Workspace surface and optionally reveal a workspace-relative file path."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Workspace-relative or workspace-contained file path to reveal."
                },
                "revealPath": {
                    "type": "string",
                    "description": "Alias for path, accepted for frontend a2u compatibility."
                },
                "file": {
                    "type": "string",
                    "description": "Alias for path."
                },
                "relativePath": {
                    "type": "string",
                    "description": "Alias for path."
                }
            }
        })
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let args: WorkspaceOpenArgs =
            serde_json::from_value(arguments.clone()).map_err(|error| {
                AgentError::ToolExecution(format!("invalid workspace.open args: {error}"))
            })?;
        let reveal_path = first_trimmed([
            args.reveal_path.as_deref(),
            args.path.as_deref(),
            args.file.as_deref(),
            args.relative_path.as_deref(),
        ])
        .and_then(|path| normalize_workspace_relative_path(&path));
        let content = json!({
            "surface": "workspace",
            "revealPath": reveal_path,
            "opened": true
        });

        Ok(ToolOutput { content: content.to_string(), metadata: Some(content) })
    }
}

pub(crate) struct ReviewShowTool;

impl ReviewShowTool {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[derive(Debug, Deserialize)]
struct ReviewShowArgs {
    #[serde(default)]
    diff: Option<String>,
    #[serde(default)]
    patch: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    file: Option<String>,
    #[serde(default, alias = "relativePath")]
    relative_path: Option<String>,
}

#[async_trait]
impl ToolHandler for ReviewShowTool {
    fn name(&self) -> &str {
        "review.show"
    }

    fn description(&self) -> &str {
        "Open the trusted review surface for a diff, patch, or workspace path that needs user review."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "diff": {
                    "type": "string",
                    "description": "Unified diff or short review payload."
                },
                "patch": {
                    "type": "string",
                    "description": "Alias for diff."
                },
                "path": {
                    "type": "string",
                    "description": "Workspace path related to the review."
                },
                "file": {
                    "type": "string",
                    "description": "Alias for path."
                },
                "relativePath": {
                    "type": "string",
                    "description": "Alias for path."
                }
            }
        })
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let args: ReviewShowArgs = serde_json::from_value(arguments.clone()).map_err(|error| {
            AgentError::ToolExecution(format!("invalid review.show args: {error}"))
        })?;
        let diff = first_trimmed([args.diff.as_deref(), args.patch.as_deref()]);
        let path = first_trimmed([
            args.path.as_deref(),
            args.file.as_deref(),
            args.relative_path.as_deref(),
        ])
        .and_then(|path| normalize_workspace_relative_path(&path));
        let content = json!({
            "surface": "review",
            "diff": diff,
            "path": path,
            "opened": true
        });

        Ok(ToolOutput { content: content.to_string(), metadata: Some(content) })
    }
}

pub(crate) struct ImageEditTool;

impl ImageEditTool {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[derive(Debug, Deserialize)]
struct ImageEditArgs {
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

#[async_trait]
impl ToolHandler for ImageEditTool {
    fn name(&self) -> &str {
        "image.edit"
    }

    fn description(&self) -> &str {
        "Open the trusted image surface with an optional generation or edit prompt."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "Prompt or edit instruction to preload in the image workbench."
                },
                "description": {
                    "type": "string",
                    "description": "Alias for prompt."
                }
            }
        })
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let args: ImageEditArgs = serde_json::from_value(arguments.clone()).map_err(|error| {
            AgentError::ToolExecution(format!("invalid image.edit args: {error}"))
        })?;
        let prompt = first_trimmed([args.prompt.as_deref(), args.description.as_deref()]);
        let content = json!({
            "surface": "image",
            "prompt": prompt,
            "opened": true
        });

        Ok(ToolOutput { content: content.to_string(), metadata: Some(content) })
    }
}

pub(crate) struct HubBrowseTool;

impl HubBrowseTool {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ToolHandler for HubBrowseTool {
    fn name(&self) -> &str {
        "hub.browse"
    }

    fn description(&self) -> &str {
        "Open the trusted Hub surface for browsing available models and capabilities."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        _arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let content = json!({
            "surface": "hub",
            "opened": true
        });

        Ok(ToolOutput { content: content.to_string(), metadata: Some(content) })
    }
}

pub(crate) fn register_builtin_a2u_tools(router: &slab_agent::ToolRouter) {
    router.register(Box::new(WorkspaceOpenTool::new()));
    router.register(Box::new(ReviewShowTool::new()));
    router.register(Box::new(ImageEditTool::new()));
    router.register(Box::new(HubBrowseTool::new()));
}

fn first_trimmed<const N: usize>(values: [Option<&str>; N]) -> Option<String> {
    values.into_iter().flatten().map(str::trim).find(|value| !value.is_empty()).map(str::to_owned)
}

fn normalize_workspace_relative_path(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() || is_absolute_or_drive_path(trimmed) {
        return None;
    }

    let normalized = trimmed.replace('\\', "/");
    let parts =
        normalized.split('/').filter(|part| !part.is_empty() && *part != ".").collect::<Vec<_>>();
    if parts.is_empty() || parts.contains(&"..") {
        return None;
    }

    Some(parts.join("/"))
}

fn is_absolute_or_drive_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    path.starts_with('/')
        || path.starts_with('\\')
        || bytes.first().is_some_and(u8::is_ascii_alphabetic) && bytes.get(1) == Some(&b':')
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use slab_agent::ToolRouter;
    use slab_agent::{ToolContext, ToolHandler};

    use super::{
        HubBrowseTool, ImageEditTool, ReviewShowTool, WorkspaceOpenTool, register_builtin_a2u_tools,
    };

    fn ctx() -> ToolContext {
        ToolContext::for_thread("thread").build()
    }

    #[tokio::test]
    async fn workspace_open_accepts_frontend_aliases() {
        let tool = WorkspaceOpenTool::new();

        let output = tool
            .execute(&ctx(), &json!({ "reveal_path": "src\\main.rs" }))
            .await
            .expect("workspace.open should accept reveal_path");
        let value: Value = serde_json::from_str(&output.content).unwrap();

        assert_eq!(value["surface"], "workspace");
        assert_eq!(value["revealPath"], "src/main.rs");
        assert_eq!(value["opened"], true);
    }

    #[tokio::test]
    async fn workspace_open_drops_unsafe_artifact_paths() {
        let tool = WorkspaceOpenTool::new();

        for path in
            ["C:/Users/example/.ssh/id_rsa", "C:relative.txt", "/etc/passwd", "../outside.rs"]
        {
            let output = tool
                .execute(&ctx(), &json!({ "path": path }))
                .await
                .expect("workspace.open should still open the surface");
            let value: Value = serde_json::from_str(&output.content).unwrap();

            assert_eq!(value["surface"], "workspace");
            assert!(value["revealPath"].is_null());
        }
    }

    #[tokio::test]
    async fn review_show_normalizes_diff_and_path_aliases() {
        let tool = ReviewShowTool::new();

        let output = tool
            .execute(&ctx(), &json!({ "patch": "+ added", "file": "src/lib.rs" }))
            .await
            .expect("review.show should accept aliases");
        let value: Value = serde_json::from_str(&output.content).unwrap();

        assert_eq!(value["surface"], "review");
        assert_eq!(value["diff"], "+ added");
        assert_eq!(value["path"], "src/lib.rs");
    }

    #[tokio::test]
    async fn review_show_preserves_diff_but_drops_unsafe_paths() {
        let tool = ReviewShowTool::new();

        let output = tool
            .execute(&ctx(), &json!({ "diff": "+ added", "path": "../outside.rs" }))
            .await
            .expect("review.show should still open the surface");
        let value: Value = serde_json::from_str(&output.content).unwrap();

        assert_eq!(value["surface"], "review");
        assert_eq!(value["diff"], "+ added");
        assert!(value["path"].is_null());
    }

    #[tokio::test]
    async fn image_edit_normalizes_prompt_aliases() {
        let tool = ImageEditTool::new();

        let output = tool
            .execute(&ctx(), &json!({ "description": "render the logo" }))
            .await
            .expect("image.edit should accept description");
        let value: Value = serde_json::from_str(&output.content).unwrap();

        assert_eq!(value["surface"], "image");
        assert_eq!(value["prompt"], "render the logo");
    }

    #[tokio::test]
    async fn hub_browse_returns_surface_metadata() {
        let tool = HubBrowseTool::new();

        let output = tool.execute(&ctx(), &json!({})).await.expect("hub.browse should execute");
        let value: Value = serde_json::from_str(&output.content).unwrap();

        assert_eq!(value["surface"], "hub");
        assert_eq!(value["opened"], true);
    }

    #[test]
    fn registers_builtin_a2u_tools_in_agent_router() {
        let router = ToolRouter::new();

        register_builtin_a2u_tools(&router);

        assert!(router.get("workspace.open").is_some());
        assert!(router.get("review.show").is_some());
        assert!(router.get("image.edit").is_some());
        assert!(router.get("hub.browse").is_some());
    }
}
