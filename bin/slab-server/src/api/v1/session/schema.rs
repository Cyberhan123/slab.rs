//! Session request / response types.
//!
//! These types are defined in `slab-app-core` and re-exported here so that
//! the HTTP server uses the same shared DTOs as the Tauri IPC bridge.

pub use slab_app_core::schemas::session::{
    CreateSessionRequest, MessageResponse, SessionResponse,
};
