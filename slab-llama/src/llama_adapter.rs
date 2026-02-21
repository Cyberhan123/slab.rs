use std::ffi::CString;
use std::sync::Arc;

use crate::error::LlamaError;
use crate::llama_model::LlamaModelInner;
use crate::token::LlamaToken;

/// A safe wrapper around a `llama_adapter_lora` LoRA adapter.
///
/// Created via [`crate::llama_model::LlamaModel::adapter_lora_init`].
///
/// Adapters are automatically freed when the associated model is freed.
/// Holding a [`LlamaLoraAdapter`] keeps the underlying model alive via its
/// `Arc<LlamaModelInner>`.
pub struct LlamaLoraAdapter {
    pub(crate) adapter: *mut slab_llama_sys::llama_adapter_lora,
    /// Keep the model alive as long as this adapter exists.
    pub(crate) model: Arc<LlamaModelInner>,
}

unsafe impl Send for LlamaLoraAdapter {}
unsafe impl Sync for LlamaLoraAdapter {}

impl LlamaLoraAdapter {
    // ── Metadata helpers ─────────────────────────────────────────────────────

    /// Retrieve a metadata value string by key.
    ///
    /// # Returns
    /// `Ok(String)` on success, `Err(LlamaError)` if the key is not found or
    /// the value cannot be decoded as UTF-8.
    pub fn meta_val_str(&self, key: &str) -> Result<String, LlamaError> {
        let c_key = CString::new(key)?;
        let mut buf = vec![0u8; 512];
        let n = unsafe {
            self.model.lib.llama_adapter_meta_val_str(
                self.adapter as *const _,
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

    /// Return the number of metadata key/value pairs in the adapter.
    pub fn meta_count(&self) -> i32 {
        unsafe { self.model.lib.llama_adapter_meta_count(self.adapter as *const _) }
    }

    /// Retrieve a metadata key name by its index.
    ///
    /// # Errors
    /// Returns `Err(LlamaError)` if `i` is out of range or on UTF-8 error.
    pub fn meta_key_by_index(&self, i: i32) -> Result<String, LlamaError> {
        let mut buf = vec![0u8; 256];
        let n = unsafe {
            self.model.lib.llama_adapter_meta_key_by_index(
                self.adapter as *const _,
                i,
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

    /// Retrieve a metadata value string by its index.
    ///
    /// # Errors
    /// Returns `Err(LlamaError)` if `i` is out of range or on UTF-8 error.
    pub fn meta_val_str_by_index(&self, i: i32) -> Result<String, LlamaError> {
        let mut buf = vec![0u8; 512];
        let n = unsafe {
            self.model.lib.llama_adapter_meta_val_str_by_index(
                self.adapter as *const _,
                i,
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

    // ── ALora helpers ────────────────────────────────────────────────────────

    /// Return the number of invocation tokens for an ALora adapter.
    ///
    /// Returns `0` for regular LoRA adapters.
    pub fn get_alora_n_invocation_tokens(&self) -> u64 {
        unsafe {
            self.model
                .lib
                .llama_adapter_get_alora_n_invocation_tokens(self.adapter as *const _)
        }
    }

    /// Return the invocation tokens for an ALora adapter as a slice.
    ///
    /// Returns an empty slice for regular LoRA adapters or if the pointer is null.
    pub fn get_alora_invocation_tokens(&self) -> &[LlamaToken] {
        let n = self.get_alora_n_invocation_tokens() as usize;
        if n == 0 {
            return &[];
        }
        let ptr = unsafe {
            self.model
                .lib
                .llama_adapter_get_alora_invocation_tokens(self.adapter as *const _)
        };
        if ptr.is_null() {
            return &[];
        }
        unsafe { std::slice::from_raw_parts(ptr, n) }
    }

    /// Convenience wrapper that reads the `"general.name"` metadata key.
    /// Returns `None` if the key is absent.
    pub fn name(&self) -> Option<String> {
        self.meta_val_str("general.name").ok()
    }
}

impl std::fmt::Debug for LlamaLoraAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlamaLoraAdapter").finish()
    }
}
