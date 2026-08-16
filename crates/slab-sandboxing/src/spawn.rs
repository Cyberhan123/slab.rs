//! Thin spawn helpers for one-shot, capture-stdout commands.
//!
//! The shell tool owns the streaming path (`OutputSink` + `ShellExecutor`). The
//! agent-driven fixed-argv spawns (`cargo check`, `git status`, …) only need a
//! captured `SandboxedOutput`, and they must not go through `ShellExecutor` —
//! their argv is already a real `[cargo, check, …]`, not a shell string, so the
//! shell launcher would wrongly wrap them. These helpers give those call sites a
//! uniform chokepoint over `SandboxDriver::run`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::driver::PassThroughDriver;
use crate::driver::{SandboxDriver, SandboxedCommand, SandboxedOutput};
use crate::error::SandboxError;

/// Build a capture-only `SandboxedCommand` and run it through `driver`.
///
/// `env` is empty so the child inherits the parent environment (lets `git` /
/// `cargo` resolve on `PATH`); `output_sink` is `None` (no live streaming — the
/// caller wants the fully accumulated output); `timeout` is `None` (caller
/// controls lifetime).
pub async fn spawn_sandboxed(
    driver: &dyn SandboxDriver,
    argv: Vec<String>,
    cwd: Option<PathBuf>,
) -> Result<SandboxedOutput, SandboxError> {
    let program = argv.first().cloned().unwrap_or_default();
    let args = if argv.len() > 1 { argv[1..].join(" ") } else { String::new() };
    slab_utils::log::SandboxAudit::new(
        slab_utils::log::AuditKind::Spawned,
        "slab-sandboxing::spawn",
    )
    .decision(slab_utils::log::AuditDecision::Allow)
    .program(&program)
    .args(&args)
    .record();

    let cmd = SandboxedCommand { argv, env: HashMap::new(), cwd, timeout: None, output_sink: None };
    match driver.run(cmd).await {
        Ok(output) => {
            let kind = if output.timed_out {
                slab_utils::log::AuditKind::Killed
            } else {
                slab_utils::log::AuditKind::Exited
            };
            let decision = if output.timed_out {
                slab_utils::log::AuditDecision::Deny
            } else {
                slab_utils::log::AuditDecision::Allow
            };
            slab_utils::log::SandboxAudit::new(kind, "slab-sandboxing::spawn")
                .decision(decision)
                .program(&program)
                .exit_code(output.exit_code)
                .record();
            Ok(output)
        }
        Err(err) => {
            slab_utils::log::SandboxAudit::new(
                slab_utils::log::AuditKind::SpawnFailed,
                "slab-sandboxing::spawn",
            )
            .decision(slab_utils::log::AuditDecision::Deny)
            .program(&program)
            .error(err.to_string())
            .record();
            Err(err)
        }
    }
}

/// Like [`spawn_sandboxed`], but accepts an optional driver.
///
/// `None` falls back to [`PassThroughDriver`] (no isolation) — the current
/// pre-sandbox behavior. This is the uniform call site for agent tools that hold
/// `Option<Arc<dyn SandboxDriver>>`.
pub async fn spawn_sandboxed_option(
    driver: Option<&Arc<dyn SandboxDriver>>,
    argv: Vec<String>,
    cwd: Option<PathBuf>,
) -> Result<SandboxedOutput, SandboxError> {
    match driver {
        Some(handle) => spawn_sandboxed(handle.as_ref(), argv, cwd).await,
        None => spawn_sandboxed(&PassThroughDriver, argv, cwd).await,
    }
}
