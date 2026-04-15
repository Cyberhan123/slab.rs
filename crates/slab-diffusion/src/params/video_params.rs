use std::ptr;

use serde::{Deserialize, Serialize};
use slab_diffusion_sys::{sd_image_t, sd_lora_t, sd_vid_gen_params_t};

use crate::params::support::{
    empty_image, image_view, new_c_string, sync_image_views, sync_lora_views,
};
use crate::params::{
    CacheParams, Image, InnerCacheParams, InnerSampleParams, Lora, SampleParams, TilingParams,
};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Video {
    pub frames: Vec<Image>,
    pub num_frames: i32,
}

/// Stable Rust-native video inference parameters shared across the runtime chain.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct VideoParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loras: Option<Vec<Lora>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_skip: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub init_image: Option<Image>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_image: Option<Image>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_frames: Option<Vec<Image>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_params: Option<SampleParams>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high_noise_sample_params: Option<SampleParams>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moe_boundary: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strength: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_frames: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vace_strength: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vae_tiling_params: Option<TilingParams>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<CacheParams>,
}

/// FFI-only video inference parameter backing struct.
pub(crate) struct InnerVideoParams {
    pub(crate) fp: Box<sd_vid_gen_params_t>,
    prompt: Option<std::ffi::CString>,
    negative_prompt: Option<std::ffi::CString>,
    loras: Vec<Lora>,
    lora_paths: Vec<std::ffi::CString>,
    c_loras: Vec<sd_lora_t>,
    init_image: Option<Image>,
    end_image: Option<Image>,
    control_frames: Vec<Image>,
    c_control_frames: Vec<sd_image_t>,
    sample_params: Option<InnerSampleParams>,
    high_noise_sample_params: Option<InnerSampleParams>,
    cache: Option<InnerCacheParams>,
}

impl Clone for InnerVideoParams {
    fn clone(&self) -> Self {
        let mut cloned = Self {
            fp: self.fp.clone(),
            prompt: self.prompt.clone(),
            negative_prompt: self.negative_prompt.clone(),
            loras: self.loras.clone(),
            lora_paths: self.lora_paths.clone(),
            c_loras: self.c_loras.clone(),
            init_image: self.init_image.clone(),
            end_image: self.end_image.clone(),
            control_frames: self.control_frames.clone(),
            c_control_frames: self.c_control_frames.clone(),
            sample_params: self.sample_params.clone(),
            high_noise_sample_params: self.high_noise_sample_params.clone(),
            cache: self.cache.clone(),
        };
        cloned.sync_backing();
        cloned
    }
}

impl Default for InnerVideoParams {
    fn default() -> Self {
        Self {
            fp: Box::new(unsafe { std::mem::zeroed::<sd_vid_gen_params_t>() }),
            prompt: None,
            negative_prompt: None,
            loras: Vec::new(),
            lora_paths: Vec::new(),
            c_loras: Vec::new(),
            init_image: None,
            end_image: None,
            control_frames: Vec::new(),
            c_control_frames: Vec::new(),
            sample_params: None,
            high_noise_sample_params: None,
            cache: None,
        }
    }
}

impl std::fmt::Debug for InnerVideoParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InnerVideoParams").finish_non_exhaustive()
    }
}

impl InnerVideoParams {
    pub(crate) fn with_native_init(lib: &slab_diffusion_sys::DiffusionLib) -> Self {
        let mut inner = Self::default();
        unsafe { lib.sd_vid_gen_params_init(inner.fp.as_mut()) };
        inner
    }

