use std::ffi::CString;
use std::sync::Arc;

use tracing::debug;

use crate::Llama;
use crate::LlamaSampler;
use crate::context_params::LlamaContextParams;
use crate::error::LlamaError;
use crate::llama_adapter::LlamaLoraAdapter;
use crate::llama_context::LlamaContext;
use crate::llama_sampler::SamplerChainBuilder;
use crate::runtime::LlamaLogitBias;
use crate::token::LlamaToken;

/// Inner (non-Clone) model data.  Wrapped in Arc so that LlamaContext can keep
/// the model alive without copying the raw pointer.
pub(crate) struct LlamaModelInner {
    pub(crate) model: Option<std::ptr::NonNull<slab_llama_sys::llama_model>>,
    pub(crate) lib: Arc<slab_llama_sys::LlamaLib>,
    pub(crate) eog_tokens: Box<[LlamaToken]>,
    pub(crate) eog_logit_bias: Box<[slab_llama_sys::llama_logit_bias]>,
}

// SAFETY: The underlying `llama_model` pointer is only accessed through
// `&mut self` methods in this crate, ensuring exclusive access. The
// `llama.cpp` library does not use thread-local state for model operations,
// and all mutable access to the model is mediated through the `LlamaModel`
// wrapper which enforces Rust's borrowing rules.
unsafe impl Send for LlamaModelInner {}

// SAFETY: Same as Send - all mutable access is exclusive, and the library
// does not use interior mutability or thread-local state.
unsafe impl Sync for LlamaModelInner {}

impl Drop for LlamaModelInner {
    fn drop(&mut self) {
        if let Some(model) = self.model.take() {
            unsafe { self.lib.llama_model_free(model.as_ptr()) };
        }
    }
}

/// A safe wrapper around a loaded `llama_model`.
///
/// Created via [`Llama::load_model_from_file`].
#[derive(Clone)]
pub struct LlamaModel {
    pub(crate) inner: Arc<LlamaModelInner>,
}

impl Llama {
    /// Load a model from a GGUF file.
    ///
    /// # Arguments
    /// * `path`   – path to the `.gguf` model file.
    /// * `params` – model loading parameters.
    ///
    /// # Errors
    /// Returns [`LlamaError::ModelLoadFailed`] if loading fails.
    pub fn load_model_from_file(
        &self,
        path: &str,
        params: crate::model_params::LlamaModelParams,
    ) -> Result<LlamaModel, LlamaError> {
        let c_path = CString::new(path)?;
        let c_params = params.to_c_params(&self.lib);
        let model = unsafe { self.lib.llama_model_load_from_file(c_path.as_ptr(), c_params) };
        if model.is_null() {
            Err(LlamaError::ModelLoadFailed)
        } else {
            let vocab = unsafe { self.lib.llama_model_get_vocab(model) };
            let (eog_tokens, eog_logit_bias) = collect_eog_bias(&self.lib, vocab);
            Ok(LlamaModel {
                inner: Arc::new(LlamaModelInner {
                    model: Some(unsafe { std::ptr::NonNull::new_unchecked(model) }),
                    lib: Arc::clone(&self.lib),
                    eog_tokens,
                    eog_logit_bias,
                }),
            })
        }
    }
}

fn collect_eog_bias(
    lib: &Arc<slab_llama_sys::LlamaLib>,
    vocab: *const slab_llama_sys::llama_vocab,
) -> (Box<[LlamaToken]>, Box<[slab_llama_sys::llama_logit_bias]>) {
    if vocab.is_null() {
        return (Vec::new().into_boxed_slice(), Vec::new().into_boxed_slice());
    }

    let n_vocab = unsafe { lib.llama_vocab_n_tokens(vocab) };
    let mut eog_tokens = Vec::new();
    let mut eog_logit_bias = Vec::new();

    for token in 0..n_vocab {
        if unsafe { lib.llama_vocab_is_eog(vocab, token) } {
            eog_tokens.push(token);
            eog_logit_bias
                .push(slab_llama_sys::llama_logit_bias { token, bias: f32::NEG_INFINITY });
        }
    }

    debug!(count = eog_tokens.len(), "initialized llama EOG token cache");
    (eog_tokens.into_boxed_slice(), eog_logit_bias.into_boxed_slice())
}

