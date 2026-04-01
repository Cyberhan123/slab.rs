//! Native Tauri IPC surface for the desktop host.
//!
//! This mirrors the feature-oriented layout of `bin/slab-server/src/api`
//! while delegating business logic directly to `slab-app-core`.

pub mod health;
mod state;
pub mod v1;
pub mod validation;

pub use state::init_state;
