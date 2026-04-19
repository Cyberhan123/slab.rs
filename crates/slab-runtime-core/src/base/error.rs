use thiserror::Error;

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

    /// The runtime detected split-brain risk after a failed global operation.
    #[error("global state is inconsistent (failed operation {op_id})")]
    GlobalStateInconsistent { op_id: u64 },

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
    #[error("GGML engine error: {0}")]
    GGMLEngine(String),

    /// An error raised by the ONNX Runtime engine backend.
    #[error("ONNX engine error: {0}")]
    OnnxEngine(String),

    /// An error raised by a Candle engine backend.
    #[error("Candle engine error: {0}")]
    CandleEngine(String),
}

impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        CoreError::EngineIo(e.to_string())
    }
}
