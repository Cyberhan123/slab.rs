use std::ffi::CString;
use std::ptr;

use serde::{Deserialize, Serialize};
use slab_diffusion_sys::{sd_cache_mode_t, sd_cache_params_t};

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
#[derive(Debug, Copy, Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CacheMode {
    #[default]
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

/// Stable Rust-native cache tuning parameters used across the runtime chain.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CacheParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<CacheMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reuse_threshold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_percent: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_percent: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_decay_rate: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_relative_threshold: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_error_on_compute: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fn_compute_blocks: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bn_compute_blocks: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub residual_diff_threshold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_warmup_steps: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_cached_steps: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_continuous_cached_steps: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taylorseer_n_derivatives: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taylorseer_skip_interval: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scm_mask: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scm_policy_dynamic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectrum_w: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectrum_m: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectrum_lam: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectrum_window_size: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectrum_flex_window: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectrum_warmup_steps: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectrum_stop_percent: Option<f32>,
}

/// FFI-only cache parameter backing struct.
pub(crate) struct InnerCacheParams {
    pub(crate) fp: Box<sd_cache_params_t>,
    scm_mask: Option<CString>,
}

impl Clone for InnerCacheParams {
    fn clone(&self) -> Self {
        let mut cloned = Self { fp: self.fp.clone(), scm_mask: self.scm_mask.clone() };
        cloned.sync_backing();
        cloned
    }
}

impl Default for InnerCacheParams {
    fn default() -> Self {
        Self { fp: Box::new(unsafe { std::mem::zeroed::<sd_cache_params_t>() }), scm_mask: None }
    }
}

impl InnerCacheParams {
    pub(crate) fn with_native_init(lib: &slab_diffusion_sys::DiffusionLib) -> Self {
        let mut inner = Self::default();
        unsafe { lib.sd_cache_params_init(inner.fp.as_mut()) };
        inner
    }

    pub(crate) fn from_canonical(
        lib: &slab_diffusion_sys::DiffusionLib,
        value: &CacheParams,
    ) -> Self {
        let mut inner = InnerCacheParams::with_native_init(lib);

        if let Some(mode) = value.mode {
            inner.set_mode(mode);
        }
        if let Some(reuse_threshold) = value.reuse_threshold {
            inner.set_reuse_threshold(reuse_threshold);
        }
        if let Some(start_percent) = value.start_percent {
            inner.set_start_percent(start_percent);
        }
        if let Some(end_percent) = value.end_percent {
            inner.set_end_percent(end_percent);
        }
        if let Some(error_decay_rate) = value.error_decay_rate {
            inner.set_error_decay_rate(error_decay_rate);
        }
        if let Some(use_relative_threshold) = value.use_relative_threshold {
            inner.set_use_relative_threshold(use_relative_threshold);
        }
        if let Some(reset_error_on_compute) = value.reset_error_on_compute {
            inner.set_reset_error_on_compute(reset_error_on_compute);
        }
        if let Some(fn_compute_blocks) = value.fn_compute_blocks {
            inner.set_fn_compute_blocks(fn_compute_blocks);
        }
        if let Some(bn_compute_blocks) = value.bn_compute_blocks {
            inner.set_bn_compute_blocks(bn_compute_blocks);
        }
        if let Some(residual_diff_threshold) = value.residual_diff_threshold {
            inner.set_residual_diff_threshold(residual_diff_threshold);
        }
        if let Some(max_warmup_steps) = value.max_warmup_steps {
            inner.set_max_warmup_steps(max_warmup_steps);
        }
        if let Some(max_cached_steps) = value.max_cached_steps {
            inner.set_max_cached_steps(max_cached_steps);
        }
        if let Some(max_continuous_cached_steps) = value.max_continuous_cached_steps {
            inner.set_max_continuous_cached_steps(max_continuous_cached_steps);
        }
        if let Some(taylorseer_n_derivatives) = value.taylorseer_n_derivatives {
            inner.set_taylorseer_n_derivatives(taylorseer_n_derivatives);
        }
        if let Some(taylorseer_skip_interval) = value.taylorseer_skip_interval {
            inner.set_taylorseer_skip_interval(taylorseer_skip_interval);
        }
        if value.scm_mask.is_some() {
            inner.set_scm_mask(value.scm_mask.as_deref());
        }
        if let Some(scm_policy_dynamic) = value.scm_policy_dynamic {
            inner.set_scm_policy_dynamic(scm_policy_dynamic);
        }
        if let Some(spectrum_w) = value.spectrum_w {
            inner.set_spectrum_w(spectrum_w);
        }
        if let Some(spectrum_m) = value.spectrum_m {
            inner.set_spectrum_m(spectrum_m);
        }
        if let Some(spectrum_lam) = value.spectrum_lam {
            inner.set_spectrum_lam(spectrum_lam);
        }
        if let Some(spectrum_window_size) = value.spectrum_window_size {
            inner.set_spectrum_window_size(spectrum_window_size);
        }
        if let Some(spectrum_flex_window) = value.spectrum_flex_window {
            inner.set_spectrum_flex_window(spectrum_flex_window);
        }
        if let Some(spectrum_warmup_steps) = value.spectrum_warmup_steps {
            inner.set_spectrum_warmup_steps(spectrum_warmup_steps);
        }
        if let Some(spectrum_stop_percent) = value.spectrum_stop_percent {
            inner.set_spectrum_stop_percent(spectrum_stop_percent);
        }

        inner
    }

