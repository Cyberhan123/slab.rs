use std::ptr;
use std::slice;

use libc::free;
use slab_diffusion_sys::{sd_image_t, sd_img_gen_params_t, sd_lora_t};

use crate::Diffusion;
use crate::params::support::{
    copy_and_free_c_string, empty_image, image_view, new_c_string, sync_image_views,
    sync_lora_views,
};
use crate::params::{CacheParams, Lora, PmParams, SampleParams, TilingParams};

/// Rust image container.
#[derive(Debug, Clone, Default)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub channel: u32,
    pub data: Vec<u8>,
}

impl From<sd_image_t> for Image {
    fn from(c_img: sd_image_t) -> Self {
        let len = (c_img.width as usize)
            .saturating_mul(c_img.height as usize)
            .saturating_mul(c_img.channel as usize);

        let data = if c_img.data.is_null() || len == 0 {
            Vec::new()
        } else {
            unsafe { slice::from_raw_parts(c_img.data, len) }.to_vec()
        };

        if !c_img.data.is_null() {
            unsafe { free(c_img.data.cast()) };
        }

        Image { width: c_img.width, height: c_img.height, channel: c_img.channel, data }
    }
}

pub(crate) fn owned_image_from_raw(raw: sd_image_t) -> Image {
    Image::from(raw)
}

pub struct ImgParams {
    pub(crate) fp: Box<sd_img_gen_params_t>,
    prompt: Option<std::ffi::CString>,
    negative_prompt: Option<std::ffi::CString>,
    loras: Vec<Lora>,
    lora_paths: Vec<std::ffi::CString>,
    c_loras: Vec<sd_lora_t>,
    init_image: Option<Image>,
    ref_images: Vec<Image>,
    c_ref_images: Vec<sd_image_t>,
    mask_image: Option<Image>,
    sample_params: Option<SampleParams>,
    control_image: Option<Image>,
    pm_params: Option<PmParams>,
    cache: Option<CacheParams>,
}

impl Clone for ImgParams {
    fn clone(&self) -> Self {
        let mut cloned = Self {
            fp: self.fp.clone(),
            prompt: self.prompt.clone(),
            negative_prompt: self.negative_prompt.clone(),
            loras: self.loras.clone(),
            lora_paths: self.lora_paths.clone(),
            c_loras: self.c_loras.clone(),
            init_image: self.init_image.clone(),
            ref_images: self.ref_images.clone(),
            c_ref_images: self.c_ref_images.clone(),
            mask_image: self.mask_image.clone(),
            sample_params: self.sample_params.clone(),
            control_image: self.control_image.clone(),
            pm_params: self.pm_params.clone(),
            cache: self.cache.clone(),
        };
        cloned.sync_backing();
        cloned
    }
}

impl std::fmt::Debug for ImgParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImgParams").finish_non_exhaustive()
    }
}

impl Diffusion {
    pub fn new_image_params(&self) -> ImgParams {
        let mut fp = Box::new(unsafe { std::mem::zeroed::<sd_img_gen_params_t>() });
        unsafe { self.lib.sd_img_gen_params_init(fp.as_mut()) };
        ImgParams {
            fp,
            prompt: None,
            negative_prompt: None,
            loras: Vec::new(),
            lora_paths: Vec::new(),
            c_loras: Vec::new(),
            init_image: None,
            ref_images: Vec::new(),
            c_ref_images: Vec::new(),
            mask_image: None,
            sample_params: None,
            control_image: None,
            pm_params: None,
            cache: None,
        }
    }

    pub fn image_params_to_str(&self, image_params: &ImgParams) -> Option<String> {
        let c_buf = unsafe { self.lib.sd_img_gen_params_to_str(&*image_params.fp) };
        copy_and_free_c_string(c_buf)
    }
}

impl ImgParams {
    fn sync_images(&mut self) {
        self.fp.init_image = self.init_image.as_ref().map_or_else(empty_image, image_view);
        sync_image_views(&self.ref_images, &mut self.c_ref_images);
        self.fp.ref_images = if self.c_ref_images.is_empty() {
            ptr::null_mut()
        } else {
            self.c_ref_images.as_mut_ptr()
        };
        self.fp.ref_images_count = self.c_ref_images.len().min(i32::MAX as usize) as i32;
        self.fp.mask_image = self.mask_image.as_ref().map_or_else(empty_image, image_view);
        self.fp.control_image = self.control_image.as_ref().map_or_else(empty_image, image_view);
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
    }

