/// Self-guidance layer configuration.
use slab_diffusion_sys::sd_slg_params_t;

#[derive(Debug, Clone)]
pub struct SlgParams {
    pub layers: Vec<i32>,
    pub layer_start: f32,
    pub layer_end: f32,
    pub scale: f32,
}

impl SlgParams {
    pub(crate) fn to_c_params(&mut self) -> sd_slg_params_t {
        sd_slg_params_t {
            layers: self.layers.as_mut_ptr(),
            layer_count: self.layers.len(),
            layer_start: self.layer_start,
            layer_end: self.layer_end,
            scale: self.scale,
        }
    }
}
