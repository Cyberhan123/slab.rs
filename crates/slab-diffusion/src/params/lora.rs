use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use slab_diffusion_sys::lora_apply_mode_t;
/// lora parameters must keep code order
#[rustfmt::skip]
pub use slab_diffusion_sys::{
    lora_apply_mode_t_LORA_APPLY_AUTO,
    lora_apply_mode_t_LORA_APPLY_IMMEDIATELY,
    lora_apply_mode_t_LORA_APPLY_AT_RUNTIME,
    lora_apply_mode_t_LORA_APPLY_MODE_COUNT,
};

#[cfg_attr(any(not(windows), target_env = "gnu"), repr(u32))] // include windows-gnu
#[cfg_attr(all(windows, not(target_env = "gnu")), repr(i32))]
#[derive(Debug, Copy, Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum LoraApplyMode {
    #[default]
    Auto = lora_apply_mode_t_LORA_APPLY_AUTO,
    Immediately = lora_apply_mode_t_LORA_APPLY_IMMEDIATELY,
    AtRuntime = lora_apply_mode_t_LORA_APPLY_AT_RUNTIME,
    Unknown = lora_apply_mode_t_LORA_APPLY_MODE_COUNT,
}

impl From<LoraApplyMode> for lora_apply_mode_t {
    fn from(value: LoraApplyMode) -> Self {
        value as Self
    }
}

/// A LoRA entry consumed by `sd_img_gen_params_t.loras`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Lora {
    pub is_high_noise: bool,
    pub multiplier: f32,
    pub path: PathBuf,
}
