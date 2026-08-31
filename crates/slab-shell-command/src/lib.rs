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
use slab_sandboxing::{
    OutputSink, PassThroughDriver, SandboxDriver, SandboxError, SandboxedCommand,
};
use slab_utils::string::decode_truncated_head_tail;
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

/// Per-stream (stdout/stderr) context-injection budget. Oversized streams
/// keep the head 70% / tail 30% of the budget with an explicit omission
/// marker — the tail matters more for command output than the middle does.
/// `ShellTool` raises its executor's capture bound and re-applies this budget
/// at the tool layer so it can spill the full output to disk first.
pub const DEFAULT_OUTPUT_LIMIT_BYTES: usize = 30 * 1024;
/// Head fraction of the kept budget when truncating.
const OUTPUT_HEAD_RATIO: f32 = 0.7;

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
    /// Original captured stdout length in bytes (before head/tail truncation)
    /// — lets callers decide whether to spill the full output to disk.
    #[serde(default)]
    pub stdout_bytes: usize,
    /// Original captured stderr length in bytes (before head/tail truncation).
    #[serde(default)]
    pub stderr_bytes: usize,
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
    shell: ResolvedShell,
}

impl ShellExecutor {
    pub fn new(
        workspace_root: Option<PathBuf>,
        sandbox_driver: Option<Arc<dyn SandboxDriver>>,
        launcher: ShellLauncher,
        bash_path: Option<PathBuf>,
    ) -> Self {
        Self {
            workspace_root,
            sandbox_driver,
            output_limit_bytes: DEFAULT_OUTPUT_LIMIT_BYTES,
            shell: launcher.resolve(bash_path),
        }
    }

    pub fn with_output_limit_bytes(mut self, output_limit_bytes: usize) -> Self {
        self.output_limit_bytes = output_limit_bytes;
        self
    }

    pub async fn execute(&self, command: ShellCommand) -> Result<ShellOutput, ShellError> {
        self.execute_with_sink(command, None).await
    }

    /// Execute a command, optionally streaming stdout/stderr chunks to `sink` as
    /// they arrive. The finalized `ShellOutput` (fully accumulated output) is
    /// returned either way — streaming is for live display only.
    pub async fn execute_with_sink(
        &self,
        command: ShellCommand,
        sink: Option<Arc<dyn OutputSink>>,
    ) -> Result<ShellOutput, ShellError> {
        // Defense-in-depth: the exec-policy engine already hard-denies these,
        // but refuse dangerous commands here too so a caller that bypasses the
        // engine cannot run them.
        if let SafetyDecision::Dangerous(reason) = CommandSafetyChecker::check(&command.command) {
            warn!(command = %command.command, reason, "blocked dangerous shell command");
            return Err(ShellError::DangerousCommand(reason));
        }

        let argv = self.shell.argv(&command.command);
        // Route through the configured sandbox driver, or a pass-through driver
        // when no workspace/sandbox is bound — both honor `output_sink`.
        let driver: Arc<dyn SandboxDriver> =
            self.sandbox_driver.clone().unwrap_or_else(|| Arc::new(PassThroughDriver));
        debug!(driver = driver.name(), "executing shell command");
        tracing::info!(command = %command.command, driver = driver.name(), argv0 = ?argv.first(), "execute_with_sink: enter");
        let output = driver
            .run(SandboxedCommand {
                argv,
                env: command.env,
                cwd: self.workspace_root.clone(),
                timeout: Some(Duration::from_secs(command.timeout_secs)),
                output_sink: sink,
            })
            .await?;
        tracing::info!(command = %command.command, exit_code = output.exit_code, timed_out = output.timed_out, "execute_with_sink: driver returned");

        Ok(ShellOutput {
            stdout: decode_truncated_head_tail(
                &output.stdout,
                self.output_limit_bytes,
                OUTPUT_HEAD_RATIO,
            ),
            stderr: decode_truncated_head_tail(
                &output.stderr,
                self.output_limit_bytes,
                OUTPUT_HEAD_RATIO,
            ),
            exit_code: output.exit_code,
            timed_out: output.timed_out,
            stdout_bytes: output.stdout.len(),
            stderr_bytes: output.stderr.len(),
        })
    }

