#![allow(clippy::too_many_arguments)]

pub mod audio;
pub mod chat;
pub mod completions;
pub mod embeddings;
pub mod images;
pub mod models;
pub mod responses;
pub mod skills;
pub mod videos;

pub use completions::*;
#[allow(unused_imports)]
pub use embeddings::*;

// Re-export generated DTOs directly at crate::openai::* to avoid deep paths.
pub use models::*;

#[cfg(test)]
mod tests;
