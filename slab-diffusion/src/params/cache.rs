/// Cache mode for inference acceleration.
use slab_diffusion_sys::{   sd_cache_mode_t, sd_cache_params_t};

/// cache parameters must keep code order
#[rustfmt::skip]
use slab_diffusion_sys::{
    sd_cache_mode_t_SD_CACHE_DISABLED,
    sd_cache_mode_t_SD_CACHE_EASYCACHE,
    sd_cache_mode_t_SD_CACHE_UCACHE,
    sd_cache_mode_t_SD_CACHE_DBCACHE,
    sd_cache_mode_t_SD_CACHE_TAYLORSEER,
    sd_cache_mode_t_SD_CACHE_CACHE_DIT,
    sd_cache_mode_t_SD_CACHE_SPECTRUM,
};

use crate::Diffusion;

#[cfg_attr(any(not(windows), target_env = "gnu"), repr(u32))] // include windows-gnu
#[cfg_attr(all(windows, not(target_env = "gnu")), repr(i32))] // msvc being *special* again
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum CacheMode {
    DISABLED = sd_cache_mode_t_SD_CACHE_DISABLED,
    EASYCACHE = sd_cache_mode_t_SD_CACHE_EASYCACHE,
    UCACHE = sd_cache_mode_t_SD_CACHE_UCACHE,
    DBCACHE = sd_cache_mode_t_SD_CACHE_DBCACHE,
    TAYLORSEER = sd_cache_mode_t_SD_CACHE_TAYLORSEER,
    DIT = sd_cache_mode_t_SD_CACHE_CACHE_DIT,
    SPECTRUM = sd_cache_mode_t_SD_CACHE_SPECTRUM,
}

impl From<CacheMode> for sd_cache_mode_t {
    fn from(value: CacheMode) -> Self {
        value as Self
    }
}

/// Cache tuning parameters.
#[derive(Clone)]

pub struct CacheParams {
    // pub mode: Option<String>,
    // pub reuse_threshold: Option<f32>,
    // pub start_percent:  Option<f32>,
    // pub end_percent: Option<f32>,
    // pub error_decay_rate: Option<f32>,
    // pub use_relative_threshold: Option<bool>,
    // pub reset_error_on_compute: Option<bool>,
    // pub fn_compute_blocks: Option<i32>,
    // pub bn_compute_blocks: Option<i32>,
    // pub residual_diff_threshold: Option<f32>,
    // pub max_warmup_steps: Option<i32>,
    // pub max_cached_steps: Option<i32>,
    // pub max_continuous_cached_steps: Option<i32>,
    // pub taylorseer_n_derivatives: Option<i32>,
    // pub taylorseer_skip_interval: Option<i32>,
    // #[builder(setter(into, strip_option), default)]
    // pub scm_mask: Option<String>,
    // pub scm_policy_dynamic: bool,
    // #[builder(setter(strip_option), default)]
    // pub spectrum_w: Option<f32>,
    // #[builder(setter(strip_option), default)]
    // pub spectrum_m: Option<i32>,
    // #[builder(setter(strip_option), default)]
    // pub spectrum_lam: Option<f32>,
    // #[builder(setter(strip_option), default)]
    // pub spectrum_window_size: Option<i32>,
    // #[builder(setter(strip_option), default)]
    // pub spectrum_flex_window: Option<f32>,
    // #[builder(setter(strip_option), default)]
    // pub spectrum_warmup_steps: Option<i32>,
    // #[builder(setter(strip_option), default)]
    // pub spectrum_stop_percent: Option<f32>,

    pub(crate) fp: Box<sd_cache_params_t>,
    // instance: Diffusion
}

impl Diffusion {
    pub fn new_cache_params(&self) -> CacheParams {
            let mut fp_box = Box::new(unsafe { std::mem::zeroed::<sd_cache_params_t>() });
        let ptr: *mut sd_cache_params_t = Box::into_raw(fp_box);
        unsafe {
            self.lib.sd_cache_params_init(ptr);
            fp_box = Box::from_raw(ptr);
        }
        CacheParams { 
            fp: fp_box, 
            // instance: self.clone() 
        }
    }
}

