//! Windows OS-enforced sandbox for slab.rs.
//!
//! Sits beneath [`slab_sandboxing`] (which depends on this crate, cfg-gated, downward only —
//! this crate MUST NOT depend on `slab_sandboxing`). Owns the `SpawnedChild` seam and every
//! Windows isolation primitive (Job Object, restricted token, integrity-label ACLs, the
//! elevated helper IPC).
//!
//! Everything here is `#[cfg(target_os = "windows")]`; on other platforms this crate is an
//! empty shell so the workspace still builds in cross-OS CI.

#![cfg_attr(not(target_os = "windows"), allow(unused_imports, clippy::all))]

#[cfg(target_os = "windows")]
mod capability;
#[cfg(target_os = "windows")]
mod creds;
#[cfg(target_os = "windows")]
mod daemon;
#[cfg(target_os = "windows")]
mod elevation;
#[cfg(target_os = "windows")]
mod error;
#[cfg(target_os = "windows")]
mod executor;
#[cfg(target_os = "windows")]
mod helper;
#[cfg(target_os = "windows")]
mod ipc;
#[cfg(target_os = "windows")]
mod job;
#[cfg(target_os = "windows")]
mod mac;
#[cfg(target_os = "windows")]
mod marker;
#[cfg(target_os = "windows")]
mod pipe;
#[cfg(target_os = "windows")]
mod request;

#[cfg(target_os = "windows")]
pub use capability::{CapabilitySnapshot, FsIsolationStrength, WindowsSetupKind};
#[cfg(target_os = "windows")]
pub use creds::{key_fingerprint, load_or_create_key};
#[cfg(target_os = "windows")]
pub use daemon::run_daemon;
#[cfg(target_os = "windows")]
pub use elevation::{ElevatedHelper, Elevator, HelperLaunchError, ShellElevator, elevate};
#[cfg(target_os = "windows")]
pub use error::WindowsSandboxError;
#[cfg(target_os = "windows")]
pub use executor::{JobOnlyExecutor, WindowsSandboxExecutor};
#[cfg(target_os = "windows")]
pub use helper::run_payload;
#[cfg(target_os = "windows")]
pub use ipc::{
    ElevationPayload, FileFramingError, HelperResult, PayloadOp, SetupMarker, SignedPayload,
    SignedResult, read_signed_payload, read_signed_result, write_signed_payload,
    write_signed_result,
};
#[cfg(target_os = "windows")]
pub use marker::{has_drift, read_marker, write_marker};
#[cfg(target_os = "windows")]
pub use pipe::{PipeFrame, ping, ping_with_timeout};
#[cfg(target_os = "windows")]
pub use request::{
    ElevatedExit, ElevatedRun, ProvisionReport, SetupMode, SpawnRequest, SpawnedChild,
};

/// Current IPC/marker schema version. Bumping invalidates older payloads/markers (drift).
pub const SCHEMA_VERSION: u32 = 1;
