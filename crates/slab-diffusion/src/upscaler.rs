use std::ffi::CString;
use std::sync::Arc;

use slab_diffusion_sys::upscaler_ctx_t;

use crate::Diffusion;
use crate::DiffusionError;
use crate::params::{Image, image_view, owned_image_from_raw};

pub struct UpscalerContext {
    pub(crate) fp: *mut upscaler_ctx_t,
    pub(crate) lib: Arc<slab_diffusion_sys::DiffusionLib>,
    _esrgan_path: CString,
    _device: Option<CString>,
}

impl Diffusion {
    pub fn new_upscaler_context(
        &self,
        esrgan_path: &str,
        offload_params_to_cpu: bool,
        direct: bool,
        n_threads: i32,
        tile_size: i32,
        device: Option<&str>,
    ) -> Result<UpscalerContext, DiffusionError> {
        let esrgan_cstr =
            CString::new(esrgan_path).expect("ESRGAN path contains an interior NUL byte");
        let device_cstr =
            device.map(|value| CString::new(value).expect("device contains an interior NUL byte"));

        let ctx = unsafe {
            self.lib.new_upscaler_ctx(
                esrgan_cstr.as_ptr(),
                offload_params_to_cpu,
                direct,
                n_threads,
                tile_size,
                device_cstr.as_ref().map_or(std::ptr::null(), |value| value.as_ptr()),
            )
        };

        if ctx.is_null() {
            return Err(DiffusionError::ContextCreationFailed);
        }

        Ok(UpscalerContext {
            fp: ctx,
            lib: self.lib.clone(),
            _esrgan_path: esrgan_cstr,
            _device: device_cstr,
        })
    }
}

impl UpscalerContext {
    pub fn upscale(
        &self,
        input_image: Image,
        upscale_factor: u32,
    ) -> Result<Image, DiffusionError> {
        let mut image =
            unsafe { self.lib.upscale(self.fp, image_view(&input_image), upscale_factor) };

        if image.data.is_null() {
            return Err(DiffusionError::UpscalerFailed);
        }

        let owned = owned_image_from_raw(image);
        unsafe { self.lib.free_sd_image_data(&mut image) };

        Ok(owned)
    }
}

// SAFETY: Each upscaler_ctx_t owns its own model weights and intermediate buffers;
// there is no shared mutable state between separate upscaler context instances.
// The underlying C library does not use thread-local storage or global mutable state,
// so it is safe to send an UpscalerContext across threads.
unsafe impl Send for UpscalerContext {}
// SAFETY: All methods on UpscalerContext that access the raw pointer take &self
// (shared reference). The native library does not mutate the context through shared
// references in a way that would cause data races, and the Arc<DiffusionLib> itself
// is Send + Sync.
unsafe impl Sync for UpscalerContext {}

impl Drop for UpscalerContext {
    fn drop(&mut self) {
        if !self.fp.is_null() {
            unsafe { self.lib.free_upscaler_ctx(self.fp) };
        }
    }
}
