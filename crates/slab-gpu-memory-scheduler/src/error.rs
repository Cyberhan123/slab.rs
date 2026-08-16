//! Error surface for GPU memory probing and scheduling.

/// Failures raised by the probe and scheduler machinery. None of these are
/// fatal to callers — the scheduler degrades to last-good snapshots.
#[derive(Debug, thiserror::Error)]
pub enum GpuMemoryError {
    #[error("GPU telemetry backend is disabled in this build")]
    TelemetryDisabled,
    #[error("GPU probe failed: {message}")]
    Probe { message: String },
    #[error("GPU probe worker panicked")]
    WorkerPanic,
}

impl GpuMemoryError {
    /// Legacy-compatible snapshot error text, matching the strings the
    /// `/v1/system/gpu` endpoint has always surfaced.
    pub fn snapshot_error_message(&self) -> String {
        match self {
            Self::TelemetryDisabled => "GPU telemetry backend is disabled in this build".to_owned(),
            Self::Probe { message } => format!("GPU telemetry unavailable: {message}"),
            Self::WorkerPanic => "GPU telemetry worker failed".to_owned(),
        }
    }
}
