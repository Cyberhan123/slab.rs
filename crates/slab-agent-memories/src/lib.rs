//! Memory pipeline helpers for Slab agents.
//!
//! This crate owns template rendering and deterministic memory filesystem
//! behavior. Hosts still own persistence, model execution, and process
//! supervision.

mod error;
pub mod fs;
pub mod git;
pub mod hooks;
pub mod phase1;
pub mod phase2;
pub mod read;
pub mod redaction;
pub mod templates;

pub use error::{MemoryError, Result};
