use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::ptr;

use serde::{Deserialize, Serialize};
use slab_diffusion_sys::{sd_ctx_params_t, sd_embedding_t};

use crate::Diffusion;
use crate::params::support::{c_string_ptr, new_c_string, sync_embedding_views};
use crate::params::{Embedding, LoraApplyMode, Prediction, RngType, WeightType};

const fn default_flash_attn_enabled_option() -> Option<bool> {
    Some(true)
}

/// Stable Rust-native context parameters shared across the runtime chain.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_l_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_g_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_vision_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t5xxl_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_vision_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diffusion_model_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high_noise_diffusion_model_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vae_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taesd_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_net_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embeddings: Option<Vec<Embedding>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub photo_maker_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tensor_type_rules: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vae_decode_only: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub free_params_immediately: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n_threads: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wtype: Option<WeightType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rng_type: Option<RngType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampler_rng_type: Option<RngType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prediction: Option<Prediction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lora_apply_mode: Option<LoraApplyMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offload_params_to_cpu: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_mmap: Option<bool>,
    #[serde(
        default = "default_flash_attn_enabled_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub flash_attn: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diffusion_flash_attn: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tae_preview_only: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diffusion_conv_direct: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vae_conv_direct: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub circular_x: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub circular_y: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force_sdxl_vae_conv_scale: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chroma_use_dit_mask: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chroma_use_t5_mask: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chroma_t5_mask_pad: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qwen_image_zero_cond_t: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_device: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diffusion_device: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_device: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vae_device: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tae_device: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_net_device: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub photomaker_device: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vision_device: Option<String>,
}

/// FFI-only context parameter backing struct.
pub(crate) struct InnerContextParams {
    pub(crate) fp: Box<sd_ctx_params_t>,
    model_path: Option<CString>,
    clip_l_path: Option<CString>,
    clip_g_path: Option<CString>,
    clip_vision_path: Option<CString>,
    t5xxl_path: Option<CString>,
    llm_path: Option<CString>,
    llm_vision_path: Option<CString>,
    diffusion_model_path: Option<CString>,
    high_noise_diffusion_model_path: Option<CString>,
    vae_path: Option<CString>,
    taesd_path: Option<CString>,
    control_net_path: Option<CString>,
    embeddings: Vec<Embedding>,
    embedding_names: Vec<CString>,
    embedding_paths: Vec<CString>,
    embedding_views: Vec<sd_embedding_t>,
    photo_maker_path: Option<CString>,
    tensor_type_rules: Option<CString>,
}

impl Clone for InnerContextParams {
    fn clone(&self) -> Self {
        let mut cloned = Self {
            fp: self.fp.clone(),
            model_path: self.model_path.clone(),
            clip_l_path: self.clip_l_path.clone(),
            clip_g_path: self.clip_g_path.clone(),
            clip_vision_path: self.clip_vision_path.clone(),
            t5xxl_path: self.t5xxl_path.clone(),
            llm_path: self.llm_path.clone(),
            llm_vision_path: self.llm_vision_path.clone(),
            diffusion_model_path: self.diffusion_model_path.clone(),
            high_noise_diffusion_model_path: self.high_noise_diffusion_model_path.clone(),
            vae_path: self.vae_path.clone(),
            taesd_path: self.taesd_path.clone(),
            control_net_path: self.control_net_path.clone(),
            embeddings: self.embeddings.clone(),
            embedding_names: self.embedding_names.clone(),
            embedding_paths: self.embedding_paths.clone(),
            embedding_views: self.embedding_views.clone(),
            photo_maker_path: self.photo_maker_path.clone(),
            tensor_type_rules: self.tensor_type_rules.clone(),
        };
        cloned.sync_backing();
        cloned
    }
}

impl Default for InnerContextParams {
    fn default() -> Self {
        Self {
            fp: Box::new(unsafe { std::mem::zeroed::<sd_ctx_params_t>() }),
            model_path: None,
            clip_l_path: None,
            clip_g_path: None,
            clip_vision_path: None,
            t5xxl_path: None,
            llm_path: None,
            llm_vision_path: None,
            diffusion_model_path: None,
            high_noise_diffusion_model_path: None,
            vae_path: None,
            taesd_path: None,
            control_net_path: None,
            embeddings: Vec::new(),
            embedding_names: Vec::new(),
            embedding_paths: Vec::new(),
            embedding_views: Vec::new(),
            photo_maker_path: None,
            tensor_type_rules: None,
        }
    }
}

impl std::fmt::Debug for InnerContextParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InnerContextParams").finish_non_exhaustive()
    }
}

impl Diffusion {
    pub fn context_params_to_str(&self, params: &ContextParams) -> Option<String> {
        Some(format!("{params:#?}"))
    }
}

