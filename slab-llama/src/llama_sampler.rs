use std::sync::Arc;

use crate::llama_context::LlamaContext;
use crate::token::LlamaToken;
use crate::Llama;

/// A safe wrapper around a llama sampler chain.
///
/// Create one with [`LlamaSampler::chain_new`], add individual samplers with
/// the builder methods, then call [`LlamaSampler::sample`] after decoding a
/// batch to get the next token.
pub struct LlamaSampler {
    sampler: *mut slab_llama_sys::llama_sampler,
    lib: Arc<slab_llama_sys::LlamaLib>,
}

unsafe impl Send for LlamaSampler {}
unsafe impl Sync for LlamaSampler {}

impl Drop for LlamaSampler {
    fn drop(&mut self) {
        unsafe { self.lib.llama_sampler_free(self.sampler) };
    }
}

impl Llama {
    /// Create a new sampler chain with default parameters.
    pub fn new_sampler_chain(&self) -> LlamaSampler {
        LlamaSampler::chain_new(Arc::clone(&self.lib))
    }
}

impl LlamaSampler {
    /// Create a new sampler chain (internal constructor).
    pub(crate) fn chain_new(lib: Arc<slab_llama_sys::LlamaLib>) -> Self {
        let params = unsafe { lib.llama_sampler_chain_default_params() };
        let sampler = unsafe { lib.llama_sampler_chain_init(params) };
        assert!(!sampler.is_null(), "llama_sampler_chain_init returned null");
        Self { sampler, lib }
    }

    /// Add a greedy (argmax) sampler to the chain.
    pub fn add_greedy(self) -> Self {
        let s = unsafe { self.lib.llama_sampler_init_greedy() };
        unsafe { self.lib.llama_sampler_chain_add(self.sampler, s) };
        self
    }

    /// Add a stochastic distribution sampler with the given seed.
    ///
    /// # Arguments
    /// * `seed` – random seed; use `LLAMA_DEFAULT_SEED` (0xFFFFFFFF) for non-deterministic.
    pub fn add_dist(self, seed: u32) -> Self {
        let s = unsafe { self.lib.llama_sampler_init_dist(seed) };
        unsafe { self.lib.llama_sampler_chain_add(self.sampler, s) };
        self
    }

    /// Add a temperature sampler.
    ///
    /// A temperature of 1.0 means no change; < 1.0 sharpens the distribution,
    /// > 1.0 flattens it.
    pub fn add_temp(self, t: f32) -> Self {
        let s = unsafe { self.lib.llama_sampler_init_temp(t) };
        unsafe { self.lib.llama_sampler_chain_add(self.sampler, s) };
        self
    }

    /// Add a top-K sampler.
    ///
    /// Keeps only the top `k` most probable tokens.
    pub fn add_top_k(self, k: i32) -> Self {
        let s = unsafe { self.lib.llama_sampler_init_top_k(k) };
        unsafe { self.lib.llama_sampler_chain_add(self.sampler, s) };
        self
    }

    /// Add a top-P (nucleus) sampler.
    ///
    /// # Arguments
    /// * `p`        – probability threshold.
    /// * `min_keep` – minimum number of tokens to keep.
    pub fn add_top_p(self, p: f32, min_keep: usize) -> Self {
        let s = unsafe { self.lib.llama_sampler_init_top_p(p, min_keep) };
        unsafe { self.lib.llama_sampler_chain_add(self.sampler, s) };
        self
    }

    /// Add a min-P sampler.
    ///
    /// # Arguments
    /// * `p`        – minimum probability threshold relative to the top token.
    /// * `min_keep` – minimum number of tokens to keep.
    pub fn add_min_p(self, p: f32, min_keep: usize) -> Self {
        let s = unsafe { self.lib.llama_sampler_init_min_p(p, min_keep) };
        unsafe { self.lib.llama_sampler_chain_add(self.sampler, s) };
        self
    }

    /// Add a Mirostat v2 sampler.
    ///
    /// # Arguments
    /// * `seed` – random seed.
    /// * `tau`  – target entropy.
    /// * `eta`  – learning rate.
    pub fn add_mirostat_v2(self, seed: u32, tau: f32, eta: f32) -> Self {
        let s = unsafe { self.lib.llama_sampler_init_mirostat_v2(seed, tau, eta) };
        unsafe { self.lib.llama_sampler_chain_add(self.sampler, s) };
        self
    }

