//! `slab-types` – shared types, JSON schema definitions, and the PMID catalog.
//!
//! # Modules
//! - [`common`] – universal building blocks: [`common::Id`], [`common::Timestamp`].
//! - [`error`]  – [`error::SlabTypeError`], the crate-level error type.
//! - [`settings`] – PMID catalog and typed configuration snapshots for the
//!   settings system.

pub mod common;
pub mod error;
pub mod settings;

pub use error::SlabTypeError;
