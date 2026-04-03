use crate::WhisperTokenId;
use crate::error::WhisperError;
use std::borrow::Cow;
use std::ffi::{CStr, CString, c_int};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::Whisper;

/// Safe Rust wrapper around a Whisper context.
///
/// You likely want to create this with [WhisperInnerContext::new_with_params],
/// create a state with [WhisperInnerContext::create_state],
/// then run a full transcription with [WhisperState::full].
#[derive(Debug)]
pub struct WhisperInnerContext {
    pub(crate) ctx: *mut slab_whisper_sys::whisper_context,
    pub(crate) instance: Whisper,
}

impl Whisper {
    pub fn default_context_params(&self, model_path: impl Into<PathBuf>) -> ContextParams {
        ContextParams::from_native(Some(model_path.into()), unsafe {
            self.lib.whisper_context_default_params()
        })
    }

    /// Create a new WhisperContext from a file, with parameters.
    ///
    /// # Arguments
    /// * path: The path to the model file.
    /// * parameters: A parameter struct containing the parameters to use.
    ///
    /// # Returns
    /// Ok(Self) on success, Err(WhisperError) on failure.
    ///
    /// # C++ equivalent
    /// `struct whisper_context * whisper_init_from_file_with_params_no_state(const char * path_model, struct whisper_context_params params);`
    pub fn new_inner_context(
        &self,
        parameters: ContextParams,
    ) -> Result<WhisperInnerContext, WhisperError> {
        let model_path = parameters.model_path.as_ref().ok_or(WhisperError::ModelPathNotSet)?;
        let path_cstr = CString::new(model_path.to_string_lossy().as_ref())?;
        let parameters = InnerContextParams::from_canonical(self.lib.as_ref(), &parameters)?;
        let ctx = unsafe {
            self.lib.whisper_init_from_file_with_params_no_state(
                path_cstr.as_ptr(),
                parameters.into_inner(),
            )
        };
        if ctx.is_null() {
            Err(WhisperError::InitError)
        } else {
            Ok(WhisperInnerContext { ctx, instance: self.clone() })
        }
    }

    /// Create a new WhisperContext from a buffer.
    ///
    /// # Arguments
    /// * buffer: The buffer containing the model.
    ///
    /// # Returns
    /// Ok(Self) on success, Err(WhisperError) on failure.
    ///
    /// # C++ equivalent
    /// `struct whisper_context * whisper_init_from_buffer_with_params_no_state(void * buffer, size_t buffer_size, struct whisper_context_params params);`
    pub fn new_inner_context_from_buffer(
        &self,
        buffer: &[u8],
        parameters: ContextParams,
    ) -> Result<WhisperInnerContext, WhisperError> {
        let parameters = InnerContextParams::from_canonical(self.lib.as_ref(), &parameters)?;
        let ctx = unsafe {
            self.lib.whisper_init_from_buffer_with_params_no_state(
                buffer.as_ptr() as _,
                buffer.len(),
                parameters.into_inner(),
            )
        };
        if ctx.is_null() {
            Err(WhisperError::InitError)
        } else {
            Ok(WhisperInnerContext { ctx, instance: self.clone() })
        }
    }
}

impl WhisperInnerContext {
    /// Convert the provided text into tokens.
    ///
    /// # Arguments
    /// * text: The text to convert.
    ///
    /// # Returns
    /// `Ok(Vec<WhisperToken>)` on success, `Err(WhisperError)` on failure.
    ///
    /// # C++ equivalent
    /// `int whisper_tokenize(struct whisper_context * ctx, const char * text, whisper_token * tokens, int n_max_tokens);`
    pub fn tokenize(
        &self,
        text: &str,
        max_tokens: usize,
    ) -> Result<Vec<WhisperTokenId>, WhisperError> {
        // convert the text to a nul-terminated C string. Will raise an error if the text contains
        // any nul bytes.
        let text = CString::new(text)?;
        // allocate at least max_tokens to ensure the memory is valid
        let mut tokens: Vec<WhisperTokenId> = Vec::with_capacity(max_tokens);
        let ret = unsafe {
            self.instance.lib.whisper_tokenize(
                self.ctx,
                text.as_ptr(),
                tokens.as_mut_ptr(),
                max_tokens as c_int,
            )
        };
        if ret == -1 {
            Err(WhisperError::InvalidText)
        } else {
            // SAFETY: when ret != -1, we know that the length of the vector is at least ret tokens
            unsafe { tokens.set_len(ret as usize) };
            Ok(tokens)
        }
    }

