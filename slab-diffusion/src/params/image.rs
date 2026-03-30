use crate::Diffusion;
use libc::free;
use slab_diffusion_sys::{sd_image_t, sd_img_gen_params_t, sd_lora_t};
use std::ffi::{CStr, CString};

use crate::params::{CacheParams, Lora, PmParams, SampleParams, TilingParams};
//std image struct, use for input or output image data, not directly used in FFI layer
#[derive(Debug, Clone, Default)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub channel: u32,
    pub data: Vec<u8>,
}

impl From<Image> for sd_image_t {
    fn from(image: Image) -> Self {
        let data = if image.data.is_empty() {
            std::ptr::null_mut()
        } else {
            let mut data = image.data;
            let ptr = data.as_mut_ptr();
            std::mem::forget(data);
            ptr
        };
        sd_image_t { width: image.width, height: image.height, channel: image.channel, data }
    }
}

impl From<sd_image_t> for Image {
    fn from(c_img: sd_image_t) -> Self {
        let len = (c_img.width * c_img.height * c_img.channel) as usize;
        let data = unsafe { std::slice::from_raw_parts(c_img.data, len).to_vec() };
        //free memory allocated by C code after copying data into Rust-owned Vec
        if !c_img.data.is_null() {
            unsafe {
                free(c_img.data as *mut libc::c_void);
            }
        }
        Image { width: c_img.width, height: c_img.height, channel: c_img.channel, data }
    }
}

#[derive(Clone)]
pub struct ImgParams {
    pub(crate) fp: Box<sd_img_gen_params_t>,
    // instance: Diffusion,
}

impl Diffusion {
    pub fn new_image_params(&self) -> ImgParams {
        let mut fp_box = Box::new(unsafe { std::mem::zeroed::<sd_img_gen_params_t>() });
        let ptr: *mut sd_img_gen_params_t = Box::into_raw(fp_box);
        unsafe {
            self.lib.sd_img_gen_params_init(ptr);
            fp_box = Box::from_raw(ptr);
        }
        ImgParams {
            fp: fp_box,
            // instance: self.clone()
        }
    }

    pub fn image_params_to_str(&self, image_params: ImgParams) -> Option<&'static str> {
        let c_buf = unsafe { self.lib.sd_img_gen_params_to_str(&*image_params.fp) };
        if c_buf.is_null() {
            None
        } else {
            let c_str = unsafe { CStr::from_ptr(c_buf) };
            Some(c_str.to_str().unwrap())
        }
    }
}

impl ImgParams {
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

    pub fn set_ref_images(&mut self, images: Vec<Image>) {
        let mut c_images: Vec<sd_image_t> = images.into_iter().map(|image| image.into()).collect();
        c_images.shrink_to_fit();
        self.fp.ref_images = c_images.as_ptr() as *mut sd_image_t;
        self.fp.ref_images_count = c_images.len() as i32;
    }

    pub fn set_auto_resize_ref_image(&mut self, auto_resize: bool) {
        self.fp.auto_resize_ref_image = auto_resize;
    }

    pub fn set_increase_ref_index(&mut self, increase: bool) {
        self.fp.increase_ref_index = increase;
    }

    pub fn set_mask_image(&mut self, mask: Image) {
        self.fp.mask_image = mask.into();
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

    pub fn set_strength(&mut self, strength: f32) {
        self.fp.strength = strength;
    }

    pub fn set_seed(&mut self, seed: i64) {
        self.fp.seed = seed;
    }

    pub fn set_batch_count(&mut self, batch_count: i32) {
        self.fp.batch_count = batch_count;
    }

    pub fn set_control_image(&mut self, control_image: Image) {
        self.fp.control_image = control_image.into();
    }

    pub fn set_control_strength(&mut self, control_strength: f32) {
        self.fp.control_strength = control_strength;
    }

    pub fn set_pm_params(&mut self, mut pm_params: PmParams) {
        self.fp.pm_params = pm_params.to_c_params();
    }

    pub fn set_vae_tiling_params(&mut self, vae_tiling_params: TilingParams) {
        self.fp.vae_tiling_params = vae_tiling_params.into();
    }

    pub fn set_cache(&mut self, cache: CacheParams) {
        self.fp.cache = *cache.fp;
    }

    pub(crate) fn get_batch_count(&self) -> i32 {
        self.fp.batch_count
    }
}