    /// Add a repetition-penalty sampler.
    ///
    /// # Arguments
    /// * `penalty_last_n`   – number of last tokens to penalise (-1 = context size, 0 = disabled).
    /// * `penalty_repeat`   – repetition penalty factor (1.0 = disabled).
    /// * `penalty_freq`     – frequency penalty factor (0.0 = disabled).
    /// * `penalty_present`  – presence penalty factor (0.0 = disabled).
    pub fn add_penalties(
        self,
        penalty_last_n: i32,
        penalty_repeat: f32,
        penalty_freq: f32,
        penalty_present: f32,
    ) -> Self {
        let s = unsafe {
            self.lib.llama_sampler_init_penalties(
                penalty_last_n,
                penalty_repeat,
                penalty_freq,
                penalty_present,
            )
        };
        unsafe { self.lib.llama_sampler_chain_add(self.sampler, s) };
        self
    }

    /// Sample the next token from the context at position `idx` in the last
    /// decoded batch.
    ///
    /// # Arguments
    /// * `ctx` – the inference context (used to read logits).
    /// * `idx` – index of the token in the last decoded batch whose logits to use.
    pub fn sample(&mut self, ctx: &mut LlamaContext, idx: i32) -> LlamaToken {
        unsafe { self.lib.llama_sampler_sample(self.sampler, ctx.ctx, idx) }
    }

    /// Inform the sampler that `token` was accepted (for stateful samplers like
    /// Mirostat and repetition-penalty).
    pub fn accept(&mut self, token: LlamaToken) {
        unsafe { self.lib.llama_sampler_accept(self.sampler, token) }
    }

    /// Reset the sampler state.
    pub fn reset(&mut self) {
        unsafe { self.lib.llama_sampler_reset(self.sampler) }
    }

    /// Get the seed used by this sampler (only meaningful for seeded samplers).
    pub fn get_seed(&self) -> u32 {
        unsafe { self.lib.llama_sampler_get_seed(self.sampler) }
    }

    /// Print performance statistics to stderr.
    pub fn perf_print(&self) {
        unsafe { self.lib.llama_perf_sampler_print(self.sampler) }
    }

    /// Reset performance statistics.
    pub fn perf_reset(&mut self) {
        unsafe { self.lib.llama_perf_sampler_reset(self.sampler) }
    }
}

/// A convenience builder for common sampler chain configurations.
pub struct SamplerChainBuilder {
    lib: Arc<slab_llama_sys::LlamaLib>,
    /// Temperature (default 0.8).
    pub temperature: f32,
    /// Top-K (default 40, 0 = disabled).
    pub top_k: i32,
    /// Top-P (default 0.9, 1.0 = disabled).
    pub top_p: f32,
    /// Min-P (default 0.05, 0.0 = disabled).
    pub min_p: f32,
    /// Repetition penalty (default 1.0 = disabled).
    pub repeat_penalty: f32,
    /// Number of tokens to consider for repetition penalty (default 64).
    pub repeat_last_n: i32,
    /// Random seed (default `LLAMA_DEFAULT_SEED` = 0xFFFF_FFFF).
    pub seed: u32,
}

impl SamplerChainBuilder {
    /// Create a builder with sensible defaults.
    pub fn new(lib: Arc<slab_llama_sys::LlamaLib>) -> Self {
        Self {
            lib,
            temperature: 0.8,
            top_k: 40,
            top_p: 0.9,
            min_p: 0.05,
            repeat_penalty: 1.0,
            repeat_last_n: 64,
            seed: slab_llama_sys::LLAMA_DEFAULT_SEED,
        }
    }

    /// Build and return a [`LlamaSampler`] chain.
    pub fn build(self) -> LlamaSampler {
        let mut chain = LlamaSampler::chain_new(Arc::clone(&self.lib));

        // penalties first (they observe the logits before sampling).
        if self.repeat_penalty != 1.0 || self.repeat_last_n != 0 {
            chain = chain.add_penalties(self.repeat_last_n, self.repeat_penalty, 0.0, 0.0);
        }

        if self.top_k > 0 {
            chain = chain.add_top_k(self.top_k);
        }
        if self.top_p < 1.0 {
            chain = chain.add_top_p(self.top_p, 1);
        }
        if self.min_p > 0.0 {
            chain = chain.add_min_p(self.min_p, 1);
        }
        chain = chain.add_temp(self.temperature);
        chain = chain.add_dist(self.seed);

        chain
    }
}