    /// Spawn a DETACHED background command: stdout/stderr append to the given
    /// files (no pipes to drain), the tree stays resident, and the returned
    /// [`BackgroundChild`] hands the caller the wait/kill lifecycle. The
    /// dangerous-command defense-in-depth check applies exactly as the
    /// foreground path; `timeout_secs` is ignored (background tasks have no
    /// timeout — the caller stops them explicitly).
    pub async fn execute_background(
        &self,
        command: ShellCommand,
        stdout: std::fs::File,
        stderr: std::fs::File,
    ) -> Result<slab_sandboxing::BackgroundChild, ShellError> {
        if let SafetyDecision::Dangerous(reason) = CommandSafetyChecker::check(&command.command) {
            warn!(command = %command.command, reason, "blocked dangerous shell command");
            return Err(ShellError::DangerousCommand(reason));
        }

        let argv = self.shell.argv(&command.command);
        let driver: Arc<dyn SandboxDriver> =
            self.sandbox_driver.clone().unwrap_or_else(|| Arc::new(PassThroughDriver));
        debug!(driver = driver.name(), "spawning background shell command");
        Ok(driver
            .spawn_background(
                SandboxedCommand {
                    argv,
                    env: command.env,
                    cwd: self.workspace_root.clone(),
                    timeout: None,
                    output_sink: None,
                },
                stdout,
                stderr,
            )
            .await?)
    }
}

/// Configurable shell launcher preference. `Auto` probes for a POSIX shell
/// (`bash`/`sh`, including Git for Windows) and falls back to PowerShell on
/// Windows. The host converts its own config enum into this 1:1 at the boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShellLauncher {
    /// Probe for a POSIX shell; fall back to PowerShell (Windows) / bash (Unix).
    #[default]
    Auto,
    /// Always invoke a POSIX shell.
    Bash,
    /// Always invoke Windows PowerShell.
    PowerShell,
    /// Always invoke cmd.exe.
    Cmd,
}

/// Concrete shell resolved from a [`ShellLauncher`] (`Auto` folded to a choice).
#[derive(Debug, Clone, PartialEq, Eq)]
enum ResolvedShell {
    /// POSIX shell; the path is explicit when discovered, else a bare `bash`.
    Bash(PathBuf),
    PowerShell,
    Cmd,
}

/// The shell family a [`ShellLauncher`] would actually run — the single source
/// of truth for `Auto`'s probing semantics. Hosts that merely *describe* the
/// shell (e.g. the agent environment context) MUST resolve through this API
/// instead of guessing from the platform: on a Windows machine with Git Bash
/// installed, `Auto` runs bash, and a hardcoded `cfg!(windows) → PowerShell`
/// would lie to the model about which syntax to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellFamily {
    Bash,
    PowerShell,
    Cmd,
}

impl ShellLauncher {
    /// Resolve `Auto` to a concrete shell by probing for a POSIX shell.
    fn resolve(self, bash_path: Option<PathBuf>) -> ResolvedShell {
        match self {
            ShellLauncher::Bash => {
                ResolvedShell::Bash(resolve_bash(bash_path).unwrap_or_else(|| {
                    PathBuf::from(if cfg!(windows) { "bash.exe" } else { "bash" })
                }))
            }
            ShellLauncher::PowerShell => ResolvedShell::PowerShell,
            ShellLauncher::Cmd => ResolvedShell::Cmd,
            ShellLauncher::Auto => match resolve_bash(bash_path) {
                Some(bash) => ResolvedShell::Bash(bash),
                None => {
                    #[cfg(windows)]
                    {
                        ResolvedShell::PowerShell
                    }
                    #[cfg(not(windows))]
                    {
                        ResolvedShell::Bash(PathBuf::from("bash"))
                    }
                }
            },
        }
    }

    /// Which shell family this launcher would actually execute, including
    /// `Auto`'s bash probing (explicit path -> well-known -> PATH). Callers
    /// that only need the family (not the concrete argv) should prefer this
    /// over [`ShellExecutor::new`] — it performs the same resolution without
    /// keeping a resolved shell around.
    pub fn resolve_family(self, bash_path: Option<PathBuf>) -> ShellFamily {
        match self.resolve(bash_path) {
            ResolvedShell::Bash(_) => ShellFamily::Bash,
            ResolvedShell::PowerShell => ShellFamily::PowerShell,
            ResolvedShell::Cmd => ShellFamily::Cmd,
        }
    }
}

