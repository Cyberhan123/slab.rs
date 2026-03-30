use crate::params::LoraApplyMode;
use crate::params::Prediction;
use crate::params::Embedding;
use crate::Diffusion;
use crate::RngType;
use crate::WeightType;

use slab_diffusion_sys::sd_ctx_params_t;
use slab_diffusion_sys::sd_embedding_t;
use std::ffi::CStr;
use std::ffi::CString;

#[derive(Clone)]
pub struct ContextParams {
    pub(crate) fp: Box<sd_ctx_params_t>,
    instance: Diffusion,
}

impl Diffusion {
    pub fn new_context_params(&self) -> ContextParams {
        let mut fp_box = Box::new(unsafe { std::mem::zeroed::<sd_ctx_params_t>() });
        let ptr: *mut sd_ctx_params_t = Box::into_raw(fp_box);
        unsafe {
            self.lib.sd_ctx_params_init(ptr);
            fp_box = Box::from_raw(ptr);
        }
        ContextParams { fp: fp_box, instance: self.clone() }
    }
}

impl ContextParams {
    pub fn to_str(&self) -> Option<&'static str> {
        let c_buf = unsafe { self.instance.lib.sd_ctx_params_to_str(&*self.fp) };
        if c_buf.is_null() {
            None
        } else {
            let c_str = unsafe { CStr::from_ptr(c_buf) };
            Some(c_str.to_str().unwrap())
        }
    }

    pub fn set_model_path(&mut self, path: &str) {
        self.fp.model_path = CString::new(path).unwrap().into_raw();
    }

    pub fn set_clip_l_path(&mut self, path: &str) {
        self.fp.clip_l_path = CString::new(path).unwrap().into_raw();
    }

    pub fn set_clip_g_path(&mut self, path: &str) {
        self.fp.clip_g_path = CString::new(path).unwrap().into_raw();
    }

    pub fn set_clip_vision_path(&mut self, path: &str) {
        self.fp.clip_vision_path = CString::new(path).unwrap().into_raw();
    }

    pub fn set_t5xxl_path(&mut self, path: &str) {
        self.fp.t5xxl_path = CString::new(path).unwrap().into_raw();
    }

    pub fn set_llm_path(&mut self, path: &str) {
        self.fp.llm_path = CString::new(path).unwrap().into_raw();
    }

    pub fn set_llm_vision_path(&mut self, path: &str) {
        self.fp.llm_vision_path = CString::new(path).unwrap().into_raw();
    }

    pub fn set_diffusion_model_path(&mut self, path: &str) {
        self.fp.diffusion_model_path = CString::new(path).unwrap().into_raw();
    }

    pub fn set_high_noise_diffusion_model_path(&mut self, path: &str) {
        self.fp.high_noise_diffusion_model_path = CString::new(path).unwrap().into_raw();
    }

    pub fn set_vae_path(&mut self, path: &str) {
        self.fp.vae_path = CString::new(path).unwrap().into_raw();
    }

    pub fn set_taesd_path(&mut self, path: &str) {
        self.fp.taesd_path = CString::new(path).unwrap().into_raw();
    }

    pub fn set_control_net_path(&mut self, path: &str) {
        self.fp.control_net_path = CString::new(path).unwrap().into_raw();
    }

    pub fn set_embeddings(&mut self, embeddings: Vec<Embedding>) {
        let mut c_list: Vec<sd_embedding_t> =
            embeddings.into_iter().map(|embedding| embedding.into()).collect();
        c_list.shrink_to_fit();

        self.fp.embeddings = c_list.as_ptr();
        self.fp.embedding_count = c_list.len() as u32;
    }

    pub fn set_photo_maker_path(&mut self, path: &str) {
        self.fp.photo_maker_path = CString::new(path).unwrap().into_raw();
    }

    pub fn set_tensor_type_rules(&mut self, rules: &str) {
        self.fp.tensor_type_rules = CString::new(rules).unwrap().into_raw();
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
        self.fp.main_device = CString::new(path).unwrap().into_raw();
    }

    pub fn set_diffusion_device(&mut self, path: &str) {
        self.fp.diffusion_device = CString::new(path).unwrap().into_raw();
    }

    pub fn set_clip_device(&mut self, path: &str) {
        self.fp.clip_device = CString::new(path).unwrap().into_raw();
    }

    pub fn set_vae_device(&mut self, path: &str) {
        self.fp.vae_device = CString::new(path).unwrap().into_raw();
    }

    pub fn set_tae_device(&mut self, path: &str) {
        self.fp.tae_device = CString::new(path).unwrap().into_raw();
    }

    pub fn set_control_net_device(&mut self, path: &str) {
        self.fp.control_net_device = CString::new(path).unwrap().into_raw();
    }

    pub fn set_photomaker_device(&mut self, path: &str) {
        self.fp.photomaker_device = CString::new(path).unwrap().into_raw();
    }

    pub fn set_vision_device(&mut self, path: &str) {
        self.fp.vision_device = CString::new(path).unwrap().into_raw();
    }
}