    pub(crate) fn sync_backing(&mut self) {
        self.fp.scm_mask = self.scm_mask.as_ref().map_or(ptr::null(), c_string_ptr);
    }

    fn set_mode(&mut self, mode: CacheMode) {
        self.fp.mode = mode.into();
    }

    fn set_reuse_threshold(&mut self, reuse_threshold: f32) {
        self.fp.reuse_threshold = reuse_threshold;
    }

    fn set_start_percent(&mut self, start_percent: f32) {
        self.fp.start_percent = start_percent;
    }

    fn set_end_percent(&mut self, end_percent: f32) {
        self.fp.end_percent = end_percent;
    }

    fn set_error_decay_rate(&mut self, error_decay_rate: f32) {
        self.fp.error_decay_rate = error_decay_rate;
    }

    fn set_use_relative_threshold(&mut self, use_relative_threshold: bool) {
        self.fp.use_relative_threshold = use_relative_threshold;
    }

    fn set_reset_error_on_compute(&mut self, reset_error_on_compute: bool) {
        self.fp.reset_error_on_compute = reset_error_on_compute;
    }

    fn set_fn_compute_blocks(&mut self, fn_compute_blocks: i32) {
        self.fp.Fn_compute_blocks = fn_compute_blocks;
    }

    fn set_bn_compute_blocks(&mut self, bn_compute_blocks: i32) {
        self.fp.Bn_compute_blocks = bn_compute_blocks;
    }

    fn set_residual_diff_threshold(&mut self, residual_diff_threshold: f32) {
        self.fp.residual_diff_threshold = residual_diff_threshold;
    }

    fn set_max_warmup_steps(&mut self, max_warmup_steps: i32) {
        self.fp.max_warmup_steps = max_warmup_steps;
    }

    fn set_max_cached_steps(&mut self, max_cached_steps: i32) {
        self.fp.max_cached_steps = max_cached_steps;
    }

    fn set_max_continuous_cached_steps(&mut self, max_continuous_cached_steps: i32) {
        self.fp.max_continuous_cached_steps = max_continuous_cached_steps;
    }

    fn set_taylorseer_n_derivatives(&mut self, taylorseer_n_derivatives: i32) {
        self.fp.taylorseer_n_derivatives = taylorseer_n_derivatives;
    }

    fn set_taylorseer_skip_interval(&mut self, taylorseer_skip_interval: i32) {
        self.fp.taylorseer_skip_interval = taylorseer_skip_interval;
    }

    fn set_scm_mask(&mut self, scm_mask: Option<&str>) {
        self.scm_mask = scm_mask.map(new_c_string);
        self.sync_backing();
    }

    fn set_scm_policy_dynamic(&mut self, scm_policy_dynamic: bool) {
        self.fp.scm_policy_dynamic = scm_policy_dynamic;
    }

    fn set_spectrum_w(&mut self, spectrum_w: f32) {
        self.fp.spectrum_w = spectrum_w;
    }

    fn set_spectrum_m(&mut self, spectrum_m: i32) {
        self.fp.spectrum_m = spectrum_m;
    }

    fn set_spectrum_lam(&mut self, spectrum_lam: f32) {
        self.fp.spectrum_lam = spectrum_lam;
    }

    fn set_spectrum_window_size(&mut self, spectrum_window_size: i32) {
        self.fp.spectrum_window_size = spectrum_window_size;
    }

    fn set_spectrum_flex_window(&mut self, spectrum_flex_window: f32) {
        self.fp.spectrum_flex_window = spectrum_flex_window;
    }

    fn set_spectrum_warmup_steps(&mut self, spectrum_warmup_steps: i32) {
        self.fp.spectrum_warmup_steps = spectrum_warmup_steps;
    }

    fn set_spectrum_stop_percent(&mut self, spectrum_stop_percent: f32) {
        self.fp.spectrum_stop_percent = spectrum_stop_percent;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn canonical_cache_params_convert_to_inner_backing() {
        let params = CacheParams {
            mode: Some(CacheMode::Spectrum),
            scm_mask: Some("101010".to_owned()),
            spectrum_window_size: Some(64),
            use_relative_threshold: Some(true),
            ..Default::default()
        };
        let inner = {
            let mut inner = InnerCacheParams::default();
            inner.set_mode(params.mode.expect("mode should be present"));
            inner.set_scm_mask(params.scm_mask.as_deref());
            inner.set_spectrum_window_size(
                params.spectrum_window_size.expect("window size should be present"),
            );
            inner.set_use_relative_threshold(
                params.use_relative_threshold.expect("threshold flag should be present"),
            );
            inner
        };

        assert_eq!(inner.fp.mode, sd_cache_mode_t::from(CacheMode::Spectrum));
        assert_eq!(unsafe { CStr::from_ptr(inner.fp.scm_mask) }.to_str().unwrap(), "101010");
        assert_eq!(inner.fp.spectrum_window_size, 64);
        assert!(inner.fp.use_relative_threshold);
    }

    #[test]
    fn clone_rebinds_scm_mask_storage() {
        let params =
            CacheParams { scm_mask: Some("dynamic-mask".to_owned()), ..Default::default() };
        let mut inner = InnerCacheParams::default();
        inner.set_scm_mask(params.scm_mask.as_deref());
        let cloned = inner.clone();

        assert_ne!(cloned.fp.scm_mask, inner.fp.scm_mask);
        assert_eq!(unsafe { CStr::from_ptr(cloned.fp.scm_mask) }.to_str().unwrap(), "dynamic-mask");
    }
}