    /// Get n_vocab.
    ///
    /// # Returns
    /// c_int
    ///
    /// # C++ equivalent
    /// `int whisper_n_vocab        (struct whisper_context * ctx)`
    pub fn n_vocab(&self) -> c_int {
        unsafe { self.instance.lib.whisper_n_vocab(self.ctx) }
    }

    /// Get n_text_ctx.
    ///
    /// # Returns
    /// c_int
    ///
    /// # C++ equivalent
    /// `int whisper_n_text_ctx     (struct whisper_context * ctx);`
    pub fn n_text_ctx(&self) -> c_int {
        unsafe { self.instance.lib.whisper_n_text_ctx(self.ctx) }
    }

    /// Get n_audio_ctx.
    ///
    /// # Returns
    /// c_int
    ///
    /// # C++ equivalent
    /// `int whisper_n_audio_ctx     (struct whisper_context * ctx);`
    pub fn n_audio_ctx(&self) -> c_int {
        unsafe { self.instance.lib.whisper_n_audio_ctx(self.ctx) }
    }

    /// Does this model support multiple languages?
    ///
    /// # C++ equivalent
    /// `int whisper_is_multilingual(struct whisper_context * ctx)`
    pub fn is_multilingual(&self) -> bool {
        unsafe { self.instance.lib.whisper_is_multilingual(self.ctx) != 0 }
    }

    /// Get model_n_vocab.
    ///
    /// # Returns
    /// c_int
    ///
    /// # C++ equivalent
    /// `int whisper_model_n_vocab      (struct whisper_context * ctx);`
    pub fn model_n_vocab(&self) -> c_int {
        unsafe { self.instance.lib.whisper_model_n_vocab(self.ctx) }
    }

    /// Get model_n_audio_ctx.
    ///
    /// # Returns
    /// c_int
    ///
    /// # C++ equivalent
    /// `int whisper_model_n_audio_ctx    (struct whisper_context * ctx)`
    pub fn model_n_audio_ctx(&self) -> c_int {
        unsafe { self.instance.lib.whisper_model_n_audio_ctx(self.ctx) }
    }

    /// Get model_n_audio_state.
    ///
    /// # Returns
    /// c_int
    ///
    /// # C++ equivalent
    /// `int whisper_model_n_audio_state(struct whisper_context * ctx);`
    pub fn model_n_audio_state(&self) -> c_int {
        unsafe { self.instance.lib.whisper_model_n_audio_state(self.ctx) }
    }

    /// Get model_n_audio_head.
    ///
    /// # Returns
    /// c_int
    ///
    /// # C++ equivalent
    /// `int whisper_model_n_audio_head (struct whisper_context * ctx);`
    pub fn model_n_audio_head(&self) -> c_int {
        unsafe { self.instance.lib.whisper_model_n_audio_head(self.ctx) }
    }

    /// Get model_n_audio_layer.
    ///
    /// # Returns
    /// c_int
    ///
    /// # C++ equivalent
    /// `int whisper_model_n_audio_layer(struct whisper_context * ctx);`
    pub fn model_n_audio_layer(&self) -> c_int {
        unsafe { self.instance.lib.whisper_model_n_audio_layer(self.ctx) }
    }

    /// Get model_n_text_ctx.
    ///
    /// # Returns
    /// c_int
    ///
    /// # C++ equivalent
    /// `int whisper_model_n_text_ctx     (struct whisper_context * ctx)`
    pub fn model_n_text_ctx(&self) -> c_int {
        unsafe { self.instance.lib.whisper_model_n_text_ctx(self.ctx) }
    }

    /// Get model_n_text_state.
    ///
    /// # Returns
    /// c_int
    ///
    /// # C++ equivalent
    /// `int whisper_model_n_text_state (struct whisper_context * ctx);`
    pub fn model_n_text_state(&self) -> c_int {
        unsafe { self.instance.lib.whisper_model_n_text_state(self.ctx) }
    }

    /// Get model_n_text_head.
    ///
    /// # Returns
    /// c_int
    ///
    /// # C++ equivalent
    /// `int whisper_model_n_text_head  (struct whisper_context * ctx);`
    pub fn model_n_text_head(&self) -> c_int {
        unsafe { self.instance.lib.whisper_model_n_text_head(self.ctx) }
    }

