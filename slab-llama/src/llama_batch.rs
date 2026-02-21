use crate::error::LlamaError;
use crate::token::{LlamaPos, LlamaSeqId, LlamaToken};

/// A safe wrapper around `llama_batch` that manages its own memory.
///
/// Use [`LlamaBatch::new`] to create a batch, [`LlamaBatch::add`] to add tokens,
/// and [`LlamaBatch::clear`] to reset it for reuse.
pub struct LlamaBatch {
    /// Owned token buffer.
    tokens: Vec<LlamaToken>,
    /// Owned position buffer.
    pos: Vec<LlamaPos>,
    /// Owned per-token sequence-id arrays.
    seq_ids: Vec<Vec<LlamaSeqId>>,
    /// Owned pointers into seq_ids (kept in sync with seq_ids).
    seq_id_ptrs: Vec<*mut LlamaSeqId>,
    /// Per-token sequence-id counts.
    n_seq_id: Vec<i32>,
    /// Per-token logit flags.
    logits: Vec<i8>,
    /// Maximum number of tokens this batch can hold.
    capacity: usize,
}

unsafe impl Send for LlamaBatch {}
unsafe impl Sync for LlamaBatch {}

impl LlamaBatch {
    /// Create a new batch with the given maximum token capacity.
    ///
    /// # Arguments
    /// * `capacity` – maximum number of tokens that can be added to this batch.
    pub fn new(capacity: usize) -> Self {
        Self {
            tokens: Vec::with_capacity(capacity),
            pos: Vec::with_capacity(capacity),
            seq_ids: Vec::with_capacity(capacity),
            seq_id_ptrs: Vec::with_capacity(capacity),
            n_seq_id: Vec::with_capacity(capacity),
            logits: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// Number of tokens currently in the batch.
    pub fn n_tokens(&self) -> i32 {
        self.tokens.len() as i32
    }

    /// Returns the maximum number of tokens this batch can hold.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Add a single token to the batch.
    ///
    /// # Arguments
    /// * `token`   – the token id.
    /// * `pos`     – the position in the sequence.
    /// * `seq_ids` – the sequence(s) this token belongs to.
    /// * `logit`   – whether to compute logits for this token.
    ///
    /// # Errors
    /// Returns [`LlamaError::BatchFull`] if the batch is at capacity.
    pub fn add(
        &mut self,
        token: LlamaToken,
        pos: LlamaPos,
        seq_ids: &[LlamaSeqId],
        logit: bool,
    ) -> Result<(), LlamaError> {
        if self.tokens.len() >= self.capacity {
            return Err(LlamaError::BatchFull);
        }

        self.tokens.push(token);
        self.pos.push(pos);

        let ids: Vec<LlamaSeqId> = seq_ids.to_vec();
        self.seq_ids.push(ids);
        // Store a placeholder; actual pointers are synchronized in `as_llama_batch`.
        self.seq_id_ptrs.push(std::ptr::null_mut());
        self.n_seq_id.push(seq_ids.len() as i32);
        self.logits.push(if logit { 1 } else { 0 });

        Ok(())
    }

    /// Clear all tokens from the batch, allowing it to be reused.
    pub fn clear(&mut self) {
        self.tokens.clear();
        self.pos.clear();
        self.seq_ids.clear();
        self.seq_id_ptrs.clear();
        self.n_seq_id.clear();
        self.logits.clear();
    }

    /// Build the raw `llama_batch` struct for passing to `llama_decode`.
    ///
    /// # Safety
    /// The returned `llama_batch` borrows from this struct.  It must not outlive `self`,
    /// and `self` must not be mutated while the returned struct is in use.
    pub(crate) fn as_llama_batch(&mut self) -> slab_llama_sys::llama_batch {
        // Re-sync seq_id_ptrs in case Vec reallocated after previous add() calls.
        for (i, ids) in self.seq_ids.iter_mut().enumerate() {
            self.seq_id_ptrs[i] = ids.as_mut_ptr();
        }

        slab_llama_sys::llama_batch {
            n_tokens: self.tokens.len() as i32,
            token: self.tokens.as_mut_ptr(),
            embd: std::ptr::null_mut(),
            pos: self.pos.as_mut_ptr(),
            n_seq_id: self.n_seq_id.as_mut_ptr(),
            seq_id: self.seq_id_ptrs.as_mut_ptr(),
            logits: self.logits.as_mut_ptr(),
        }
    }
}
