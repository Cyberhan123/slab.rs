use std::ffi::{CStr, CString};
use std::sync::Arc;

use crate::context_params::LlamaContextParams;
use crate::error::LlamaError;
use crate::llama_adapter::LlamaLoraAdapter;
use crate::llama_context::LlamaContext;
use crate::llama_sampler::SamplerChainBuilder;
use crate::token::LlamaToken;
use crate::Llama;
use crate::LlamaSampler;

/// Inner (non-Clone) model data.  Wrapped in Arc so that LlamaContext can keep
/// the model alive without copying the raw pointer.
pub(crate) struct LlamaModelInner {
    pub(crate) model: *mut slab_llama_sys::llama_model,
    pub(crate) lib: Arc<slab_llama_sys::LlamaLib>,
}

unsafe impl Send for LlamaModelInner {}
unsafe impl Sync for LlamaModelInner {}

impl Drop for LlamaModelInner {
    fn drop(&mut self) {
        unsafe { self.lib.llama_model_free(self.model) };
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
        let model = unsafe {
            self.lib
                .llama_model_load_from_file(c_path.as_ptr(), c_params)
        };
        if model.is_null() {
            Err(LlamaError::ModelLoadFailed)
        } else {
            Ok(LlamaModel {
                inner: Arc::new(LlamaModelInner {
                    model,
                    lib: Arc::clone(&self.lib),
                }),
            })
        }
    }
}

impl LlamaModel {
    /// Return the vocab pointer (for tokenization helpers).
    fn vocab(&self) -> *const slab_llama_sys::llama_vocab {
        unsafe { self.inner.lib.llama_model_get_vocab(self.inner.model) }
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
        let ctx = unsafe {
            self.inner
                .lib
                .llama_init_from_model(self.inner.model, c_params)
        };
        if ctx.is_null() {
            Err(LlamaError::ContextCreateFailed)
        } else {
            Ok(LlamaContext {
                ctx,
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
        let adapter = unsafe {
            self.inner
                .lib
                .llama_adapter_lora_init(self.inner.model, c_path.as_ptr())
        };
        if adapter.is_null() {
            Err(LlamaError::LoraAdapterLoadFailed)
        } else {
            Ok(LlamaLoraAdapter {
                adapter,
                model: Arc::clone(&self.inner),
            })
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

    /// Convert a token id to its string representation (piece).
    ///
    /// # Arguments
    /// * `token`   – the token id.
    /// * `special` – whether to render special tokens as text.
    ///
    /// # Errors
    /// Returns [`LlamaError::TokenToPieceFailed`] or [`LlamaError::InvalidUtf8`] on failure.
    pub fn token_to_piece(&self, token: LlamaToken, special: bool) -> Result<String, LlamaError> {
        let vocab = self.vocab();
        // First call to get required buffer length.
        let len = unsafe {
            self.inner
                .lib
                .llama_token_to_piece(vocab, token, std::ptr::null_mut(), 0, 0, special)
        };
        if len < 0 {
            return Err(LlamaError::TokenToPieceFailed(len));
        }
        if len == 0 {
            return Ok(String::new());
        }
        let mut buf: Vec<u8> = vec![0u8; len as usize];
        let len2 = unsafe {
            self.inner.lib.llama_token_to_piece(
                vocab,
                token,
                buf.as_mut_ptr() as *mut std::os::raw::c_char,
                len,
                0,
                special,
            )
        };
        if len2 < 0 {
            return Err(LlamaError::TokenToPieceFailed(len2));
        }
        buf.truncate(len2 as usize);
        String::from_utf8(buf).map_err(|e| LlamaError::from(e.utf8_error()))
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
        let len = unsafe {
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
        if len < 0 {
            return Err(LlamaError::TokenToPieceFailed(len));
        }
        if len == 0 {
            return Ok(String::new());
        }
        let mut buf: Vec<u8> = vec![0u8; len as usize];
        let len2 = unsafe {
            self.inner.lib.llama_detokenize(
                vocab,
                tokens.as_ptr(),
                tokens.len() as i32,
                buf.as_mut_ptr() as *mut std::os::raw::c_char,
                len,
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

    // ── model metadata ───────────────────────────────────────────────────────

    /// Training context length.
    pub fn n_ctx_train(&self) -> i32 {
        unsafe { self.inner.lib.llama_model_n_ctx_train(self.inner.model) }
    }

    /// Embedding dimension.
    pub fn n_embd(&self) -> i32 {
        unsafe { self.inner.lib.llama_model_n_embd(self.inner.model) }
    }

    /// Number of layers.
    pub fn n_layer(&self) -> i32 {
        unsafe { self.inner.lib.llama_model_n_layer(self.inner.model) }
    }

    /// Number of attention heads.
    pub fn n_head(&self) -> i32 {
        unsafe { self.inner.lib.llama_model_n_head(self.inner.model) }
    }

    /// Number of KV attention heads.
    pub fn n_head_kv(&self) -> i32 {
        unsafe { self.inner.lib.llama_model_n_head_kv(self.inner.model) }
    }

    /// Total number of parameters.
    pub fn n_params(&self) -> u64 {
        unsafe { self.inner.lib.llama_model_n_params(self.inner.model) }
    }

    /// Model size in bytes.
    pub fn model_size(&self) -> u64 {
        unsafe { self.inner.lib.llama_model_size(self.inner.model) }
    }

    /// A human-readable description of the model.
    ///
    /// # Returns
    /// `Ok(String)` on success, `Err(LlamaError)` on failure.
    pub fn desc(&self) -> Result<String, LlamaError> {
        let mut buf = vec![0u8; 256];
        let n = unsafe {
            self.inner.lib.llama_model_desc(
                self.inner.model,
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
        unsafe { self.inner.lib.llama_model_has_encoder(self.inner.model) }
    }

    /// Whether the model has a decoder.
    pub fn has_decoder(&self) -> bool {
        unsafe { self.inner.lib.llama_model_has_decoder(self.inner.model) }
    }

    /// Whether the model is a recurrent model.
    pub fn is_recurrent(&self) -> bool {
        unsafe { self.inner.lib.llama_model_is_recurrent(self.inner.model) }
    }

    /// Create a new default sampler chain for this model.
    ///
    /// Convenience helper so callers do not need direct access to the
    /// underlying `LlamaLib` handle.
    pub fn new_sampler(&self) -> LlamaSampler {
        SamplerChainBuilder::new(Arc::clone(&self.inner.lib)).build()
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
                self.inner.model,
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

    /// Retrieve the built-in chat template for this model (if any).
    ///
    /// # Arguments
    /// * `name` – optional template name; pass `None` for the default template.
    pub fn chat_template(&self, name: Option<&str>) -> Result<&str, LlamaError> {
        let c_name: Option<CString> = name.map(CString::new).transpose()?;
        let ptr = unsafe {
            self.inner.lib.llama_model_chat_template(
                self.inner.model,
                c_name.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
            )
        };
        if ptr.is_null() {
            return Err(LlamaError::NullPointer);
        }
        let cstr = unsafe { CStr::from_ptr(ptr) };
        cstr.to_str().map_err(LlamaError::from)
    }
}

impl std::fmt::Debug for LlamaModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlamaModel").finish()
    }
}
