//! High-level Rust bindings for llama.cpp via dynamic library loading.
//!
//! # Usage
//!
//! ```rust,no_run
//! use slab_llama::{Llama, LlamaModelParams, LlamaContextParams, LlamaBatch, SamplerChainBuilder};
//!
//! let llama = Llama::new("/path/to/libllama.so").unwrap();
//! llama.backend_init();
//!
//! let model = llama
//!     .load_model_from_file("/path/to/model.gguf", LlamaModelParams::default())
//!     .unwrap();
//!
//! let mut ctx = model
//!     .new_context(LlamaContextParams::default())
//!     .unwrap();
//!
//! let tokens = model.tokenize("Hello, world!", true, true).unwrap();
//!
//! let mut batch = LlamaBatch::new(tokens.len() + 1);
//! for (i, &token) in tokens.iter().enumerate() {
//!     batch.add(token, i as i32, &[0], i == tokens.len() - 1).unwrap();
//! }
//!
//! ctx.decode(&mut batch).unwrap();
//!
//! let mut sampler = SamplerChainBuilder::new(llama.lib_arc()).build();
//! let next_token = sampler.sample(&mut ctx, (tokens.len() - 1) as i32);
//! let piece = model.token_to_piece(next_token, true).unwrap();
//! println!("{}", piece);
//!
//! llama.backend_free();
//! ```

use std::fmt;
use std::path::Path;
use std::sync::Arc;

mod context_params;
mod error;
mod llama_adapter;
mod llama_batch;
mod llama_context;
mod llama_model;
mod llama_sampler;
mod model_params;
mod token;

pub use context_params::LlamaContextParams;
pub use error::LlamaError;
pub use llama_adapter::LlamaLoraAdapter;
pub use llama_batch::LlamaBatch;
pub use llama_context::LlamaContext;
pub use llama_model::LlamaModel;
pub use llama_sampler::{LlamaSampler, SamplerChainBuilder};
pub use model_params::LlamaModelParams;
pub use token::{LlamaPos, LlamaSeqId, LlamaToken};

/// The type alias for per-sequence state flags (used in `state_seq_*_ext` methods).
pub type LlamaStateSeqFlags = slab_llama_sys::llama_state_seq_flags;

/// The default seed value for samplers.
pub const LLAMA_DEFAULT_SEED: u32 = slab_llama_sys::LLAMA_DEFAULT_SEED;

/// Entry point for the llama.cpp dynamic library.
///
/// Load the shared library (`.so` / `.dylib` / `.dll`) with [`Llama::new`],
/// then use it to initialise the backend, load models and create contexts.
#[derive(Clone)]
pub struct Llama {
    pub(crate) lib: Arc<slab_llama_sys::LlamaLib>,
}

impl Llama {
    /// Load the llama.cpp shared library from the given path.
    ///
    /// # Errors
    /// Returns a [`libloading::Error`] if the library cannot be opened or if
    /// a required symbol is missing.
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, ::libloading::Error> {
        let lib = unsafe { slab_llama_sys::LlamaLib::new(path.as_ref())? };
        Ok(Self {
            lib: Arc::new(lib),
        })
    }

    /// Initialise the llama.cpp backend.
    ///
    /// Must be called once before loading any model.  It is idempotent on most
    /// back-ends but should still only be called once per process.
    pub fn backend_init(&self) {
        unsafe { self.lib.llama_backend_init() }
    }

    /// Free the llama.cpp backend resources.
    ///
    /// Should be called after all models and contexts have been dropped.
    pub fn backend_free(&self) {
        unsafe { self.lib.llama_backend_free() }
    }

    /// Enable NUMA-aware memory allocation.
    ///
    /// # Arguments
    /// * `strategy` – the NUMA strategy to use.
    pub fn numa_init(&self, strategy: slab_llama_sys::ggml_numa_strategy) {
        unsafe { self.lib.llama_numa_init(strategy) }
    }

    /// Return a clone of the underlying `Arc<LlamaLib>` for use in samplers
    /// and other helpers that need direct access to the library.
    pub fn lib_arc(&self) -> Arc<slab_llama_sys::LlamaLib> {
        Arc::clone(&self.lib)
    }

    /// Return a human-readable description of the current system (CPU, BLAS, …).
    pub fn print_system_info(&self) -> &str {
        let ptr = unsafe { self.lib.llama_print_system_info() };
        if ptr.is_null() {
            return "";
        }
        let cstr = unsafe { std::ffi::CStr::from_ptr(ptr) };
        cstr.to_str().unwrap_or("")
    }

    /// Returns `true` if the library was compiled with mmap support.
    pub fn supports_mmap(&self) -> bool {
        unsafe { self.lib.llama_supports_mmap() }
    }

    /// Returns `true` if the library was compiled with mlock support.
    pub fn supports_mlock(&self) -> bool {
        unsafe { self.lib.llama_supports_mlock() }
    }

    /// Returns `true` if the library was compiled with GPU offload support.
    pub fn supports_gpu_offload(&self) -> bool {
        unsafe { self.lib.llama_supports_gpu_offload() }
    }
}

impl fmt::Debug for Llama {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Llama").finish()
    }
}
