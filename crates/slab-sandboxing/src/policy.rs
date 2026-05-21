use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxPermissions {
    #[serde(default)]
    pub writable_roots: Vec<PathBuf>,
    #[serde(default)]
    pub readable_roots: Vec<PathBuf>,
    #[serde(default)]
    pub denied_paths: Vec<PathBuf>,
    #[serde(default)]
    pub denied_globs: Vec<String>,
    #[serde(default = "default_protected_path_names")]
    pub protected_path_names: Vec<String>,
    #[serde(default)]
    pub managed_proxy: Option<SandboxManagedProxy>,
    #[serde(default)]
    pub platform: SandboxPlatformConfig,
    #[serde(default)]
    pub network: NetworkPolicy,
}

impl Default for SandboxPermissions {
    fn default() -> Self {
        Self {
            writable_roots: Vec::new(),
            readable_roots: Vec::new(),
            denied_paths: Vec::new(),
            denied_globs: Vec::new(),
            protected_path_names: default_protected_path_names(),
            managed_proxy: None,
            platform: SandboxPlatformConfig::default(),
            network: NetworkPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SandboxManagedProxy {
    pub http_proxy: Option<String>,
    pub https_proxy: Option<String>,
    #[serde(default)]
    pub no_proxy: Vec<String>,
    #[serde(default)]
    pub allowed_loopback_ports: Vec<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxPlatformConfig {
    pub windows_setup_required: bool,
    pub linux_allow_landlock_fallback: bool,
    pub macos_use_sandbox_exec: bool,
}

impl Default for SandboxPlatformConfig {
    fn default() -> Self {
        Self {
            windows_setup_required: false,
            linux_allow_landlock_fallback: true,
            macos_use_sandbox_exec: true,
        }
    }
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

    pub fn with_permissions(
        workspace_root: Option<PathBuf>,
        policy: SandboxPolicy,
        permissions: SandboxPermissions,
    ) -> Self {
        Self { workspace_root, policy, permissions }
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
                if matches!(
                    tool_name,
                    "write_file" | "shell" | "exec" | "run_command" | "git_commit"
                ) {
                    ExecPolicy::Deny
                } else {
                    ExecPolicy::AutoApprove
                }
            }
        }
    }
}

pub fn default_protected_path_names() -> Vec<String> {
    [".git", ".slab", ".agents"].into_iter().map(str::to_owned).collect()
}
