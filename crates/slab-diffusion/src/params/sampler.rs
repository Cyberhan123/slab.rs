use std::ptr;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use slab_diffusion_sys::sample_method_t;
use slab_diffusion_sys::sd_sample_params_t;

use crate::Diffusion;
use crate::params::guidance::GuidanceParams;
use crate::params::scheduler::{Scheduler, scheduler_t};

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
#[derive(Debug, Copy, Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
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
    #[default]
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

impl FromStr for SampleMethod {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "euler" => Ok(Self::Euler),
            "euler_a" => Ok(Self::EULER_A),
            "heun" => Ok(Self::HEUN),
            "dpm2" => Ok(Self::DPM2),
            "dpm++2s_a" => Ok(Self::DPMPP2S_A),
            "dpm++2m" => Ok(Self::DPMPP2M),
            "dpm++2mv2" => Ok(Self::DPMPP2Mv2),
            "ipndm" => Ok(Self::IPNDM),
            "ipndm_v" => Ok(Self::IPNDM_V),
            "lcm" => Ok(Self::LCM),
            "ddim_trailing" => Ok(Self::DDIM_TRAILING),
            "tcd" => Ok(Self::TCD),
            "res_multistep" => Ok(Self::RES_MULTISTEP),
            "res_2s" => Ok(Self::RES_2S),
            other => Err(format!("unsupported sample_method: {other}")),
        }
    }
}

/// Stable Rust-native sampling parameters shared across the runtime chain.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SampleParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guidance: Option<GuidanceParams>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduler: Option<Scheduler>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_method: Option<SampleMethod>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_steps: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shifted_timestep: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_sigmas: Option<Vec<f32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow_shift: Option<f32>,
}

/// FFI-only sampling parameter backing struct.
pub(crate) struct InnerSampleParams {
    pub(crate) fp: Box<sd_sample_params_t>,
    guidance: Option<GuidanceParams>,
    custom_sigmas: Vec<f32>,
}

impl Clone for InnerSampleParams {
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

impl Default for InnerSampleParams {
    fn default() -> Self {
        Self {
            fp: Box::new(unsafe { std::mem::zeroed::<sd_sample_params_t>() }),
            guidance: None,
            custom_sigmas: Vec::new(),
        }
    }
}

impl std::fmt::Debug for InnerSampleParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InnerSampleParams").finish_non_exhaustive()
    }
}

impl Diffusion {
    pub fn sample_params_to_str(&self, sample_params: &SampleParams) -> Option<String> {
        Some(format!("{sample_params:#?}"))
    }
}

impl InnerSampleParams {
    pub(crate) fn with_native_init(lib: &slab_diffusion_sys::DiffusionLib) -> Self {
        let mut inner = Self::default();
        unsafe { lib.sd_sample_params_init(inner.fp.as_mut()) };
        inner
    }

    pub(crate) fn from_canonical(
        lib: &slab_diffusion_sys::DiffusionLib,
        value: &SampleParams,
    ) -> Result<Self, String> {
        let mut inner = InnerSampleParams::with_native_init(lib);

        if let Some(guidance) = value.guidance.clone() {
            inner.set_guidance(guidance);
        }
        if let Some(scheduler) = value.scheduler {
            inner.set_scheduler(scheduler);
        }
        if let Some(sample_method) = value.sample_method {
            inner.set_sample_method(sample_method);
        }
        if let Some(sample_steps) = value.sample_steps {
            if sample_steps < 1 {
                return Err(format!("sample_steps must be >= 1, got {sample_steps}"));
            }
            inner.set_sample_steps(sample_steps);
        }
        if let Some(eta) = value.eta {
            inner.set_eta(eta);
        }
        if let Some(shifted_timestep) = value.shifted_timestep {
            inner.set_shifted_timestep(shifted_timestep);
        }
        if let Some(custom_sigmas) = value.custom_sigmas.clone() {
            inner.set_custom_sigmas(custom_sigmas);
        }
        if let Some(flow_shift) = value.flow_shift {
            inner.set_flow_shift(flow_shift);
        }

        // important: must set a sample method and scheduler for the native layer to consider the params valid
        if inner.fp.sample_method == sample_method_t::from(SampleMethod::Unknown) {
            inner.set_sample_method(SampleMethod::Euler);
        }

        if inner.fp.scheduler == scheduler_t::from(Scheduler::UNKNOWN) {
            inner.set_scheduler(Scheduler::DISCRETE);
        }

        Ok(inner)
    }

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

