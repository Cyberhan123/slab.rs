use std::ffi::CString;
use std::path::PathBuf;
use std::ptr;

use serde::{Deserialize, Serialize};
use slab_diffusion_sys::{sd_image_t, sd_pm_params_t};

use crate::params::Image;
use crate::params::support::{c_string_ptr, new_c_string, sync_image_views};

/// Rust mirror of `sd_pm_params_t`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PmParams {
    pub id_images: Option<Vec<Image>>,
    pub id_embed_path: Option<PathBuf>,
    pub style_strength: f32,
}

pub(crate) struct InnerPmParams {
    pub(crate) fp: sd_pm_params_t,
    canonical: PmParams,
    c_id_images: Vec<sd_image_t>,
    c_id_embed_path: Option<CString>,
}

impl Clone for InnerPmParams {
    fn clone(&self) -> Self {
        Self::from_canonical(self.canonical.clone())
    }
}

impl InnerPmParams {
    pub(crate) fn from_canonical(canonical: PmParams) -> Self {
        let mut inner = Self {
            fp: unsafe { std::mem::zeroed::<sd_pm_params_t>() },
            canonical,
            c_id_images: Vec::new(),
            c_id_embed_path: None,
        };
        inner.sync_backing();
        inner
    }

    pub(crate) fn sync_backing(&mut self) {
        self.c_id_images.clear();
        if let Some(images) = self.canonical.id_images.as_ref() {
            sync_image_views(images, &mut self.c_id_images);
        }

        self.c_id_embed_path =
            self.canonical.id_embed_path.as_ref().map(|path| new_c_string(&path.to_string_lossy()));

        self.fp = sd_pm_params_t {
            id_images: if self.c_id_images.is_empty() {
                ptr::null_mut()
            } else {
                self.c_id_images.as_mut_ptr()
            },
            id_images_count: self.c_id_images.len().min(i32::MAX as usize) as i32,
            id_embed_path: self.c_id_embed_path.as_ref().map_or(ptr::null(), c_string_ptr),
            style_strength: self.canonical.style_strength,
        };
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
    fn inner_pm_params_handles_absent_fields() {
        let inner = InnerPmParams::from_canonical(PmParams::default());

        assert!(inner.fp.id_images.is_null());
        assert_eq!(inner.fp.id_images_count, 0);
        assert!(inner.fp.id_embed_path.is_null());
        assert_eq!(inner.fp.style_strength, 0.0);
    }

    #[test]
    fn inner_pm_params_exposes_images_and_embed_path() {
        let inner = InnerPmParams::from_canonical(PmParams {
            id_images: Some(vec![sample_image(2), sample_image(9)]),
            id_embed_path: Some(PathBuf::from("photo-maker/embed.bin")),
            style_strength: 0.75,
        });

        assert_eq!(inner.fp.id_images_count, 2);
        assert_eq!(
            unsafe { (*inner.fp.id_images).data },
            inner.canonical.id_images.as_ref().unwrap()[0].data.as_ptr().cast_mut()
        );
        assert_eq!(
            unsafe { CStr::from_ptr(inner.fp.id_embed_path) }.to_str().unwrap(),
            "photo-maker/embed.bin"
        );
        assert_eq!(inner.fp.style_strength, 0.75);
    }
}
