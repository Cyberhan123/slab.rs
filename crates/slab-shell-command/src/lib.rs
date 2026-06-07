//! Policy-aware shell command execution.

use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};
use slab_sandboxing::{ExecPolicy, SandboxDriver, SandboxError, SandboxPolicy, SandboxedCommand};
use slab_utils::string::decode_truncated_prefix;
use thiserror::Error;
use tracing::{debug, warn};

mod rules;

pub use rules::{
    RuleSource, ShellRule, ShellRuleAction, ShellRuleError, ShellRuleMatcher, ShellRuleSet,
};

const DEFAULT_OUTPUT_LIMIT_BYTES: usize = 100 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShellPolicy {
    #[default]
    Allow,
    RequireApproval,
    Block,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SafetyDecision {
    Safe,
    Dangerous(String),
}

pub struct CommandSafetyChecker;

impl CommandSafetyChecker {
    pub fn check(command: &str) -> SafetyDecision {
        let trimmed = command.trim();
        let compact = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");

        let dangerous_patterns = [
            ("rm -rf /", "refuses to delete the filesystem root"),
            ("rm -rf /*", "refuses to delete filesystem root children"),
            ("sudo rm -rf", "refuses privileged recursive deletion"),
            (":(){ :|:& };:", "refuses fork bomb pattern"),
            (":() { :|:& };:", "refuses fork bomb pattern"),
            ("> /proc/sysrq-trigger", "refuses kernel sysrq trigger writes"),
            ("echo c > /proc/sysrq-trigger", "refuses kernel crash trigger"),
            ("chmod -R 777 /", "refuses broad root permission change"),
            ("chown -R", "refuses broad ownership rewrite"),
            ("mkfs.", "refuses filesystem formatting command"),
            ("mkswap ", "refuses swap formatting command"),
            ("dd if=", "refuses raw dd patterns"),
            ("of=/dev/", "refuses raw device writes"),
            ("grub-install", "refuses bootloader writes"),
            ("bootrec", "refuses boot repair writes"),
            ("bcdedit", "refuses boot configuration writes"),
            ("diskpart", "refuses disk partitioning command"),
            ("format c:", "refuses drive formatting command"),
        ];

        for (pattern, reason) in dangerous_patterns {
            if compact.to_ascii_lowercase().contains(&pattern.to_ascii_lowercase()) {
                return SafetyDecision::Dangerous(reason.to_string());
            }
        }

        for pipe_shell in [
            "| sh",
            "| bash",
            "| zsh",
            "| dash",
            "| fish",
            "| ksh",
            "| sudo sh",
            "| sudo bash",
            "| sudo zsh",
            "iex (",
            "Invoke-Expression",
        ] {
            if trimmed.contains(pipe_shell) {
                return SafetyDecision::Dangerous(
                    "refuses piping or evaluating remote content through a shell".to_string(),
                );
            }
        }

        if trimmed.contains(">/etc/passwd")
            || trimmed.contains("> /etc/passwd")
            || trimmed.contains(">/etc/shadow")
            || trimmed.contains("> /etc/shadow")
        {
            return SafetyDecision::Dangerous(
                "refuses writes to critical account files".to_string(),
            );
        }

        SafetyDecision::Safe
    }
}

pub struct ExecPolicyChecker;

impl ExecPolicyChecker {
    pub fn check(shell_policy: ShellPolicy, sandbox_policy: SandboxPolicy) -> ExecPolicy {
        match shell_policy {
            ShellPolicy::Block => ExecPolicy::Deny,
            ShellPolicy::RequireApproval => ExecPolicy::RequireApproval,
            ShellPolicy::Allow => ExecPolicy::from_sandbox_policy(sandbox_policy, "shell"),
        }
    }
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
    shell_policy: ShellPolicy,
    sandbox_policy: SandboxPolicy,
    rules: ShellRuleSet,
    workspace_root: Option<PathBuf>,
    sandbox_driver: Option<Arc<dyn SandboxDriver>>,
    output_limit_bytes: usize,
}

impl ShellExecutor {
    pub fn new(
        shell_policy: ShellPolicy,
        workspace_root: Option<PathBuf>,
        sandbox_driver: Option<Arc<dyn SandboxDriver>>,
    ) -> Self {
        Self {
            shell_policy,
            sandbox_policy: SandboxPolicy::WorkspaceWrite,
            rules: ShellRuleSet::default(),
            workspace_root,
            sandbox_driver,
            output_limit_bytes: DEFAULT_OUTPUT_LIMIT_BYTES,
        }
    }

    pub fn with_sandbox_policy(mut self, sandbox_policy: SandboxPolicy) -> Self {
        self.sandbox_policy = sandbox_policy;
        self
    }

    pub fn with_rules(mut self, rules: ShellRuleSet) -> Self {
        self.rules = rules;
        self
    }

    pub fn with_output_limit_bytes(mut self, output_limit_bytes: usize) -> Self {
        self.output_limit_bytes = output_limit_bytes;
        self
    }

    pub fn approval_required(&self) -> bool {
        matches!(
            ExecPolicyChecker::check(self.shell_policy, self.sandbox_policy),
            ExecPolicy::RequireApproval
        )
    }

    pub fn approval_required_for_command(&self, command: &str) -> bool {
        matches!(self.policy_for_command(command), ExecPolicy::RequireApproval)
    }

    pub fn policy_for_command(&self, command: &str) -> ExecPolicy {
        let base = ExecPolicyChecker::check(self.shell_policy, self.sandbox_policy);
        if matches!(base, ExecPolicy::Deny) {
            return ExecPolicy::Deny;
        }

        if let Some(rule) = self.rules.evaluate(command) {
            return match rule.action {
                ShellRuleAction::Allow => ExecPolicy::AutoApprove,
                ShellRuleAction::RequireApproval => ExecPolicy::RequireApproval,
                ShellRuleAction::Block => ExecPolicy::Deny,
            };
        }

        base
    }

    pub fn shell_policy(&self) -> ShellPolicy {
        self.shell_policy
    }

    pub async fn execute(&self, command: ShellCommand) -> Result<ShellOutput, ShellError> {
        match self.policy_for_command(&command.command) {
            ExecPolicy::Deny => return Err(ShellError::BlockedByPolicy),
            ExecPolicy::AutoApprove | ExecPolicy::RequireApproval => {}
        }

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
                stdout: truncate_output(&output.stdout, self.output_limit_bytes),
                stderr: truncate_output(&output.stderr, self.output_limit_bytes),
                exit_code: output.exit_code,
                timed_out: output.timed_out,
            });
        }

        let output = execute_direct(command, self.workspace_root.clone()).await?;
        Ok(ShellOutput {
            stdout: truncate_output(&output.stdout, self.output_limit_bytes),
            stderr: truncate_output(&output.stderr, self.output_limit_bytes),
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

fn truncate_output(bytes: &[u8], limit: usize) -> String {
    decode_truncated_prefix(bytes, limit, "\n[output truncated]\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use slab_sandboxing::SandboxedOutput;

    #[test]
    fn detects_destructive_commands() {
        assert!(matches!(CommandSafetyChecker::check("rm -rf /"), SafetyDecision::Dangerous(_)));
        assert!(matches!(
            CommandSafetyChecker::check(":(){ :|:& };:"),
            SafetyDecision::Dangerous(_)
        ));
        assert!(matches!(
            CommandSafetyChecker::check("chmod -R 777 /"),
            SafetyDecision::Dangerous(_)
        ));
        assert!(matches!(CommandSafetyChecker::check("echo hello"), SafetyDecision::Safe));
    }

    #[test]
    fn resolves_exec_policy() {
        assert_eq!(
            ExecPolicyChecker::check(ShellPolicy::RequireApproval, SandboxPolicy::WorkspaceWrite),
            ExecPolicy::RequireApproval
        );
        assert_eq!(
            ExecPolicyChecker::check(ShellPolicy::Block, SandboxPolicy::DangerFullAccess),
            ExecPolicy::Deny
        );
    }

    #[test]
    fn command_rules_override_approval_but_not_policy_denial() {
        let rules = ShellRuleSet::from_rules(vec![
            ShellRule::new(ShellRuleAction::Allow, ShellRuleMatcher::Prefix, "cargo check"),
            ShellRule::new(ShellRuleAction::Block, ShellRuleMatcher::Contains, "Remove-Item"),
        ]);
        let executor = ShellExecutor::new(ShellPolicy::Allow, None, None).with_rules(rules.clone());

        assert_eq!(
            executor.policy_for_command("cargo check -p slab-agent"),
            ExecPolicy::AutoApprove
        );
        assert_eq!(executor.policy_for_command("Remove-Item file.txt"), ExecPolicy::Deny);

        let blocked = ShellExecutor::new(ShellPolicy::Block, None, None).with_rules(rules);
        assert_eq!(blocked.policy_for_command("cargo check -p slab-agent"), ExecPolicy::Deny);
    }

    #[tokio::test]
    async fn delegates_to_sandbox_driver_and_truncates_output() {
        #[derive(Clone)]
        struct RecordingDriver {
            seen: Arc<Mutex<Option<SandboxedCommand>>>,
        }

        #[async_trait]
        impl SandboxDriver for RecordingDriver {
            fn name(&self) -> &str {
                "recording"
            }

            async fn run(&self, cmd: SandboxedCommand) -> Result<SandboxedOutput, SandboxError> {
                *self.seen.lock().unwrap() = Some(cmd);
                Ok(SandboxedOutput {
                    stdout: b"abcdefghij".to_vec(),
                    stderr: Vec::new(),
                    exit_code: 0,
                    timed_out: false,
                })
            }
        }

        let seen = Arc::new(Mutex::new(None));
        let driver = RecordingDriver { seen: Arc::clone(&seen) };
        let executor = ShellExecutor::new(
            ShellPolicy::Allow,
            Some(PathBuf::from("workspace")),
            Some(Arc::new(driver)),
        )
        .with_sandbox_policy(SandboxPolicy::DangerFullAccess)
        .with_output_limit_bytes(4);

        let mut env = HashMap::new();
        env.insert("KEY".to_string(), "value".to_string());
        let output = executor
            .execute(ShellCommand { command: "echo hello".to_string(), timeout_secs: 30, env })
            .await
            .expect("sandboxed shell should run");

        assert_eq!(output.exit_code, 0);
        assert_eq!(output.stdout, "abcd\n[output truncated]\n");
        let command = seen.lock().unwrap().clone().expect("driver should receive command");
        assert_eq!(command.cwd.as_deref(), Some(PathBuf::from("workspace").as_path()));
        assert_eq!(command.env.get("KEY").map(String::as_str), Some("value"));
    }

    #[tokio::test]
    async fn direct_execution_reports_timeout() {
        #[cfg(windows)]
        let command = "Start-Sleep -Seconds 2";
        #[cfg(not(windows))]
        let command = "sleep 2";

        let output = ShellExecutor::new(ShellPolicy::Allow, None, None)
            .with_sandbox_policy(SandboxPolicy::DangerFullAccess)
            .execute(ShellCommand {
                command: command.to_string(),
                timeout_secs: 1,
                env: HashMap::new(),
            })
            .await
            .expect("timed out commands should return output");

        assert!(output.timed_out);
        assert_eq!(output.exit_code, 1);
    }
}