    fn set_guidance(&mut self, guidance: GuidanceParams) {
        self.guidance = Some(guidance);
        self.sync_backing();
    }

    fn set_scheduler(&mut self, scheduler: Scheduler) {
        self.fp.scheduler = scheduler.into();
    }

    fn set_sample_method(&mut self, method: SampleMethod) {
        self.fp.sample_method = method.into();
    }

    fn set_sample_steps(&mut self, steps: i32) {
        self.fp.sample_steps = steps;
    }

    fn set_eta(&mut self, eta: f32) {
        self.fp.eta = eta;
    }

    fn set_shifted_timestep(&mut self, timestep: i32) {
        self.fp.shifted_timestep = timestep;
    }

    fn set_custom_sigmas(&mut self, sigmas: Vec<f32>) {
        self.custom_sigmas = sigmas;
        self.sync_backing();
    }

    fn set_flow_shift(&mut self, flow_shift: f32) {
        self.fp.flow_shift = flow_shift;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::SlgParams;

    #[test]
    fn sample_method_and_scheduler_round_trip_known_and_unknown_values() {
        let scheduler: slab_diffusion_sys::scheduler_t = Scheduler::KARRAS.into();

        assert_eq!(SampleMethod::from(sample_method_t_EULER_SAMPLE_METHOD), SampleMethod::Euler);
        assert_eq!(SampleMethod::from(sample_method_t_SAMPLE_METHOD_COUNT), SampleMethod::Unknown);
        assert_eq!(Scheduler::from(scheduler), Scheduler::KARRAS);
        assert_eq!(
            Scheduler::from(slab_diffusion_sys::scheduler_t_SCHEDULER_COUNT),
            Scheduler::UNKNOWN
        );
    }

    #[test]
    fn canonical_sample_params_sync_nested_backing_fields() {
        let params = SampleParams {
            guidance: Some(GuidanceParams {
                txt_cfg: 7.5,
                img_cfg: 1.25,
                distilled_guidance: 2.0,
                slg: SlgParams {
                    layers: vec![1, 4, 7],
                    layer_start: 0.1,
                    layer_end: 0.9,
                    scale: 0.8,
                },
            }),
            scheduler: Some(Scheduler::LCM),
            sample_method: Some(SampleMethod::DPM2),
            sample_steps: Some(12),
            custom_sigmas: Some(vec![0.1, 0.2, 0.3]),
            ..Default::default()
        };

        let mut inner = InnerSampleParams::default();
        inner.set_guidance(params.guidance.clone().expect("guidance should be present"));
        inner.set_scheduler(params.scheduler.expect("scheduler should be set"));
        inner.set_sample_method(params.sample_method.expect("sample method should be set"));
        inner.set_sample_steps(params.sample_steps.expect("sample_steps should be set"));
        inner.set_custom_sigmas(params.custom_sigmas.clone().expect("custom sigmas should be set"));

        let guidance = params.guidance.expect("guidance should be present");
        assert_eq!(inner.fp.guidance.txt_cfg, guidance.txt_cfg);
        assert_eq!(inner.fp.guidance.img_cfg, guidance.img_cfg);
        assert_eq!(inner.fp.guidance.slg.layer_count, 3);
        assert_eq!(
            unsafe { std::slice::from_raw_parts(inner.fp.guidance.slg.layers, 3) },
            &[1, 4, 7]
        );
        assert_eq!(inner.fp.custom_sigmas_count, 3);
        assert_eq!(
            unsafe { std::slice::from_raw_parts(inner.fp.custom_sigmas, 3) },
            &[0.1, 0.2, 0.3]
        );
        assert_eq!(Scheduler::from(inner.fp.scheduler), Scheduler::LCM);
        assert_eq!(SampleMethod::from(inner.fp.sample_method), SampleMethod::DPM2);
    }

    #[test]
    fn clone_rebinds_custom_sigma_storage() {
        let mut inner = InnerSampleParams::default();
        inner.set_custom_sigmas(vec![0.5, 0.75]);
        let cloned = inner.clone();

        assert_eq!(cloned.fp.custom_sigmas_count, 2);
        assert_ne!(cloned.fp.custom_sigmas, inner.fp.custom_sigmas);
        assert_eq!(unsafe { std::slice::from_raw_parts(cloned.fp.custom_sigmas, 2) }, &[0.5, 0.75]);
    }
}
