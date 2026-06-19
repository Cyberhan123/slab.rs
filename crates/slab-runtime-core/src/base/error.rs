use thiserror::Error;

pub const RUNTIME_ERROR_CODE_METADATA: &str = "x-slab-runtime-error-code";
pub const RUNTIME_ERROR_DETAIL_METADATA_BIN: &str = "x-slab-runtime-error-detail-bin";

/// Backend-facing error type for worker registration, admission, control, and
/// engine adapters.
///
/// This type deliberately does not model runtime application/domain failures
/// such as model resolution, public task lifecycle, or transport mapping.
/// Those concerns belong in `bin/slab-runtime` and higher layers.
#[derive(Debug, Clone, Error)]
pub enum CoreError {
    /// The ingress queue for the named backend is at capacity.
    #[error("queue full: {queue} (capacity {capacity})")]
    QueueFull { queue: String, capacity: usize },

    /// All admission permits for the backend are held; request denied.
    #[error("backend busy: {backend_id}")]
    Busy { backend_id: String },

    /// The backend worker channel closed unexpectedly.
    #[error("backend worker shutdown")]
    BackendShutdown,

    /// A timed wait exceeded its deadline.
    #[error("operation timed out")]
    Timeout,

    /// Requested operation is not implemented for a backend.
    #[error("unsupported operation '{op}' for backend '{backend}'")]
    UnsupportedOperation { backend: String, op: String },

    /// The requested driver is not registered in the runtime.
    #[error("driver not registered: {driver_id}")]
    DriverNotRegistered { driver_id: String },

    /// An internal lock was poisoned, indicating a thread panic during access.
    #[error("internal lock poisoned: {lock_name}")]
    InternalPoisoned { lock_name: String },

    /// An I/O error raised by an engine backend.
    #[error("engine I/O error: {0}")]
    EngineIo(String),

    /// An error raised by a GGML engine backend.
    #[error("GGML engine error in {component}: {message}")]
    GGMLEngine { component: String, message: String },

    /// An error raised by the ONNX Runtime engine backend.
    #[error("ONNX engine error: {0}")]
    OnnxEngine(String),

    /// An error raised by a Candle engine backend.
    #[error("Candle engine error in {component}: {message}")]
    CandleEngine { component: String, message: String },
}

impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        CoreError::EngineIo(e.to_string())
    }
}

impl CoreError {
    pub fn runtime_code(&self) -> &'static str {
        match self {
            Self::QueueFull { .. } => "runtime_queue_full",
            Self::Busy { .. } => "runtime_backend_busy",
            Self::BackendShutdown => "runtime_backend_shutdown",
            Self::Timeout => "runtime_timeout",
            Self::UnsupportedOperation { .. } => "runtime_unsupported_operation",
            Self::DriverNotRegistered { .. } => "runtime_driver_not_registered",
            Self::InternalPoisoned { .. } => "runtime_internal_poisoned",
            Self::EngineIo(_) => "runtime_engine_io",
            Self::GGMLEngine { .. } => "runtime_ggml_engine",
            Self::OnnxEngine(_) => "runtime_onnx_engine",
            Self::CandleEngine { .. } => "runtime_candle_engine",
        }
    }

    pub fn runtime_detail(&self) -> serde_json::Value {
        match self {
            Self::QueueFull { queue, capacity } => serde_json::json!({
                "queue": queue,
                "capacity": capacity,
                "message": self.to_string(),
            }),
            Self::Busy { backend_id } => serde_json::json!({
                "backend_id": backend_id,
                "message": self.to_string(),
            }),
            Self::BackendShutdown | Self::Timeout => serde_json::json!({
                "message": self.to_string(),
            }),
            Self::UnsupportedOperation { backend, op } => serde_json::json!({
                "backend": backend,
                "operation": op,
                "message": self.to_string(),
            }),
            Self::DriverNotRegistered { driver_id } => serde_json::json!({
                "driver_id": driver_id,
                "message": self.to_string(),
            }),
            Self::InternalPoisoned { lock_name } => serde_json::json!({
                "lock_name": lock_name,
                "message": self.to_string(),
            }),
            Self::EngineIo(message) | Self::OnnxEngine(message) => serde_json::json!({
                "message": message,
            }),
            Self::GGMLEngine { component, message } | Self::CandleEngine { component, message } => {
                serde_json::json!({
                    "component": component,
                    "message": message,
                })
            }
        }
    }
}
