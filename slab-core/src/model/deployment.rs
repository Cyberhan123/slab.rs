use crate::dispatch::ResolvedInvocation;
use crate::spec::ModelSpec;

#[derive(Debug, Clone)]
pub struct ModelDeployment {
    pub spec: ModelSpec,
    pub resolved: ResolvedInvocation,
}