    /// Get model_n_text_layer.
    ///
    /// # Returns
    /// c_int
    ///
    /// # C++ equivalent
    /// `int whisper_model_n_text_layer (struct whisper_context * ctx);`
    pub fn model_n_text_layer(&self) -> c_int {
        unsafe { self.instance.lib.whisper_model_n_text_layer(self.ctx) }
    }

    /// Get model_n_mels.
    ///
    /// # Returns
    /// c_int
    ///
    /// # C++ equivalent
    /// `int whisper_model_n_mels       (struct whisper_context * ctx);`
    pub fn model_n_mels(&self) -> c_int {
        unsafe { self.instance.lib.whisper_model_n_mels(self.ctx) }
    }

    /// Get model_ftype.
    ///
    /// # Returns
    /// c_int
    ///
    /// # C++ equivalent
    /// `int whisper_model_ftype          (struct whisper_context * ctx);`
    pub fn model_ftype(&self) -> c_int {
        unsafe { self.instance.lib.whisper_model_ftype(self.ctx) }
    }

    /// Get model_type.
    ///
    /// # Returns
    /// c_int
    ///
    /// # C++ equivalent
    /// `int whisper_model_type         (struct whisper_context * ctx);`
    pub fn model_type(&self) -> c_int {
        unsafe { self.instance.lib.whisper_model_type(self.ctx) }
    }

    // --- begin model_type_readable helpers ---
    fn model_type_readable_cstr(&self) -> Result<&CStr, WhisperError> {
        let ret = unsafe { self.instance.lib.whisper_model_type_readable(self.ctx) };
        if ret.is_null() {
            return Err(WhisperError::NullPointer);
        }
        Ok(unsafe { CStr::from_ptr(ret) })
    }
    pub fn model_type_readable_bytes(&self) -> Result<&[u8], WhisperError> {
        Ok(self.model_type_readable_cstr()?.to_bytes())
    }
    pub fn model_type_readable_str(&self) -> Result<&str, WhisperError> {
        Ok(self.model_type_readable_cstr()?.to_str()?)
    }
    pub fn model_type_readable_str_lossy(&self) -> Result<Cow<'_, str>, WhisperError> {
        Ok(self.model_type_readable_cstr()?.to_string_lossy())
    }
    // --- end model_type_readable helpers ---

    // --- begin token functions ---
    fn token_to_cstr(&self, token_id: WhisperTokenId) -> Result<&CStr, WhisperError> {
        let ret = unsafe { self.instance.lib.whisper_token_to_str(self.ctx, token_id) };
        if ret.is_null() {
            return Err(WhisperError::NullPointer);
        }
        Ok(unsafe { CStr::from_ptr(ret) })
    }
    pub fn token_to_bytes(&self, token_id: WhisperTokenId) -> Result<&[u8], WhisperError> {
        Ok(self.token_to_cstr(token_id)?.to_bytes())
    }
    pub fn token_to_str(&self, token_id: WhisperTokenId) -> Result<&str, WhisperError> {
        Ok(self.token_to_cstr(token_id)?.to_str()?)
    }
    pub fn token_to_str_lossy(
        &self,
        token_id: WhisperTokenId,
    ) -> Result<Cow<'_, str>, WhisperError> {
        Ok(self.token_to_cstr(token_id)?.to_string_lossy())
    }

    /// Get the ID of the eot token.
    ///
    /// # C++ equivalent
    /// `whisper_token whisper_token_eot (struct whisper_context * ctx)`
    pub fn token_eot(&self) -> WhisperTokenId {
        unsafe { self.instance.lib.whisper_token_eot(self.ctx) }
    }

    /// Get the ID of the sot token.
    ///
    /// # C++ equivalent
    /// `whisper_token whisper_token_sot (struct whisper_context * ctx)`
    pub fn token_sot(&self) -> WhisperTokenId {
        unsafe { self.instance.lib.whisper_token_sot(self.ctx) }
    }

    /// Get the ID of the solm token.
    ///
    /// # C++ equivalent
    /// `whisper_token whisper_token_solm(struct whisper_context * ctx)`
    pub fn token_solm(&self) -> WhisperTokenId {
        unsafe { self.instance.lib.whisper_token_solm(self.ctx) }
    }

    /// Get the ID of the prev token.
    ///
    /// # C++ equivalent
    /// `whisper_token whisper_token_prev(struct whisper_context * ctx)`
    pub fn token_prev(&self) -> WhisperTokenId {
        unsafe { self.instance.lib.whisper_token_prev(self.ctx) }
    }

    /// Get the ID of the nosp token.
    ///
    /// # C++ equivalent
    /// `whisper_token whisper_token_nosp(struct whisper_context * ctx)`
    pub fn token_nosp(&self) -> WhisperTokenId {
        unsafe { self.instance.lib.whisper_token_nosp(self.ctx) }
    }

    /// Get the ID of the not token.
    ///
    /// # C++ equivalent
    /// `whisper_token whisper_token_not (struct whisper_context * ctx)`
    pub fn token_not(&self) -> WhisperTokenId {
        unsafe { self.instance.lib.whisper_token_not(self.ctx) }
    }

    /// Get the ID of the beg token.
    ///
    /// # C++ equivalent
    /// `whisper_token whisper_token_beg (struct whisper_context * ctx)`
    pub fn token_beg(&self) -> WhisperTokenId {
        unsafe { self.instance.lib.whisper_token_beg(self.ctx) }
    }

    /// Get the ID of a specified language token
    ///
    /// # Arguments
    /// * lang_id: ID of the language
    ///
    /// # C++ equivalent
    /// `whisper_token whisper_token_lang(struct whisper_context * ctx, int lang_id)`
    pub fn token_lang(&self, lang_id: c_int) -> WhisperTokenId {
        unsafe { self.instance.lib.whisper_token_lang(self.ctx, lang_id) }
    }
    // --- end token functions ---

    /// Print performance statistics to stderr.
    ///
    /// # C++ equivalent
    /// `void whisper_print_timings(struct whisper_context * ctx)`
    pub fn print_timings(&self) {
        unsafe { self.instance.lib.whisper_print_timings(self.ctx) }
    }

    /// Reset performance statistics.
    ///
    /// # C++ equivalent
    /// `void whisper_reset_timings(struct whisper_context * ctx)`
    pub fn reset_timings(&self) {
        unsafe { self.instance.lib.whisper_reset_timings(self.ctx) }
    }

    /// Get performance timings from the default state.
    pub fn timings(&self) -> Option<WhisperTimings> {
        let timings = unsafe { self.instance.lib.whisper_get_timings(self.ctx) };
        (!timings.is_null()).then(|| WhisperTimings::from(unsafe { *timings }))
    }

    // task tokens
    /// Get the ID of the translate task token.
    ///
    /// # C++ equivalent
    /// `whisper_token whisper_token_translate ()`
    pub fn token_translate(&self) -> WhisperTokenId {
        unsafe { self.instance.lib.whisper_token_translate(self.ctx) }
    }

    /// Get the ID of the transcribe task token.
    ///
    /// # C++ equivalent
    /// `whisper_token whisper_token_transcribe()`
    pub fn token_transcribe(&self) -> WhisperTokenId {
        unsafe { self.instance.lib.whisper_token_transcribe(self.ctx) }
    }
}