    pub(crate) fn from_canonical(
        lib: &slab_diffusion_sys::DiffusionLib,
        _ctx: *mut slab_diffusion_sys::sd_ctx_t,
        value: &VideoParams,
    ) -> Result<Self, String> {
        let mut inner = InnerVideoParams::with_native_init(lib);

        if let Some(prompt) = value.prompt.as_deref() {
            inner.set_prompt(prompt);
        }
        if value.negative_prompt.is_some() {
            inner.set_negative_prompt(value.negative_prompt.as_deref());
        }
        if let Some(loras) = value.loras.clone() {
            inner.set_loras(loras);
        }
        if let Some(clip_skip) = value.clip_skip {
            inner.set_clip_skip(clip_skip);
        }
        if value.init_image.is_some() {
            inner.set_init_image(value.init_image.clone());
        }
        if value.end_image.is_some() {
            inner.set_end_image(value.end_image.clone());
        }
        if let Some(control_frames) = value.control_frames.clone() {
            inner.set_control_frames(control_frames);
        }
        if let Some(width) = value.width {
            if width < 1 {
                return Err(format!("width must be >= 1, got {width}"));
            }
            inner.set_width(
                i32::try_from(width).map_err(|_| format!("width {width} exceeds i32 range"))?,
            );
        }
        if let Some(height) = value.height {
            if height < 1 {
                return Err(format!("height must be >= 1, got {height}"));
            }
            inner.set_height(
                i32::try_from(height).map_err(|_| format!("height {height} exceeds i32 range"))?,
            );
        }
        if let Some(sample_params) = value.sample_params.as_ref() {
            inner.set_sample_params(InnerSampleParams::from_canonical(lib, sample_params)?);
        }
        if let Some(high_noise_sample_params) = value.high_noise_sample_params.as_ref() {
            inner.set_high_noise_sample_params(InnerSampleParams::from_canonical(
                lib,
                high_noise_sample_params,
            )?);
        }
        if let Some(moe_boundary) = value.moe_boundary {
            inner.set_moe_boundary(moe_boundary);
        }
        if let Some(strength) = value.strength {
            inner.set_strength(strength);
        }
        if let Some(seed) = value.seed {
            inner.set_seed(seed);
        }
        if let Some(video_frames) = value.video_frames {
            if video_frames < 1 {
                return Err(format!("video_frames must be >= 1, got {video_frames}"));
            }
            inner.set_video_frames(
                i32::try_from(video_frames)
                    .map_err(|_| format!("video_frames {video_frames} exceeds i32 range"))?,
            );
        }
        if let Some(vace_strength) = value.vace_strength {
            inner.set_vace_strength(vace_strength);
        }
        if value.vae_tiling_params.is_some() {
            inner.set_vae_tiling_params(value.vae_tiling_params.clone());
        }
        if let Some(cache) = value.cache.as_ref() {
            inner.set_cache(InnerCacheParams::from_canonical(lib, cache));
        }

        Ok(inner)
    }

    fn sync_images(&mut self) {
        self.fp.init_image = self.init_image.as_ref().map_or_else(empty_image, image_view);
        self.fp.end_image = self.end_image.as_ref().map_or_else(empty_image, image_view);
        sync_image_views(&self.control_frames, &mut self.c_control_frames);
        self.fp.control_frames = if self.c_control_frames.is_empty() {
            ptr::null_mut()
        } else {
            self.c_control_frames.as_mut_ptr()
        };
        self.fp.control_frames_size = self.c_control_frames.len().min(i32::MAX as usize) as i32;
    }

    fn sync_loras(&mut self) {
        sync_lora_views(&self.loras, &mut self.lora_paths, &mut self.c_loras);
        self.fp.loras = if self.c_loras.is_empty() { ptr::null() } else { self.c_loras.as_ptr() };
        self.fp.lora_count = self.c_loras.len().min(u32::MAX as usize) as u32;
    }

    fn sync_sample_params(&mut self) {
        if let Some(sample_params) = self.sample_params.as_mut() {
            sample_params.sync_backing();
            self.fp.sample_params = *sample_params.fp;
        }

        if let Some(sample_params) = self.high_noise_sample_params.as_mut() {
            sample_params.sync_backing();
            self.fp.high_noise_sample_params = *sample_params.fp;
        }
    }

    fn sync_cache(&mut self) {
        if let Some(cache) = self.cache.as_mut() {
            cache.sync_backing();
            self.fp.cache = *cache.fp;
        }
    }

    fn sync_backing(&mut self) {
        self.fp.prompt = self.prompt.as_ref().map_or(ptr::null(), |prompt| prompt.as_ptr());
        self.fp.negative_prompt =
            self.negative_prompt.as_ref().map_or(ptr::null(), |prompt| prompt.as_ptr());
        self.sync_loras();
        self.sync_images();
        self.sync_sample_params();
        self.sync_cache();
    }

    fn set_loras(&mut self, loras: Vec<Lora>) {
        self.loras = loras;
        self.sync_loras();
    }

