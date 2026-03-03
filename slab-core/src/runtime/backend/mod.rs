pub mod admission;
pub mod protocol;

use async_trait::async_trait;

use crate::runtime::backend::protocol::{BackendReply, BackendRequest, ManagementEvent};
use crate::runtime::types::RuntimeError;

/// Backend execution contract used by runtime-managed worker loops.
///
/// Backends provide concrete behavior through explicit `handle_xxx` methods.
#[async_trait]
pub trait BackendHandler: Send + Sync {
    /// Backend id used for error messages.
    fn backend_id(&self) -> &str;

    async fn handle_initialize(
        &mut self,
        _req: BackendRequest,
    ) -> Result<BackendReply, RuntimeError> {
        Err(RuntimeError::UnsupportedOperation {
            backend: self.backend_id().to_owned(),
            op: "initialize".into(),
        })
    }

    async fn handle_load_model(&mut self, _req: BackendRequest) -> Result<BackendReply, RuntimeError> {
        Err(RuntimeError::UnsupportedOperation {
            backend: self.backend_id().to_owned(),
            op: "load_model".into(),
        })
    }

    async fn handle_unload_model(
        &mut self,
        _req: BackendRequest,
    ) -> Result<BackendReply, RuntimeError> {
        Err(RuntimeError::UnsupportedOperation {
            backend: self.backend_id().to_owned(),
            op: "unload_model".into(),
        })
    }

    async fn handle_inference(&mut self, _req: BackendRequest) -> Result<BackendReply, RuntimeError> {
        Err(RuntimeError::UnsupportedOperation {
            backend: self.backend_id().to_owned(),
            op: "inference".into(),
        })
    }

    async fn handle_inference_stream(
        &mut self,
        _req: BackendRequest,
    ) -> Result<BackendReply, RuntimeError> {
        Err(RuntimeError::UnsupportedOperation {
            backend: self.backend_id().to_owned(),
            op: "inference_stream".into(),
        })
    }

    async fn dispatch(&mut self, event: ManagementEvent, req: BackendRequest) -> Result<BackendReply, RuntimeError> {
        match event {
            ManagementEvent::Initialize => self.handle_initialize(req).await,
            ManagementEvent::LoadModel => self.handle_load_model(req).await,
            ManagementEvent::UnloadModel => self.handle_unload_model(req).await,
        }
    }
}
