use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SandboxPolicy {
    #[default]
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NetworkPolicy {
    #[default]
    Blocked,
    Allowed,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SandboxPermissions {
    pub writable_roots: Vec<PathBuf>,
    pub readable_roots: Vec<PathBuf>,
    pub denied_paths: Vec<PathBuf>,
    pub network: NetworkPolicy,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SandboxEnvironment {
    pub workspace_root: Option<PathBuf>,
    pub policy: SandboxPolicy,
    pub permissions: SandboxPermissions,
}

impl SandboxEnvironment {
    pub fn new(workspace_root: Option<PathBuf>, policy: SandboxPolicy) -> Self {
        Self { workspace_root, policy, permissions: SandboxPermissions::default() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecPolicy {
    AutoApprove,
    RequireApproval,
    Deny,
}

impl ExecPolicy {
    /// Resolve an execution policy from the sandbox policy and the tool name.
    pub fn from_sandbox_policy(sandbox: SandboxPolicy, tool_name: &str) -> Self {
        match sandbox {
            SandboxPolicy::DangerFullAccess => ExecPolicy::AutoApprove,
            SandboxPolicy::WorkspaceWrite => {
                if matches!(tool_name, "shell" | "exec" | "run_command") {
                    ExecPolicy::RequireApproval
                } else {
                    ExecPolicy::AutoApprove
                }
            }
            SandboxPolicy::ReadOnly => {
                if matches!(tool_name, "write_file" | "shell" | "exec" | "run_command" | "git_commit") {
                    ExecPolicy::Deny
                } else {
                    ExecPolicy::AutoApprove
                }
            }
        }
    }
}