impl ResolvedShell {
    /// Build the argv that launches `command` through this shell.
    fn argv(&self, command: &str) -> Vec<String> {
        match self {
            // `--noprofile --norc` (matching PowerShell's `-NoProfile`): a
            // LOGIN shell made every command inherit `~/.bash_profile` —
            // profile echos polluted stdout, a profile `set -e` aborted
            // perfectly valid commands, and `conda init` slowed every spawn.
            // Coreutils still resolve: the MSYS runtime prepends its own
            // `/mingw64/bin:/usr/bin` to PATH, and the server's inherited
            // Windows PATH keeps user-installed tools reachable.
            ResolvedShell::Bash(path) => vec![
                path.to_string_lossy().into_owned(),
                "--noprofile".to_string(),
                "--norc".to_string(),
                "-c".to_string(),
                command.to_string(),
            ],
            ResolvedShell::PowerShell => vec![
                "powershell.exe".to_string(),
                "-NoLogo".to_string(),
                "-NoProfile".to_string(),
                "-NonInteractive".to_string(),
                "-Command".to_string(),
                command.to_string(),
            ],
            ResolvedShell::Cmd => {
                vec!["cmd.exe".to_string(), "/S".to_string(), "/C".to_string(), command.to_string()]
            }
        }
    }
}

/// Resolve a POSIX shell binary. Honors an explicit `preferred` path, then
/// well-known install locations, then a `PATH` walk for `bash`/`sh`.
fn resolve_bash(preferred: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(path) = preferred.filter(|p| p.is_file()) {
        return Some(path);
    }
    for candidate in well_known_bash_paths() {
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    find_on_path("bash").or_else(|| find_on_path("sh"))
}

/// Canonical Git for Windows / POSIX bash locations, most specific first.
fn well_known_bash_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    #[cfg(windows)]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            paths.push(
                PathBuf::from(&local).join("Programs").join("Git").join("bin").join("bash.exe"),
            );
        }
        paths.push(PathBuf::from(r"C:\Program Files\Git\bin\bash.exe"));
        paths.push(PathBuf::from(r"C:\Program Files\Git\usr\bin\bash.exe"));
        paths.push(PathBuf::from(r"C:\Program Files (x86)\Git\bin\bash.exe"));
    }
    #[cfg(not(windows))]
    {
        paths.push(PathBuf::from("/bin/bash"));
        paths.push(PathBuf::from("/usr/bin/bash"));
        paths.push(PathBuf::from("/usr/local/bin/bash"));
    }
    paths
}

