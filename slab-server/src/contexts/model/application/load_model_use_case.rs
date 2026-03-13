use std::future::Future;
use std::pin::Pin;

use crate::error::ServerError;
use crate::schemas::v1::models::{LoadModelRequest, ModelStatusResponse};

pub trait ModelLoadPort: Send + Sync {
    fn load_model(
        &self,
        req: LoadModelRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ModelStatusResponse, ServerError>> + Send + '_>>;
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

    pub async fn execute(&self, req: LoadModelRequest) -> Result<ModelStatusResponse, ServerError> {
        self.port.load_model(req).await
    }
}
