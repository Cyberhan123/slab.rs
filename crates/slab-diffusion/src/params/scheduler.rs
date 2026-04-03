/// Sigma schedule (scheduler).
use std::str::FromStr;

use serde::{Deserialize, Serialize};
pub use slab_diffusion_sys::scheduler_t;
/// scheduler parameters must keep code order
#[rustfmt::skip]
pub use slab_diffusion_sys::{
    scheduler_t_DISCRETE_SCHEDULER,
    scheduler_t_KARRAS_SCHEDULER,
    scheduler_t_EXPONENTIAL_SCHEDULER,
    scheduler_t_AYS_SCHEDULER,
    scheduler_t_GITS_SCHEDULER,
    scheduler_t_SGM_UNIFORM_SCHEDULER,
    scheduler_t_SIMPLE_SCHEDULER,
    scheduler_t_SMOOTHSTEP_SCHEDULER,
    scheduler_t_KL_OPTIMAL_SCHEDULER,
    scheduler_t_LCM_SCHEDULER,
    scheduler_t_BONG_TANGENT_SCHEDULER,
    scheduler_t_SCHEDULER_COUNT,
};

#[allow(non_camel_case_types)]
#[cfg_attr(any(not(windows), target_env = "gnu"), repr(u32))] // include windows-gnu
#[cfg_attr(all(windows, not(target_env = "gnu")), repr(i32))] // msvc being *special* again
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Scheduler {
    DISCRETE = scheduler_t_DISCRETE_SCHEDULER,
    KARRAS = scheduler_t_KARRAS_SCHEDULER,
    EXPONENTIAL = scheduler_t_EXPONENTIAL_SCHEDULER,
    AYS = scheduler_t_AYS_SCHEDULER,
    GITS = scheduler_t_GITS_SCHEDULER,
    SGM_UNIFORM = scheduler_t_SGM_UNIFORM_SCHEDULER,
    SIMPLE = scheduler_t_SIMPLE_SCHEDULER,
    SMOOTHSTEP = scheduler_t_SMOOTHSTEP_SCHEDULER,
    KL_OPTIMAL = scheduler_t_KL_OPTIMAL_SCHEDULER,
    LCM = scheduler_t_LCM_SCHEDULER,
    BONG_TANGENT = scheduler_t_BONG_TANGENT_SCHEDULER,
    UNKNOWN = scheduler_t_SCHEDULER_COUNT,
}

impl From<Scheduler> for scheduler_t {
    fn from(value: Scheduler) -> Self {
        value as scheduler_t
    }
}

impl From<scheduler_t> for Scheduler {
    fn from(value: scheduler_t) -> Self {
        #[allow(non_upper_case_globals)]
        match value {
            scheduler_t_DISCRETE_SCHEDULER => Scheduler::DISCRETE,
            scheduler_t_KARRAS_SCHEDULER => Scheduler::KARRAS,
            scheduler_t_EXPONENTIAL_SCHEDULER => Scheduler::EXPONENTIAL,
            scheduler_t_AYS_SCHEDULER => Scheduler::AYS,
            scheduler_t_GITS_SCHEDULER => Scheduler::GITS,
            scheduler_t_SGM_UNIFORM_SCHEDULER => Scheduler::SGM_UNIFORM,
            scheduler_t_SIMPLE_SCHEDULER => Scheduler::SIMPLE,
            scheduler_t_SMOOTHSTEP_SCHEDULER => Scheduler::SMOOTHSTEP,
            scheduler_t_KL_OPTIMAL_SCHEDULER => Scheduler::KL_OPTIMAL,
            scheduler_t_LCM_SCHEDULER => Scheduler::LCM,
            scheduler_t_BONG_TANGENT_SCHEDULER => Scheduler::BONG_TANGENT,
            _ => Scheduler::UNKNOWN, // Handle unknown values gracefully
        }
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::UNKNOWN
    }
}

impl FromStr for Scheduler {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "discrete" => Ok(Self::DISCRETE),
            "karras" => Ok(Self::KARRAS),
            "exponential" => Ok(Self::EXPONENTIAL),
            "ays" => Ok(Self::AYS),
            "gits" => Ok(Self::GITS),
            "sgm_uniform" => Ok(Self::SGM_UNIFORM),
            "simple" => Ok(Self::SIMPLE),
            "smoothstep" => Ok(Self::SMOOTHSTEP),
            "kl_optimal" => Ok(Self::KL_OPTIMAL),
            "lcm" => Ok(Self::LCM),
            "bong_tangent" => Ok(Self::BONG_TANGENT),
            other => Err(format!("unsupported scheduler: {other}")),
        }
    }
}
