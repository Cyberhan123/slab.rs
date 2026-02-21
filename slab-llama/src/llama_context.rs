use std::ffi::CString;
use std::sync::Arc;

use crate::error::LlamaError;
use crate::llama_adapter::LlamaLoraAdapter;
use crate::llama_batch::LlamaBatch;
use crate::llama_model::LlamaModelInner;
use crate::token::{LlamaSeqId, LlamaToken};

/// A safe wrapper around a llama inference context.
///
/// Created via [`crate::llama_model::LlamaModel::new_context`].
pub struct LlamaContext {
    pub(crate) ctx: *mut slab_llama_sys::llama_context,
    /// Keep the model alive as long as the context exists.
    pub(crate) model: Arc<LlamaModelInner>,
}

unsafe impl Send for LlamaContext {}
unsafe impl Sync for LlamaContext {}

impl Drop for LlamaContext {
    fn drop(&mut self) {
        unsafe { self.model.lib.llama_free(self.ctx) };
    }
}

impl LlamaContext {
    // ── Context size helpers ─────────────────────────────────────────────────

    /// Returns the context window size.
    pub fn n_ctx(&self) -> u32 {
        unsafe { self.model.lib.llama_n_ctx(self.ctx) }
    }

    /// Returns the batch size.
    pub fn n_batch(&self) -> u32 {
        unsafe { self.model.lib.llama_n_batch(self.ctx) }
    }

    /// Returns the physical batch size.
    pub fn n_ubatch(&self) -> u32 {
        unsafe { self.model.lib.llama_n_ubatch(self.ctx) }
    }

    /// Returns the maximum number of sequences.
    pub fn n_seq_max(&self) -> u32 {
        unsafe { self.model.lib.llama_n_seq_max(self.ctx) }
    }

    // ── Decoding ─────────────────────────────────────────────────────────────

    /// Decode a batch of tokens.
    ///
    /// This is the core inference step.  After decoding you can read logits
    /// with [`Self::get_logits_ith`] and sample the next token.
    ///
    /// # Arguments
    /// * `batch` – the batch to decode.
    ///
    /// # Errors
    /// Returns [`LlamaError::DecodeFailed`] if llama.cpp reports an error.
    pub fn decode(&mut self, batch: &mut LlamaBatch) -> Result<(), LlamaError> {
        let raw_batch = batch.as_llama_batch();
        let ret = unsafe { self.model.lib.llama_decode(self.ctx, raw_batch) };
        if ret != 0 {
            Err(LlamaError::DecodeFailed(ret))
        } else {
            Ok(())
        }
    }

    // ── Internal helpers ─────────────────────────────────────────────────────

    fn n_vocab(&self) -> usize {
        unsafe {
            self.model.lib.llama_vocab_n_tokens(
                self.model.lib.llama_model_get_vocab(self.model.model)
            ) as usize
        }
    }

    // ── Logits ───────────────────────────────────────────────────────────────

    /// Return a slice of logits for the i-th token in the last decoded batch.
    ///
    /// # Safety
    /// `i` must be a valid index into the decoded batch where logits were
    /// requested (i.e. `logit == true` when calling [`LlamaBatch::add`]).
    ///
    /// # Panics
    /// Panics if the returned pointer is null.
    pub fn get_logits_ith(&self, i: i32) -> &[f32] {
        let n_vocab = self.n_vocab();
        let ptr = unsafe { self.model.lib.llama_get_logits_ith(self.ctx, i) };
        assert!(!ptr.is_null(), "llama_get_logits_ith returned null");
        unsafe { std::slice::from_raw_parts(ptr, n_vocab) }
    }

    /// Return the logits for a single output token from the last decoded batch.
    ///
    /// This returns a slice of length `n_vocab` corresponding to one token's
    /// logits (typically the last token processed by the most recent decode).
    /// For individual token logits by index use [`Self::get_logits_ith`].
    ///
    /// # Panics
    /// Panics if the returned pointer is null.
    pub fn get_logits(&self) -> &[f32] {
        let n_vocab = self.n_vocab();
        let ptr = unsafe { self.model.lib.llama_get_logits(self.ctx) };
        assert!(!ptr.is_null(), "llama_get_logits returned null");
        unsafe { std::slice::from_raw_parts(ptr, n_vocab) }
    }

