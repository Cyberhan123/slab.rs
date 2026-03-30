use crate::params::{CacheParams, Image, Lora, SampleParams, TilingParams};
use crate::Diffusion;
use slab_diffusion_sys::{sd_image_t, sd_lora_t, sd_vid_gen_params_t};
use std::ffi::CString;

#[derive(Debug, Clone, Default)]
pub struct Video {
    pub frames: Vec<Image>,
    pub num_frames: i32,
}

#[derive(Clone)]
pub struct VideoParams {
    pub(crate) fp: Box<sd_vid_gen_params_t>,
    // instance: Diffusion,
}

impl Diffusion {
    pub fn new_video_params(&self) -> VideoParams {
        let mut fp_box = Box::new(unsafe { std::mem::zeroed::<sd_vid_gen_params_t>() });
        let ptr: *mut sd_vid_gen_params_t = Box::into_raw(fp_box);
        unsafe {
            self.lib.sd_vid_gen_params_init(ptr);
            fp_box = Box::from_raw(ptr);
        }
        VideoParams {
            fp: fp_box,
            // instance: self.clone()
        }
    }
}

impl VideoParams {
    pub fn set_loras(&mut self, loras: Vec<Lora>) {
        let mut c_loras: Vec<sd_lora_t> = loras.into_iter().map(|lora| lora.into()).collect();
        c_loras.shrink_to_fit();

        self.fp.loras = c_loras.as_ptr() as *mut sd_lora_t;
        self.fp.lora_count = c_loras.len() as u32;
    }

    pub fn set_prompt(&mut self, prompt: &str) {
        self.fp.prompt = CString::new(prompt).unwrap().into_raw();
    }

    pub fn set_negative_prompt(&mut self, negative_prompt: &str) {
        self.fp.negative_prompt = CString::new(negative_prompt).unwrap().into_raw();
    }

    pub fn set_init_image(&mut self, image: Image) {
        self.fp.init_image = image.into();
    }

    pub fn set_end_image(&mut self, image: Image) {
        self.fp.end_image = image.into();
    }

    pub fn set_control_frames(&mut self, images: Vec<Image>) {
        let mut c_images: Vec<sd_image_t> = images.into_iter().map(|image| image.into()).collect();
        c_images.shrink_to_fit();
        self.fp.control_frames = c_images.as_ptr() as *mut sd_image_t;
        self.fp.control_frames_size = c_images.len() as i32;
    }

    pub fn set_width(&mut self, width: i32) {
        self.fp.width = width;
    }

    pub fn set_height(&mut self, height: i32) {
        self.fp.height = height;
    }

    // check this carefully
    pub fn set_sample_params(&mut self, sample_params: SampleParams) {
        self.fp.sample_params = *sample_params.fp;
    }

    // check this carefully
    pub fn set_high_noise_sample_params(&mut self, high_noise_sample_params: SampleParams) {
        self.fp.high_noise_sample_params = *high_noise_sample_params.fp;
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
        self.fp.video_frames = video_frames;
    }

    pub fn set_vace_strength(&mut self, vace_strength: f32) {
        self.fp.vace_strength = vace_strength;
    }

    pub fn set_vae_tiling_params(&mut self, vae_tiling_params: TilingParams) {
        self.fp.vae_tiling_params = vae_tiling_params.into();
    }

    pub fn set_cache(&mut self, cache: CacheParams) {
        self.fp.cache = *cache.fp;
    }
}