fn collect_sampler_logit_bias(
    logit_bias: &[LlamaLogitBias],
    ignore_eos: bool,
    eog_logit_bias: &[slab_llama_sys::llama_logit_bias],
) -> Vec<slab_llama_sys::llama_logit_bias> {
    let mut raw =
        Vec::with_capacity(logit_bias.len() + if ignore_eos { eog_logit_bias.len() } else { 0 });
    raw.extend(
        logit_bias
            .iter()
            .map(|entry| slab_llama_sys::llama_logit_bias { token: entry.token, bias: entry.bias }),
    );

    if ignore_eos {
        raw.extend(eog_logit_bias.iter().map(|entry| slab_llama_sys::llama_logit_bias {
            token: entry.token,
            bias: entry.bias,
        }));
    }

    raw
}

impl LlamaModel {
    /// Return the vocab pointer (for tokenization helpers).
    fn vocab(&self) -> *const slab_llama_sys::llama_vocab {
        unsafe { self.inner.lib.llama_model_get_vocab(self.inner.model.unwrap().as_ptr()) }
    }

    /// Create an inference context for this model.
    ///
    /// # Arguments
    /// * `params` – context creation parameters.
    ///
    /// # Errors
    /// Returns [`LlamaError::ContextCreateFailed`] if context creation fails.
    pub fn new_context(&self, params: LlamaContextParams) -> Result<LlamaContext, LlamaError> {
        let c_params = params.to_c_params(&self.inner.lib);
        let ctx = unsafe { self.inner.lib.llama_init_from_model(self.inner.model.unwrap().as_ptr(), c_params) };
        if ctx.is_null() {
            Err(LlamaError::ContextCreateFailed)
        } else {
            Ok(LlamaContext {
                ctx: Some(unsafe { std::ptr::NonNull::new_unchecked(ctx) }),
                model: Arc::clone(&self.inner),
            })
        }
    }

    /// Load a LoRA adapter from a file and associate it with this model.
    ///
    /// # Arguments
    /// * `path_lora` – path to the LoRA adapter file (`.gguf` format).
    ///
    /// # Precondition
    /// The underlying `llama_adapter_lora_init` API requires that **all** LoRA
    /// adapters be loaded **before** any [`LlamaContext`] is created from this
    /// model (for example, via [`LlamaModel::new_context`]). Calling this
    /// method after a context has been created for the model is undefined or
    /// unsupported behavior and may cause failures.
    ///
    /// # Errors
    /// Returns [`LlamaError::LoraAdapterLoadFailed`] if loading fails.
    pub fn adapter_lora_init(&self, path_lora: &str) -> Result<LlamaLoraAdapter, LlamaError> {
        let c_path = CString::new(path_lora)?;
        let adapter =
            unsafe { self.inner.lib.llama_adapter_lora_init(self.inner.model.unwrap().as_ptr(), c_path.as_ptr()) };
        if adapter.is_null() {
            Err(LlamaError::LoraAdapterLoadFailed)
        } else {
            Ok(LlamaLoraAdapter { adapter, model: Arc::clone(&self.inner) })
        }
    }

    /// Tokenize a UTF-8 string.
    ///
    /// # Arguments
    /// * `text`          – the text to tokenize.
    /// * `add_special`   – whether to add BOS/EOS special tokens.
    /// * `parse_special` – whether to parse special tokens (e.g. `<|user|>`).
    ///
    /// # Errors
    /// Returns [`LlamaError::TokenizeFailed`] if tokenization fails.
    pub fn tokenize(
        &self,
        text: &str,
        add_special: bool,
        parse_special: bool,
    ) -> Result<Vec<LlamaToken>, LlamaError> {
        let vocab = self.vocab();
        let text_bytes = text.as_bytes();
        // First call with zero-sized buffer to get the required token count.
        let n = unsafe {
            self.inner.lib.llama_tokenize(
                vocab,
                text_bytes.as_ptr() as *const std::os::raw::c_char,
                text_bytes.len() as i32,
                std::ptr::null_mut(),
                0,
                add_special,
                parse_special,
            )
        };
        // n is negative when the buffer is too small; its absolute value is the
        // required size.
        let required = if n < 0 { (-n) as usize } else { n as usize };
        if required == 0 {
            return Ok(Vec::new());
        }
        let mut tokens: Vec<LlamaToken> = vec![0; required];
        let n2 = unsafe {
            self.inner.lib.llama_tokenize(
                vocab,
                text_bytes.as_ptr() as *const std::os::raw::c_char,
                text_bytes.len() as i32,
                tokens.as_mut_ptr(),
                required as i32,
                add_special,
                parse_special,
            )
        };
        if n2 < 0 {
            return Err(LlamaError::TokenizeFailed(n2));
        }
        tokens.truncate(n2 as usize);
        Ok(tokens)
    }