    // ── Thread control ────────────────────────────────────────────────────────

    /// Set the number of threads for generation and batch processing.
    pub fn set_n_threads(&mut self, n_threads: i32, n_threads_batch: i32) {
        unsafe { self.model.lib.llama_set_n_threads(self.ctx, n_threads, n_threads_batch) }
    }

    // ── Performance ──────────────────────────────────────────────────────────

    /// Print performance statistics to stderr.
    pub fn perf_print(&self) {
        unsafe { self.model.lib.llama_perf_context_print(self.ctx) }
    }

    /// Reset performance statistics.
    pub fn perf_reset(&mut self) {
        unsafe { self.model.lib.llama_perf_context_reset(self.ctx) }
    }

    // ── KV-cache management ──────────────────────────────────────────────────

    /// Clear all tokens from all sequences in the KV cache.
    pub fn kv_cache_clear(&mut self) {
        let mem = unsafe { self.model.lib.llama_get_memory(self.ctx) };
        if !mem.is_null() {
            unsafe { self.model.lib.llama_memory_clear(mem, true) }
        }
    }

    /// Remove a range of tokens `[p0, p1)` from sequence `seq_id` in the KV cache.
    pub fn kv_cache_seq_rm(&mut self, seq_id: i32, p0: i32, p1: i32) -> bool {
        let mem = unsafe { self.model.lib.llama_get_memory(self.ctx) };
        if mem.is_null() {
            return false;
        }
        unsafe { self.model.lib.llama_memory_seq_rm(mem, seq_id, p0, p1) }
    }

    // ── LoRA adapters ────────────────────────────────────────────────────────

    /// Apply LoRA adapters to this context.
    ///
    /// Pass an empty slice to remove all currently active adapters.  The
    /// `scales` slice must have the same length as `adapters`; each element is
    /// the scale factor for the corresponding adapter (typically `1.0`).
    ///
    /// # Errors
    /// Returns [`LlamaError::SetAdaptersFailed`] if the underlying call fails.
    pub fn set_adapters_lora(
        &mut self,
        adapters: &[&LlamaLoraAdapter],
        scales: &[f32],
    ) -> Result<(), LlamaError> {
        assert_eq!(
            adapters.len(),
            scales.len(),
            "adapters and scales must have the same length"
        );
        let mut ptrs: Vec<*mut slab_llama_sys::llama_adapter_lora> =
            adapters.iter().map(|a| a.adapter).collect();
        let scales_ptr = if scales.is_empty() {
            std::ptr::null_mut()
        } else {
            // The llama.cpp C API declares `float *` (non-const) for `scales`
            // but only reads the values. Cast from `*const f32` is safe here.
            scales.as_ptr() as *mut f32
        };
        let ret = unsafe {
            self.model.lib.llama_set_adapters_lora(
                self.ctx,
                ptrs.as_mut_ptr(),
                ptrs.len(),
                scales_ptr,
            )
        };
        if ret != 0 {
            Err(LlamaError::SetAdaptersFailed(ret))
        } else {
            Ok(())
        }
    }

    // ── State management ─────────────────────────────────────────────────────

    /// Return the exact number of bytes needed to store the full context state.
    pub fn state_get_size(&self) -> usize {
        unsafe { self.model.lib.llama_state_get_size(self.ctx) }
    }

    /// Copy the full context state into `dst`.
    ///
    /// `dst` must have at least [`Self::state_get_size`] bytes.
    ///
    /// # Returns
    /// Number of bytes written.
    ///
    /// # Errors
    /// Returns [`LlamaError::StateFailed`] if 0 bytes were written.
    pub fn state_get_data(&self, dst: &mut [u8]) -> Result<usize, LlamaError> {
        let n = unsafe {
            self.model
                .lib
                .llama_state_get_data(self.ctx, dst.as_mut_ptr(), dst.len())
        };
        if n == 0 {
            Err(LlamaError::StateFailed)
        } else {
            Ok(n)
        }
    }

