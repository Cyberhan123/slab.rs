use crate::params::slg::SlgParams;
use slab_diffusion_sys::sd_guidance_params_t;
/// Rust mirror of `sd_guidance_params_t`.
#[derive(Debug, Clone)]
pub struct GuidanceParams {
    pub txt_cfg: f32,
    pub img_cfg: f32,
    pub distilled_guidance: f32,
    pub slg: SlgParams,
}

impl From<GuidanceParams> for sd_guidance_params_t {
    fn from(mut value: GuidanceParams) -> Self {
        Self {
            txt_cfg: value.txt_cfg,
            img_cfg: value.img_cfg,
            distilled_guidance: value.distilled_guidance,
            slg: value.slg.to_c_params(),
        }
    }
}
