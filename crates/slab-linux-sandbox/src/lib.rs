//! Linux OS-enforced sandbox for slab.rs.
//!
//! Sits beneath [`slab_sandboxing`] (which depends on this crate, cfg-gated, downward only —
//! this crate MUST NOT depend on `slab_sandboxing`). Owns the `SpawnedChild` seam and the Linux
//! isolation primitives: bubblewrap (`bwrap`) for the filesystem namespace view, a network-only
//! seccomp BPF filter (always stacked under the network predicate), and a landlock
//! filesystem-access fallback for hosts where `bwrap` is unavailable (containers without user
//! namespaces). bwrap and landlock are mutually exclusive on the filesystem dimension.
//!
//! Everything here is `#[cfg(target_os = "linux")]`; on other platforms this crate is an empty
//! shell so the workspace still builds in cross-OS CI.

#![cfg_attr(not(target_os = "linux"), allow(unused_imports, clippy::all))]

#[cfg(target_os = "linux")]
mod bwrap;
#[cfg(target_os = "linux")]
mod capability;
#[cfg(target_os = "linux")]
mod error;
#[cfg(target_os = "linux")]
mod executor;
#[cfg(target_os = "linux")]
mod landlock;
#[cfg(target_os = "linux")]
mod request;
#[cfg(target_os = "linux")]
mod seccomp;

#[cfg(target_os = "linux")]
pub use capability::{CapabilitySnapshot, FsIsolationStrength, LinuxSetupKind};
#[cfg(target_os = "linux")]
pub use error::LinuxSandboxError;
#[cfg(target_os = "linux")]
pub use executor::{
    BwrapExecutor, DegradedLandlockRequiredExecutor, LandlockFallbackExecutor,
    LinuxSandboxExecutor, UnsupportedExecutor, select_executor,
};
#[cfg(target_os = "linux")]
pub use landlock::{build_ruleset_fd, probe_abi_version};
#[cfg(target_os = "linux")]
pub use request::{SandboxPolicyMirror, SpawnRequest, SpawnedChild};