    /// Restore context state from `src`.
    ///
    /// # Returns
    /// Number of bytes consumed from `src`.
    ///
    /// # Errors
    /// Returns [`LlamaError::StateFailed`] if 0 bytes were consumed.
    pub fn state_set_data(&mut self, src: &[u8]) -> Result<usize, LlamaError> {
        let n = unsafe {
            self.model
                .lib
                .llama_state_set_data(self.ctx, src.as_ptr(), src.len())
        };
        if n == 0 {
            Err(LlamaError::StateFailed)
        } else {
            Ok(n)
        }
    }

    /// Load context state and the prompt tokens from a session file.
    ///
    /// # Arguments
    /// * `path`            – path to the session file.
    /// * `token_capacity`  – maximum number of tokens to read back.
    ///
    /// # Returns
    /// `Ok(Vec<LlamaToken>)` with the saved prompt tokens on success.
    ///
    /// # Errors
    /// Returns [`LlamaError::StateFailed`] if the file could not be read.
    pub fn state_load_file(
        &mut self,
        path: &str,
        token_capacity: usize,
    ) -> Result<Vec<LlamaToken>, LlamaError> {
        let c_path = CString::new(path)?;
        let mut tokens: Vec<LlamaToken> = vec![0; token_capacity];
        let mut n_token_count: usize = 0;
        let ok = unsafe {
            self.model.lib.llama_state_load_file(
                self.ctx,
                c_path.as_ptr(),
                tokens.as_mut_ptr(),
                token_capacity,
                &mut n_token_count,
            )
        };
        if !ok {
            return Err(LlamaError::StateFailed);
        }
        tokens.truncate(n_token_count);
        Ok(tokens)
    }

    /// Save context state and the given prompt tokens to a session file.
    ///
    /// # Arguments
    /// * `path`   – path to the session file to write.
    /// * `tokens` – the prompt tokens to embed in the session.
    ///
    /// # Errors
    /// Returns [`LlamaError::StateFailed`] if the file could not be written.
    pub fn state_save_file(
        &self,
        path: &str,
        tokens: &[LlamaToken],
    ) -> Result<(), LlamaError> {
        let c_path = CString::new(path)?;
        let ok = unsafe {
            self.model.lib.llama_state_save_file(
                self.ctx,
                c_path.as_ptr(),
                tokens.as_ptr(),
                tokens.len(),
            )
        };
        if !ok {
            Err(LlamaError::StateFailed)
        } else {
            Ok(())
        }
    }

    // ── Per-sequence state ────────────────────────────────────────────────────

    /// Return the exact number of bytes needed to store the state of `seq_id`.
    pub fn state_seq_get_size(&self, seq_id: LlamaSeqId) -> usize {
        unsafe { self.model.lib.llama_state_seq_get_size(self.ctx, seq_id) }
    }

    /// Copy the state of `seq_id` into `dst`.
    ///
    /// # Returns
    /// Number of bytes written.
    ///
    /// # Errors
    /// Returns [`LlamaError::StateFailed`] if 0 bytes were written.
    pub fn state_seq_get_data(
        &self,
        dst: &mut [u8],
        seq_id: LlamaSeqId,
    ) -> Result<usize, LlamaError> {
        let n = unsafe {
            self.model.lib.llama_state_seq_get_data(
                self.ctx,
                dst.as_mut_ptr(),
                dst.len(),
                seq_id,
            )
        };
        if n == 0 {
            Err(LlamaError::StateFailed)
        } else {
            Ok(n)
        }
    }

    /// Restore the state of `dest_seq_id` from `src`.
    ///
    /// # Returns
    /// Number of bytes consumed.
    ///
    /// # Errors
    /// Returns [`LlamaError::StateFailed`] if 0 bytes were consumed.
    pub fn state_seq_set_data(
        &mut self,
        src: &[u8],
        dest_seq_id: LlamaSeqId,
    ) -> Result<usize, LlamaError> {
        let n = unsafe {
            self.model.lib.llama_state_seq_set_data(
                self.ctx,
                src.as_ptr(),
                src.len(),
                dest_seq_id,
            )
        };
        if n == 0 {
            Err(LlamaError::StateFailed)
        } else {
            Ok(n)
        }
    }

