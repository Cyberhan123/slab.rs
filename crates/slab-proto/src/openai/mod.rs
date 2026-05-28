#![allow(clippy::too_many_arguments)]
pub(crate) mod models;

/// Re-export generated DTOs directly at crate::openai::* to avoid deep paths.
/// These DTOs are maintained against `crates/slab-proto/openai/openapi/openapi.yaml`.
/// When adjusting a DTO, align it with the OpenAPI spec and add or update a test
/// under `src/openai/tests`.
/// Keep this module DTO-only: do not add client or server logic here.
pub use models::*;

#[cfg(test)]
mod tests;