    /// Convert a token id to its raw byte representation (piece).
    ///
    /// # Arguments
    /// * `token`   – the token id.
    /// * `special` – whether to render special tokens as text.
    ///
    /// # Errors
    /// Returns [`LlamaError::TokenToPieceFailed`] on failure.
    pub fn token_to_piece_bytes(
        &self,
        token: LlamaToken,
        special: bool,
    ) -> Result<Vec<u8>, LlamaError> {
        let vocab = self.vocab();
        // First call to get required buffer length.
        let n = unsafe {
            self.inner.lib.llama_token_to_piece(vocab, token, std::ptr::null_mut(), 0, 0, special)
        };
        // Like llama_tokenize/llama_detokenize, negative means the buffer was too
        // small and abs(n) is the required byte length.
        let required = if n < 0 {
            n.checked_abs().ok_or(LlamaError::TokenToPieceFailed(n))? as usize
        } else {
            n as usize
        };
        if required == 0 {
            return Ok(Vec::new());
        }
        let mut buf: Vec<u8> = vec![0u8; required];
        let len2 = unsafe {
            self.inner.lib.llama_token_to_piece(
                vocab,
                token,
                buf.as_mut_ptr() as *mut std::os::raw::c_char,
                required as i32,
                0,
                special,
            )
        };
        if len2 < 0 {
            return Err(LlamaError::TokenToPieceFailed(len2));
        }
        buf.truncate(len2 as usize);
        Ok(buf)
    }

    /// Convert a token id to its string representation (piece).
    ///
    /// # Arguments
    /// * `token`   鈥?the token id.
    /// * `special` 鈥?whether to render special tokens as text.
    ///
    /// # Errors
    /// Returns [`LlamaError::TokenToPieceFailed`] or [`LlamaError::InvalidUtf8`] on failure.
    pub fn token_to_piece(&self, token: LlamaToken, special: bool) -> Result<String, LlamaError> {
        let bytes = self.token_to_piece_bytes(token, special)?;
        String::from_utf8(bytes).map_err(|e| LlamaError::from(e.utf8_error()))
    }

    /// Detokenize a list of token ids into a string.
    ///
    /// # Arguments
    /// * `tokens`        – slice of token ids.
    /// * `remove_special` – remove leading/trailing special tokens from output.
    /// * `unparse_special` – render special tokens as their text forms.
    ///
    /// # Errors
    /// Returns [`LlamaError::TokenToPieceFailed`] or [`LlamaError::InvalidUtf8`] on failure.
    pub fn tokens_to_str(
        &self,
        tokens: &[LlamaToken],
        remove_special: bool,
        unparse_special: bool,
    ) -> Result<String, LlamaError> {
        let vocab = self.vocab();
        // Determine required buffer size.
        let n = unsafe {
            self.inner.lib.llama_detokenize(
                vocab,
                tokens.as_ptr(),
                tokens.len() as i32,
                std::ptr::null_mut(),
                0,
                remove_special,
                unparse_special,
            )
        };
        // Negative means the destination buffer was too small; abs(n) is the
        // required number of bytes.
        let required = if n < 0 {
            n.checked_abs().ok_or(LlamaError::TokenToPieceFailed(n))? as usize
        } else {
            n as usize
        };
        if required == 0 {
            return Ok(String::new());
        }
        let mut buf: Vec<u8> = vec![0u8; required];
        let len2 = unsafe {
            self.inner.lib.llama_detokenize(
                vocab,
                tokens.as_ptr(),
                tokens.len() as i32,
                buf.as_mut_ptr() as *mut std::os::raw::c_char,
                required as i32,
                remove_special,
                unparse_special,
            )
        };
        if len2 < 0 {
            return Err(LlamaError::TokenToPieceFailed(len2));
        }
        buf.truncate(len2 as usize);
        String::from_utf8(buf).map_err(|e| LlamaError::from(e.utf8_error()))
    }

