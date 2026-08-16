//! Unified logging support: a size-rotating, secret-redacting file appender
//! shared by `slab-server` and `slab-runtime`, plus a stderr-only tracing init
//! for minimal binaries.
//!
//! `slab-server` already shipped this appender locally; it has been lifted here
//! so both the HTTP gateway and the runtime worker rotate and redact the same
//! way. The sandbox audit log (see [`audit`], added separately) also reuses the
//! redaction + rotating primitives.

pub mod audit;
pub mod redaction;
pub mod rotating;

pub use audit::{AuditDecision, AuditKind, SandboxAudit};
pub use redaction::redact_log_text;
pub use rotating::{
    DEFAULT_MAX_LOG_BYTES, DEFAULT_MAX_LOG_FILES, RedactingSizeRotatingWriter, SizeRotatingLogFile,
};

/// Initialize a stderr-only `tracing_subscriber` using `RUST_LOG` (falling back
/// to `default_filter`).
///
/// Intended for minimal binaries (`slab-mcp-server`, `slab-python-runtime`)
/// that don't need file output or rotation.
pub fn init_stderr_tracing(default_filter: &str) {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter));
    tracing_subscriber::fmt().with_env_filter(env_filter).with_writer(std::io::stderr).init();
}
