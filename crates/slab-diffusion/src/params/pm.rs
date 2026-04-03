use std::ffi::CString;
use std::ptr;

use slab_diffusion_sys::{sd_image_t, sd_pm_params_t};

use crate::params::Image;
use crate::params::support::{c_string_ptr, new_c_string, sync_image_views};

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    fn sample_image(value: u8) -> Image {
        Image { width: 1, height: 1, channel: 3, data: vec![value; 3] }
    }

    #[test]
    fn build_c_params_handles_absent_fields() {
        let mut params = PmParams::default();
        let built = params.build_c_params();

        assert!(built.id_images.is_null());
        assert_eq!(built.id_images_count, 0);
        assert!(built.id_embed_path.is_null());
        assert_eq!(built.style_strength, 0.0);
    }

    #[test]
    fn build_c_params_exposes_images_and_embed_path() {
        let mut params = PmParams {
            id_images: Some(vec![sample_image(2), sample_image(9)]),
            id_embed_path: Some("photo-maker/embed.bin".to_owned()),
            style_strength: 0.75,
            ..PmParams::default()
        };

        let built = params.build_c_params();

        assert_eq!(built.id_images_count, 2);
        assert_eq!(
            unsafe { (*built.id_images).data },
            params.id_images.as_ref().unwrap()[0].data.as_ptr().cast_mut()
        );
        assert_eq!(
            unsafe { CStr::from_ptr(built.id_embed_path) }.to_str().unwrap(),
            "photo-maker/embed.bin"
        );
        assert_eq!(built.style_strength, 0.75);
    }
}