    /// Save the state of `seq_id` and the given tokens to a file.
    ///
    /// # Returns
    /// Number of bytes written to the file.
    ///
    /// # Errors
    /// Returns [`LlamaError::StateFailed`] if 0 bytes were written.
    pub fn state_seq_save_file(
        &self,
        filepath: &str,
        seq_id: LlamaSeqId,
        tokens: &[LlamaToken],
    ) -> Result<usize, LlamaError> {
        let c_path = CString::new(filepath)?;
        let n = unsafe {
            self.model.lib.llama_state_seq_save_file(
                self.ctx,
                c_path.as_ptr(),
                seq_id,
                tokens.as_ptr(),
                tokens.len(),
            )
        };
        if n == 0 {
            Err(LlamaError::StateFailed)
        } else {
            Ok(n)
        }
    }

    /// Load the state of `dest_seq_id` and the saved tokens from a file.
    ///
    /// # Arguments
    /// * `filepath`        – path to the sequence state file.
    /// * `dest_seq_id`     – the sequence to restore the state into.
    /// * `token_capacity`  – maximum number of tokens to read back.
    ///
    /// # Returns
    /// `Ok(Vec<LlamaToken>)` with the saved tokens on success.
    ///
    /// # Errors
    /// Returns [`LlamaError::StateFailed`] if the file could not be read.
    pub fn state_seq_load_file(
        &mut self,
        filepath: &str,
        dest_seq_id: LlamaSeqId,
        token_capacity: usize,
    ) -> Result<Vec<LlamaToken>, LlamaError> {
        let c_path = CString::new(filepath)?;
        let mut tokens: Vec<LlamaToken> = vec![0; token_capacity];
        let mut n_token_count: usize = 0;
        let n = unsafe {
            self.model.lib.llama_state_seq_load_file(
                self.ctx,
                c_path.as_ptr(),
                dest_seq_id,
                tokens.as_mut_ptr(),
                token_capacity,
                &mut n_token_count,
            )
        };
        if n == 0 {
            return Err(LlamaError::StateFailed);
        }
        tokens.truncate(n_token_count);
        Ok(tokens)
    }

    /// Return the exact number of bytes needed to store the state of `seq_id`
    /// with the given flags.
    pub fn state_seq_get_size_ext(
        &self,
        seq_id: LlamaSeqId,
        flags: slab_llama_sys::llama_state_seq_flags,
    ) -> usize {
        unsafe {
            self.model
                .lib
                .llama_state_seq_get_size_ext(self.ctx, seq_id, flags)
        }
    }

    /// Copy the state of `seq_id` into `dst` with the given flags.
    ///
    /// # Returns
    /// Number of bytes written.
    ///
    /// # Errors
    /// Returns [`LlamaError::StateFailed`] if 0 bytes were written.
    pub fn state_seq_get_data_ext(
        &self,
        dst: &mut [u8],
        seq_id: LlamaSeqId,
        flags: slab_llama_sys::llama_state_seq_flags,
    ) -> Result<usize, LlamaError> {
        let n = unsafe {
            self.model.lib.llama_state_seq_get_data_ext(
                self.ctx,
                dst.as_mut_ptr(),
                dst.len(),
                seq_id,
                flags,
            )
        };
        if n == 0 {
            Err(LlamaError::StateFailed)
        } else {
            Ok(n)
        }
    }

    /// Restore the state of `dest_seq_id` from `src` with the given flags.
    ///
    /// # Returns
    /// Number of bytes consumed.
    ///
    /// # Errors
    /// Returns [`LlamaError::StateFailed`] if 0 bytes were consumed.
    pub fn state_seq_set_data_ext(
        &mut self,
        src: &[u8],
        dest_seq_id: LlamaSeqId,
        flags: slab_llama_sys::llama_state_seq_flags,
    ) -> Result<usize, LlamaError> {
        let n = unsafe {
            self.model.lib.llama_state_seq_set_data_ext(
                self.ctx,
                src.as_ptr(),
                src.len(),
                dest_seq_id,
                flags,
            )
        };
        if n == 0 {
            Err(LlamaError::StateFailed)
        } else {
            Ok(n)
        }
    }
}

impl std::fmt::Debug for LlamaContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlamaContext").finish()
    }
}
