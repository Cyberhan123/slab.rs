use std::collections::{HashMap, HashSet};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use async_trait::async_trait;
use slab_types::RuntimeBackendId;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use crate::error::AppCoreError;
use crate::launch::{ResolvedLaunchSpec, ResolvedRuntimeChildSpec};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeChildExit {
    pub code: Option<i32>,
    pub signal: Option<i32>,
    pub message: Option<String>,
}

impl RuntimeChildExit {
    pub fn success() -> Self {
        Self { code: Some(0), signal: None, message: None }
    }

    pub fn description(&self) -> String {
        if let Some(message) =
            self.message.as_deref().map(str::trim).filter(|value| !value.is_empty())
        {
            return message.to_owned();
        }

        match (self.signal, self.code) {
            (Some(signal), Some(code)) => format!("signal {signal}, code {code}"),
            (Some(signal), None) => format!("signal {signal}"),
            (None, Some(code)) => format!("code {code}"),
            (None, None) => "unknown exit status".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnexpectedRuntimeExit {
    pub backend: RuntimeBackendId,
    pub bind_address: String,
    pub exit: RuntimeChildExit,
    pub consecutive_failures: usize,
    pub restart_delay: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeBackendRuntimeStatus {
    Disabled,
    Ready,
    Restarting,
    Unavailable,
}

impl RuntimeBackendRuntimeStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Ready => "ready",
            Self::Restarting => "restarting",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeBackendStatusSnapshot {
    pub status: RuntimeBackendRuntimeStatus,
    pub consecutive_failures: usize,
    pub restart_attempts: usize,
    pub last_error: Option<String>,
    pub last_unexpected_exit: Option<UnexpectedRuntimeExit>,
}

impl RuntimeBackendStatusSnapshot {
    fn new(status: RuntimeBackendRuntimeStatus) -> Self {
        Self {
            status,
            consecutive_failures: 0,
            restart_attempts: 0,
            last_error: None,
            last_unexpected_exit: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RuntimeSupervisorStatus {
    inner: Arc<Mutex<HashMap<RuntimeBackendId, RuntimeBackendStatusSnapshot>>>,
}

impl RuntimeSupervisorStatus {
    pub fn from_launch_spec(spec: &ResolvedLaunchSpec) -> Self {
        let managed: HashSet<_> = spec.children.iter().map(|child| child.backend).collect();
        let mut states = HashMap::new();

        for backend in RuntimeBackendId::ALL {
            let status = if managed.contains(&backend) {
                RuntimeBackendRuntimeStatus::Restarting
            } else if spec.endpoints.backend_endpoint(backend).is_some() {
                RuntimeBackendRuntimeStatus::Ready
            } else {
                RuntimeBackendRuntimeStatus::Disabled
            };
            states.insert(backend, RuntimeBackendStatusSnapshot::new(status));
        }

        Self { inner: Arc::new(Mutex::new(states)) }
    }

    pub fn snapshot(&self, backend: RuntimeBackendId) -> RuntimeBackendStatusSnapshot {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(&backend)
            .cloned()
            .unwrap_or_else(|| {
                RuntimeBackendStatusSnapshot::new(RuntimeBackendRuntimeStatus::Disabled)
            })
    }

    pub fn status(&self, backend: RuntimeBackendId) -> RuntimeBackendRuntimeStatus {
        self.snapshot(backend).status
    }

    fn mark_ready(&self, backend: RuntimeBackendId, restart_attempts: usize) {
        let mut guard = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        let entry = guard.entry(backend).or_insert_with(|| {
            RuntimeBackendStatusSnapshot::new(RuntimeBackendRuntimeStatus::Disabled)
        });
        entry.status = RuntimeBackendRuntimeStatus::Ready;
        entry.restart_attempts = restart_attempts;
        entry.last_error = None;
    }

    fn mark_disabled(&self, backend: RuntimeBackendId) {
        let mut guard = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        let entry = guard.entry(backend).or_insert_with(|| {
            RuntimeBackendStatusSnapshot::new(RuntimeBackendRuntimeStatus::Disabled)
        });
        entry.status = RuntimeBackendRuntimeStatus::Disabled;
        entry.last_error = None;
    }

    fn mark_failure(
        &self,
        backend: RuntimeBackendId,
        status: RuntimeBackendRuntimeStatus,
        consecutive_failures: usize,
        restart_attempts: usize,
        error: String,
        unexpected_exit: UnexpectedRuntimeExit,
    ) {
        let mut guard = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        let entry = guard.entry(backend).or_insert_with(|| {
            RuntimeBackendStatusSnapshot::new(RuntimeBackendRuntimeStatus::Disabled)
        });
        entry.status = status;
        entry.consecutive_failures = consecutive_failures;
        entry.restart_attempts = restart_attempts;
        entry.last_error = Some(error);
        entry.last_unexpected_exit = Some(unexpected_exit);
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeSupervisorOptions {
    pub restart_backoff: Vec<Duration>,
    pub max_restart_delay: Duration,
    pub unavailable_after_consecutive_failures: usize,
    pub stable_uptime_before_failure_reset: Duration,
    pub graceful_shutdown_timeout: Duration,
    pub force_shutdown_timeout: Duration,
}

impl Default for RuntimeSupervisorOptions {
    fn default() -> Self {
        Self {
            restart_backoff: vec![
                Duration::from_secs(1),
                Duration::from_secs(2),
                Duration::from_secs(4),
                Duration::from_secs(8),
                Duration::from_secs(16),
            ],
            max_restart_delay: Duration::from_secs(30),
            unavailable_after_consecutive_failures: 5,
            stable_uptime_before_failure_reset: Duration::from_secs(60),
            graceful_shutdown_timeout: Duration::from_secs(5),
            force_shutdown_timeout: Duration::from_secs(5),
        }
    }
}

impl RuntimeSupervisorOptions {
    fn restart_delay(&self, consecutive_failures: usize) -> Duration {
        if consecutive_failures == 0 {
            return Duration::ZERO;
        }

        self.restart_backoff
            .get(consecutive_failures.saturating_sub(1))
            .copied()
            .unwrap_or(self.max_restart_delay)
    }

    fn status_for_failures(&self, consecutive_failures: usize) -> RuntimeBackendRuntimeStatus {
        if consecutive_failures >= self.unavailable_after_consecutive_failures {
            RuntimeBackendRuntimeStatus::Unavailable
        } else {
            RuntimeBackendRuntimeStatus::Restarting
        }
    }
}

#[async_trait]
pub trait RuntimeChildHandle: Send {
    async fn wait_for_exit(&mut self) -> Result<RuntimeChildExit, AppCoreError>;
    async fn request_graceful_shutdown(&mut self) -> Result<(), AppCoreError>;
    async fn force_kill(&mut self) -> Result<(), AppCoreError>;
}

#[async_trait]
pub trait RuntimeChildSpawner: Send + Sync {
    async fn spawn_child(
        &self,
        child_spec: &ResolvedRuntimeChildSpec,
    ) -> Result<Box<dyn RuntimeChildHandle>, AppCoreError>;
}

pub struct ManagedRuntimeSupervisor {
    launch_spec: ResolvedLaunchSpec,
    status: Arc<RuntimeSupervisorStatus>,
    shutdown_tx: watch::Sender<bool>,
    tasks: Mutex<Vec<JoinHandle<()>>>,
    shutdown_started: AtomicBool,
}

impl ManagedRuntimeSupervisor {
    pub async fn start(
        launch_spec: ResolvedLaunchSpec,
        spawner: Arc<dyn RuntimeChildSpawner>,
        options: RuntimeSupervisorOptions,
    ) -> Result<Self, AppCoreError> {
        let status = Arc::new(RuntimeSupervisorStatus::from_launch_spec(&launch_spec));
        let mut started_children = Vec::new();

        for child_spec in &launch_spec.children {
            match spawner.spawn_child(child_spec).await {
                Ok(handle) => {
                    status.mark_ready(child_spec.backend, 0);
                    info!(
                        backend = child_spec.backend.canonical_id(),
                        bind_address = %child_spec.grpc_bind_address,
                        transport = child_spec.transport.as_str(),
                        queue_capacity = child_spec.queue_capacity,
                        backend_capacity = child_spec.backend_capacity,
                        log_file = %child_spec.log_file.display(),
                        shutdown_on_stdin_close = child_spec.shutdown_on_stdin_close,
                        "runtime child started"
                    );
                    started_children.push((child_spec.clone(), handle));
                }
                Err(error) => {
                    error!(
                        backend = child_spec.backend.canonical_id(),
                        bind_address = %child_spec.grpc_bind_address,
                        log_file = %child_spec.log_file.display(),
                        error = %error,
                        "failed to start runtime child"
                    );
                    rollback_started_children(&mut started_children, &options).await;
                    return Err(error);
                }
            }
        }

        let (shutdown_tx, _) = watch::channel(false);
        let mut tasks = Vec::new();
        for (child_spec, handle) in started_children {
            tasks.push(tokio::spawn(supervise_backend(
                child_spec,
                handle,
                Arc::clone(&spawner),
                Arc::clone(&status),
                shutdown_tx.subscribe(),
                options.clone(),
            )));
        }

        Ok(Self {
            launch_spec,
            status,
            shutdown_tx,
            tasks: Mutex::new(tasks),
            shutdown_started: AtomicBool::new(false),
        })
    }

    pub fn launch_spec(&self) -> &ResolvedLaunchSpec {
        &self.launch_spec
    }

    pub fn status_registry(&self) -> Arc<RuntimeSupervisorStatus> {
        Arc::clone(&self.status)
    }

    pub fn trigger_shutdown(&self) {
        if self.shutdown_started.swap(true, Ordering::SeqCst) {
            return;
        }
        let _ = self.shutdown_tx.send(true);
    }

    pub async fn shutdown(&self) {
        self.trigger_shutdown();

        let handles = {
            let mut guard = self.tasks.lock().unwrap_or_else(|error| error.into_inner());
            std::mem::take(&mut *guard)
        };

        for handle in handles {
            if let Err(error) = handle.await {
                warn!(error = %error, "runtime supervisor task join failed");
            }
        }

        for child_spec in &self.launch_spec.children {
            self.status.mark_disabled(child_spec.backend);
        }
    }
}

async fn rollback_started_children(
    children: &mut Vec<(ResolvedRuntimeChildSpec, Box<dyn RuntimeChildHandle>)>,
    options: &RuntimeSupervisorOptions,
) {
    while let Some((child_spec, mut handle)) = children.pop() {
        shutdown_child(&child_spec, &mut handle, options).await;
    }
}

async fn supervise_backend(
    child_spec: ResolvedRuntimeChildSpec,
    mut handle: Box<dyn RuntimeChildHandle>,
    spawner: Arc<dyn RuntimeChildSpawner>,
    status: Arc<RuntimeSupervisorStatus>,
    mut shutdown_rx: watch::Receiver<bool>,
    options: RuntimeSupervisorOptions,
) {
    let backend = child_spec.backend;
    let bind_address = child_spec.grpc_bind_address.clone();
    let mut consecutive_failures = 0usize;
    let mut restart_attempts = 0usize;
    let mut last_started_at = tokio::time::Instant::now();

    loop {
        tokio::select! {
            changed = shutdown_rx.changed() => {
                if changed.is_err() || *shutdown_rx.borrow() {
                    shutdown_child(&child_spec, &mut handle, &options).await;
                    return;
                }
            }
            exit = handle.wait_for_exit() => {
                let exit = match exit {
                    Ok(exit) => exit,
                    Err(error) => RuntimeChildExit {
                        code: None,
                        signal: None,
                        message: Some(format!("failed waiting for runtime child exit: {error}")),
                    },
                };

                if last_started_at.elapsed() >= options.stable_uptime_before_failure_reset {
                    consecutive_failures = 0;
                }
                consecutive_failures = consecutive_failures.saturating_add(1);
                let restart_delay = options.restart_delay(consecutive_failures);
                let transition_status = options.status_for_failures(consecutive_failures);
                let exit_detail = exit.description();

                status.mark_failure(
                    backend,
                    transition_status,
                    consecutive_failures,
                    restart_attempts,
                    format!("runtime child exited unexpectedly: {exit_detail}"),
                    UnexpectedRuntimeExit {
                        backend,
                        bind_address: bind_address.clone(),
                        exit: exit.clone(),
                        consecutive_failures,
                        restart_delay,
                    },
                );
                warn!(
                    backend = backend.canonical_id(),
                    bind_address = %bind_address,
                    consecutive_failures,
                    restart_delay_ms = restart_delay.as_millis(),
                    exit = %exit_detail,
                    log_file = %child_spec.log_file.display(),
                    "runtime child exited unexpectedly; scheduling restart"
                );

                let mut retry_delay = restart_delay;
                loop {
                    tokio::select! {
                        changed = shutdown_rx.changed() => {
                            if changed.is_err() || *shutdown_rx.borrow() {
                                return;
                            }
                        }
                        _ = tokio::time::sleep(retry_delay) => {}
                    }

                    match spawner.spawn_child(&child_spec).await {
                        Ok(new_handle) => {
                            handle = new_handle;
                            last_started_at = tokio::time::Instant::now();
                            restart_attempts = restart_attempts.saturating_add(1);
                            status.mark_ready(backend, restart_attempts);
                            info!(
                                backend = backend.canonical_id(),
                                bind_address = %bind_address,
                                consecutive_failures,
                                restart_attempts,
                                log_file = %child_spec.log_file.display(),
                                "runtime child restarted"
                            );
                            break;
                        }
                        Err(error) => {
                            consecutive_failures = consecutive_failures.saturating_add(1);
                            retry_delay = options.restart_delay(consecutive_failures);
                            let transition_status = options.status_for_failures(consecutive_failures);
                            let error_text = error.to_string();
                            status.mark_failure(
                                backend,
                                transition_status,
                                consecutive_failures,
                                restart_attempts,
                                format!("failed to restart runtime child: {error_text}"),
                                UnexpectedRuntimeExit {
                                    backend,
                                    bind_address: bind_address.clone(),
                                    exit: RuntimeChildExit {
                                        code: None,
                                        signal: None,
                                        message: Some(format!("restart spawn failed: {error_text}")),
                                    },
                                    consecutive_failures,
                                    restart_delay: retry_delay,
                                },
                            );
                            warn!(
                                backend = backend.canonical_id(),
                                bind_address = %bind_address,
                                consecutive_failures,
                                retry_delay_ms = retry_delay.as_millis(),
                                error = %error_text,
                                log_file = %child_spec.log_file.display(),
                                "failed to restart runtime child; will retry"
                            );
                        }
                    }
                }
            }
        }
    }
}

async fn shutdown_child(
    child_spec: &ResolvedRuntimeChildSpec,
    handle: &mut Box<dyn RuntimeChildHandle>,
    options: &RuntimeSupervisorOptions,
) {
    let backend = child_spec.backend.canonical_id();
    let bind_address = &child_spec.grpc_bind_address;

    if let Err(error) = handle.request_graceful_shutdown().await {
        warn!(
            backend,
            bind_address = %bind_address,
            log_file = %child_spec.log_file.display(),
            error = %error,
            "failed to request graceful runtime shutdown"
        );
    }

    match tokio::time::timeout(options.graceful_shutdown_timeout, handle.wait_for_exit()).await {
        Ok(Ok(exit)) => {
            info!(
                backend,
                bind_address = %bind_address,
                log_file = %child_spec.log_file.display(),
                exit = %exit.description(),
                "runtime child exited during graceful shutdown"
            );
            return;
        }
        Ok(Err(error)) => {
            warn!(
                backend,
                bind_address = %bind_address,
                log_file = %child_spec.log_file.display(),
                error = %error,
                "failed while waiting for graceful runtime shutdown"
            );
        }
        Err(_) => {
            warn!(
                backend,
                bind_address = %bind_address,
                log_file = %child_spec.log_file.display(),
                timeout_ms = options.graceful_shutdown_timeout.as_millis(),
                "timed out waiting for graceful runtime shutdown"
            );
        }
    }

    if let Err(error) = handle.force_kill().await {
        error!(
            backend,
            bind_address = %bind_address,
            log_file = %child_spec.log_file.display(),
            error = %error,
            "failed to force kill runtime child"
        );
        return;
    }

    match tokio::time::timeout(options.force_shutdown_timeout, handle.wait_for_exit()).await {
        Ok(Ok(exit)) => {
            warn!(
                backend,
                bind_address = %bind_address,
                log_file = %child_spec.log_file.display(),
                exit = %exit.description(),
                "runtime child exited after force kill"
            );
        }
        Ok(Err(error)) => {
            warn!(
                backend,
                bind_address = %bind_address,
                log_file = %child_spec.log_file.display(),
                error = %error,
                "failed while waiting for forced runtime shutdown"
            );
        }
        Err(_) => {
            error!(
                backend,
                bind_address = %bind_address,
                log_file = %child_spec.log_file.display(),
                timeout_ms = options.force_shutdown_timeout.as_millis(),
                "timed out waiting for runtime child after force kill"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::atomic::AtomicUsize;

    use tokio::sync::oneshot;

    use crate::launch::{LaunchProfile, ResolvedGatewaySpec, ResolvedRuntimeEndpoints};

    #[derive(Debug)]
    struct FakeChildControl {
        exit_tx: Mutex<Option<oneshot::Sender<RuntimeChildExit>>>,
        graceful_shutdowns: AtomicUsize,
        force_kills: AtomicUsize,
    }

    impl FakeChildControl {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                exit_tx: Mutex::new(None),
                graceful_shutdowns: AtomicUsize::new(0),
                force_kills: AtomicUsize::new(0),
            })
        }

        fn install_sender(&self, sender: oneshot::Sender<RuntimeChildExit>) {
            let mut guard = self.exit_tx.lock().unwrap_or_else(|error| error.into_inner());
            *guard = Some(sender);
        }

        fn exit(&self, exit: RuntimeChildExit) {
            let sender = self.exit_tx.lock().unwrap_or_else(|error| error.into_inner()).take();
            if let Some(sender) = sender {
                let _ = sender.send(exit);
            }
        }
    }

    struct FakeChildHandle {
        control: Arc<FakeChildControl>,
        exit_rx: Option<oneshot::Receiver<RuntimeChildExit>>,
        graceful_exit: RuntimeChildExit,
        force_exit: RuntimeChildExit,
    }

    #[async_trait]
    impl RuntimeChildHandle for FakeChildHandle {
        async fn wait_for_exit(&mut self) -> Result<RuntimeChildExit, AppCoreError> {
            let exit = self
                .exit_rx
                .as_mut()
                .ok_or_else(|| AppCoreError::Internal("fake exit receiver missing".to_owned()))?
                .await
                .map_err(|_| AppCoreError::Internal("fake child exit sender dropped".to_owned()))?;
            self.exit_rx = None;
            Ok(exit)
        }

        async fn request_graceful_shutdown(&mut self) -> Result<(), AppCoreError> {
            self.control.graceful_shutdowns.fetch_add(1, Ordering::SeqCst);
            self.control.exit(self.graceful_exit.clone());
            Ok(())
        }

        async fn force_kill(&mut self) -> Result<(), AppCoreError> {
            self.control.force_kills.fetch_add(1, Ordering::SeqCst);
            self.control.exit(self.force_exit.clone());
            Ok(())
        }
    }

    enum SpawnPlan {
        Success(Arc<FakeChildControl>),
        Fail(String),
    }

    #[derive(Clone, Default)]
    struct FakeSpawner {
        plans: Arc<Mutex<HashMap<RuntimeBackendId, VecDeque<SpawnPlan>>>>,
        spawned: Arc<Mutex<Vec<Arc<FakeChildControl>>>>,
    }

    impl FakeSpawner {
        fn push_success(&self, backend: RuntimeBackendId, control: Arc<FakeChildControl>) {
            self.plans
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .entry(backend)
                .or_default()
                .push_back(SpawnPlan::Success(control));
        }

        fn push_failure(&self, backend: RuntimeBackendId, error: impl Into<String>) {
            self.plans
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .entry(backend)
                .or_default()
                .push_back(SpawnPlan::Fail(error.into()));
        }

        fn spawn_count(&self) -> usize {
            self.spawned.lock().unwrap_or_else(|error| error.into_inner()).len()
        }
    }

    #[async_trait]
    impl RuntimeChildSpawner for FakeSpawner {
        async fn spawn_child(
            &self,
            child_spec: &ResolvedRuntimeChildSpec,
        ) -> Result<Box<dyn RuntimeChildHandle>, AppCoreError> {
            let plan = self
                .plans
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .get_mut(&child_spec.backend)
                .and_then(VecDeque::pop_front)
                .ok_or_else(|| {
                    AppCoreError::Internal(format!(
                        "no fake spawn plan configured for backend {}",
                        child_spec.backend.canonical_id()
                    ))
                })?;

            match plan {
                SpawnPlan::Success(control) => {
                    let (exit_tx, exit_rx) = oneshot::channel();
                    control.install_sender(exit_tx);
                    self.spawned
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .push(Arc::clone(&control));
                    Ok(Box::new(FakeChildHandle {
                        control,
                        exit_rx: Some(exit_rx),
                        graceful_exit: RuntimeChildExit::success(),
                        force_exit: RuntimeChildExit {
                            code: Some(-9),
                            signal: None,
                            message: Some("forced kill".to_owned()),
                        },
                    }))
                }
                SpawnPlan::Fail(error) => Err(AppCoreError::Internal(error)),
            }
        }
    }

    fn launch_spec(children: Vec<ResolvedRuntimeChildSpec>) -> ResolvedLaunchSpec {
        let mut endpoints = ResolvedRuntimeEndpoints::default();
        for child in &children {
            match child.backend {
                RuntimeBackendId::GgmlWhisper => {
                    endpoints.whisper = Some(child.grpc_bind_address.clone())
                }
                RuntimeBackendId::GgmlLlama => {
                    endpoints.llama = Some(child.grpc_bind_address.clone())
                }
                RuntimeBackendId::GgmlDiffusion => {
                    endpoints.diffusion = Some(child.grpc_bind_address.clone())
                }
                _ => {}
            }
        }

        ResolvedLaunchSpec {
            profile: LaunchProfile::Desktop,
            transport: slab_types::settings::RuntimeTransportMode::Http,
            runtime_log_dir: "C:/runtime/logs".into(),
            runtime_ipc_dir: None,
            extra_dirs: Vec::new(),
            children,
            endpoints,
            gateway: Some(ResolvedGatewaySpec { bind_address: "127.0.0.1:3000".to_owned() }),
        }
    }

    fn child_spec(backend: RuntimeBackendId, bind_address: &str) -> ResolvedRuntimeChildSpec {
        ResolvedRuntimeChildSpec {
            backend,
            grpc_bind_address: bind_address.to_owned(),
            transport: slab_types::settings::RuntimeTransportMode::Http,
            queue_capacity: 64,
            backend_capacity: 4,
            lib_dir: None,
            log_level: None,
            log_json: Some(false),
            log_file: format!("C:/runtime/logs/{}.log", backend.canonical_id()).into(),
            shutdown_on_stdin_close: true,
        }
    }

    async fn yield_until(predicate: impl Fn() -> bool) {
        for _ in 0..128 {
            if predicate() {
                return;
            }
            tokio::task::yield_now().await;
        }
        assert!(predicate(), "condition was not met in time");
    }

    async fn advance_and_settle(duration: Duration) {
        tokio::task::yield_now().await;
        tokio::time::advance(duration).await;
        tokio::task::yield_now().await;
    }

    fn test_options() -> RuntimeSupervisorOptions {
        RuntimeSupervisorOptions {
            restart_backoff: vec![Duration::from_secs(1)],
            max_restart_delay: Duration::from_secs(1),
            unavailable_after_consecutive_failures: 2,
            stable_uptime_before_failure_reset: Duration::from_secs(5),
            graceful_shutdown_timeout: Duration::from_secs(1),
            force_shutdown_timeout: Duration::from_secs(1),
        }
    }

    #[tokio::test]
    async fn startup_failure_rolls_back_started_children() {
        let spawner = FakeSpawner::default();
        let llama = FakeChildControl::new();
        spawner.push_success(RuntimeBackendId::GgmlLlama, Arc::clone(&llama));
        spawner.push_failure(RuntimeBackendId::GgmlWhisper, "boom");

        let result = ManagedRuntimeSupervisor::start(
            launch_spec(vec![
                child_spec(RuntimeBackendId::GgmlLlama, "127.0.0.1:50051"),
                child_spec(RuntimeBackendId::GgmlWhisper, "127.0.0.1:50052"),
            ]),
            Arc::new(spawner),
            RuntimeSupervisorOptions::default(),
        )
        .await;

        assert!(result.is_err());
        assert_eq!(llama.graceful_shutdowns.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn unexpected_exit_triggers_restart_with_backoff() {
        let spawner = FakeSpawner::default();
        let first = FakeChildControl::new();
        let second = FakeChildControl::new();
        spawner.push_success(RuntimeBackendId::GgmlLlama, Arc::clone(&first));
        spawner.push_success(RuntimeBackendId::GgmlLlama, Arc::clone(&second));

        let supervisor = ManagedRuntimeSupervisor::start(
            launch_spec(vec![child_spec(RuntimeBackendId::GgmlLlama, "127.0.0.1:50051")]),
            Arc::new(spawner.clone()),
            test_options(),
        )
        .await
        .unwrap();

        assert_eq!(
            supervisor.status_registry().status(RuntimeBackendId::GgmlLlama),
            RuntimeBackendRuntimeStatus::Ready
        );

        first.exit(RuntimeChildExit {
            code: Some(1),
            signal: None,
            message: Some("panic".to_owned()),
        });
        yield_until(|| {
            supervisor.status_registry().status(RuntimeBackendId::GgmlLlama)
                == RuntimeBackendRuntimeStatus::Restarting
        })
        .await;

        advance_and_settle(Duration::from_secs(1)).await;
        yield_until(|| spawner.spawn_count() == 2).await;

        assert_eq!(
            supervisor.status_registry().status(RuntimeBackendId::GgmlLlama),
            RuntimeBackendRuntimeStatus::Ready
        );
        supervisor.shutdown().await;
    }

    #[tokio::test(start_paused = true)]
    async fn repeated_failures_transition_to_unavailable_and_keep_retrying() {
        let spawner = FakeSpawner::default();
        let first = FakeChildControl::new();
        let recovered = FakeChildControl::new();
        spawner.push_success(RuntimeBackendId::GgmlLlama, Arc::clone(&first));
        spawner.push_failure(RuntimeBackendId::GgmlLlama, "restart-1 failed");
        spawner.push_failure(RuntimeBackendId::GgmlLlama, "restart-2 failed");
        spawner.push_success(RuntimeBackendId::GgmlLlama, Arc::clone(&recovered));

        let supervisor = ManagedRuntimeSupervisor::start(
            launch_spec(vec![child_spec(RuntimeBackendId::GgmlLlama, "127.0.0.1:50051")]),
            Arc::new(spawner.clone()),
            test_options(),
        )
        .await
        .unwrap();

        first.exit(RuntimeChildExit {
            code: Some(2),
            signal: None,
            message: Some("crash".to_owned()),
        });
        yield_until(|| {
            supervisor.status_registry().status(RuntimeBackendId::GgmlLlama)
                == RuntimeBackendRuntimeStatus::Restarting
        })
        .await;

        advance_and_settle(Duration::from_secs(1)).await;
        yield_until(|| {
            supervisor.status_registry().status(RuntimeBackendId::GgmlLlama)
                == RuntimeBackendRuntimeStatus::Unavailable
        })
        .await;

        advance_and_settle(Duration::from_secs(1)).await;
        advance_and_settle(Duration::from_secs(1)).await;
        yield_until(|| spawner.spawn_count() == 2).await;

        assert_eq!(
            supervisor.status_registry().status(RuntimeBackendId::GgmlLlama),
            RuntimeBackendRuntimeStatus::Ready
        );
        supervisor.shutdown().await;
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_does_not_trigger_restart() {
        let spawner = FakeSpawner::default();
        let first = FakeChildControl::new();
        spawner.push_success(RuntimeBackendId::GgmlLlama, Arc::clone(&first));

        let supervisor = ManagedRuntimeSupervisor::start(
            launch_spec(vec![child_spec(RuntimeBackendId::GgmlLlama, "127.0.0.1:50051")]),
            Arc::new(spawner.clone()),
            test_options(),
        )
        .await
        .unwrap();

        supervisor.shutdown().await;

        assert_eq!(spawner.spawn_count(), 1);
        assert_eq!(first.graceful_shutdowns.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn stable_run_resets_failure_window() {
        let spawner = FakeSpawner::default();
        let first = FakeChildControl::new();
        let second = FakeChildControl::new();
        let third = FakeChildControl::new();
        spawner.push_success(RuntimeBackendId::GgmlLlama, Arc::clone(&first));
        spawner.push_success(RuntimeBackendId::GgmlLlama, Arc::clone(&second));
        spawner.push_success(RuntimeBackendId::GgmlLlama, Arc::clone(&third));

        let supervisor = ManagedRuntimeSupervisor::start(
            launch_spec(vec![child_spec(RuntimeBackendId::GgmlLlama, "127.0.0.1:50051")]),
            Arc::new(spawner.clone()),
            test_options(),
        )
        .await
        .unwrap();

        first.exit(RuntimeChildExit {
            code: Some(1),
            signal: None,
            message: Some("first crash".to_owned()),
        });
        yield_until(|| {
            supervisor.status_registry().status(RuntimeBackendId::GgmlLlama)
                == RuntimeBackendRuntimeStatus::Restarting
        })
        .await;
        advance_and_settle(Duration::from_secs(1)).await;
        yield_until(|| spawner.spawn_count() == 2).await;

        advance_and_settle(Duration::from_secs(6)).await;
        second.exit(RuntimeChildExit {
            code: Some(1),
            signal: None,
            message: Some("second crash".to_owned()),
        });
        yield_until(|| {
            supervisor.status_registry().status(RuntimeBackendId::GgmlLlama)
                == RuntimeBackendRuntimeStatus::Restarting
        })
        .await;

        advance_and_settle(Duration::from_secs(1)).await;
        yield_until(|| spawner.spawn_count() == 3).await;
        assert_eq!(
            supervisor.status_registry().status(RuntimeBackendId::GgmlLlama),
            RuntimeBackendRuntimeStatus::Ready
        );
        supervisor.shutdown().await;
    }
}
