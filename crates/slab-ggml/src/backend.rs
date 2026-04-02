use std::fmt;
use std::fmt::Debug;

/// Opaque backend registry handle managed by ggml.
///
/// This is a borrowed/native handle and must not be released with `libc::free`.
#[derive(Clone, Copy)]
pub struct GGMLBackendReg {
    pub(crate) reg: slab_ggml_sys::ggml_backend_reg_t,
}

impl GGMLBackendReg {}

impl Debug for GGMLBackendReg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GGMLBackendReg").finish()
    }
}

/// Opaque backend device handle managed by ggml.
///
/// This is a borrowed/native handle and must not be released with `libc::free`.
#[derive(Clone, Copy)]
pub struct GGMLBackendDevice {
    pub(crate) device: slab_ggml_sys::ggml_backend_dev_t,
}
