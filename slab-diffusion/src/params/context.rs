use std::ffi::CString;
use std::ptr;

use slab_diffusion_sys::sd_ctx_params_t;
use slab_diffusion_sys::sd_embedding_t;

use crate::Diffusion;
use crate::RngType;
use crate::WeightType;
use crate::params::support::{
    c_string_ptr, copy_and_free_c_string, new_c_string, sync_embedding_views,
};
use crate::params::{Embedding, LoraApplyMode, Prediction};

pub struct ContextParams {
    pub(crate) fp: Box<sd_ctx_params_t>,
    instance: Diffusion,
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
    main_device: Option<CString>,
    diffusion_device: Option<CString>,
    clip_device: Option<CString>,
    vae_device: Option<CString>,
    tae_device: Option<CString>,
    control_net_device: Option<CString>,
    photomaker_device: Option<CString>,
    vision_device: Option<CString>,
}

impl Clone for ContextParams {
    fn clone(&self) -> Self {
        let mut cloned = Self {
            fp: self.fp.clone(),
            instance: self.instance.clone(),
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
            main_device: self.main_device.clone(),
            diffusion_device: self.diffusion_device.clone(),
            clip_device: self.clip_device.clone(),
            vae_device: self.vae_device.clone(),
            tae_device: self.tae_device.clone(),
            control_net_device: self.control_net_device.clone(),
            photomaker_device: self.photomaker_device.clone(),
            vision_device: self.vision_device.clone(),
        };
        cloned.sync_backing();
        cloned
    }
}

impl std::fmt::Debug for ContextParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContextParams").finish_non_exhaustive()
    }
}

impl Diffusion {
    pub fn new_context_params(&self) -> ContextParams {
        let mut fp = Box::new(unsafe { std::mem::zeroed::<sd_ctx_params_t>() });
        unsafe { self.lib.sd_ctx_params_init(fp.as_mut()) };
        ContextParams {
            fp,
            instance: self.clone(),
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
            main_device: None,
            diffusion_device: None,
            clip_device: None,
            vae_device: None,
            tae_device: None,
            control_net_device: None,
            photomaker_device: None,
            vision_device: None,
        }
    }
}

impl ContextParams {
    fn set_string(
        slot: &mut Option<CString>,
        field: &mut *const std::os::raw::c_char,
        value: &str,
    ) {
        *slot = Some(new_c_string(value));
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
        self.fp.main_device = self.main_device.as_ref().map_or(ptr::null(), c_string_ptr);
        self.fp.diffusion_device = self.diffusion_device.as_ref().map_or(ptr::null(), c_string_ptr);
        self.fp.clip_device = self.clip_device.as_ref().map_or(ptr::null(), c_string_ptr);
        self.fp.vae_device = self.vae_device.as_ref().map_or(ptr::null(), c_string_ptr);
        self.fp.tae_device = self.tae_device.as_ref().map_or(ptr::null(), c_string_ptr);
        self.fp.control_net_device =
            self.control_net_device.as_ref().map_or(ptr::null(), c_string_ptr);
        self.fp.photomaker_device =
            self.photomaker_device.as_ref().map_or(ptr::null(), c_string_ptr);
        self.fp.vision_device = self.vision_device.as_ref().map_or(ptr::null(), c_string_ptr);
    }

    pub fn to_str(&self) -> Option<String> {
        let c_buf = unsafe { self.instance.lib.sd_ctx_params_to_str(&*self.fp) };
        copy_and_free_c_string(c_buf)
    }

    pub fn set_model_path(&mut self, path: &str) {
        Self::set_string(&mut self.model_path, &mut self.fp.model_path, path);
    }

    pub fn set_clip_l_path(&mut self, path: &str) {
        Self::set_string(&mut self.clip_l_path, &mut self.fp.clip_l_path, path);
    }

    pub fn set_clip_g_path(&mut self, path: &str) {
        Self::set_string(&mut self.clip_g_path, &mut self.fp.clip_g_path, path);
    }

    pub fn set_clip_vision_path(&mut self, path: &str) {
        Self::set_string(&mut self.clip_vision_path, &mut self.fp.clip_vision_path, path);
    }

    pub fn set_t5xxl_path(&mut self, path: &str) {
        Self::set_string(&mut self.t5xxl_path, &mut self.fp.t5xxl_path, path);
    }

    pub fn set_llm_path(&mut self, path: &str) {
        Self::set_string(&mut self.llm_path, &mut self.fp.llm_path, path);
    }

    pub fn set_llm_vision_path(&mut self, path: &str) {
        Self::set_string(&mut self.llm_vision_path, &mut self.fp.llm_vision_path, path);
    }

    pub fn set_diffusion_model_path(&mut self, path: &str) {
        Self::set_string(&mut self.diffusion_model_path, &mut self.fp.diffusion_model_path, path);
    }

    pub fn set_high_noise_diffusion_model_path(&mut self, path: &str) {
        Self::set_string(
            &mut self.high_noise_diffusion_model_path,
            &mut self.fp.high_noise_diffusion_model_path,
            path,
        );
    }

    pub fn set_vae_path(&mut self, path: &str) {
        Self::set_string(&mut self.vae_path, &mut self.fp.vae_path, path);
    }

    pub fn set_taesd_path(&mut self, path: &str) {
        Self::set_string(&mut self.taesd_path, &mut self.fp.taesd_path, path);
    }

