use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait RuntimeHost: Send + Sync {
    async fn request(&self, method: &str, params: Value) -> Result<Value, String>;
}
