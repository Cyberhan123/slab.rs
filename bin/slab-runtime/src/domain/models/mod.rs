mod backend;
mod task;

pub(crate) use backend::{BackendCatalog, InvocationPlan, ResolvedBackend};
pub(crate) use task::TaskCodec;
pub use task::{TaskHandle, TaskSnapshot, TaskState};