use crate::params::guidance::GuidanceParams;
use crate::params::scheduler::Scheduler;
use crate::Diffusion;
use slab_diffusion_sys::sample_method_t;
use slab_diffusion_sys::sd_sample_params_t;

// Sampling parameters must keep code order
#[rustfmt::skip]
use slab_diffusion_sys::{
    sample_method_t_EULER_SAMPLE_METHOD,
    sample_method_t_EULER_A_SAMPLE_METHOD,
    sample_method_t_HEUN_SAMPLE_METHOD,
    sample_method_t_DPM2_SAMPLE_METHOD,
    sample_method_t_DPMPP2S_A_SAMPLE_METHOD,
    sample_method_t_DPMPP2M_SAMPLE_METHOD,
    sample_method_t_DPMPP2Mv2_SAMPLE_METHOD,
    sample_method_t_IPNDM_SAMPLE_METHOD,
    sample_method_t_IPNDM_V_SAMPLE_METHOD,
    sample_method_t_LCM_SAMPLE_METHOD,
    sample_method_t_DDIM_TRAILING_SAMPLE_METHOD,
    sample_method_t_TCD_SAMPLE_METHOD,
    sample_method_t_RES_MULTISTEP_SAMPLE_METHOD,
    sample_method_t_RES_2S_SAMPLE_METHOD,
    sample_method_t_SAMPLE_METHOD_COUNT,
};

#[allow(non_camel_case_types)]
#[cfg_attr(any(not(windows), target_env = "gnu"), repr(u32))] // include windows-gnu
#[cfg_attr(all(windows, not(target_env = "gnu")), repr(i32))] // msvc being *special* again
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
            _ => SampleMethod::Unknown, // Handle unknown values gracefully
        }
    }
}

/// Rust mirror of `sd_sample_params_t`.
#[derive(Debug, Clone)]
pub struct SampleParams {
    // pub guidance: GuidanceParams,
    // pub scheduler: Scheduler,
    // pub sample_method: Option<SampleMethod>,
    // pub sample_steps: i32,
    // pub eta: f32,
    // pub shifted_timestep: i32,
    // pub custom_sigmas: Vec<f32>,
    // pub flow_shift: Option<f32>,
    pub(crate) fp: Box<sd_sample_params_t>,
    // instance: Diffusion,
}

impl Diffusion {
    pub fn new_sample_params(&self) -> SampleParams {
        let mut fp_box = Box::new(unsafe { std::mem::zeroed::<sd_sample_params_t>() });
        let ptr: *mut sd_sample_params_t = Box::into_raw(fp_box);
        unsafe {
            self.lib.sd_sample_params_init(ptr);
            fp_box = Box::from_raw(ptr);
        }
        SampleParams {
            fp: fp_box,
            // instance: self.clone()
        }
    }

    pub fn sample_params_to_str(&self, sample_params: SampleParams) -> Option<&'static str> {
        let c_buf = unsafe { self.lib.sd_sample_params_to_str(&*sample_params.fp) };
        if c_buf.is_null() {
            None
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(c_buf) };
            Some(c_str.to_str().unwrap())
        }
    }
}

impl SampleParams {
    pub fn set_guidance(&mut self, guidance: GuidanceParams) {
        self.fp.guidance = guidance.into();
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

    //TODO: check lifetime of sigmas vec
    pub fn set_custom_sigmas(&mut self, mut sigmas: Vec<f32>) {
        self.fp.custom_sigmas = sigmas.as_mut_ptr();
        self.fp.custom_sigmas_count = sigmas.len() as i32;
    }

    pub fn set_flow_shift(&mut self, flow_shift: f32) {
        self.fp.flow_shift = flow_shift;
    }
}
