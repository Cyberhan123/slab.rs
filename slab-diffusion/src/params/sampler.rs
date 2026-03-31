use crate::Diffusion;
use crate::params::guidance::GuidanceParams;
use crate::params::scheduler::Scheduler;
use crate::params::support::copy_and_free_c_string;
use slab_diffusion_sys::sample_method_t;
use slab_diffusion_sys::sd_sample_params_t;
use std::ptr;

#[rustfmt::skip]
use slab_diffusion_sys::{
    sample_method_t_DDIM_TRAILING_SAMPLE_METHOD,
    sample_method_t_DPMPP2M_SAMPLE_METHOD,
    sample_method_t_DPMPP2Mv2_SAMPLE_METHOD,
    sample_method_t_DPMPP2S_A_SAMPLE_METHOD,
    sample_method_t_DPM2_SAMPLE_METHOD,
    sample_method_t_EULER_A_SAMPLE_METHOD,
    sample_method_t_EULER_SAMPLE_METHOD,
    sample_method_t_HEUN_SAMPLE_METHOD,
    sample_method_t_IPNDM_SAMPLE_METHOD,
    sample_method_t_IPNDM_V_SAMPLE_METHOD,
    sample_method_t_LCM_SAMPLE_METHOD,
    sample_method_t_RES_2S_SAMPLE_METHOD,
    sample_method_t_RES_MULTISTEP_SAMPLE_METHOD,
    sample_method_t_SAMPLE_METHOD_COUNT,
    sample_method_t_TCD_SAMPLE_METHOD,
};

#[allow(non_camel_case_types)]
#[cfg_attr(any(not(windows), target_env = "gnu"), repr(u32))]
#[cfg_attr(all(windows, not(target_env = "gnu")), repr(i32))]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum SampleMethod {
    Euler = sample_method_t_EULER_SAMPLE_METHOD,
    EULER_A = sample_method_t_EULER_A_SAMPLE_METHOD,

    HEUN = sample_method_t_HEUN_SAMPLE_METHOD,

    DPM2 = sample_method_t_DPM2_SAMPLE_METHOD,
    DPMPP2S_A = sample_method_t_DPMPP2S_A_SAMPLE_METHOD,
    DPMPP2M = sample_method_t_DPMPP2M_SAMPLE_METHOD,
    DPMPP2Mv2 = sample_method_t_DPMPP2Mv2_SAMPLE_METHOD,

    IPNDM = sample_method_t_IPNDM_SAMPLE_METHOD,
    IPNDM_V = sample_method_t_IPNDM_V_SAMPLE_METHOD,

    LCM = sample_method_t_LCM_SAMPLE_METHOD,

    DDIM_TRAILING = sample_method_t_DDIM_TRAILING_SAMPLE_METHOD,

    TCD = sample_method_t_TCD_SAMPLE_METHOD,

    RES_MULTISTEP = sample_method_t_RES_MULTISTEP_SAMPLE_METHOD,
    RES_2S = sample_method_t_RES_2S_SAMPLE_METHOD,

    Unknown = sample_method_t_SAMPLE_METHOD_COUNT,
}

impl From<SampleMethod> for sample_method_t {
    fn from(value: SampleMethod) -> Self {
        value as Self
    }
}

impl From<sample_method_t> for SampleMethod {
    fn from(value: sample_method_t) -> Self {
        #[allow(non_upper_case_globals)]
        match value {
            sample_method_t_EULER_SAMPLE_METHOD => SampleMethod::Euler,
            sample_method_t_EULER_A_SAMPLE_METHOD => SampleMethod::EULER_A,
            sample_method_t_HEUN_SAMPLE_METHOD => SampleMethod::HEUN,
            sample_method_t_DPM2_SAMPLE_METHOD => SampleMethod::DPM2,
            sample_method_t_DPMPP2S_A_SAMPLE_METHOD => SampleMethod::DPMPP2S_A,
            sample_method_t_DPMPP2M_SAMPLE_METHOD => SampleMethod::DPMPP2M,
            sample_method_t_DPMPP2Mv2_SAMPLE_METHOD => SampleMethod::DPMPP2Mv2,
            sample_method_t_IPNDM_SAMPLE_METHOD => SampleMethod::IPNDM,
            sample_method_t_IPNDM_V_SAMPLE_METHOD => SampleMethod::IPNDM_V,
            sample_method_t_LCM_SAMPLE_METHOD => SampleMethod::LCM,
            sample_method_t_DDIM_TRAILING_SAMPLE_METHOD => SampleMethod::DDIM_TRAILING,
            sample_method_t_TCD_SAMPLE_METHOD => SampleMethod::TCD,
            sample_method_t_RES_MULTISTEP_SAMPLE_METHOD => SampleMethod::RES_MULTISTEP,
            sample_method_t_RES_2S_SAMPLE_METHOD => SampleMethod::RES_2S,
            _ => SampleMethod::Unknown,
        }
    }
}

/// Rust mirror of `sd_sample_params_t`.
pub struct SampleParams {
    pub(crate) fp: Box<sd_sample_params_t>,
    guidance: Option<GuidanceParams>,
    custom_sigmas: Vec<f32>,
}

impl Clone for SampleParams {
    fn clone(&self) -> Self {
        let mut cloned = Self {
            fp: self.fp.clone(),
            guidance: self.guidance.clone(),
            custom_sigmas: self.custom_sigmas.clone(),
        };
        cloned.sync_backing();
        cloned
    }
}

impl std::fmt::Debug for SampleParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SampleParams").finish_non_exhaustive()
    }
}

impl Diffusion {
    pub fn new_sample_params(&self) -> SampleParams {
        let mut fp = Box::new(unsafe { std::mem::zeroed::<sd_sample_params_t>() });
        unsafe { self.lib.sd_sample_params_init(fp.as_mut()) };
        SampleParams { fp, guidance: None, custom_sigmas: Vec::new() }
    }

    pub fn sample_params_to_str(&self, sample_params: &SampleParams) -> Option<String> {
        let c_buf = unsafe { self.lib.sd_sample_params_to_str(&*sample_params.fp) };
        copy_and_free_c_string(c_buf)
    }
}

impl SampleParams {
    pub(crate) fn sync_backing(&mut self) {
        if let Some(guidance) = self.guidance.as_ref() {
            self.fp.guidance = guidance.build_c_params();
        }

        self.fp.custom_sigmas = if self.custom_sigmas.is_empty() {
            ptr::null_mut()
        } else {
            self.custom_sigmas.as_mut_ptr()
        };
        self.fp.custom_sigmas_count = self.custom_sigmas.len().min(i32::MAX as usize) as i32;
    }

    pub fn set_guidance(&mut self, guidance: GuidanceParams) {
        self.guidance = Some(guidance);
        self.sync_backing();
    }

    pub fn set_scheduler(&mut self, scheduler: Scheduler) {
        self.fp.scheduler = scheduler.into();
    }

    pub fn set_sample_method(&mut self, method: SampleMethod) {
        self.fp.sample_method = method.into();
    }

    pub fn set_sample_steps(&mut self, steps: i32) {
        self.fp.sample_steps = steps;
    }

    pub fn set_eta(&mut self, eta: f32) {
        self.fp.eta = eta;
    }

    pub fn set_shifted_timestep(&mut self, timestep: i32) {
        self.fp.shifted_timestep = timestep;
    }

    pub fn set_custom_sigmas(&mut self, sigmas: Vec<f32>) {
        self.custom_sigmas = sigmas;
        self.sync_backing();
    }

    pub fn set_flow_shift(&mut self, flow_shift: f32) {
        self.fp.flow_shift = flow_shift;
    }
}
