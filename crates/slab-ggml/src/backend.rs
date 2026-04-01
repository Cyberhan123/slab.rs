use std::fmt;
use std::{fmt::Debug, rc::Rc};

#[derive(Clone)]
pub struct GGMLBackendReg {
    pub(crate) _reg: Rc<slab_ggml_sys::ggml_backend_reg_t>,
}

impl GGMLBackendReg {}

impl Debug for GGMLBackendReg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GGMLBackendReg").finish()
    }
}
