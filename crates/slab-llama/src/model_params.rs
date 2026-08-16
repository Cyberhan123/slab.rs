/// Parameters for loading a llama model.
#[derive(Debug, Clone)]
pub struct LlamaModelParams {
    /// Number of GPU layers to offload (-1 = all).
    pub n_gpu_layers: i32,
    /// Load only the vocabulary, not the weights.
    pub vocab_only: bool,
    /// Use memory-mapped I/O if available.
    pub use_mmap: bool,
    /// Lock model weights in RAM (prevent swapping).
    pub use_mlock: bool,
}

impl Default for LlamaModelParams {
    fn default() -> Self {
        Self { n_gpu_layers: 0, vocab_only: false, use_mmap: true, use_mlock: false }
    }
}

impl LlamaModelParams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn n_gpu_layers(mut self, n: i32) -> Self {
        self.n_gpu_layers = n;
        self
    }

    pub fn vocab_only(mut self, v: bool) -> Self {
        self.vocab_only = v;
        self
    }

    pub fn use_mmap(mut self, v: bool) -> Self {
        self.use_mmap = v;
        self
    }

    pub fn use_mlock(mut self, v: bool) -> Self {
        self.use_mlock = v;
        self
    }

    pub(crate) fn to_c_params(
        &self,
        lib: &slab_llama_sys::LlamaLib,
    ) -> slab_llama_sys::llama_model_params {
        let mut params = unsafe { lib.llama_model_default_params() };
        params.n_gpu_layers = self.n_gpu_layers;
        params.vocab_only = self.vocab_only;
        // Upstream replaced the `use_mmap`/`use_mlock` booleans with a single
        // `llama_load_mode` enum; translate the legacy flags to the closest mode.
        params.load_mode = match (self.use_mmap, self.use_mlock) {
            (true, true) => slab_llama_sys::llama_load_mode_LLAMA_LOAD_MODE_MMAP_MLOCK,
            (true, false) => slab_llama_sys::llama_load_mode_LLAMA_LOAD_MODE_MMAP,
            (false, true) => slab_llama_sys::llama_load_mode_LLAMA_LOAD_MODE_MLOCK,
            (false, false) => slab_llama_sys::llama_load_mode_LLAMA_LOAD_MODE_NONE,
        };
        params
    }
}