    fn set_prompt(&mut self, prompt: &str) {
        self.prompt = Some(new_c_string(prompt));
        self.fp.prompt = self.prompt.as_ref().map_or(ptr::null(), |value| value.as_ptr());
    }

    fn set_negative_prompt(&mut self, negative_prompt: Option<&str>) {
        self.negative_prompt = negative_prompt.map(new_c_string);
        self.fp.negative_prompt =
            self.negative_prompt.as_ref().map_or(ptr::null(), |value| value.as_ptr());
    }

    fn set_clip_skip(&mut self, clip_skip: i32) {
        self.fp.clip_skip = clip_skip;
    }

    fn set_init_image(&mut self, image: Option<Image>) {
        self.init_image = image;
        self.fp.init_image = self.init_image.as_ref().map_or_else(empty_image, image_view);
    }

    fn set_end_image(&mut self, image: Option<Image>) {
        self.end_image = image;
        self.fp.end_image = self.end_image.as_ref().map_or_else(empty_image, image_view);
    }

    fn set_control_frames(&mut self, images: Vec<Image>) {
        self.control_frames = images;
        sync_image_views(&self.control_frames, &mut self.c_control_frames);
        self.fp.control_frames = if self.c_control_frames.is_empty() {
            ptr::null_mut()
        } else {
            self.c_control_frames.as_mut_ptr()
        };
        self.fp.control_frames_size = self.c_control_frames.len().min(i32::MAX as usize) as i32;
    }

    fn set_width(&mut self, width: i32) {
        self.fp.width = width;
    }

    fn set_height(&mut self, height: i32) {
        self.fp.height = height;
    }

    fn set_sample_params(&mut self, sample_params: InnerSampleParams) {
        self.sample_params = Some(sample_params);
        self.sync_sample_params();
    }

    fn set_high_noise_sample_params(&mut self, high_noise_sample_params: InnerSampleParams) {
        self.high_noise_sample_params = Some(high_noise_sample_params);
        self.sync_sample_params();
    }

    fn set_moe_boundary(&mut self, moe_boundary: f32) {
        self.fp.moe_boundary = moe_boundary;
    }

    fn set_strength(&mut self, strength: f32) {
        self.fp.strength = strength;
    }

    fn set_seed(&mut self, seed: i64) {
        self.fp.seed = seed;
    }

    fn set_video_frames(&mut self, video_frames: i32) {
        self.fp.video_frames = video_frames;
    }

    fn set_vace_strength(&mut self, vace_strength: f32) {
        self.fp.vace_strength = vace_strength;
    }

    fn set_vae_tiling_params(&mut self, vae_tiling_params: Option<TilingParams>) {
        if let Some(vae_tiling_params) = vae_tiling_params {
            self.fp.vae_tiling_params = vae_tiling_params.into();
        }
    }

    fn set_cache(&mut self, cache: InnerCacheParams) {
        self.cache = Some(cache);
        self.sync_cache();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    fn sample_image(value: u8) -> Image {
        Image { width: 2, height: 1, channel: 3, data: vec![value; 6] }
    }

    #[test]
    fn clone_resyncs_prompt_and_control_frame_views() {
        let params = VideoParams {
            prompt: Some("animated skyline".to_owned()),
            negative_prompt: Some("noisy".to_owned()),
            control_frames: Some(vec![sample_image(4), sample_image(8)]),
            init_image: Some(sample_image(1)),
            end_image: Some(sample_image(2)),
            ..Default::default()
        };
        let mut inner = InnerVideoParams::default();
        inner.set_prompt(params.prompt.as_deref().expect("prompt should be present"));
        inner.set_negative_prompt(params.negative_prompt.as_deref());
        inner.set_control_frames(params.control_frames.clone().expect("frames should be present"));
        inner.set_init_image(params.init_image.clone());
        inner.set_end_image(params.end_image.clone());
        let cloned = inner.clone();

        assert_eq!(
            unsafe { CStr::from_ptr(cloned.fp.prompt) }.to_str().unwrap(),
            "animated skyline"
        );
        assert_ne!(cloned.fp.prompt, inner.fp.prompt);
        assert_eq!(cloned.fp.control_frames_size, 2);
        assert_eq!(cloned.fp.init_image.width, 2);
        assert_ne!(unsafe { (*cloned.fp.control_frames).data }, unsafe {
            (*inner.fp.control_frames).data
        });
    }
}
