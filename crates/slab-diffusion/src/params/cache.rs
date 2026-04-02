use std::ffi::CString;
use std::ptr;

use slab_diffusion_sys::{sd_cache_mode_t, sd_cache_params_t};

use crate::Diffusion;
use crate::params::support::{c_string_ptr, new_c_string};

#[rustfmt::skip]
use slab_diffusion_sys::{
    sd_cache_mode_t_SD_CACHE_CACHE_DIT,
    sd_cache_mode_t_SD_CACHE_DBCACHE,
    sd_cache_mode_t_SD_CACHE_DISABLED,
    sd_cache_mode_t_SD_CACHE_EASYCACHE,
    sd_cache_mode_t_SD_CACHE_SPECTRUM,
    sd_cache_mode_t_SD_CACHE_TAYLORSEER,
    sd_cache_mode_t_SD_CACHE_UCACHE,
};

#[cfg_attr(any(not(windows), target_env = "gnu"), repr(u32))]
#[cfg_attr(all(windows, not(target_env = "gnu")), repr(i32))]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum CacheMode {
    Disabled = sd_cache_mode_t_SD_CACHE_DISABLED,
    EasyCache = sd_cache_mode_t_SD_CACHE_EASYCACHE,
    UCache = sd_cache_mode_t_SD_CACHE_UCACHE,
    DBCache = sd_cache_mode_t_SD_CACHE_DBCACHE,
    TaylorSeer = sd_cache_mode_t_SD_CACHE_TAYLORSEER,
    Dit = sd_cache_mode_t_SD_CACHE_CACHE_DIT,
    Spectrum = sd_cache_mode_t_SD_CACHE_SPECTRUM,
}

impl From<CacheMode> for sd_cache_mode_t {
    fn from(value: CacheMode) -> Self {
        value as Self
    }
}

/// Cache tuning parameters.
pub struct CacheParams {
    pub(crate) fp: Box<sd_cache_params_t>,
    scm_mask: Option<CString>,
}

impl Clone for CacheParams {
    fn clone(&self) -> Self {
        let mut cloned = Self { fp: self.fp.clone(), scm_mask: self.scm_mask.clone() };
        cloned.sync_backing();
        cloned
    }
}

impl Diffusion {
    pub fn new_cache_params(&self) -> CacheParams {
        let mut fp = Box::new(unsafe { std::mem::zeroed::<sd_cache_params_t>() });
        unsafe { self.lib.sd_cache_params_init(fp.as_mut()) };
        CacheParams { fp, scm_mask: None }
    }
}

impl CacheParams {
    pub(crate) fn sync_backing(&mut self) {
        self.fp.scm_mask = self.scm_mask.as_ref().map_or(ptr::null(), c_string_ptr);
    }

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
        self.scm_mask = Some(new_c_string(&scm_mask));
        self.sync_backing();
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    fn new_cache_params() -> CacheParams {
        CacheParams {
            fp: Box::new(unsafe { std::mem::zeroed::<sd_cache_params_t>() }),
            scm_mask: None,
        }
    }

    #[test]
    fn set_scm_mask_and_numeric_fields_sync_backing() {
        let mut params = new_cache_params();
        params.set_mode(CacheMode::Spectrum);
        params.set_scm_mask("101010".to_owned());
        params.set_spectrum_window_size(64);
        params.set_use_relative_threshold(true);

        assert_eq!(params.fp.mode, CacheMode::Spectrum.into());
        assert_eq!(unsafe { CStr::from_ptr(params.fp.scm_mask) }.to_str().unwrap(), "101010");
        assert_eq!(params.fp.spectrum_window_size, 64);
        assert!(params.fp.use_relative_threshold);
    }

    #[test]
    fn clone_rebinds_scm_mask_storage() {
        let mut params = new_cache_params();
        params.set_scm_mask("dynamic-mask".to_owned());

        let cloned = params.clone();

        assert_ne!(cloned.fp.scm_mask, params.fp.scm_mask);
        assert_eq!(unsafe { CStr::from_ptr(cloned.fp.scm_mask) }.to_str().unwrap(), "dynamic-mask");
    }
}
