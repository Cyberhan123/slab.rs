//! Shared request / response DTO types for both HTTP API and Tauri IPC.
//!
//! These schemas are used by both `bin/slab-server` (HTTP API) and
//! `bin/slab-app` (Tauri IPC native commands), ensuring a consistent API
//! contract across both entry points.

pub mod agent;
pub mod audio;
pub mod backend;
pub mod chat;
pub mod ffmpeg;
pub mod images;
pub mod models;
pub mod setup;
pub mod system;
pub mod tasks;
pub mod validation;
pub mod video;