    // ── vocabulary helpers ────────────────────────────────────────────────────

    /// Number of tokens in the vocabulary.
    pub fn n_vocab(&self) -> i32 {
        unsafe { self.inner.lib.llama_vocab_n_tokens(self.vocab()) }
    }

    /// BOS (beginning-of-sentence) token id.
    pub fn token_bos(&self) -> LlamaToken {
        unsafe { self.inner.lib.llama_vocab_bos(self.vocab()) }
    }

    /// EOS (end-of-sentence) token id.
    pub fn token_eos(&self) -> LlamaToken {
        unsafe { self.inner.lib.llama_vocab_eos(self.vocab()) }
    }

    /// EOT (end-of-turn) token id.
    pub fn token_eot(&self) -> LlamaToken {
        unsafe { self.inner.lib.llama_vocab_eot(self.vocab()) }
    }

    /// NL (newline) token id.
    pub fn token_nl(&self) -> LlamaToken {
        unsafe { self.inner.lib.llama_vocab_nl(self.vocab()) }
    }

    /// Padding token id.
    pub fn token_pad(&self) -> LlamaToken {
        unsafe { self.inner.lib.llama_vocab_pad(self.vocab()) }
    }

    /// Returns `true` if `token` is an end-of-generation token.
    pub fn token_is_eog(&self, token: LlamaToken) -> bool {
        unsafe { self.inner.lib.llama_vocab_is_eog(self.vocab(), token) }
    }

    /// Cached list of end-of-generation token ids discovered when the model was loaded.
    pub fn eog_tokens(&self) -> &[LlamaToken] {
        &self.inner.eog_tokens
    }

    /// Cached `-inf` logit biases for all end-of-generation tokens.
    pub fn eog_logit_bias(&self) -> &[slab_llama_sys::llama_logit_bias] {
        &self.inner.eog_logit_bias
    }

