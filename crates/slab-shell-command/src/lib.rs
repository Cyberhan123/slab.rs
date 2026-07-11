//! Policy-aware shell command execution.
//!
//! This crate is now **execution-only**: the permission decision (allow /
//! require-approval / deny) lives in `slab-exec-policy`. `ShellExecutor` runs
//! a command the kernel has already authorized, with a defense-in-depth
//! dangerous-command refusal. The legacy `ShellPolicy` and rule types are
//! re-exported from `slab-exec-policy` for backward compatibility.

use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};
use slab_exec_policy::{CommandSafetyChecker, SafetyDecision};
use slab_sandboxing::{SandboxDriver, SandboxError, SandboxedCommand};
use slab_utils::string::decode_truncated_output;
use thiserror::Error;
use tracing::{debug, warn};

// Re-export the policy/rule vocabulary (now owned by `slab-exec-policy`) so
// existing imports of `slab_shell_command::{ShellPolicy, ShellRule*}` keep
// working during the migration.
pub use slab_exec_policy::compat::ShellPolicy;
pub use slab_exec_policy::{Rule, RuleAction, RuleError, RuleMatcher, RuleSet, RuleSource};

/// Deprecated aliases for the old shell-only rule names.
pub type ShellRule = Rule;
pub type ShellRuleAction = RuleAction;
pub type ShellRuleError = RuleError;
pub type ShellRuleMatcher = RuleMatcher;
pub type ShellRuleSet = RuleSet;