    pub fn set_control_net_path(&mut self, path: &str) {
        Self::set_string(&mut self.control_net_path, &mut self.fp.control_net_path, path);
    }

    pub fn set_embeddings(&mut self, embeddings: Vec<Embedding>) {
        self.embeddings = embeddings;
        self.sync_embeddings();
    }

    pub fn set_photo_maker_path(&mut self, path: &str) {
        Self::set_string(&mut self.photo_maker_path, &mut self.fp.photo_maker_path, path);
    }

    pub fn set_tensor_type_rules(&mut self, rules: &str) {
        Self::set_string(&mut self.tensor_type_rules, &mut self.fp.tensor_type_rules, rules);
    }

    pub fn set_vae_decode_only(&mut self, decode_only: bool) {
        self.fp.vae_decode_only = decode_only;
    }

    pub fn set_free_params_immediately(&mut self, free_params_immediately: bool) {
        self.fp.free_params_immediately = free_params_immediately;
    }

    pub fn set_n_threads(&mut self, n_threads: i32) {
        self.fp.n_threads = n_threads;
    }

    pub fn set_wtype(&mut self, wtype: WeightType) {
        self.fp.wtype = wtype.into();
    }

    pub fn set_rng_type(&mut self, rng_type: RngType) {
        self.fp.rng_type = rng_type.into();
    }

    pub fn set_sampler_rng_type(&mut self, sampler_rng_type: RngType) {
        self.fp.sampler_rng_type = sampler_rng_type.into();
    }

    pub fn set_prediction(&mut self, prediction: Prediction) {
        self.fp.prediction = prediction.into();
    }

    pub fn set_lora_apply_mode(&mut self, lora_apply_mode: LoraApplyMode) {
        self.fp.lora_apply_mode = lora_apply_mode.into();
    }

    pub fn set_offload_params_to_cpu(&mut self, offload_params_to_cpu: bool) {
        self.fp.offload_params_to_cpu = offload_params_to_cpu;
    }

    pub fn set_enable_mmap(&mut self, enable_mmap: bool) {
        self.fp.enable_mmap = enable_mmap;
    }

    pub fn set_flash_attn(&mut self, enable_flash_attn: bool) {
        self.fp.flash_attn = enable_flash_attn;
    }

    pub fn set_diffusion_flash_attn(&mut self, diffusion_flash_attn: bool) {
        self.fp.diffusion_flash_attn = diffusion_flash_attn;
    }

    pub fn set_tae_preview_only(&mut self, tae_preview_only: bool) {
        self.fp.tae_preview_only = tae_preview_only;
    }

    pub fn set_diffusion_conv_direct(&mut self, diffusion_conv_direct: bool) {
        self.fp.diffusion_conv_direct = diffusion_conv_direct;
    }

    pub fn set_vae_conv_direct(&mut self, vae_conv_direct: bool) {
        self.fp.vae_conv_direct = vae_conv_direct;
    }

    pub fn set_circular_x(&mut self, circular_x: bool) {
        self.fp.circular_x = circular_x;
    }

    pub fn set_circular_y(&mut self, circular_y: bool) {
        self.fp.circular_y = circular_y;
    }

    pub fn set_force_sdxl_vae_conv_scale(&mut self, force_sdxl_vae_conv_scale: bool) {
        self.fp.force_sdxl_vae_conv_scale = force_sdxl_vae_conv_scale;
    }

    pub fn set_chroma_use_dit_mask(&mut self, chroma_use_dit_mask: bool) {
        self.fp.chroma_use_dit_mask = chroma_use_dit_mask;
    }

    pub fn set_chroma_use_t5_mask(&mut self, chroma_use_t5_mask: bool) {
        self.fp.chroma_use_t5_mask = chroma_use_t5_mask;
    }

    pub fn set_chroma_t5_mask_pad(&mut self, chroma_t5_mask_pad: i32) {
        self.fp.chroma_t5_mask_pad = chroma_t5_mask_pad;
    }

    pub fn set_qwen_image_zero_cond_t(&mut self, qwen_image_zero_cond_t: bool) {
        self.fp.qwen_image_zero_cond_t = qwen_image_zero_cond_t;
    }

    pub fn set_main_device(&mut self, path: &str) {
        Self::set_string(&mut self.main_device, &mut self.fp.main_device, path);
    }

    pub fn set_diffusion_device(&mut self, path: &str) {
        Self::set_string(&mut self.diffusion_device, &mut self.fp.diffusion_device, path);
    }

    pub fn set_clip_device(&mut self, path: &str) {
        Self::set_string(&mut self.clip_device, &mut self.fp.clip_device, path);
    }

    pub fn set_vae_device(&mut self, path: &str) {
        Self::set_string(&mut self.vae_device, &mut self.fp.vae_device, path);
    }

    pub fn set_tae_device(&mut self, path: &str) {
        Self::set_string(&mut self.tae_device, &mut self.fp.tae_device, path);
    }

    pub fn set_control_net_device(&mut self, path: &str) {
        Self::set_string(&mut self.control_net_device, &mut self.fp.control_net_device, path);
    }

    pub fn set_photomaker_device(&mut self, path: &str) {
        Self::set_string(&mut self.photomaker_device, &mut self.fp.photomaker_device, path);
    }

    pub fn set_vision_device(&mut self, path: &str) {
        Self::set_string(&mut self.vision_device, &mut self.fp.vision_device, path);
    }
}
