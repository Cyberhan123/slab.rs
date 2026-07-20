//! Structured performance / token-usage data for a [`crate::LlamaContext`].
//!
//! `llama_perf_context_data` exposes the authoritative token-usage counters
//! from llama.cpp: `n_p_eval` (prompt tokens processed during prefill) and
//! `n_eval` (tokens generated during decode), plus timings. Read it via
//! [`crate::LlamaContext::perf_data`] and pair with
//! [`crate::LlamaContext::perf_reset`] so the counters reflect a single turn.

use serde::{Deserialize, Serialize};

/// Mirror of `llama_perf_context_data` — timing and token counts for a context.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct LlamaPerfContextData {
    /// Wall-clock time at which the context was created (ms).
    pub t_start_ms: f64,
    /// Time spent loading the model (ms); usually 0 for a hot context.
    pub t_load_ms: f64,
    /// Time spent processing prompt tokens (prefill), ms.
    pub t_p_eval_ms: f64,
    /// Time spent generating tokens (decode), ms.
    pub t_eval_ms: f64,
    /// Number of prompt tokens processed since the last perf reset.
    pub n_p_eval: i32,
    /// Number of tokens generated since the last perf reset.
    pub n_eval: i32,
    /// Number of times a ggml compute graph was reused.
    pub n_reused: i32,
}

impl From<slab_llama_sys::llama_perf_context_data> for LlamaPerfContextData {
    fn from(data: slab_llama_sys::llama_perf_context_data) -> Self {
        let slab_llama_sys::llama_perf_context_data {
            t_start_ms,
            t_load_ms,
            t_p_eval_ms,
            t_eval_ms,
            n_p_eval,
            n_eval,
            n_reused,
        } = data;
        Self { t_start_ms, t_load_ms, t_p_eval_ms, t_eval_ms, n_p_eval, n_eval, n_reused }
    }
}

impl LlamaPerfContextData {
    /// Prompt (prefill) token count, clamped to `u32`.
    pub fn prompt_tokens(&self) -> u32 {
        self.n_p_eval.max(0) as u32
    }

    /// Generated (decode) token count, clamped to `u32`.
    pub fn generated_tokens(&self) -> u32 {
        self.n_eval.max(0) as u32
    }

    /// Total tokens processed = prompt + generated.
    pub fn total_tokens(&self) -> u32 {
        self.prompt_tokens().saturating_add(self.generated_tokens())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(n_p_eval: i32, n_eval: i32) -> slab_llama_sys::llama_perf_context_data {
        slab_llama_sys::llama_perf_context_data {
            t_start_ms: 1.0,
            t_load_ms: 2.0,
            t_p_eval_ms: 3.0,
            t_eval_ms: 4.0,
            n_p_eval,
            n_eval,
            n_reused: 1,
        }
    }

    #[test]
    fn from_raw_preserves_fields() {
        let data = LlamaPerfContextData::from(raw(10, 20));
        assert_eq!(data.prompt_tokens(), 10);
        assert_eq!(data.generated_tokens(), 20);
        assert_eq!(data.total_tokens(), 30);
        assert_eq!(data.n_reused, 1);
    }

    #[test]
    fn token_helpers_clamp_negative() {
        let data = LlamaPerfContextData::from(raw(-1, -1));
        assert_eq!(data.prompt_tokens(), 0);
        assert_eq!(data.generated_tokens(), 0);
        assert_eq!(data.total_tokens(), 0);
    }
}
