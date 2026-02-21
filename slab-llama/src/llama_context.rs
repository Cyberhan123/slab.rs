use std::sync::Arc;

use crate::error::LlamaError;
use crate::llama_batch::LlamaBatch;
use crate::llama_model::LlamaModelInner;

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
}

impl std::fmt::Debug for LlamaContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlamaContext").finish()
    }
}
