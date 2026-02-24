use std::ptr;
use std::slice;
use std::sync::Arc;

use libc::free;

use crate::error::DiffusionError;
use crate::params::{opt_cstring, SdImage, SdImgGenParams, SAMPLE_METHOD_COUNT, SCHEDULER_COUNT};

/// Return the C string pointer from an `Option<CString>`, or the pointer of a
/// freshly-allocated empty C string stored in `fallback`.
fn cstr_ptr_or_empty<'a>(
    cs: &'a Option<std::ffi::CString>,
    fallback: &'a mut Option<std::ffi::CString>,
) -> *const std::os::raw::c_char {
    match cs.as_ref() {
        Some(s) => s.as_ptr(),
        None => fallback
            .get_or_insert_with(|| std::ffi::CString::new("").unwrap())
            .as_ptr(),
    }
}

/// A Stable Diffusion inference context.
///
/// Wraps a raw `sd_ctx_t*` produced by `new_sd_ctx`.  The underlying context
/// is freed when this value is dropped.
pub struct SdContext {
    pub(crate) ctx: *mut slab_diffusion_sys::sd_ctx_t,
    pub(crate) lib: Arc<slab_diffusion_sys::DiffusionLib>,
}

impl SdContext {
    /// Return the default sampling method recommended by the loaded model.
    pub fn get_default_sample_method(&self) -> crate::params::SampleMethod {
        unsafe { self.lib.sd_get_default_sample_method(self.ctx) }
    }

    /// Return the default scheduler for the given sampling method.
    pub fn get_default_scheduler(
        &self,
        sample_method: crate::params::SampleMethod,
    ) -> crate::params::Scheduler {
        unsafe { self.lib.sd_get_default_scheduler(self.ctx, sample_method) }
    }

