use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use eframe::egui::{self, Color32, RichText};

use crate::{
    BootstrapProgressSink, PreparedBootstrap, RunArgs, finalize_bootstrap,
    launch_prepared_bootstrap, prepare_bootstrap,
};

pub fn run_bootstrap_with_ui(args: RunArgs) -> Result<()> {
    #[cfg(not(windows))]
    {
        crate::run_bootstrap(args)
    }

    #[cfg(windows)]
    {
        run_bootstrap_with_ui_windows(args)
    }
}

#[cfg(windows)]
fn run_bootstrap_with_ui_windows(args: RunArgs) -> Result<()> {
    let state = Arc::new(SharedUiState::default());
    let worker_state = state.clone();
    let worker_args = args.clone();
    let worker = std::thread::Builder::new()
        .name("slab-installer-bootstrap".to_string())
        .spawn(move || {
            let mut progress = UiProgressSink::new(worker_state.clone());
            let outcome = match prepare_bootstrap(worker_args, &mut progress) {
                Ok(prepared) => WorkerOutcome::Success(prepared),
                Err(error) => WorkerOutcome::Error(format!("{error:#}")),
            };
            worker_state.finish(outcome);
        })
        .context("failed to start the bootstrap preparation thread")?;

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([420.0, 170.0])
            .with_min_inner_size([420.0, 170.0])
            .with_decorations(false)
            .with_resizable(false)
            .with_close_button(false)
            .with_minimize_button(false)
            .with_maximize_button(false),
        ..Default::default()
    };

    let window_state = state.clone();
    let ui_result = eframe::run_ui_native("Installing Slab", native_options, move |ui, _frame| {
        render_bootstrap_ui(ui, &window_state)
    });

    if ui_result.is_err() {
        state.request_cancel();
        let _ = worker.join();
        return crate::run_bootstrap(args);
    }

    if !state.is_finished() {
        state.request_cancel();
    }

    let worker_result = worker.join();
    let outcome = state.take_outcome();

    if worker_result.is_err() && outcome.is_none() {
        return Err(anyhow!("installer preparation thread panicked"));
    }

    match outcome {
        Some(WorkerOutcome::Success(prepared)) => {
            let result = launch_prepared_bootstrap(&prepared);
            finalize_bootstrap(prepared, args.keep_staging, result)
        }
        Some(WorkerOutcome::Error(message)) => Err(anyhow!(message)),
        None => Err(anyhow!("installer preparation ended unexpectedly")),
    }
}

#[cfg(windows)]
fn render_bootstrap_ui(ui: &mut egui::Ui, state: &Arc<SharedUiState>) {
    state.center_once(ui.ctx());
    let snapshot = state.snapshot();
    if matches!(snapshot.status, SnapshotStatus::Ready) {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
    }

    egui::Frame::central_panel(ui.style()).show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);
            ui.label(
                RichText::new("Preparing Slab Installer")
                    .size(22.0)
                    .strong()
                    .color(Color32::from_rgb(38, 70, 83)),
            );
            ui.add_space(8.0);

            let percent = (snapshot.progress_fraction * 100.0).round() as u32;
            ui.label(
                RichText::new(format!("{percent}%"))
                    .size(28.0)
                    .strong()
                    .color(Color32::from_rgb(42, 157, 143)),
            );

            ui.add(
                egui::ProgressBar::new(snapshot.progress_fraction)
                    .desired_width(320.0)
                    .show_percentage(),
            );

            ui.add_space(6.0);
            ui.label(RichText::new(snapshot.stage.clone()).strong());
            ui.label(snapshot.detail.clone());

            if let SnapshotStatus::Error(message) = snapshot.status {
                ui.add_space(8.0);
                ui.colored_label(Color32::from_rgb(175, 57, 51), message);
                ui.add_space(6.0);
                if ui.button("Close").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        });
    });

    ui.ctx().request_repaint_after(Duration::from_millis(50));
}

#[cfg(windows)]
#[derive(Clone, Debug)]
struct UiProgress {
    stage: String,
    detail: String,
    completed_bytes: u64,
    total_bytes: u64,
}

#[cfg(windows)]
impl Default for UiProgress {
    fn default() -> Self {
        Self {
            stage: "Preparing installer".to_string(),
            detail: "Starting bootstrap".to_string(),
            completed_bytes: 0,
            total_bytes: 1,
        }
    }
}

#[cfg(windows)]
#[derive(Debug)]
enum WorkerOutcome {
    Success(PreparedBootstrap),
    Error(String),
}

