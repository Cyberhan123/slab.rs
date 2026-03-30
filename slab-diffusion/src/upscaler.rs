use crate::params::Image;
use crate::Diffusion;
use crate::DiffusionError;
use slab_diffusion_sys::upscaler_ctx_t;
use std::ffi::CString;
use std::sync::Arc;

pub struct UpscalerContext {
    pub(crate) fp: *mut upscaler_ctx_t,
    pub(crate) lib: Arc<slab_diffusion_sys::DiffusionLib>,
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
        let esrgan_cstr = CString::new(esrgan_path).unwrap();
        let device_cstr = device.map(|d| CString::new(d).unwrap());
        let ctx = unsafe {
            self.lib.new_upscaler_ctx(
                esrgan_cstr.as_ptr(),
                offload_params_to_cpu,
                direct,
                n_threads,
                tile_size,
                device_cstr.as_ref().map_or(std::ptr::null(), |d| d.as_ptr()),
            )
        };

        if ctx.is_null() {
            return Err(DiffusionError::ContextCreationFailed);
        }
        Ok(UpscalerContext { fp: ctx, lib: self.lib.clone() })
    }
}

impl UpscalerContext {
    pub fn upscale(&self, init_iamge: Image, upscale_factor: u32) -> Result<Image, DiffusionError> {
        let image = unsafe { self.lib.upscale(self.fp, init_iamge.into(), upscale_factor) };

        if image.data.is_null() {
            return Err(DiffusionError::UpscalerFailed);
        }

        Ok(image.into())
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