    /// Generate one or more images from the supplied parameters.
    ///
    /// The returned `Vec` contains exactly `params.batch_count` images.
    ///
    /// # Errors
    /// Returns [`DiffusionError::GenerationFailed`] when the native library
    /// returns a null pointer (e.g. out of memory or bad parameters).
    pub fn generate_image(&self, params: &SdImgGenParams) -> Result<Vec<SdImage>, DiffusionError> {
        // ── Resolve auto sample-method / scheduler ────────────────────────────
        let sample_method = if params.sample_method == SAMPLE_METHOD_COUNT {
            self.get_default_sample_method()
        } else {
            params.sample_method
        };

        let scheduler = if params.scheduler == SCHEDULER_COUNT {
            self.get_default_scheduler(sample_method)
        } else {
            params.scheduler
        };

        // ── Build CStrings (kept alive for the duration of the C call) ────────
        let prompt_cs = opt_cstring(&params.prompt);
        let neg_prompt_cs = opt_cstring(&params.negative_prompt);

        // The C API expects non-null string pointers; supply an empty string
        // when the caller didn't provide one.
        let mut prompt_fallback = None;
        let mut neg_fallback = None;
        let prompt_ptr = cstr_ptr_or_empty(&prompt_cs, &mut prompt_fallback);
        let neg_ptr = cstr_ptr_or_empty(&neg_prompt_cs, &mut neg_fallback);

        // Clamp batch_count to at least 1 so neither the C function nor the
        // slice interpretation ever receives a zero/negative count.
        let batch = params.batch_count.max(1) as usize;

        // ── Build the C parameter struct ───────────────────────────────────────
        let guidance = slab_diffusion_sys::sd_guidance_params_t {
            txt_cfg: params.cfg_scale,
            img_cfg: params.cfg_scale,
            distilled_guidance: params.guidance,
            slg: slab_diffusion_sys::sd_slg_params_t {
                layers: ptr::null_mut(),
                layer_count: 0,
                layer_start: 0.01,
                layer_end: 0.2,
                scale: 0.0,
            },
        };

        let sample_params = slab_diffusion_sys::sd_sample_params_t {
            guidance,
            scheduler,
            sample_method,
            sample_steps: params.sample_steps,
            eta: params.eta,
            shifted_timestep: 0,
            custom_sigmas: ptr::null_mut(),
            custom_sigmas_count: 0,
        };

        let null_image = slab_diffusion_sys::sd_image_t {
            width: 0,
            height: 0,
            channel: 3,
            data: ptr::null_mut(),
        };

        let mask_image = slab_diffusion_sys::sd_image_t {
            width: params.width,
            height: params.height,
            channel: 1,
            data: ptr::null_mut(),
        };

        let vae_tiling_params = slab_diffusion_sys::sd_tiling_params_t {
            enabled: false,
            tile_size_x: 32,
            tile_size_y: 32,
            target_overlap: 0.5,
            rel_size_x: 0.0,
            rel_size_y: 0.0,
        };

        let pm_params = slab_diffusion_sys::sd_pm_params_t {
            id_images: ptr::null_mut(),
            id_images_count: 0,
            id_embed_path: ptr::null(),
            style_strength: 20.0,
        };

        let cache = slab_diffusion_sys::sd_cache_params_t {
            mode: slab_diffusion_sys::sd_cache_mode_t_SD_CACHE_DISABLED,
            reuse_threshold: 1.0,
            start_percent: 0.15,
            end_percent: 0.95,
            error_decay_rate: 1.0,
            use_relative_threshold: true,
            reset_error_on_compute: true,
            Fn_compute_blocks: 8,
            Bn_compute_blocks: 0,
            residual_diff_threshold: 0.08,
            max_warmup_steps: 8,
            max_cached_steps: -1,
            max_continuous_cached_steps: -1,
            taylorseer_n_derivatives: 1,
            taylorseer_skip_interval: 1,
            scm_mask: ptr::null(),
            scm_policy_dynamic: true,
        };

        let gen_params = slab_diffusion_sys::sd_img_gen_params_t {
            loras: ptr::null(),
            lora_count: 0,
            prompt: prompt_ptr,
            negative_prompt: neg_ptr,
            clip_skip: params.clip_skip,
            init_image: null_image,
            ref_images: ptr::null_mut(),
            ref_images_count: 0,
            auto_resize_ref_image: true,
            increase_ref_index: false,
            mask_image,
            width: params.width as i32,
            height: params.height as i32,
            sample_params,
            strength: params.strength,
            seed: params.seed,
            // Use the validated batch (>= 1) so the C function always gets a
            // sensible value even if the caller passed 0 or a negative number.
            batch_count: batch as i32,
            control_image: slab_diffusion_sys::sd_image_t {
                width: 0,
                height: 0,
                channel: 3,
                data: ptr::null_mut(),
            },
            control_strength: 0.9,
            pm_params,
            vae_tiling_params,
            cache,
        };

        // ── Call generate_image ───────────────────────────────────────────────
        let images_ptr = unsafe { self.lib.generate_image(self.ctx, &gen_params) };

        if images_ptr.is_null() {
            return Err(DiffusionError::GenerationFailed);
        }

        // ── Copy pixel data into Rust-owned SdImage values ────────────────────
        // stable-diffusion.cpp allocates image data with standard malloc (see
        // stable-diffusion.cpp source); libc::free is therefore the correct
        // deallocation function. There is no dedicated sd_free_image helper.
        let images: Vec<SdImage> = unsafe {
            slice::from_raw_parts(images_ptr, batch)
                .iter()
                .map(|img| {
                    // Use saturating_mul to avoid u32 overflow for pathologically
                    // large dimensions before casting to usize.
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
                    SdImage {
                        width: img.width,
                        height: img.height,
                        channel: img.channel,
                        data,
                    }
                })
                .collect()
        };

        // Free the sd_image_t array itself (pixel data was freed above).
        unsafe { free(images_ptr as *mut libc::c_void) };

        Ok(images)
    }
}

impl Drop for SdContext {
    fn drop(&mut self) {
        if !self.ctx.is_null() {
            unsafe { self.lib.free_sd_ctx(self.ctx) };
        }
    }
}

// Each sd_ctx_t owns its own model weights and intermediate tensors; there is
// no shared mutable state between separate context instances.  Callers should
// use one SdContext per thread for concurrent inference.
// See: https://github.com/leejet/stable-diffusion.cpp (README / architecture)
unsafe impl Send for SdContext {}
unsafe impl Sync for SdContext {}

impl std::fmt::Debug for SdContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SdContext").finish()
    }
}
