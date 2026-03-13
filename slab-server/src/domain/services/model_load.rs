use std::future::Future;
use std::pin::Pin;

use crate::domain::models::{ModelLoadCommand, ModelStatus};
use crate::error::ServerError;

pub trait ModelLoadPort: Send + Sync {
    fn load_model(
        &self,
        command: ModelLoadCommand,
    ) -> Pin<Box<dyn Future<Output = Result<ModelStatus, ServerError>> + Send + '_>>;
}

pub struct LoadModelUseCase<P> {
    port: P,
}

impl<P> LoadModelUseCase<P>
where
    P: ModelLoadPort,
{
    pub fn new(port: P) -> Self {
        Self { port }
    }

    pub async fn execute(&self, command: ModelLoadCommand) -> Result<ModelStatus, ServerError> {
        self.port.load_model(command).await
    }
}