const DEFAULT_OUTPUT_LIMIT_BYTES: usize = 100 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellCommand {
    pub command: String,
    pub timeout_secs: u64,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

impl ShellCommand {
    pub fn new(command: impl Into<String>) -> Self {
        Self { command: command.into(), timeout_secs: 30, env: HashMap::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub timed_out: bool,
}

#[derive(Debug, Error)]
pub enum ShellError {
    #[error("shell command is blocked by policy")]
    BlockedByPolicy,
    #[error("command blocked: {0}")]
    DangerousCommand(String),
    #[error("failed to spawn command: {0}")]
    SpawnFailed(String),
    #[error("failed to wait for command: {0}")]
    WaitFailed(String),
    #[error("sandbox execution failed: {0}")]
    Sandbox(#[from] SandboxError),
}

pub struct ShellExecutor {
    workspace_root: Option<PathBuf>,
    sandbox_driver: Option<Arc<dyn SandboxDriver>>,
    output_limit_bytes: usize,
}

impl ShellExecutor {
    pub fn new(
        workspace_root: Option<PathBuf>,
        sandbox_driver: Option<Arc<dyn SandboxDriver>>,
    ) -> Self {
        Self { workspace_root, sandbox_driver, output_limit_bytes: DEFAULT_OUTPUT_LIMIT_BYTES }
    }

    pub fn with_output_limit_bytes(mut self, output_limit_bytes: usize) -> Self {
        self.output_limit_bytes = output_limit_bytes;
        self
    }

    pub async fn execute(&self, command: ShellCommand) -> Result<ShellOutput, ShellError> {
        // Defense-in-depth: the exec-policy engine already hard-denies these,
        // but refuse dangerous commands here too so a caller that bypasses the
        // engine cannot run them.
        if let SafetyDecision::Dangerous(reason) = CommandSafetyChecker::check(&command.command) {
            warn!(command = %command.command, reason, "blocked dangerous shell command");
            return Err(ShellError::DangerousCommand(reason));
        }

        if let Some(driver) = &self.sandbox_driver {
            let argv = shell_argv(&command.command);
            debug!(driver = driver.name(), "executing shell command through sandbox driver");
            let output = driver
                .run(SandboxedCommand {
                    argv,
                    env: command.env,
                    cwd: self.workspace_root.clone(),
                    timeout: Some(Duration::from_secs(command.timeout_secs)),
                })
                .await?;

            return Ok(ShellOutput {
                stdout: decode_truncated_output(&output.stdout, self.output_limit_bytes),
                stderr: decode_truncated_output(&output.stderr, self.output_limit_bytes),
                exit_code: output.exit_code,
                timed_out: output.timed_out,
            });
        }

        let output = execute_direct(command, self.workspace_root.clone()).await?;
        Ok(ShellOutput {
            stdout: decode_truncated_output(&output.stdout, self.output_limit_bytes),
            stderr: decode_truncated_output(&output.stderr, self.output_limit_bytes),
            exit_code: output.status.code().unwrap_or(-1),
            timed_out: output.timed_out,
        })
    }
}

struct DirectOutput {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    status: std::process::ExitStatus,
    timed_out: bool,
}

async fn execute_direct(
    command: ShellCommand,
    workspace_root: Option<PathBuf>,
) -> Result<DirectOutput, ShellError> {
    let mut child = platform_command(&command.command);
    for (key, value) in command.env {
        child.env(key, value);
    }
    if let Some(root) = workspace_root {
        child.current_dir(root);
    }
    child.kill_on_drop(true);

    let child = child.spawn().map_err(|e| ShellError::SpawnFailed(e.to_string()))?;
    let wait =
        tokio::time::timeout(Duration::from_secs(command.timeout_secs), child.wait_with_output())
            .await;

    match wait {
        Ok(Ok(output)) => Ok(DirectOutput {
            stdout: output.stdout,
            stderr: output.stderr,
            status: output.status,
            timed_out: false,
        }),
        Ok(Err(e)) => Err(ShellError::WaitFailed(e.to_string())),
        Err(_) => {
            #[cfg(windows)]
            let status = std::os::windows::process::ExitStatusExt::from_raw(1);
            #[cfg(unix)]
            let status = std::os::unix::process::ExitStatusExt::from_raw(1);
            Ok(DirectOutput {
                stdout: Vec::new(),
                stderr: b"command timed out".to_vec(),
                status,
                timed_out: true,
            })
        }
    }
}

fn platform_command(command: &str) -> tokio::process::Command {
    let argv = shell_argv(command);
    let mut process = tokio::process::Command::new(&argv[0]);
    process.args(&argv[1..]);
    process
}

fn shell_argv(command: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        vec![
            "powershell.exe".to_string(),
            "-NoLogo".to_string(),
            "-NoProfile".to_string(),
            "-Command".to_string(),
            command.to_string(),
        ]
    }

    #[cfg(not(windows))]
    {
        vec!["sh".to_string(), "-lc".to_string(), command.to_string()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct RecordingDriver {
        seen: Arc<Mutex<Option<SandboxedCommand>>>,
        output: slab_sandboxing::SandboxedOutput,
    }

    #[async_trait::async_trait]
    impl SandboxDriver for RecordingDriver {
        fn name(&self) -> &str {
            "recording"
        }

        async fn run(
            &self,
            cmd: SandboxedCommand,
        ) -> Result<slab_sandboxing::SandboxedOutput, SandboxError> {
            *self.seen.lock().unwrap() = Some(cmd);
            Ok(self.output.clone())
        }
    }

    #[tokio::test]
    async fn shell_executor_maps_sandbox_output_to_json_and_filters_env_values() {
        let seen = Arc::new(Mutex::new(None));
        let executor = ShellExecutor::new(
            Some(PathBuf::from("workspace")),
            Some(Arc::new(RecordingDriver {
                seen: Arc::clone(&seen),
                output: slab_sandboxing::SandboxedOutput {
                    stdout: b"ok".to_vec(),
                    stderr: b"warn".to_vec(),
                    exit_code: 7,
                    timed_out: true,
                },
            })),
        );

        let output = executor
            .execute(ShellCommand {
                command: "echo ok".to_string(),
                timeout_secs: 5,
                env: HashMap::from([("TEXT".to_string(), "value".to_string())]),
            })
            .await
            .expect("shell output");

        assert_eq!(output.exit_code, 7);
        assert!(output.timed_out);

        let command = seen.lock().unwrap().clone().expect("driver command");
        assert_eq!(command.cwd.as_deref(), Some(PathBuf::from("workspace").as_path()));
        assert_eq!(command.timeout.map(|timeout| timeout.as_secs()), Some(5));
        assert_eq!(command.env.get("TEXT").map(String::as_str), Some("value"));
    }

    #[tokio::test]
    async fn shell_executor_refuses_dangerous_command() {
        let executor = ShellExecutor::new(None, None);
        let error =
            executor.execute(ShellCommand::new("rm -rf /")).await.expect_err("dangerous command");
        assert!(matches!(error, ShellError::DangerousCommand(_)));
    }

    #[tokio::test]
    async fn direct_execution_reports_timeout() {
        #[cfg(windows)]
        let command = "Start-Sleep -Seconds 2";
        #[cfg(not(windows))]
        let command = "sleep 2";

        let output = ShellExecutor::new(None, None)
            .execute(ShellCommand {
                command: command.to_string(),
                timeout_secs: 1,
                env: HashMap::new(),
            })
            .await
            .expect("timed out commands should return output");

        assert!(output.timed_out);
        #[cfg(windows)]
        assert_eq!(output.exit_code, 1);
        #[cfg(not(windows))]
        assert_eq!(output.exit_code, -1);
    }
}
