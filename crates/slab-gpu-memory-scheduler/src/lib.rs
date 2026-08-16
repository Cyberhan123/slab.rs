//! The single implementation of the GPU-memory probing/sizing formulas.
//!
//! Everything GPU-memory-formula-related lives here: the all-smi probe (the
//! only touchpoint), the scheduler with its periodic refresh + last-good
//! cache, the model-memory ledger, lifecycle hooks, and the pure
//! policy/sizing math that hosts (`slab-app-core`, `bin/slab-runtime`)
//! consume. This crate is not a decision authority — eviction, `n_ctx`
//! resolution, and compaction remain host decisions that read these numbers.
//! See the crate README for boundaries — in particular, this crate never
//! depends on app layers and never fires hooks inside the runtime process.

mod error;
mod hooks;
mod ledger;
mod params;
mod policy;
mod probe;
mod scheduler;
mod sizing;
mod snapshot;

pub use error::GpuMemoryError;
pub use hooks::{
    HookRegistry, InferenceContext, LoadContext, LoadOutcome, ModelLifecycleHook, UnloadContext,
    UnloadReason,
};
pub use ledger::{DeviceLedger, LedgerEntry, ModelLedger};
pub use params::SchedulerParams;
pub use policy::{
    EvictionCandidate, MemoryGauge, MemoryPressureInput, PressureThresholds,
    choose_pressure_eviction_candidate, is_oom_message, is_under_memory_pressure,
};
pub use probe::{AllSmiProbe, GpuProbe, NoopGpuProbe};
pub use scheduler::GpuMemoryScheduler;
pub use sizing::{AutoContextInput, kv_bytes_per_token, resolve_auto_context};
pub use snapshot::{DeviceMemoryGauge, GpuDeviceSnapshot, GpuStatusSnapshot};