    /// Classify a stop token into the most specific llama.cpp category we know.
    pub fn token_stop_kind(&self, token: LlamaToken) -> Option<&'static str> {
        if token == self.token_eos() {
            Some("eos")
        } else if token == self.token_eot() {
            Some("eot")
        } else if self.inner.eog_tokens.contains(&token) {
            Some("eog")
        } else {
            None
        }
    }

    // ── model metadata ───────────────────────────────────────────────────────

    /// Training context length.
    pub fn n_ctx_train(&self) -> i32 {
        unsafe { self.inner.lib.llama_model_n_ctx_train(self.inner.model.unwrap().as_ptr()) }
    }

    /// Embedding dimension.
    pub fn n_embd(&self) -> i32 {
        unsafe { self.inner.lib.llama_model_n_embd(self.inner.model.unwrap().as_ptr()) }
    }

    /// Number of layers.
    pub fn n_layer(&self) -> i32 {
        unsafe { self.inner.lib.llama_model_n_layer(self.inner.model.unwrap().as_ptr()) }
    }

    /// Number of attention heads.
    pub fn n_head(&self) -> i32 {
        unsafe { self.inner.lib.llama_model_n_head(self.inner.model.unwrap().as_ptr()) }
    }

    /// Number of KV attention heads.
    pub fn n_head_kv(&self) -> i32 {
        unsafe { self.inner.lib.llama_model_n_head_kv(self.inner.model.unwrap().as_ptr()) }
    }

    /// Total number of parameters.
    pub fn n_params(&self) -> u64 {
        unsafe { self.inner.lib.llama_model_n_params(self.inner.model.unwrap().as_ptr()) }
    }

    /// Model size in bytes.
    pub fn model_size(&self) -> u64 {
        unsafe { self.inner.lib.llama_model_size(self.inner.model.unwrap().as_ptr()) }
    }

    /// A human-readable description of the model.
    ///
    /// # Returns
    /// `Ok(String)` on success, `Err(LlamaError)` on failure.
    pub fn desc(&self) -> Result<String, LlamaError> {
        let mut buf = vec![0u8; 256];
        let n = unsafe {
            self.inner.lib.llama_model_desc(
                self.inner.model.unwrap().as_ptr(),
                buf.as_mut_ptr() as *mut std::os::raw::c_char,
                buf.len(),
            )
        };
        if n < 0 {
            return Err(LlamaError::NullPointer);
        }
        buf.truncate(n as usize);
        String::from_utf8(buf).map_err(|e| LlamaError::from(e.utf8_error()))
    }

    /// Whether the model has an encoder (e.g. encoder-decoder models).
    pub fn has_encoder(&self) -> bool {
        unsafe { self.inner.lib.llama_model_has_encoder(self.inner.model.unwrap().as_ptr()) }
    }

    /// Whether the model has a decoder.
    pub fn has_decoder(&self) -> bool {
        unsafe { self.inner.lib.llama_model_has_decoder(self.inner.model.unwrap().as_ptr()) }
    }

    /// Whether the model is a recurrent model.
    pub fn is_recurrent(&self) -> bool {
        unsafe { self.inner.lib.llama_model_is_recurrent(self.inner.model.unwrap().as_ptr()) }
    }

    /// Create a new default sampler chain for this model.
    ///
    /// Convenience helper so callers do not need direct access to the
    /// underlying `LlamaLib` handle.
    pub fn new_sampler(&self) -> LlamaSampler {
        SamplerChainBuilder::new(Arc::clone(&self.inner.lib)).build()
    }

    /// Create a sampler chain with an optional raw GBNF constraint.
    ///
    /// When `gbnf` is `Some(raw_gbnf)` the grammar sampler is inserted into the
    /// chain after the temperature sampler and before the final distribution
    /// sampler.  If grammar initialisation fails (invalid GBNF, null vocab, or
    /// unsupported runtime) a warning is logged and the chain falls back to
    /// standard unconstrained sampling — identical to calling [`new_sampler`].
    ///
    /// When `gbnf` is `None` this is equivalent to [`new_sampler`].
    ///
    /// [`new_sampler`]: LlamaModel::new_sampler
    pub fn new_sampler_with_gbnf(&self, gbnf: Option<&str>) -> LlamaSampler {
        self.new_sampler_with_options(gbnf, None, None, None, None, None, None, false, &[])
    }

    /// Create a sampler chain with optional GBNF constraint and explicit
    /// sampling overrides.
    ///
    /// When an override is `None`, the builder default is used.
    pub fn new_sampler_with_options(
        &self,
        gbnf: Option<&str>,
        temperature: Option<f32>,
        top_p: Option<f32>,
        top_k: Option<i32>,
        min_p: Option<f32>,
        repetition_penalty: Option<f32>,
        presence_penalty: Option<f32>,
        ignore_eos: bool,
        logit_bias: &[LlamaLogitBias],
    ) -> LlamaSampler {
        let mut builder = SamplerChainBuilder::new(Arc::clone(&self.inner.lib));
        if let Some(t) = temperature {
            builder.temperature = t;
        }
        if let Some(p) = top_p {
            builder.top_p = p;
        }
        if let Some(k) = top_k {
            builder.top_k = k;
        }
        if let Some(p) = min_p {
            builder.min_p = p;
        }
        if let Some(penalty) = repetition_penalty {
            builder.repeat_penalty = penalty;
        }
        if let Some(penalty) = presence_penalty {
            builder.presence_penalty = penalty;
        }
        let raw_logit_bias =
            collect_sampler_logit_bias(logit_bias, ignore_eos, self.eog_logit_bias());
        if !raw_logit_bias.is_empty() {
            builder.set_logit_bias(self.n_vocab(), raw_logit_bias);
        }
        match gbnf {
            None | Some("") => builder.build(),
            Some(gbnf_str) => builder.build_with_grammar(self.vocab(), gbnf_str),
        }
    }

    /// Retrieve a metadata value by key.
    ///
    /// # Returns
    /// `Ok(String)` if found, `Err(LlamaError)` if not.
    pub fn meta_val_str(&self, key: &str) -> Result<String, LlamaError> {
        let c_key = CString::new(key)?;
        let mut buf = vec![0u8; 512];
        let n = unsafe {
            self.inner.lib.llama_model_meta_val_str(
                self.inner.model.unwrap().as_ptr(),
                c_key.as_ptr(),
                buf.as_mut_ptr() as *mut std::os::raw::c_char,
                buf.len(),
            )
        };
        if n < 0 {
            return Err(LlamaError::NullPointer);
        }
        buf.truncate(n as usize);
        String::from_utf8(buf).map_err(|e| LlamaError::from(e.utf8_error()))
    }
}

impl std::fmt::Debug for LlamaModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlamaModel").finish()
    }
}