impl Drop for WhisperInnerContext {
    fn drop(&mut self) {
        unsafe { self.instance.lib.whisper_free(self.ctx) };
    }
}

// following implementations are safe
// see https://github.com/ggerganov/whisper.cpp/issues/32#issuecomment-1272790388
unsafe impl Send for WhisperInnerContext {}
unsafe impl Sync for WhisperInnerContext {}
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub struct WhisperTimings {
    pub sample_ms: f32,
    pub encode_ms: f32,
    pub decode_ms: f32,
    pub batchd_ms: f32,
    pub prompt_ms: f32,
}

impl From<slab_whisper_sys::whisper_timings> for WhisperTimings {
    fn from(value: slab_whisper_sys::whisper_timings) -> Self {
        Self {
            sample_ms: value.sample_ms,
            encode_ms: value.encode_ms,
            decode_ms: value.decode_ms,
            batchd_ms: value.batchd_ms,
            prompt_ms: value.prompt_ms,
        }
    }
}

/// Stable Rust-native context parameters shared across the runtime chain.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ContextParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_gpu: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flash_attn: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_device: Option<c_int>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dtw_parameters: Option<DtwParameters>,
}

impl ContextParams {
    pub fn new(model_path: impl Into<PathBuf>) -> Self {
        Self { model_path: Some(model_path.into()), ..Self::default() }
    }