impl InnerContextParams {
    pub(crate) fn with_native_init(lib: &slab_diffusion_sys::DiffusionLib) -> Self {
        let mut inner = Self::default();
        unsafe { lib.sd_ctx_params_init(inner.fp.as_mut()) };
        inner
    }

    fn apply_server_compatible_defaults(&mut self, value: &ContextParams) {
        // `sd_ctx_params_init` enables short-lived one-shot defaults that do not match
        // the upstream long-lived server context behavior. Keep unset flags aligned with
        // the server path so reused contexts survive multiple inference calls.
        // NOTE: upstream removed `vae_decode_only` / `free_params_immediately` from
        // `sd_ctx_params_t`; only `tae_preview_only` remains of the server-compat flags.
        self.fp.tae_preview_only = value.tae_preview_only.unwrap_or(false);
    }

    pub(crate) fn from_canonical(
        lib: &slab_diffusion_sys::DiffusionLib,
        value: &ContextParams,
    ) -> Self {
        let mut inner = InnerContextParams::with_native_init(lib);
        inner.apply_server_compatible_defaults(value);

        if value.model_path.is_some() {
            Self::set_path(
                &mut inner.model_path,
                &mut inner.fp.model_path,
                value.model_path.as_deref(),
            );
        }
        if value.clip_l_path.is_some() {
            Self::set_path(
                &mut inner.clip_l_path,
                &mut inner.fp.clip_l_path,
                value.clip_l_path.as_deref(),
            );
        }
        if value.clip_g_path.is_some() {
            Self::set_path(
                &mut inner.clip_g_path,
                &mut inner.fp.clip_g_path,
                value.clip_g_path.as_deref(),
            );
        }
        if value.clip_vision_path.is_some() {
            Self::set_path(
                &mut inner.clip_vision_path,
                &mut inner.fp.clip_vision_path,
                value.clip_vision_path.as_deref(),
            );
        }
        if value.t5xxl_path.is_some() {
            Self::set_path(
                &mut inner.t5xxl_path,
                &mut inner.fp.t5xxl_path,
                value.t5xxl_path.as_deref(),
            );
        }
        if value.llm_path.is_some() {
            Self::set_path(&mut inner.llm_path, &mut inner.fp.llm_path, value.llm_path.as_deref());
        }
        if value.llm_vision_path.is_some() {
            Self::set_path(
                &mut inner.llm_vision_path,
                &mut inner.fp.llm_vision_path,
                value.llm_vision_path.as_deref(),
            );
        }
        if value.diffusion_model_path.is_some() {
            Self::set_path(
                &mut inner.diffusion_model_path,
                &mut inner.fp.diffusion_model_path,
                value.diffusion_model_path.as_deref(),
            );
        }
        if value.high_noise_diffusion_model_path.is_some() {
            Self::set_path(
                &mut inner.high_noise_diffusion_model_path,
                &mut inner.fp.high_noise_diffusion_model_path,
                value.high_noise_diffusion_model_path.as_deref(),
            );
        }
        if value.vae_path.is_some() {
            Self::set_path(&mut inner.vae_path, &mut inner.fp.vae_path, value.vae_path.as_deref());
        }
        if value.taesd_path.is_some() {
            Self::set_path(
                &mut inner.taesd_path,
                &mut inner.fp.taesd_path,
                value.taesd_path.as_deref(),
            );
        }
        if value.control_net_path.is_some() {
            Self::set_path(
                &mut inner.control_net_path,
                &mut inner.fp.control_net_path,
                value.control_net_path.as_deref(),
            );
        }
        if let Some(embeddings) = value.embeddings.clone() {
            inner.embeddings = embeddings;
            inner.sync_embeddings();
        }
        if value.photo_maker_path.is_some() {
            Self::set_path(
                &mut inner.photo_maker_path,
                &mut inner.fp.photo_maker_path,
                value.photo_maker_path.as_deref(),
            );
        }
        if value.tensor_type_rules.is_some() {
            Self::set_c_string(
                &mut inner.tensor_type_rules,
                &mut inner.fp.tensor_type_rules,
                value.tensor_type_rules.as_deref(),
            );
        }
        if let Some(n_threads) = value.n_threads {
            inner.fp.n_threads = n_threads;
        }
        if let Some(wtype) = value.wtype {
            inner.fp.wtype = wtype.into();
        }
        if let Some(rng_type) = value.rng_type {
            inner.fp.rng_type = rng_type.into();
        }
        if let Some(sampler_rng_type) = value.sampler_rng_type {
            inner.fp.sampler_rng_type = sampler_rng_type.into();
        }
        if let Some(prediction) = value.prediction {
            inner.fp.prediction = prediction.into();
        }
        if let Some(lora_apply_mode) = value.lora_apply_mode {
            inner.fp.lora_apply_mode = lora_apply_mode.into();
        }
        // NOTE: upstream `sd_ctx_params_t` dropped `offload_params_to_cpu`,
        // `circular_x`/`circular_y`, the `chroma_*` masks, `qwen_image_zero_cond_t`,
        // and the per-stage `*_device` strings (device assignment is now a single
        // `backend` spec). Their public `ContextParams` options remain accepted for
        // API stability but no longer map to a native field.
        if let Some(enable_mmap) = value.enable_mmap {
            inner.fp.enable_mmap = enable_mmap;
        }
        if let Some(flash_attn) = value.flash_attn {
            inner.fp.flash_attn = flash_attn;
        }
        if let Some(diffusion_flash_attn) = value.diffusion_flash_attn {
            inner.fp.diffusion_flash_attn = diffusion_flash_attn;
        }
        if let Some(diffusion_conv_direct) = value.diffusion_conv_direct {
            inner.fp.diffusion_conv_direct = diffusion_conv_direct;
        }
        if let Some(vae_conv_direct) = value.vae_conv_direct {
            inner.fp.vae_conv_direct = vae_conv_direct;
        }
        if let Some(force_sdxl_vae_conv_scale) = value.force_sdxl_vae_conv_scale {
            inner.fp.force_sdxl_vae_conv_scale = force_sdxl_vae_conv_scale;
        }

        inner
    }

