/// Parameters for creating a llama inference context.
#[derive(Clone)]
pub struct LlamaContextParams {
    /// Context window size (0 = use model default).
    pub n_ctx: u32,
    /// Maximum batch size for decoding.
    pub n_batch: u32,
    /// Physical batch size (must be <= n_batch, 0 = use n_batch).
    pub n_ubatch: u32,
    /// Number of threads for generation.
    pub n_threads: i32,
    /// Number of threads for batch processing.
    pub n_threads_batch: i32,
    /// Offload KV cache to GPU.
    pub offload_kqv: bool,
    /// Enable flash attention.
    pub flash_attn: bool,
    /// Disable performance metrics.
    pub no_perf: bool,
}

impl Default for LlamaContextParams {
    fn default() -> Self {
        Self {
            n_ctx: 512,
            n_batch: 512,
            n_ubatch: 0,
            n_threads: 4,
            n_threads_batch: 4,
            offload_kqv: true,
            flash_attn: false,
            no_perf: false,
        }
    }
}

impl LlamaContextParams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn n_ctx(mut self, v: u32) -> Self {
        self.n_ctx = v;
        self
    }

    pub fn n_batch(mut self, v: u32) -> Self {
        self.n_batch = v;
        self
    }

    pub fn n_ubatch(mut self, v: u32) -> Self {
        self.n_ubatch = v;
        self
    }

    pub fn n_threads(mut self, v: i32) -> Self {
        self.n_threads = v;
        self
    }

    pub fn n_threads_batch(mut self, v: i32) -> Self {
        self.n_threads_batch = v;
        self
    }

    pub fn offload_kqv(mut self, v: bool) -> Self {
        self.offload_kqv = v;
        self
    }

    pub fn flash_attn(mut self, v: bool) -> Self {
        self.flash_attn = v;
        self
    }

    pub fn no_perf(mut self, v: bool) -> Self {
        self.no_perf = v;
        self
    }

    pub(crate) fn to_c_params(&self, lib: &slab_llama_sys::LlamaLib) -> slab_llama_sys::llama_context_params {
        let mut params = unsafe { lib.llama_context_default_params() };
        params.n_ctx = self.n_ctx;
        params.n_batch = self.n_batch;
        if self.n_ubatch > 0 {
            params.n_ubatch = self.n_ubatch;
        }
        params.n_threads = self.n_threads;
        params.n_threads_batch = self.n_threads_batch;
        params.offload_kqv = self.offload_kqv;
        params.no_perf = self.no_perf;
        // flash_attn is controlled via flash_attn_type field
        if self.flash_attn {
            params.flash_attn_type = slab_llama_sys::llama_flash_attn_type_LLAMA_FLASH_ATTN_TYPE_ENABLED;
        }
        params
    }
}
