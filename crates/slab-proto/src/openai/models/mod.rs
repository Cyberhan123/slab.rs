// OpenAI API type definitions
// Organized by domain: audio, chat, common, images, model_types, responses, skills, tools, videos

pub mod _stubs;
pub mod apply_patch;
pub mod audio;
pub mod chat;
pub mod common;
pub mod images;
pub mod model_types;
pub mod responses;
pub mod skills;
pub mod tools;
pub mod videos;
pub mod embeddings;

// Re-export all types for backward compatibility
pub use _stubs::*;
pub use apply_patch::*;
pub use audio::*;
pub use chat::*;
pub use common::*;
pub use images::*;
pub use model_types::*;
pub use responses::*;
pub use skills::*;
pub use tools::*;
pub use videos::*;
pub use embeddings::*;
