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

impl GuidanceParams {
    pub(crate) fn build_c_params(&self) -> sd_guidance_params_t {
        sd_guidance_params_t {
            txt_cfg: self.txt_cfg,
            img_cfg: self.img_cfg,
            distilled_guidance: self.distilled_guidance,
            slg: self.slg.build_c_params(),
        }
    }
}