    fn set_c_string(
        slot: &mut Option<CString>,
        field: &mut *const std::os::raw::c_char,
        value: Option<&str>,
    ) {
        *slot = value.map(new_c_string);
        *field = slot.as_ref().map_or(ptr::null(), c_string_ptr);
    }

    fn set_path(
        slot: &mut Option<CString>,
        field: &mut *const std::os::raw::c_char,
        value: Option<&Path>,
    ) {
        *slot = value.map(|path| new_c_string(&path.to_string_lossy()));
        *field = slot.as_ref().map_or(ptr::null(), c_string_ptr);
    }

    fn sync_embeddings(&mut self) {
        sync_embedding_views(
            &self.embeddings,
            &mut self.embedding_names,
            &mut self.embedding_paths,
            &mut self.embedding_views,
        );
        self.fp.embeddings = if self.embedding_views.is_empty() {
            ptr::null()
        } else {
            self.embedding_views.as_ptr()
        };
        self.fp.embedding_count = self.embedding_views.len().min(u32::MAX as usize) as u32;
    }

    pub(crate) fn sync_backing(&mut self) {
        self.fp.model_path = self.model_path.as_ref().map_or(ptr::null(), c_string_ptr);
        self.fp.clip_l_path = self.clip_l_path.as_ref().map_or(ptr::null(), c_string_ptr);
        self.fp.clip_g_path = self.clip_g_path.as_ref().map_or(ptr::null(), c_string_ptr);
        self.fp.clip_vision_path = self.clip_vision_path.as_ref().map_or(ptr::null(), c_string_ptr);
        self.fp.t5xxl_path = self.t5xxl_path.as_ref().map_or(ptr::null(), c_string_ptr);
        self.fp.llm_path = self.llm_path.as_ref().map_or(ptr::null(), c_string_ptr);
        self.fp.llm_vision_path = self.llm_vision_path.as_ref().map_or(ptr::null(), c_string_ptr);
        self.fp.diffusion_model_path =
            self.diffusion_model_path.as_ref().map_or(ptr::null(), c_string_ptr);
        self.fp.high_noise_diffusion_model_path =
            self.high_noise_diffusion_model_path.as_ref().map_or(ptr::null(), c_string_ptr);
        self.fp.vae_path = self.vae_path.as_ref().map_or(ptr::null(), c_string_ptr);
        self.fp.taesd_path = self.taesd_path.as_ref().map_or(ptr::null(), c_string_ptr);
        self.fp.control_net_path = self.control_net_path.as_ref().map_or(ptr::null(), c_string_ptr);
        self.sync_embeddings();
        self.fp.photo_maker_path = self.photo_maker_path.as_ref().map_or(ptr::null(), c_string_ptr);
        self.fp.tensor_type_rules =
            self.tensor_type_rules.as_ref().map_or(ptr::null(), c_string_ptr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_context_flags_use_server_compatible_defaults() {
        let mut inner = InnerContextParams::default();
        inner.fp.tae_preview_only = true;

        inner.apply_server_compatible_defaults(&ContextParams::default());

        assert!(!inner.fp.tae_preview_only);
    }

    #[test]
    fn explicit_context_flags_override_server_compatible_defaults() {
        let mut inner = InnerContextParams::default();
        let params = ContextParams { tae_preview_only: Some(true), ..Default::default() };

        inner.apply_server_compatible_defaults(&params);

        assert!(inner.fp.tae_preview_only);
    }
}
