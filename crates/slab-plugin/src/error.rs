use thiserror::Error;

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("plugin runtime is not callable for frontend-only plugin `{plugin_id}`")]
    NotCallable { plugin_id: String },
    #[error("plugin `{plugin_id}` has no supported runtime backend")]
    NoBackend { plugin_id: String },
    #[error("plugin runtime error: {0}")]
    Runtime(String),
    #[error("plugin `{0}` is not available")]
    NotFound(String),
    #[error("plugin registry error: {0}")]
    Registry(String),
}

impl From<anyhow::Error> for PluginError {
    fn from(value: anyhow::Error) -> Self {
        Self::Runtime(value.to_string())
    }
}