/// Walk `PATH` for an executable named `name` (appends `.exe` on Windows).
fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let exe = dir.join(format!("{name}.exe"));
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
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
            ShellLauncher::PowerShell,
            None,
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

    /// Oversized stdout keeps head 70% / tail 30% of the budget with an
    /// explicit omission marker; the original size is reported separately.
    #[tokio::test]
    async fn shell_executor_truncates_stdout_head_and_tail() {
        // 200 lines x 1000 bytes = 200KB of stdout.
        let lines: Vec<String> =
            (0..200).map(|idx| format!("line-{idx:03}-{}", "x".repeat(990))).collect();
        let stdout = lines.join("\n");
        let stdout_bytes = stdout.len();
        let executor = ShellExecutor::new(
            None,
            Some(Arc::new(RecordingDriver {
                seen: Arc::new(Mutex::new(None)),
                output: slab_sandboxing::SandboxedOutput {
                    stdout: stdout.into_bytes(),
                    stderr: b"short stderr".to_vec(),
                    exit_code: 0,
                    timed_out: false,
                },
            })),
            ShellLauncher::PowerShell,
            None,
        );

        let output = executor.execute(ShellCommand::new("huge")).await.expect("shell output");

        assert_eq!(output.stdout_bytes, stdout_bytes);
        assert_eq!(output.stderr_bytes, b"short stderr".len());
        assert!(
            output.stdout.len() < DEFAULT_OUTPUT_LIMIT_BYTES + 256,
            "stdout not bounded: {}",
            output.stdout.len()
        );
        assert!(output.stdout.contains("line-000-"), "head must survive");
        assert!(output.stdout.contains("line-199-"), "tail must survive");
        assert!(!output.stdout.contains("line-100-"), "middle must be omitted");
        assert!(
            output.stdout.contains("bytes omitted"),
            "omission marker missing: {}",
            output.stdout
        );
        assert_eq!(output.stderr, "short stderr"); // small stream untouched
    }

    #[tokio::test]
    async fn shell_executor_preserves_short_output_verbatim() {
        let executor = ShellExecutor::new(
            None,
            Some(Arc::new(RecordingDriver {
                seen: Arc::new(Mutex::new(None)),
                output: slab_sandboxing::SandboxedOutput {
                    stdout: b"all good".to_vec(),
                    stderr: Vec::new(),
                    exit_code: 0,
                    timed_out: false,
                },
            })),
            ShellLauncher::PowerShell,
            None,
        );

        let output = executor.execute(ShellCommand::new("echo")).await.expect("shell output");
        assert_eq!(output.stdout, "all good");
        assert_eq!(output.stderr, "");
        assert!(!output.stdout.contains("omitted"));
    }

    #[tokio::test]
    async fn shell_executor_truncates_stderr_independently() {
        let stderr: String = "e".repeat(DEFAULT_OUTPUT_LIMIT_BYTES * 4);
        let stderr_bytes = stderr.len();
        let executor = ShellExecutor::new(
            None,
            Some(Arc::new(RecordingDriver {
                seen: Arc::new(Mutex::new(None)),
                output: slab_sandboxing::SandboxedOutput {
                    stdout: Vec::new(),
                    stderr: stderr.into_bytes(),
                    exit_code: 1,
                    timed_out: false,
                },
            })),
            ShellLauncher::PowerShell,
            None,
        );

        let output = executor.execute(ShellCommand::new("fail")).await.expect("shell output");
        assert_eq!(output.stderr_bytes, stderr_bytes);
        assert!(
            output.stderr.len() < DEFAULT_OUTPUT_LIMIT_BYTES + 256,
            "stderr not bounded: {}",
            output.stderr.len()
        );
        assert!(output.stderr.contains("bytes omitted"));
        assert_eq!(output.stdout, "");
    }

    /// Live acceptance check: a real `seq 1 100000` run comes back bounded
    /// with head and tail intact.
    #[tokio::test]
    async fn shell_executor_live_seq_output_is_bounded() {
        #[cfg(windows)]
        let (launcher, command) = (ShellLauncher::PowerShell, "1..100000 | ForEach-Object { $_ }");
        #[cfg(not(windows))]
        let (launcher, command) = (ShellLauncher::Bash, "seq 1 100000");

        let output = ShellExecutor::new(None, None, launcher, None)
            .execute(ShellCommand::new(command))
            .await
            .expect("live output");

        assert_eq!(output.exit_code, 0);
        assert!(output.stdout.contains("1\r\n") || output.stdout.contains("1\n"), "head missing");
        assert!(output.stdout.contains("100000"), "tail missing");
        assert!(
            output.stdout.len() < DEFAULT_OUTPUT_LIMIT_BYTES + 256,
            "live output not bounded: {}",
            output.stdout.len()
        );
        assert!(output.stdout.contains("bytes omitted"), "marker missing");
    }

    #[tokio::test]
    async fn shell_executor_refuses_dangerous_command() {
        let executor = ShellExecutor::new(None, None, ShellLauncher::default(), None);
        let error =
            executor.execute(ShellCommand::new("rm -rf /")).await.expect_err("dangerous command");
        assert!(matches!(error, ShellError::DangerousCommand(_)));
    }

    #[tokio::test]
    async fn direct_execution_reports_timeout() {
        // Match the launcher to the platform's command syntax so the process
        // actually runs long enough to hit the timeout.
        #[cfg(windows)]
        let (launcher, command) = (ShellLauncher::PowerShell, "Start-Sleep -Seconds 2");
        #[cfg(not(windows))]
        let (launcher, command) = (ShellLauncher::Bash, "sleep 2");

        let output = ShellExecutor::new(None, None, launcher, None)
            .execute(ShellCommand {
                command: command.to_string(),
                timeout_secs: 1,
                env: HashMap::new(),
            })
            .await
            .expect("timed out commands should return output");

        assert!(output.timed_out);
        // Both the sandbox driver and PassThroughDriver route timeouts through
        // `wait_for_child`, which reports the fixed timeout exit code (124,
        // GNU `timeout` convention) regardless of platform.
        assert_eq!(output.exit_code, 124);
    }

    #[test]
    fn shell_launcher_argv_per_variant() {
        // PowerShell argv now carries -NonInteractive (prevents prompt-hangs).
        let ps = ShellLauncher::PowerShell.resolve(None);
        assert_eq!(
            ps.argv("date +%A"),
            ["powershell.exe", "-NoLogo", "-NoProfile", "-NonInteractive", "-Command", "date +%A"]
        );

        // Bash argv shape: `<bash> --noprofile --norc -c <cmd>` (non-login,
        // non-rc — profile side effects must not leak into agent commands).
        let bash = ResolvedShell::Bash(PathBuf::from("/usr/local/bin/bash"));
        assert_eq!(
            bash.argv("echo hi"),
            ["/usr/local/bin/bash", "--noprofile", "--norc", "-c", "echo hi"]
        );

        let cmd = ShellLauncher::Cmd.resolve(None);
        assert_eq!(cmd.argv("dir"), ["cmd.exe", "/S", "/C", "dir"]);
    }

    #[test]
    fn resolve_bash_honors_explicit_existing_path_and_ignores_missing() {
        // A real file on disk is trusted as the explicit bash path.
        let exe = std::env::current_exe().expect("current exe");
        assert_eq!(resolve_bash(Some(exe.clone())), Some(exe));

        // A non-existent preferred path is ignored, never blindly trusted.
        let bogus = PathBuf::from("/does/not/exist/bash");
        assert_ne!(resolve_bash(Some(bogus.clone())), Some(bogus));
    }

    #[test]
    fn launcher_auto_resolves_family_from_explicit_bash_path() {
        // P1 regression: `Auto` must fold through the actual bash probing, not
        // a platform guess. An explicit existing bash path pins the family to
        // Bash even on Windows (where a hardcoded cfg! used to say PowerShell).
        let exe = std::env::current_exe().expect("current exe");
        assert_eq!(ShellLauncher::Auto.resolve_family(Some(exe)), ShellFamily::Bash);
        // No `Auto + no bash -> PowerShell` assertion here: on a machine with
        // Git Bash installed the well-known/PATH probes still resolve bash, so
        // the no-argument outcome is environment-dependent by design.
    }

    #[test]
    fn launcher_explicit_kinds_resolve_their_own_family() {
        assert_eq!(ShellLauncher::Bash.resolve_family(None), ShellFamily::Bash);
        assert_eq!(ShellLauncher::PowerShell.resolve_family(None), ShellFamily::PowerShell);
        assert_eq!(ShellLauncher::Cmd.resolve_family(None), ShellFamily::Cmd);
    }

    #[tokio::test]
    async fn execute_with_sink_threads_sink_into_command() {
        struct NoopSink;
        impl slab_sandboxing::OutputSink for NoopSink {
            fn on_output(&self, _stream: slab_sandboxing::OutputStream, _delta: &str) {}
        }
        let seen = Arc::new(Mutex::new(None));
        let executor = ShellExecutor::new(
            Some(PathBuf::from("workspace")),
            Some(Arc::new(RecordingDriver {
                seen: Arc::clone(&seen),
                output: slab_sandboxing::SandboxedOutput {
                    stdout: b"ok".to_vec(),
                    stderr: vec![],
                    exit_code: 0,
                    timed_out: false,
                },
            })),
            ShellLauncher::PowerShell,
            None,
        );
        let sink: Arc<dyn slab_sandboxing::OutputSink> = Arc::new(NoopSink);
        executor
            .execute_with_sink(
                ShellCommand {
                    command: "echo x".to_string(),
                    timeout_secs: 5,
                    env: HashMap::new(),
                },
                Some(sink),
            )
            .await
            .unwrap();
        let cmd = seen.lock().unwrap().clone().expect("driver command");
        assert!(cmd.output_sink.is_some(), "sink must be threaded into SandboxedCommand");
    }

    #[tokio::test]
    async fn execute_with_sink_streams_chunks_and_returns_full_output() {
        struct CapturingSink(Arc<Mutex<Vec<String>>>);
        impl slab_sandboxing::OutputSink for CapturingSink {
            fn on_output(&self, _stream: slab_sandboxing::OutputStream, delta: &str) {
                self.0.lock().unwrap().push(delta.to_string());
            }
        }
        let captured: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let sink: Arc<dyn slab_sandboxing::OutputSink> =
            Arc::new(CapturingSink(Arc::clone(&captured)));
        // No sandbox driver → PassThroughDriver path, which forwards via read_stream.
        let executor = ShellExecutor::new(None, None, ShellLauncher::Auto, None);
        let output = executor
            .execute_with_sink(
                ShellCommand {
                    command: "echo slab-stream-marker".to_string(),
                    timeout_secs: 10,
                    env: HashMap::new(),
                },
                Some(sink),
            )
            .await
            .expect("output");
        assert_eq!(output.exit_code, 0, "stderr: {}", output.stderr);
        let streamed: String = captured.lock().unwrap().concat();
        assert!(streamed.contains("slab-stream-marker"), "streamed = {streamed:?}");
        assert!(output.stdout.contains("slab-stream-marker"), "stdout = {}", output.stdout);
    }
}
