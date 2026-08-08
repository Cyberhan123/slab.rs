//! Operation categories and descriptors consumed by the permission engine.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Coarse category of a tool operation, used to route it through the unified
/// permission engine. A shell command that edits files or reaches the network
/// is still categorized as [`OperationCategory::Shell`] — policy does not
/// introspect what a shell command does; the sandbox handles execution safety.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationCategory {
    Shell,
    FileEdit,
    Network,
    ReadOnly,
}

impl OperationCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Shell => "shell",
            Self::FileEdit => "file_edit",
            Self::Network => "network",
            Self::ReadOnly => "read_only",
        }
    }

    /// Parse a category token from a rule file. Returns `None` for non-category
    /// tokens so the rule parser can disambiguate the 3-token (legacy,
    /// shell-only) vs 4-token (category-prefixed) line format.
    pub fn parse(token: &str) -> Option<Self> {
        match token.to_ascii_lowercase().as_str() {
            "shell" => Some(Self::Shell),
            "file_edit" | "fileedit" | "file" => Some(Self::FileEdit),
            "network" => Some(Self::Network),
            "read_only" | "readonly" | "read" => Some(Self::ReadOnly),
            _ => None,
        }
    }
}

impl std::fmt::Display for OperationCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Description of a single tool invocation, supplied by the tool (or inferred
/// by the kernel from the tool name) and consumed by the permission engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationDescriptor {
    pub category: OperationCategory,
    /// Command text / file path / search query — category-dependent.
    pub subject: String,
    /// Optional secondary payload (diff text, env, extra target paths).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Workspace root the operation is scoped to, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<PathBuf>,
    /// The invoking tool's name (e.g. `shell`, `write_file`, or a namespaced
    /// plugin/MCP name like `mcp__github__create_issue`). The kernel sets this
    /// for every call so rules can scope decisions to a specific tool — the
    /// optional per-tool-name axis (Claude-Code-style `Bash(git *)`). `None`
    /// means "unknown" and a tool-scoped rule will not match it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

impl OperationDescriptor {
    pub fn new(category: OperationCategory, subject: impl Into<String>) -> Self {
        Self {
            category,
            subject: subject.into(),
            detail: None,
            workspace_root: None,
            tool_name: None,
        }
    }

    pub fn shell(command: impl Into<String>) -> Self {
        Self::new(OperationCategory::Shell, command)
    }

    pub fn file_edit(path: impl Into<String>) -> Self {
        Self::new(OperationCategory::FileEdit, path)
    }

    pub fn network(query: impl Into<String>) -> Self {
        Self::new(OperationCategory::Network, query)
    }

    pub fn read_only(subject: impl Into<String>) -> Self {
        Self::new(OperationCategory::ReadOnly, subject)
    }

    #[must_use]
    pub fn with_workspace(mut self, workspace_root: Option<PathBuf>) -> Self {
        self.workspace_root = workspace_root;
        self
    }

    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    #[must_use]
    pub fn with_tool_name(mut self, tool_name: impl Into<String>) -> Self {
        self.tool_name = Some(tool_name.into());
        self
    }
}
