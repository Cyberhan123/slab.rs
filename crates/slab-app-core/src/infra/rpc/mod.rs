pub mod client;
pub mod codec;
pub mod gateway;
pub mod runtime_gateway;
mod runtime_protocol;

pub use runtime_gateway::GrpcRuntimeInferenceGateway;
pub use slab_proto::slab::ipc::v1 as pb;
