mod error;
mod registry;
mod runtime;
mod types;

pub mod backend;

pub use error::PluginError;
pub use registry::{PluginRegistry, is_path_within_root, normalize_relative_path};
pub use runtime::{PluginBackend, PluginRuntime};
pub use types::{
    LoadedPlugin, PluginApiRequest, PluginApiResponse, PluginCallRequest, PluginCallResponse,
    PluginEmitRequest, PluginEventPayload,
};