#[cfg(windows)]
#[derive(Clone, Debug)]
enum SnapshotStatus {
    Running,
    Ready,
    Error(String),
}

#[cfg(windows)]
#[derive(Clone, Debug)]
struct UiSnapshot {
    stage: String,
    detail: String,
    progress_fraction: f32,
    status: SnapshotStatus,
}

#[cfg(windows)]
#[derive(Default)]
struct SharedUiState {
    progress: Mutex<UiProgress>,
    outcome: Mutex<Option<WorkerOutcome>>,
    cancel_requested: AtomicBool,
    centered: AtomicBool,
}

#[cfg(windows)]
impl SharedUiState {
    fn update_progress(
        &self,
        stage: Option<String>,
        detail: Option<String>,
        completed_bytes: Option<u64>,
        total_bytes: Option<u64>,
    ) {
        let mut progress = self.progress.lock().expect("bootstrap UI progress mutex poisoned");
        if let Some(stage) = stage {
            progress.stage = stage;
        }
        if let Some(detail) = detail {
            progress.detail = detail;
        }
        if let Some(completed_bytes) = completed_bytes {
            progress.completed_bytes = completed_bytes;
        }
        if let Some(total_bytes) = total_bytes {
            progress.total_bytes = total_bytes.max(1);
        }
    }

    fn finish(&self, outcome: WorkerOutcome) {
        let mut slot = self.outcome.lock().expect("bootstrap UI outcome mutex poisoned");
        *slot = Some(outcome);
    }

    fn snapshot(&self) -> UiSnapshot {
        let progress = self.progress.lock().expect("bootstrap UI progress mutex poisoned").clone();
        let status =
            match self.outcome.lock().expect("bootstrap UI outcome mutex poisoned").as_ref() {
                Some(WorkerOutcome::Success(_)) => SnapshotStatus::Ready,
                Some(WorkerOutcome::Error(message)) => SnapshotStatus::Error(message.clone()),
                None => SnapshotStatus::Running,
            };
        UiSnapshot {
            stage: progress.stage,
            detail: progress.detail,
            progress_fraction: progress.completed_bytes as f32 / progress.total_bytes as f32,
            status,
        }
    }

    fn take_outcome(&self) -> Option<WorkerOutcome> {
        self.outcome.lock().expect("bootstrap UI outcome mutex poisoned").take()
    }

    fn request_cancel(&self) {
        self.cancel_requested.store(true, Ordering::Relaxed);
    }

    fn is_cancel_requested(&self) -> bool {
        self.cancel_requested.load(Ordering::Relaxed)
    }

    fn is_finished(&self) -> bool {
        self.outcome.lock().expect("bootstrap UI outcome mutex poisoned").is_some()
    }

    fn center_once(&self, ctx: &egui::Context) {
        if self.centered.swap(true, Ordering::Relaxed) {
            return;
        }
        if let Some(command) = egui::ViewportCommand::center_on_screen(ctx) {
            ctx.send_viewport_cmd(command);
        }
    }
}

#[cfg(windows)]
struct UiProgressSink {
    state: Arc<SharedUiState>,
    completed_bytes: u64,
    total_bytes: u64,
}

#[cfg(windows)]
impl UiProgressSink {
    fn new(state: Arc<SharedUiState>) -> Self {
        Self { state, completed_bytes: 0, total_bytes: 1 }
    }

    fn check_cancelled(&self) -> Result<()> {
        if self.state.is_cancel_requested() {
            return Err(anyhow!("installer preparation was cancelled"));
        }
        Ok(())
    }
}

#[cfg(windows)]
impl BootstrapProgressSink for UiProgressSink {
    fn set_total_bytes(&mut self, total_bytes: u64) {
        self.total_bytes = total_bytes.max(1);
        self.state.update_progress(None, None, Some(self.completed_bytes), Some(self.total_bytes));
    }

    fn set_stage(&mut self, stage: impl Into<String>, detail: impl Into<String>) {
        self.state.update_progress(
            Some(stage.into()),
            Some(detail.into()),
            Some(self.completed_bytes),
            Some(self.total_bytes),
        );
    }

    fn advance(&mut self, bytes: u64) -> Result<()> {
        self.check_cancelled()?;
        self.completed_bytes = self.completed_bytes.saturating_add(bytes).min(self.total_bytes);
        self.state.update_progress(None, None, Some(self.completed_bytes), Some(self.total_bytes));
        Ok(())
    }
}
