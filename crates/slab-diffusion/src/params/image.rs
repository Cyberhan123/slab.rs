use std::ptr;
use std::slice;

use serde::{Deserialize, Serialize};
use slab_diffusion_sys::{sd_image_t, sd_img_gen_params_t, sd_lora_t};

use crate::Diffusion;
use crate::params::support::{
    empty_image, image_view, new_c_string, sync_image_views, sync_lora_views,
};
use crate::params::{
    CacheParams, InnerCacheParams, InnerPmParams, InnerSampleParams, Lora, PmParams, SampleParams,
    TilingParams,
};

/// Rust image container.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub channel: u32,
    pub data: Vec<u8>,
}

pub struct InnerImage {
    pub(crate) fp: sd_image_t,
    data: Option<Vec<u8>>,
}

impl From<Image> for InnerImage {
    fn from(img: Image) -> Self {
        let mut data = img.data;
        let ptr = data.as_mut_ptr();

        InnerImage {
            fp: sd_image_t {
                width: img.width,
                height: img.height,
                channel: img.channel,
                data: ptr,
            },
            data: Some(data),
        }
    }
}

impl From<InnerImage> for Image {
    fn from(mut inner: InnerImage) -> Self {
        let data = if let Some(owned_vec) = inner.data.take() {
            owned_vec
        } else {
            copy_image_data(inner.fp)
        };

        Image { width: inner.fp.width, height: inner.fp.height, channel: inner.fp.channel, data }
    }
}

pub(crate) fn owned_image_from_raw(raw: sd_image_t) -> Image {
    Image { width: raw.width, height: raw.height, channel: raw.channel, data: copy_image_data(raw) }
}

fn copy_image_data(raw: sd_image_t) -> Vec<u8> {
    let len = (raw.width * raw.height * raw.channel) as usize;

    if raw.data.is_null() || len == 0 {
        Vec::new()
    } else {
        unsafe { slice::from_raw_parts(raw.data, len) }.to_vec()
    }
}

/// Stable Rust-native image inference parameters shared across the runtime chain.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ImgParams {
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
    pub ref_images: Option<Vec<Image>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_resize_ref_image: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub increase_ref_index: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_image: Option<Image>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_params: Option<SampleParams>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strength: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_image: Option<Image>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_strength: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pm_params: Option<PmParams>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vae_tiling_params: Option<TilingParams>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<CacheParams>,
}

/// FFI-only image inference parameter backing struct.
pub(crate) struct InnerImgParams {
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
    sample_params: Option<InnerSampleParams>,
    control_image: Option<Image>,
    pm_params: Option<InnerPmParams>,
    cache: Option<InnerCacheParams>,
}

impl Clone for InnerImgParams {
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

impl Default for InnerImgParams {
    fn default() -> Self {
        Self {
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
}

impl std::fmt::Debug for InnerImgParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InnerImgParams").finish_non_exhaustive()
    }
}

impl Diffusion {
    pub fn image_params_to_str(&self, image_params: &ImgParams) -> Option<String> {
        Some(format!("{image_params:#?}"))
    }
}

impl InnerImgParams {
    pub(crate) fn with_native_init(lib: &slab_diffusion_sys::DiffusionLib) -> Self {
        let mut inner = Self::default();
        unsafe { lib.sd_img_gen_params_init(inner.fp.as_mut()) };
        inner
    }

