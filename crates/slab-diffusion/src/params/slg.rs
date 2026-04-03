/// Self-guidance layer configuration.
use serde::{Deserialize, Serialize};
use slab_diffusion_sys::sd_slg_params_t;
use std::ptr;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SlgParams {
    pub layers: Vec<i32>,
    pub layer_start: f32,
    pub layer_end: f32,
    pub scale: f32,
}

impl SlgParams {
    pub(crate) fn build_c_params(&self) -> sd_slg_params_t {
        sd_slg_params_t {
            layers: if self.layers.is_empty() {
                ptr::null_mut()
            } else {
                self.layers.as_ptr().cast_mut()
            },
            layer_count: self.layers.len(),
            layer_start: self.layer_start,
            layer_end: self.layer_end,
            scale: self.scale,
        }
    }
}
