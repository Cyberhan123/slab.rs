use crate::backend::ResourceManager;
use crate::base::error::CoreError;
use crate::internal::dispatch::DriverDescriptor;

pub type BackendRegistrar =
    Box<dyn FnOnce(&mut ResourceManager, usize) -> Result<(), CoreError> + Send>;

pub struct RuntimeBackendRegistration {
    pub descriptors: Vec<DriverDescriptor>,
    pub register: BackendRegistrar,
}

impl RuntimeBackendRegistration {
    pub fn new(
        descriptors: Vec<DriverDescriptor>,
        register: impl FnOnce(&mut ResourceManager, usize) -> Result<(), CoreError> + Send + 'static,
    ) -> Self {
        Self { descriptors, register: Box::new(register) }
    }
}