    pub(crate) fn from_canonical(
        lib: &slab_diffusion_sys::DiffusionLib,
        value: &ImgParams,
    ) -> Result<Self, String> {
        let mut inner = InnerImgParams::with_native_init(lib);

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
        if let Some(ref_images) = value.ref_images.clone() {
            inner.set_ref_images(ref_images);
        }
        if let Some(auto_resize_ref_image) = value.auto_resize_ref_image {
            inner.set_auto_resize_ref_image(auto_resize_ref_image);
        }
        if let Some(increase_ref_index) = value.increase_ref_index {
            inner.set_increase_ref_index(increase_ref_index);
        }
        if value.mask_image.is_some() {
            inner.set_mask_image(value.mask_image.clone());
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
        if let Some(strength) = value.strength {
            inner.set_strength(strength);
        }
        if let Some(seed) = value.seed {
            inner.set_seed(seed);
        }
        if let Some(batch_count) = value.batch_count {
            if batch_count < 1 {
                return Err(format!("batch_count must be >= 1, got {batch_count}"));
            }
            inner.set_batch_count(
                i32::try_from(batch_count)
                    .map_err(|_| format!("batch_count {batch_count} exceeds i32 range"))?,
            );
        }
        if value.control_image.is_some() {
            inner.set_control_image(value.control_image.clone());
        }
        if let Some(control_strength) = value.control_strength {
            inner.set_control_strength(control_strength);
        }
        if value.pm_params.is_some() {
            inner.set_pm_params(value.pm_params.clone());
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
            pm_params.sync_backing();
            self.fp.pm_params = pm_params.fp;
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

    fn set_ref_images(&mut self, images: Vec<Image>) {
        self.ref_images = images;
        sync_image_views(&self.ref_images, &mut self.c_ref_images);
        self.fp.ref_images = if self.c_ref_images.is_empty() {
            ptr::null_mut()
        } else {
            self.c_ref_images.as_mut_ptr()
        };
        self.fp.ref_images_count = self.c_ref_images.len().min(i32::MAX as usize) as i32;
    }

    fn set_auto_resize_ref_image(&mut self, auto_resize: bool) {
        self.fp.auto_resize_ref_image = auto_resize;
    }

    fn set_increase_ref_index(&mut self, increase: bool) {
        self.fp.increase_ref_index = increase;
    }

    fn set_mask_image(&mut self, mask: Option<Image>) {
        self.mask_image = mask;
        self.fp.mask_image = self.mask_image.as_ref().map_or_else(empty_image, image_view);
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

    fn set_strength(&mut self, strength: f32) {
        self.fp.strength = strength;
    }

    fn set_seed(&mut self, seed: i64) {
        self.fp.seed = seed;
    }

    fn set_batch_count(&mut self, batch_count: i32) {
        self.fp.batch_count = batch_count;
    }

    fn set_control_image(&mut self, control_image: Option<Image>) {
        self.control_image = control_image;
        self.fp.control_image = self.control_image.as_ref().map_or_else(empty_image, image_view);
    }

    fn set_control_strength(&mut self, control_strength: f32) {
        self.fp.control_strength = control_strength;
    }

    fn set_pm_params(&mut self, pm_params: Option<PmParams>) {
        self.pm_params = pm_params.map(InnerPmParams::from_canonical);
        self.sync_pm_params();
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

    pub(crate) fn get_batch_count(&self) -> i32 {
        self.fp.batch_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

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
        let raw = sd_image_t {
            width: 2,
            height: 1,
            channel: 3,
            data: alloc_image_buffer(&[1, 2, 3, 4, 5, 6]),
        };
        let image = owned_image_from_raw(raw);
        unsafe { libc::free(raw.data.cast()) };

        assert_eq!(image.width, 2);
        assert_eq!(image.height, 1);
        assert_eq!(image.channel, 3);
        assert_eq!(image.data, vec![1, 2, 3, 4, 5, 6]);

        let empty = owned_image_from_raw(sd_image_t {
            width: 0,
            height: 0,
            channel: 4,
            data: std::ptr::null_mut(),
        });
        assert_eq!(empty.channel, 4);
        assert!(empty.data.is_empty());
    }

    #[test]
    fn clone_resyncs_owned_prompt_and_ref_images() {
        let params = ImgParams {
            prompt: Some("A lovely cat".to_owned()),
            negative_prompt: Some("blurry".to_owned()),
            ref_images: Some(vec![sample_image(7)]),
            init_image: Some(sample_image(3)),
            ..Default::default()
        };
        let mut inner = InnerImgParams::default();
        inner.set_prompt(params.prompt.as_deref().expect("prompt should be set"));
        inner.set_negative_prompt(params.negative_prompt.as_deref());
        inner.set_ref_images(params.ref_images.clone().expect("ref images should be set"));
        inner.set_init_image(params.init_image.clone());
        let cloned = inner.clone();

        assert_eq!(unsafe { CStr::from_ptr(inner.fp.prompt) }.to_str().unwrap(), "A lovely cat");
        assert_eq!(unsafe { CStr::from_ptr(cloned.fp.prompt) }.to_str().unwrap(), "A lovely cat");
        assert_ne!(cloned.fp.prompt, inner.fp.prompt);
        assert_eq!(cloned.fp.ref_images_count, 1);
        assert_eq!(cloned.fp.init_image.width, 2);
        assert_ne!(unsafe { (*cloned.fp.ref_images).data }, unsafe { (*inner.fp.ref_images).data });
    }
}
