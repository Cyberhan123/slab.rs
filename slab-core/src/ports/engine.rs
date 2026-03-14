use async_trait::async_trait;

use crate::base::error::CoreError;
use crate::base::types::Payload;
use crate::scheduler::backend::protocol::StreamHandle;

/// Abstraction over an AI inference engine backend.
///
/// Implementing this trait allows a concrete GGML (or future) backend to be
/// driven through a uniform interface from the scheduler without the scheduler
/// taking a direct dependency on FFI details.
#[async_trait]
pub trait Engine: Send + Sync + 'static {
    /// Load a model into the engine using the supplied configuration payload.
    async fn load_model(&mut self, config: Payload) -> Result<(), CoreError>;

    /// Run a synchronous inference pass and return the result payload.
    async fn inference(&self, input: Payload) -> Result<Payload, CoreError>;

    /// Run a streaming inference pass and return a channel handle.
    ///
    /// The caller consumes chunks from the returned [`StreamHandle`] until a
    /// `Done` or `Error` chunk is received.
    async fn inference_stream(&self, input: Payload) -> Result<StreamHandle, CoreError>;

    /// Unload the current model and free associated resources.
    async fn unload(&mut self) -> Result<(), CoreError>;
}

/// Abstraction over a background worker that drives an [`Engine`].
///
/// A `Worker` owns the execution thread/task for one backend and is
/// responsible for forwarding requests from the scheduler to its engine,
/// managing the engine lifecycle, and broadcasting state changes to peers.
#[async_trait]
pub trait Worker: Send + 'static {
    /// Start the worker event loop.  Returns when the worker shuts down.
    async fn run(self) -> Result<(), CoreError>;

    /// Request a graceful shutdown.
    fn shutdown(&self);
}