impl CacheParams {
   pub fn set_mode(&mut self, mode: CacheMode) {
        self.fp.mode = mode.into();
    }

    pub fn set_reuse_threshold(&mut self, reuse_threshold: f32) {
        self.fp.reuse_threshold = reuse_threshold;
    }

    pub fn set_start_percent(&mut self, start_percent: f32) {
        self.fp.start_percent = start_percent;
    }

    pub fn set_end_percent(&mut self, end_percent: f32) {
        self.fp.end_percent = end_percent;
    }

    pub fn set_error_decay_rate(&mut self, error_decay_rate: f32) {
        self.fp.error_decay_rate = error_decay_rate;
    }

    pub fn set_use_relative_threshold(&mut self, use_relative_threshold: bool) {
        self.fp.use_relative_threshold = use_relative_threshold;
    }

    pub fn set_reset_error_on_compute(&mut self, reset_error_on_compute: bool) {
        self.fp.reset_error_on_compute = reset_error_on_compute;
    }

    pub fn set_fn_compute_blocks(&mut self, fn_compute_blocks: i32) {
        self.fp.Fn_compute_blocks = fn_compute_blocks;
    }

    pub fn set_bn_compute_blocks(&mut self, bn_compute_blocks: i32) {
        self.fp.Bn_compute_blocks = bn_compute_blocks;
    }

    pub fn set_residual_diff_threshold(&mut self, residual_diff_threshold: f32) {
        self.fp.residual_diff_threshold = residual_diff_threshold;
    }

    pub fn set_max_warmup_steps(&mut self, max_warmup_steps: i32) {
        self.fp.max_warmup_steps = max_warmup_steps;
    }

    pub fn set_max_cached_steps(&mut self, max_cached_steps: i32) {
        self.fp.max_cached_steps = max_cached_steps;
    }

    pub fn set_max_continuous_cached_steps(&mut self, max_continuous_cached_steps: i32) {
        self.fp.max_continuous_cached_steps = max_continuous_cached_steps;
    }

    pub fn set_taylorseer_n_derivatives(&mut self, taylorseer_n_derivatives: i32) {
        self.fp.taylorseer_n_derivatives = taylorseer_n_derivatives;
    }

    pub fn set_taylorseer_skip_interval(&mut self, taylorseer_skip_interval: i32) {
        self.fp.taylorseer_skip_interval = taylorseer_skip_interval;
    }

    pub fn set_scm_mask(&mut self, scm_mask: String) {
        let c_string = std::ffi::CString::new(scm_mask).unwrap();
        self.fp.scm_mask = c_string.into_raw();
    }

    pub fn set_scm_policy_dynamic(&mut self, scm_policy_dynamic: bool) {
        self.fp.scm_policy_dynamic = scm_policy_dynamic;
    }

    pub fn set_spectrum_w(&mut self, spectrum_w: f32) {
        self.fp.spectrum_w = spectrum_w;
    }

    pub fn set_spectrum_m(&mut self, spectrum_m: i32) {
        self.fp.spectrum_m = spectrum_m;
    }

    pub fn set_spectrum_lam(&mut self, spectrum_lam: f32) {
        self.fp.spectrum_lam = spectrum_lam;
    }

    pub fn set_spectrum_window_size(&mut self, spectrum_window_size: i32) {
        self.fp.spectrum_window_size = spectrum_window_size;
    }

    pub fn set_spectrum_flex_window(&mut self, spectrum_flex_window: f32) {
        self.fp.spectrum_flex_window = spectrum_flex_window;
    }

    pub fn set_spectrum_warmup_steps(&mut self, spectrum_warmup_steps: i32) {
        self.fp.spectrum_warmup_steps = spectrum_warmup_steps;
    }

    pub fn set_spectrum_stop_percent(&mut self, spectrum_stop_percent: f32) {
        self.fp.spectrum_stop_percent = spectrum_stop_percent;
    }
}