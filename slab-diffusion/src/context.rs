use std::slice;
use std::sync::Arc;

use libc::free;

use crate::error::DiffusionError;
use crate::params::{Image, ImgParams, SampleMethod, Scheduler, Video, VideoParams};

/// A Stable Diffusion inference context.
///
/// Wraps a raw `sd_ctx_t*` produced by `new_sd_ctx`.  The underlying context
/// is freed when this value is dropped.
pub struct Context {
    pub(crate) ctx: *mut slab_diffusion_sys::sd_ctx_t,
    pub(crate) lib: Arc<slab_diffusion_sys::DiffusionLib>,
}

impl Context {
    /// Return the default sampling method recommended by the loaded model.
    pub fn get_default_sample_method(&self) -> SampleMethod {
        let sample_method: slab_diffusion_sys::sample_method_t =
            unsafe { self.lib.sd_get_default_sample_method(self.ctx) };
        SampleMethod::from(sample_method)
    }

    /// Return the default scheduler for the given sampling method.
    pub fn get_default_scheduler(&self, sample_method: SampleMethod) -> Scheduler {
        let scheduler =
            unsafe { self.lib.sd_get_default_scheduler(self.ctx, sample_method.into()) };
        Scheduler::from(scheduler)
    }

    /// Generate one or more images from the supplied parameters.
    ///
    /// The returned `Vec` contains exactly the effective batch count sent to
    /// the native layer. Values below `1` are clamped to `1`.
    ///
    /// # Errors
    /// Returns [`DiffusionError::GenerationFailed`] when the native library
    /// returns a null pointer (e.g. out of memory or bad parameters).
    pub fn generate_image(&self, params: ImgParams) -> Result<Vec<Image>, DiffusionError> {
        let images_ptr = unsafe { self.lib.generate_image(self.ctx, &*params.fp) };

        if images_ptr.is_null() {
            return Err(DiffusionError::GenerationFailed);
        }

        let batch = params.get_batch_count() as usize;

        // Copy pixel data into Rust-owned SdImage values.
        // stable-diffusion.cpp allocates image data with standard malloc;
        // libc::free is therefore the correct deallocation function.
        let images: Vec<Image> = unsafe {
            slice::from_raw_parts(images_ptr, batch)
                .iter()
                .map(|img| {
                    let len = (img.width as usize)
                        .saturating_mul(img.height as usize)
                        .saturating_mul(img.channel as usize);
                    let data = if img.data.is_null() || len == 0 {
                        Vec::new()
                    } else {
                        let bytes = slice::from_raw_parts(img.data, len);
                        let owned = bytes.to_vec();
                        free(img.data as *mut libc::c_void);
                        owned
                    };
                    Image { width: img.width, height: img.height, channel: img.channel, data }
                })
                .collect()
        };

        // Free the sd_image_t array itself (pixel data was freed above).
        unsafe { free(images_ptr as *mut libc::c_void) };

        Ok(images)
    }

    pub fn generate_video(&self, params: VideoParams) -> Result<Video, DiffusionError> {
        let mut num_frames_out: i32 = 0;

        let frames_ptr = unsafe {
            self.lib.generate_video(self.ctx, &*params.fp, &mut num_frames_out as *mut i32)
        };

        if frames_ptr.is_null() {
            return Err(DiffusionError::GenerationFailed);
        }

        let frames: Vec<Image> = unsafe {
            slice::from_raw_parts(frames_ptr, num_frames_out as usize)
                .iter()
                .map(|img| {
                    let len = (img.width as usize)
                        .saturating_mul(img.height as usize)
                        .saturating_mul(img.channel as usize);
                    let data = if img.data.is_null() || len == 0 {
                        Vec::new()
                    } else {
                        let bytes = slice::from_raw_parts(img.data, len);
                        let owned = bytes.to_vec();
                        free(img.data as *mut libc::c_void);
                        owned
                    };
                    Image { width: img.width, height: img.height, channel: img.channel, data }
                })
                .collect()
        };

        // Free the sd_image_t array itself (pixel data was freed above).
        unsafe { free(frames_ptr as *mut libc::c_void) };

        Ok(Video { frames, num_frames: num_frames_out })
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        if !self.ctx.is_null() {
            unsafe { self.lib.free_sd_ctx(self.ctx) };
        }
    }
}

// Each sd_ctx_t owns its own model weights and intermediate tensors; there is
// no shared mutable state between separate context instances.  Callers should
// use one Context per thread for concurrent inference.
// See: https://github.com/leejet/stable-diffusion.cpp (README / architecture)
unsafe impl Send for Context {}
unsafe impl Sync for Context {}

impl std::fmt::Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context").finish()
    }
}
