use crate::params::Image;
use slab_diffusion_sys::{sd_image_t, sd_pm_params_t};

/// Rust mirror of `sd_pm_params_t`.
#[derive(Debug, Clone)]
pub struct PmParams {
    pub id_images: Option<Vec<Image>>,
    pub id_embed_path: Option<String>,
    pub style_strength: f32,
}

impl PmParams {
    pub(crate) fn to_c_params(&mut self) -> sd_pm_params_t {
        let mut c_images: Option<Vec<sd_image_t>> = None;
        let mut size = 0;

        if let Some(images) = self.id_images.take() {
            let mut c_images_vec: Vec<sd_image_t> =
                images.into_iter().map(|image| image.into()).collect();
            c_images_vec.shrink_to_fit();
            size = c_images_vec.len() as i32;
            c_images = Some(c_images_vec);
        }

        //TODO: check this carefully, make sure the memory is managed correctly
        sd_pm_params_t {
            id_images: c_images.map_or(std::ptr::null_mut(), |mut v| v.as_mut_ptr())
                as *mut sd_image_t,
            id_images_count: size,
            id_embed_path: self
                .id_embed_path
                .as_ref()
                .map(|s| std::ffi::CString::new(s.as_str()).unwrap())
                .map_or(std::ptr::null(), |c| c.into_raw()),
            style_strength: self.style_strength,
        }
    }
}