    pub(crate) fn from_native(
        model_path: Option<PathBuf>,
        params: slab_whisper_sys::whisper_context_params,
    ) -> Self {
        Self {
            model_path,
            use_gpu: Some(params.use_gpu),
            flash_attn: Some(params.flash_attn),
            gpu_device: Some(params.gpu_device),
            dtw_parameters: Some(DtwParameters::from_native(params)),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct InnerContextParams {
    cp: slab_whisper_sys::whisper_context_params,
    dtw_aheads: Vec<slab_whisper_sys::whisper_ahead>,
}

impl InnerContextParams {
    pub(crate) fn from_canonical(
        lib: &slab_whisper_sys::WhisperLib,
        value: &ContextParams,
    ) -> Result<Self, WhisperError> {
        let mut inner =
            Self { cp: unsafe { lib.whisper_context_default_params() }, dtw_aheads: Vec::new() };

        if let Some(use_gpu) = value.use_gpu {
            inner.cp.use_gpu = use_gpu;
        }
        if let Some(flash_attn) = value.flash_attn {
            inner.cp.flash_attn = flash_attn;
        }
        if let Some(gpu_device) = value.gpu_device {
            inner.cp.gpu_device = gpu_device;
        }
        if let Some(dtw_parameters) = value.dtw_parameters.as_ref() {
            dtw_parameters.apply_to(&mut inner.cp, &mut inner.dtw_aheads);
        }

        inner.sync_backing();
        Ok(inner)
    }

    fn sync_backing(&mut self) {
        self.cp.dtw_aheads = if self.dtw_aheads.is_empty() {
            slab_whisper_sys::whisper_aheads { n_heads: 0, heads: std::ptr::null() }
        } else {
            slab_whisper_sys::whisper_aheads {
                n_heads: self.dtw_aheads.len(),
                heads: self.dtw_aheads.as_ptr(),
            }
        };
    }

    pub(crate) fn into_inner(mut self) -> slab_whisper_sys::whisper_context_params {
        self.sync_backing();
        self.cp
    }
}

/// [EXPERIMENTAL] Enable token-level timestamps with DTW.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DtwParameters {
    #[serde(default)]
    pub mode: DtwMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dtw_mem_size: Option<usize>,
}

impl DtwParameters {
    fn from_native(params: slab_whisper_sys::whisper_context_params) -> Self {
        let mode = match params.dtw_aheads_preset {
            slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_NONE => DtwMode::None,
            slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_N_TOP_MOST => {
                DtwMode::TopMost { n_top: params.dtw_n_top }
            }
            slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_CUSTOM => {
                let aheads = if params.dtw_aheads.heads.is_null() || params.dtw_aheads.n_heads == 0
                {
                    Vec::new()
                } else {
                    unsafe {
                        std::slice::from_raw_parts(
                            params.dtw_aheads.heads,
                            params.dtw_aheads.n_heads,
                        )
                    }
                    .iter()
                    .copied()
                    .map(DtwAhead::from)
                    .collect()
                };
                DtwMode::Custom { aheads }
            }
            slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_TINY_EN => {
                DtwMode::ModelPreset { model_preset: DtwModelPreset::TinyEn }
            }
            slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_TINY => {
                DtwMode::ModelPreset { model_preset: DtwModelPreset::Tiny }
            }
            slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_BASE_EN => {
                DtwMode::ModelPreset { model_preset: DtwModelPreset::BaseEn }
            }
            slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_BASE => {
                DtwMode::ModelPreset { model_preset: DtwModelPreset::Base }
            }
            slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_SMALL_EN => {
                DtwMode::ModelPreset { model_preset: DtwModelPreset::SmallEn }
            }
            slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_SMALL => {
                DtwMode::ModelPreset { model_preset: DtwModelPreset::Small }
            }
            slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_MEDIUM_EN => {
                DtwMode::ModelPreset { model_preset: DtwModelPreset::MediumEn }
            }
            slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_MEDIUM => {
                DtwMode::ModelPreset { model_preset: DtwModelPreset::Medium }
            }
            slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_LARGE_V1 => {
                DtwMode::ModelPreset { model_preset: DtwModelPreset::LargeV1 }
            }
            slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_LARGE_V2 => {
                DtwMode::ModelPreset { model_preset: DtwModelPreset::LargeV2 }
            }
            slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_LARGE_V3 => {
                DtwMode::ModelPreset { model_preset: DtwModelPreset::LargeV3 }
            }
            slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_LARGE_V3_TURBO => {
                DtwMode::ModelPreset { model_preset: DtwModelPreset::LargeV3Turbo }
            }
            _ => DtwMode::None,
        };

        Self { mode, dtw_mem_size: Some(params.dtw_mem_size) }
    }

    fn apply_to(
        &self,
        params: &mut slab_whisper_sys::whisper_context_params,
        dtw_aheads: &mut Vec<slab_whisper_sys::whisper_ahead>,
    ) {
        params.dtw_token_timestamps = !matches!(self.mode, DtwMode::None);
        params.dtw_n_top = -1;
        params.dtw_aheads_preset =
            slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_NONE;
        dtw_aheads.clear();

        match &self.mode {
            DtwMode::None => {}
            DtwMode::TopMost { n_top } => {
                params.dtw_aheads_preset =
                    slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_N_TOP_MOST;
                params.dtw_n_top = *n_top;
            }
            DtwMode::Custom { aheads } => {
                params.dtw_aheads_preset =
                    slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_CUSTOM;
                dtw_aheads
                    .extend(aheads.iter().copied().map(slab_whisper_sys::whisper_ahead::from));
            }
            DtwMode::ModelPreset { model_preset } => {
                params.dtw_aheads_preset = model_preset.to_native();
            }
        }

        if let Some(dtw_mem_size) = self.dtw_mem_size {
            params.dtw_mem_size = dtw_mem_size;
        }
    }
}

#[derive(Debug, Copy, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DtwAhead {
    pub n_text_layer: c_int,
    pub n_head: c_int,
}

impl From<slab_whisper_sys::whisper_ahead> for DtwAhead {
    fn from(value: slab_whisper_sys::whisper_ahead) -> Self {
        Self { n_text_layer: value.n_text_layer, n_head: value.n_head }
    }
}

impl From<DtwAhead> for slab_whisper_sys::whisper_ahead {
    fn from(value: DtwAhead) -> Self {
        Self { n_text_layer: value.n_text_layer, n_head: value.n_head }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum DtwMode {
    /// DTW token level timestamps disabled.
    #[default]
    None,
    /// Use N top-most layers from the loaded model.
    TopMost { n_top: c_int },
    /// Use custom aheads.
    Custom { aheads: Vec<DtwAhead> },
    /// Use predefined preset for standard models.
    ModelPreset { model_preset: DtwModelPreset },
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DtwModelPreset {
    TinyEn,
    Tiny,
    BaseEn,
    Base,
    SmallEn,
    Small,
    MediumEn,
    Medium,
    LargeV1,
    LargeV2,
    LargeV3,
    LargeV3Turbo,
}

impl DtwModelPreset {
    fn to_native(self) -> slab_whisper_sys::whisper_alignment_heads_preset {
        match self {
            DtwModelPreset::TinyEn => {
                slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_TINY_EN
            }
            DtwModelPreset::Tiny => {
                slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_TINY
            }
            DtwModelPreset::BaseEn => {
                slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_BASE_EN
            }
            DtwModelPreset::Base => {
                slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_BASE
            }
            DtwModelPreset::SmallEn => {
                slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_SMALL_EN
            }
            DtwModelPreset::Small => {
                slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_SMALL
            }
            DtwModelPreset::MediumEn => {
                slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_MEDIUM_EN
            }
            DtwModelPreset::Medium => {
                slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_MEDIUM
            }
            DtwModelPreset::LargeV1 => {
                slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_LARGE_V1
            }
            DtwModelPreset::LargeV2 => {
                slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_LARGE_V2
            }
            DtwModelPreset::LargeV3 => {
                slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_LARGE_V3
            }
            DtwModelPreset::LargeV3Turbo => {
                slab_whisper_sys::whisper_alignment_heads_preset_WHISPER_AHEADS_LARGE_V3_TURBO
            }
        }
    }
}

#[cfg(test)]
#[cfg(feature = "test-with-tiny-model")]
mod test_with_tiny_model {
    use super::*;
    const MODEL_PATH: &str = "./sys/whisper.cpp/models/ggml-tiny.en.bin";

    // These tests expect that the tiny.en model has been downloaded
    // using the script `sys/whisper.cpp/models/download-ggml-model.sh tiny.en`

    #[test]
    fn test_tokenize_round_trip() {
        let ctx = WhisperInnerContext::new(MODEL_PATH).expect("Download the ggml-tiny.en model using 'sys/whisper.cpp/models/download-ggml-model.sh tiny.en'");
        let text_in = " And so my fellow Americans, ask not what your country can do for you, ask what you can do for your country.";
        let tokens = ctx.tokenize(text_in, 1024).unwrap();
        let text_out =
            tokens.into_iter().map(|t| ctx.token_to_str(t).unwrap()).collect::<Vec<_>>().join("");
        assert_eq!(text_in, text_out);
    }
}
