//! The executor trait + the non-elevated `JobOnlyExecutor` (today's behavior, moved here).
//! The elevated `ElevatedAclTokenExecutor` (restricted token + ACL + daemon) lands in S2b.

use std::process::Stdio;

use crate::capability::CapabilitySnapshot;
use crate::error::WindowsSandboxError;
use crate::job::JobHandle;
use crate::request::{SpawnRequest, SpawnedChild};

/// Produces isolated child processes on Windows. The thin shim in
/// `slab_sandboxing::platform::windows` holds one of these and delegates to it.
///
/// Trait evolution is additive: S2a only needs `capabilities` + `spawn_job_only`;
/// S2b adds `prepare` (elevation round-trip) and `spawn_elevated` (Low-IL restricted token).
pub trait WindowsSandboxExecutor: Send + Sync {
    /// Honest report of what this executor currently enforces.
    fn capabilities(&self) -> CapabilitySnapshot;

    /// Non-elevated spawn: build a `tokio::process::Child`, assign it to a Job Object, and
    /// return it with a tree-kill closure. The shim feeds both into the shared `wait_for_child`.
    fn spawn_job_only(&self, req: &SpawnRequest) -> Result<SpawnedChild, WindowsSandboxError>;
}

/// The non-elevated baseline executor: Job-Object tree-cleanup + (caller-applied) lexical guard.
/// Behaviorally identical to the pre-S2 `WindowsSandboxDriver`. This is the default until the
/// user opts into elevation (S2b).
pub struct JobOnlyExecutor {
    setup_required: bool,
}

impl JobOnlyExecutor {
    pub fn new(setup_required: bool) -> Self {
        Self { setup_required }
    }
}

impl WindowsSandboxExecutor for JobOnlyExecutor {
    fn capabilities(&self) -> CapabilitySnapshot {
        CapabilitySnapshot::job_only(self.setup_required)
    }

    fn spawn_job_only(&self, req: &SpawnRequest) -> Result<SpawnedChild, WindowsSandboxError> {
        let program = req.argv.first().ok_or(WindowsSandboxError::EmptyCommand)?;
        let mut command = tokio::process::Command::new(program);
        command.args(&req.argv[1..]);
        for (key, value) in &req.env {
            command.env(key, value);
        }
        if let Some(ref cwd) = req.cwd {
            command.current_dir(cwd);
        }
        command.kill_on_drop(true);
        command.stdin(Stdio::null());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        let spawned =
            command.spawn().map_err(|e| WindowsSandboxError::SpawnFailed(e.to_string()))?;
        let job = JobHandle::new()?;
        job.configure_kill_on_close()?;
        let process_handle = spawned.raw_handle().ok_or_else(|| {
            WindowsSandboxError::SetupFailed("spawned child has no process handle".to_string())
        })?;
        job.assign_process(process_handle as windows_sys::Win32::Foundation::HANDLE)?;

        tracing::debug!(pid = spawned.id(), "spawned process in Windows Job Object");
        // Dropping `job` fires KILL_ON_JOB_CLOSE → tree dies → pipes released. The shim's
        // `wait_for_child` invokes this closure right after the direct child exits.
        let kill_tree: Box<dyn FnOnce() + Send + 'static> = Box::new(move || drop(job));
        Ok(SpawnedChild { child: spawned, kill_tree: Some(kill_tree) })
    }
}
