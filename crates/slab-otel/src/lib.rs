//! Slab telemetry wiring and semantic helper APIs.

pub mod config;
pub mod gen_ai;
pub mod metrics;
pub mod provider;
pub mod session;
pub mod trace_context;

pub use provider::OtelProvider;
pub use session::SessionTelemetry;
