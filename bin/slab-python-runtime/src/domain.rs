use serde_json::Value;

#[async_trait::async_trait]
/// Runtime-to-host callback transport.
///
/// Implementations forward Python bridge requests back to the supervising host
/// and return the host-authorized JSON response.
pub trait RuntimeHost: Send + Sync {
    async fn request(&self, method: &str, params: Value) -> Result<Value, String>;
}

pub struct DenyRuntimeHost;

#[async_trait::async_trait]
impl RuntimeHost for DenyRuntimeHost {
    async fn request(&self, method: &str, _params: Value) -> Result<Value, String> {
        Err(format!("runtime host callback `{method}` is not available"))
    }
}
