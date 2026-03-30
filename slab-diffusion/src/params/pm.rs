use std::ffi::CString;
use std::ptr;

use slab_diffusion_sys::{sd_image_t, sd_pm_params_t};

use crate::params::support::{c_string_ptr, new_c_string, sync_image_views};
use crate::params::Image;

/// Rust mirror of `sd_pm_params_t`.
#[derive(Debug, Clone, Default)]
pub struct PmParams {
    pub id_images: Option<Vec<Image>>,
    pub id_embed_path: Option<String>,
    pub style_strength: f32,
    c_id_images: Vec<sd_image_t>,
    c_id_embed_path: Option<CString>,
}

impl PmParams {
    pub(crate) fn build_c_params(&mut self) -> sd_pm_params_t {
        self.c_id_images.clear();
        if let Some(images) = self.id_images.as_ref() {
            sync_image_views(images, &mut self.c_id_images);
        }

        self.c_id_embed_path = self.id_embed_path.as_deref().map(new_c_string);

        sd_pm_params_t {
            id_images: if self.c_id_images.is_empty() {
                ptr::null_mut()
            } else {
                self.c_id_images.as_mut_ptr()
            },
            id_images_count: self.c_id_images.len().min(i32::MAX as usize) as i32,
            id_embed_path: self.c_id_embed_path.as_ref().map_or(ptr::null(), c_string_ptr),
            style_strength: self.style_strength,
        }
    }
}