    fn sync_pm_params(&mut self) {
        if let Some(pm_params) = self.pm_params.as_mut() {
            self.fp.pm_params = pm_params.build_c_params();
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
        self.sync_pm_params();
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

    pub fn set_ref_images(&mut self, images: Vec<Image>) {
        self.ref_images = images;
        sync_image_views(&self.ref_images, &mut self.c_ref_images);
        self.fp.ref_images = if self.c_ref_images.is_empty() {
            ptr::null_mut()
        } else {
            self.c_ref_images.as_mut_ptr()
        };
        self.fp.ref_images_count = self.c_ref_images.len().min(i32::MAX as usize) as i32;
    }

    pub fn set_auto_resize_ref_image(&mut self, auto_resize: bool) {
        self.fp.auto_resize_ref_image = auto_resize;
    }

    pub fn set_increase_ref_index(&mut self, increase: bool) {
        self.fp.increase_ref_index = increase;
    }

    pub fn set_mask_image(&mut self, mask: Image) {
        self.mask_image = Some(mask);
        self.fp.mask_image = self.mask_image.as_ref().map_or_else(empty_image, image_view);
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

    pub fn set_strength(&mut self, strength: f32) {
        self.fp.strength = strength;
    }

    pub fn set_seed(&mut self, seed: i64) {
        self.fp.seed = seed;
    }

    pub fn set_batch_count(&mut self, batch_count: i32) {
        self.fp.batch_count = batch_count.max(1);
    }

    pub fn set_control_image(&mut self, control_image: Image) {
        self.control_image = Some(control_image);
        self.fp.control_image = self.control_image.as_ref().map_or_else(empty_image, image_view);
    }

    pub fn set_control_strength(&mut self, control_strength: f32) {
        self.fp.control_strength = control_strength;
    }

    pub fn set_pm_params(&mut self, pm_params: PmParams) {
        self.pm_params = Some(pm_params);
        self.sync_pm_params();
    }

    pub fn set_vae_tiling_params(&mut self, vae_tiling_params: TilingParams) {
        self.fp.vae_tiling_params = vae_tiling_params.into();
    }

    pub fn set_cache(&mut self, cache: CacheParams) {
        self.cache = Some(cache);
        self.sync_cache();
    }

    pub(crate) fn get_batch_count(&self) -> i32 {
        self.fp.batch_count.max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    fn new_img_params() -> ImgParams {
        ImgParams {
            fp: Box::new(unsafe { std::mem::zeroed::<sd_img_gen_params_t>() }),
            prompt: None,
            negative_prompt: None,
            loras: Vec::new(),
            lora_paths: Vec::new(),
            c_loras: Vec::new(),
            init_image: None,
            ref_images: Vec::new(),
            c_ref_images: Vec::new(),
            mask_image: None,
            sample_params: None,
            control_image: None,
            pm_params: None,
            cache: None,
        }
    }

    fn alloc_image_buffer(bytes: &[u8]) -> *mut u8 {
        let ptr = unsafe { libc::malloc(bytes.len()).cast::<u8>() };
        assert!(!ptr.is_null());
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len()) };
        ptr
    }

    fn sample_image(value: u8) -> Image {
        Image { width: 2, height: 1, channel: 3, data: vec![value; 6] }
    }

    #[test]
    fn image_from_raw_copies_pixels_and_handles_empty_images() {
        let raw = sd_image_t { width: 2, height: 1, channel: 3, data: alloc_image_buffer(&[1, 2, 3, 4, 5, 6]) };
        let image = Image::from(raw);

        assert_eq!(image.width, 2);
        assert_eq!(image.height, 1);
        assert_eq!(image.channel, 3);
        assert_eq!(image.data, vec![1, 2, 3, 4, 5, 6]);

        let empty = Image::from(sd_image_t { width: 0, height: 0, channel: 4, data: std::ptr::null_mut() });
        assert_eq!(empty.channel, 4);
        assert!(empty.data.is_empty());
    }

    #[test]
    fn batch_count_is_clamped_to_at_least_one() {
        let mut params = new_img_params();

        params.set_batch_count(0);
        assert_eq!(params.fp.batch_count, 1);
        assert_eq!(params.get_batch_count(), 1);

        params.fp.batch_count = -10;
        assert_eq!(params.get_batch_count(), 1);
    }

    #[test]
    fn clone_resyncs_owned_prompt_and_ref_images() {
        let mut params = new_img_params();
        params.set_prompt("A lovely cat");
        params.set_negative_prompt("blurry");
        params.set_ref_images(vec![sample_image(7)]);
        params.set_init_image(sample_image(3));

        let cloned = params.clone();

        assert_eq!(unsafe { CStr::from_ptr(params.fp.prompt) }.to_str().unwrap(), "A lovely cat");
        assert_eq!(unsafe { CStr::from_ptr(cloned.fp.prompt) }.to_str().unwrap(), "A lovely cat");
        assert_ne!(cloned.fp.prompt, params.fp.prompt);
        assert_eq!(cloned.fp.ref_images_count, 1);
        assert_eq!(cloned.fp.init_image.width, 2);
        assert_ne!(unsafe { (*cloned.fp.ref_images).data }, unsafe { (*params.fp.ref_images).data });
    }
}
