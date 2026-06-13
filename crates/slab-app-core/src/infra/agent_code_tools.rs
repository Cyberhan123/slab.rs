//! Host-side code intelligence tools for the agent runtime.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use slab_agent::{AgentError, ToolContext, ToolHandler, ToolOutput};
use slab_types::plugin::PluginLanguageServerTransport;

use crate::domain::services::WorkspaceLspService;

pub(crate) struct CodeLspStatusTool {
    workspace_lsp: WorkspaceLspService,
}

impl CodeLspStatusTool {
    pub(crate) fn new(workspace_lsp: WorkspaceLspService) -> Self {
        Self { workspace_lsp }
    }
}

#[derive(Debug, Deserialize)]
struct CodeLspStatusArgs {
    #[serde(default)]
    language_id: Option<String>,
    #[serde(default)]
    path: Option<String>,
}

#[async_trait]
impl ToolHandler for CodeLspStatusTool {
    fn name(&self) -> &str {
        "code_lsp_status"
    }

    fn description(&self) -> &str {
        "Report whether Slab can resolve a workspace language-server provider for a language or file path."
    }

    fn parameters_schema(&self) -> Value {
        code_lsp_status_schema()
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let args: CodeLspStatusArgs =
            serde_json::from_value(arguments.clone()).map_err(|error| {
                AgentError::ToolExecution(format!("invalid code_lsp_status args: {error}"))
            })?;
        let language_id = requested_language_id(&args)?;
        let workspace_root =
            self.workspace_lsp.workspace_root().map_err(to_tool_execution_error)?;
        let provider = self
            .workspace_lsp
            .resolve_provider(&language_id)
            .await
            .map_err(to_tool_execution_error)?;

        Ok(ToolOutput {
            content: serde_json::json!({
                "language_id": language_id,
                "path": args.path.as_deref().map(str::trim).filter(|value| !value.is_empty()),
                "workspace_root": workspace_root.display().to_string(),
                "available": provider.is_some(),
                "provider": provider.map(|provider| {
                    let transport = transport_kind(&provider.contribution.transport);
                    serde_json::json!({
                        "id": provider.contribution.id,
                        "languages": provider.contribution.languages,
                        "transport": transport,
                        "install_root": provider.install_root.map(|root| root.display().to_string())
                    })
                })
            })
            .to_string(),
            metadata: None,
        })
    }
}

fn code_lsp_status_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "language_id": {
                "type": "string",
                "description": "Workspace language id such as typescript, rust, go, or python."
            },
            "path": {
                "type": "string",
                "description": "Optional file path used to infer the language id when language_id is omitted."
            }
        },
        "anyOf": [
            { "required": ["language_id"] },
            { "required": ["path"] }
        ]
    })
}

fn requested_language_id(args: &CodeLspStatusArgs) -> Result<String, AgentError> {
    if let Some(language_id) =
        args.language_id.as_deref().map(str::trim).filter(|value| !value.is_empty())
    {
        return Ok(language_id.to_ascii_lowercase());
    }
    if let Some(path) = args.path.as_deref().map(str::trim).filter(|value| !value.is_empty())
        && let Some(language_id) = language_id_from_path(path)
    {
        return Ok(language_id.to_owned());
    }

    Err(AgentError::ToolExecution(
        "code_lsp_status requires language_id or a path with a known extension".to_owned(),
    ))
}

fn language_id_from_path(path: &str) -> Option<&'static str> {
    let path = path.to_ascii_lowercase();
    if path.ends_with(".tsx") {
        return Some("typescriptreact");
    }
    if path.ends_with(".ts") {
        return Some("typescript");
    }
    if path.ends_with(".jsx") {
        return Some("javascriptreact");
    }
    if path.ends_with(".js") || path.ends_with(".mjs") || path.ends_with(".cjs") {
        return Some("javascript");
    }
    if path.ends_with(".json") {
        return Some("json");
    }
    if path.ends_with(".jsonc") {
        return Some("jsonc");
    }
    if path.ends_with(".css") {
        return Some("css");
    }
    if path.ends_with(".scss") {
        return Some("scss");
    }
    if path.ends_with(".less") {
        return Some("less");
    }
    if path.ends_with(".html") || path.ends_with(".htm") {
        return Some("html");
    }
    if path.ends_with(".py") {
        return Some("python");
    }
    if path.ends_with(".rs") {
        return Some("rust");
    }
    if path.ends_with(".go") {
        return Some("go");
    }
    if path.ends_with(".c") || path.ends_with(".h") {
        return Some("c");
    }
    if path.ends_with(".cc")
        || path.ends_with(".cpp")
        || path.ends_with(".cxx")
        || path.ends_with(".hpp")
        || path.ends_with(".hh")
    {
        return Some("cpp");
    }
    None
}

fn transport_kind(transport: &PluginLanguageServerTransport) -> &'static str {
    match transport {
        PluginLanguageServerTransport::Stdio { .. } => "stdio",
        PluginLanguageServerTransport::WebSocket { .. } => "websocket",
        PluginLanguageServerTransport::NodePackage { .. } => "node_package",
    }
}

fn to_tool_execution_error(error: crate::error::AppCoreError) -> AgentError {
    AgentError::ToolExecution(error.to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn infers_language_id_from_common_workspace_paths() {
        assert_eq!(language_id_from_path("src/app.tsx"), Some("typescriptreact"));
        assert_eq!(language_id_from_path("src/lib.rs"), Some("rust"));
        assert_eq!(language_id_from_path("cmd/server/main.go"), Some("go"));
        assert_eq!(language_id_from_path("include/slab.hpp"), Some("cpp"));
    }

    #[test]
    fn requires_language_or_known_path() {
        let args = CodeLspStatusArgs { language_id: None, path: Some("README.md".to_owned()) };
        let error = requested_language_id(&args).expect_err("unknown path rejected");

        assert!(error.to_string().contains("requires language_id"));
    }

    #[test]
    fn schema_accepts_language_or_path() {
        let schema = code_lsp_status_schema();

        assert_eq!(schema["properties"]["language_id"]["type"], "string");
        assert_eq!(schema["properties"]["path"]["type"], "string");
        assert_eq!(
            schema["anyOf"],
            json!([{ "required": ["language_id"] }, { "required": ["path"] }])
        );
    }
}
