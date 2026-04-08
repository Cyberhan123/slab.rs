use crate::error::DiffusionError;
use crate::params::{
    ContextParams, Image, ImgParams, InnerImgParams, InnerVideoParams, SampleMethod, Scheduler,
    Video, VideoParams, owned_image_from_raw,
};
use std::slice;
use std::sync::Arc;

/// A Stable Diffusion inference context.
///
/// Wraps a raw `sd_ctx_t*` produced by `new_sd_ctx`. The underlying context
/// is freed when this value is dropped.
pub struct Context {
    pub(crate) ctx: *mut slab_diffusion_sys::sd_ctx_t,
    pub(crate) lib: Arc<slab_diffusion_sys::DiffusionLib>,
    pub(crate) _params: ContextParams,
}

impl Context {
    fn copy_images(
        images_ptr: *mut slab_diffusion_sys::sd_image_t,
        image_count: usize,
    ) -> Vec<Image> {
        unsafe { slice::from_raw_parts(images_ptr, image_count) }
            .iter()
            .copied()
            .map(owned_image_from_raw)
            .collect()
    }

    fn collect_images(
        lib: &slab_diffusion_sys::DiffusionLib,
        images_ptr: *mut slab_diffusion_sys::sd_image_t,
        image_count: usize,
    ) -> Vec<Image> {
        let images = Self::copy_images(images_ptr, image_count);
        let native_count =
            i32::try_from(image_count).expect("native image counts should fit into i32");
        unsafe { lib.free_sd_images(images_ptr, native_count) };
        images
    }

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
    /// the native layer.
    ///
    /// # Errors
    /// Returns [`DiffusionError::GenerationFailed`] when the native library
    /// returns a null pointer (e.g. out of memory or bad parameters).
    pub fn generate_image(&self, params: ImgParams) -> Result<Vec<Image>, DiffusionError> {
        let inner: InnerImgParams = InnerImgParams::from_canonical(self.lib.as_ref(), &params)
            .map_err(DiffusionError::InvalidParameters)?;

        if self.ctx.is_null() {
            return Err(DiffusionError::ContextNull);
        }

        let images_ptr = unsafe { self.lib.generate_image(self.ctx, &*inner.fp) };

        if images_ptr.is_null() {
            return Err(DiffusionError::GenerationFailed);
        }

        let batch = usize::try_from(inner.get_batch_count())
            .map_err(|_| DiffusionError::GenerationFailed)?;
        Ok(Self::collect_images(self.lib.as_ref(), images_ptr, batch))
    }

    pub fn generate_video(&self, params: VideoParams) -> Result<Video, DiffusionError> {
        let inner = InnerVideoParams::from_canonical(self.lib.as_ref(), self.ctx, &params)
            .map_err(DiffusionError::InvalidParameters)?;
        let mut num_frames_out: i32 = 0;

        let frames_ptr = unsafe {
            self.lib.generate_video(self.ctx, &*inner.fp, &mut num_frames_out as *mut i32)
        };

        if frames_ptr.is_null() {
            return Err(DiffusionError::GenerationFailed);
        }

        let frame_count =
            usize::try_from(num_frames_out).map_err(|_| DiffusionError::GenerationFailed)?;

        let frames = Self::collect_images(self.lib.as_ref(), frames_ptr, frame_count);
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
// no shared mutable state between separate context instances. Callers should
// use one Context per thread for concurrent inference.
// See: https://github.com/leejet/stable-diffusion.cpp (README / architecture)
unsafe impl Send for Context {}
unsafe impl Sync for Context {}

impl std::fmt::Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context").finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alloc_image(
        width: u32,
        height: u32,
        channel: u32,
        data: &[u8],
    ) -> slab_diffusion_sys::sd_image_t {
        let ptr = unsafe { libc::malloc(data.len()).cast::<u8>() };
        assert!(!ptr.is_null());
        unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len()) };

        slab_diffusion_sys::sd_image_t { width, height, channel, data: ptr }
    }

    #[test]
    fn collect_images_copies_and_returns_native_images() {
        let raw_images =
            [alloc_image(2, 1, 3, &[1, 2, 3, 4, 5, 6]), alloc_image(1, 1, 4, &[9, 8, 7, 6])];
        let bytes = std::mem::size_of_val(&raw_images);
        let ptr = unsafe { libc::malloc(bytes).cast::<slab_diffusion_sys::sd_image_t>() };
        assert!(!ptr.is_null());
        unsafe { std::ptr::copy_nonoverlapping(raw_images.as_ptr(), ptr, raw_images.len()) };

        let images = Context::copy_images(ptr, raw_images.len());
        unsafe {
            libc::free(raw_images[0].data.cast());
            libc::free(raw_images[1].data.cast());
            libc::free(ptr.cast());
        }

        assert_eq!(images.len(), 2);
        assert_eq!(images[0].width, 2);
        assert_eq!(images[0].data, vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(images[1].channel, 4);
        assert_eq!(images[1].data, vec![9, 8, 7, 6]);
    }
}
