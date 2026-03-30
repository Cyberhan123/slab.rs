use std::ffi::CString;
use std::sync::Arc;

use slab_diffusion_sys::upscaler_ctx_t;

use crate::params::{image_view, owned_image_from_raw, Image};
use crate::Diffusion;
use crate::DiffusionError;

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
        let image = unsafe { self.lib.upscale(self.fp, image_view(&input_image), upscale_factor) };

        if image.data.is_null() {
            return Err(DiffusionError::UpscalerFailed);
        }

        Ok(owned_image_from_raw(image))
    }
}

unsafe impl Send for UpscalerContext {}
unsafe impl Sync for UpscalerContext {}

impl Drop for UpscalerContext {
    fn drop(&mut self) {
        if !self.fp.is_null() {
            unsafe { self.lib.free_upscaler_ctx(self.fp) };
        }
    }
}
