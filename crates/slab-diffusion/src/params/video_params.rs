use std::ptr;

use slab_diffusion_sys::{sd_image_t, sd_lora_t, sd_vid_gen_params_t};

use crate::Diffusion;
use crate::params::support::{
    empty_image, image_view, new_c_string, sync_image_views, sync_lora_views,
};
use crate::params::{CacheParams, Image, Lora, SampleParams, TilingParams};

#[derive(Debug, Clone, Default)]
pub struct Video {
    pub frames: Vec<Image>,
    pub num_frames: i32,
}

pub struct VideoParams {
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
    sample_params: Option<SampleParams>,
    high_noise_sample_params: Option<SampleParams>,
    cache: Option<CacheParams>,
}

impl Clone for VideoParams {
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

impl std::fmt::Debug for VideoParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoParams").finish_non_exhaustive()
    }
}

impl Diffusion {
    pub fn new_video_params(&self) -> VideoParams {
        let mut fp = Box::new(unsafe { std::mem::zeroed::<sd_vid_gen_params_t>() });
        unsafe { self.lib.sd_vid_gen_params_init(fp.as_mut()) };
        VideoParams {
            fp,
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

impl VideoParams {
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

    pub fn set_loras(&mut self, loras: Vec<Lora>) {
        self.loras = loras;
        self.sync_loras();
    }

    pub fn set_prompt(&mut self, prompt: &str) {
        self.prompt = Some(new_c_string(prompt));
        self.fp.prompt = self.prompt.as_ref().map_or(ptr::null(), |value| value.as_ptr());
    }

    pub fn set_negative_prompt(&mut self, negative_prompt: &str) {
        self.negative_prompt = Some(new_c_string(negative_prompt));
        self.fp.negative_prompt =
            self.negative_prompt.as_ref().map_or(ptr::null(), |value| value.as_ptr());
    }

    pub fn set_init_image(&mut self, image: Image) {
        self.init_image = Some(image);
        self.fp.init_image = self.init_image.as_ref().map_or_else(empty_image, image_view);
    }

    pub fn set_end_image(&mut self, image: Image) {
        self.end_image = Some(image);
        self.fp.end_image = self.end_image.as_ref().map_or_else(empty_image, image_view);
    }

    pub fn set_control_frames(&mut self, images: Vec<Image>) {
        self.control_frames = images;
        sync_image_views(&self.control_frames, &mut self.c_control_frames);
        self.fp.control_frames = if self.c_control_frames.is_empty() {
            ptr::null_mut()
        } else {
            self.c_control_frames.as_mut_ptr()
        };
        self.fp.control_frames_size = self.c_control_frames.len().min(i32::MAX as usize) as i32;
    }

    pub fn set_width(&mut self, width: i32) {
        self.fp.width = width;
    }

    pub fn set_height(&mut self, height: i32) {
        self.fp.height = height;
    }

    pub fn set_sample_params(&mut self, sample_params: SampleParams) {
        self.sample_params = Some(sample_params);
        self.sync_sample_params();
    }

    pub fn set_high_noise_sample_params(&mut self, high_noise_sample_params: SampleParams) {
        self.high_noise_sample_params = Some(high_noise_sample_params);
        self.sync_sample_params();
    }

    pub fn set_moe_boundary(&mut self, moe_boundary: f32) {
        self.fp.moe_boundary = moe_boundary;
    }

    pub fn set_strength(&mut self, strength: f32) {
        self.fp.strength = strength;
    }

    pub fn set_seed(&mut self, seed: i64) {
        self.fp.seed = seed;
    }

    pub fn set_video_frames(&mut self, video_frames: i32) {
        self.fp.video_frames = video_frames.max(1);
    }

    pub fn set_vace_strength(&mut self, vace_strength: f32) {
        self.fp.vace_strength = vace_strength;
    }

    pub fn set_vae_tiling_params(&mut self, vae_tiling_params: TilingParams) {
        self.fp.vae_tiling_params = vae_tiling_params.into();
    }

    pub fn set_cache(&mut self, cache: CacheParams) {
        self.cache = Some(cache);
        self.sync_cache();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    fn new_video_params() -> VideoParams {
        VideoParams {
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

    fn sample_image(value: u8) -> Image {
        Image { width: 2, height: 1, channel: 3, data: vec![value; 6] }
    }

    #[test]
    fn set_video_frames_clamps_to_at_least_one() {
        let mut params = new_video_params();

        params.set_video_frames(0);
        assert_eq!(params.fp.video_frames, 1);

        params.set_video_frames(12);
        assert_eq!(params.fp.video_frames, 12);
    }

    #[test]
    fn clone_resyncs_prompt_and_control_frame_views() {
        let mut params = new_video_params();
        params.set_prompt("animated skyline");
        params.set_negative_prompt("noisy");
        params.set_control_frames(vec![sample_image(4), sample_image(8)]);
        params.set_init_image(sample_image(1));
        params.set_end_image(sample_image(2));

        let cloned = params.clone();

        assert_eq!(
            unsafe { CStr::from_ptr(cloned.fp.prompt) }.to_str().unwrap(),
            "animated skyline"
        );
        assert_ne!(cloned.fp.prompt, params.fp.prompt);
        assert_eq!(cloned.fp.control_frames_size, 2);
        assert_eq!(cloned.fp.init_image.width, 2);
        assert_ne!(unsafe { (*cloned.fp.control_frames).data }, unsafe {
            (*params.fp.control_frames).data
        });
    }
}
