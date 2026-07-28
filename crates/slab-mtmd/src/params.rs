use std::ffi::CString;

use crate::SharedMtmdLib;

/// Safe wrapper around `mtmd_context_params`.
///
/// Defaults are seeded from the FFI `mtmd_context_params_default()` at build
/// time (so they track the vendored build), then the overridable fields are
/// applied from this struct. Fields left at their Rust `Default` are sensible
/// for local vision inference.
#[derive(Debug, Clone)]
pub struct MtmdContextParams {
    /// Place the vision projector on the GPU when available.
    pub use_gpu: bool,
    /// Number of threads for CPU image encoding (`<= 0` keeps the FFI default).
    pub n_threads: i32,
    /// Run a warmup pass after loading the projector.
    pub warmup: bool,
    /// Minimum/maximum image-token budget per image (`<= 0` keeps the FFI
    /// default, which is model-dependent).
    pub image_min_tokens: i32,
    pub image_max_tokens: i32,
    /// Maximum tokens submitted to a single encoder batch (`0` keeps the FFI
    /// default, currently 1024).
    pub batch_max_tokens: i32,
    /// Override the media marker placed in the prompt text. `None` keeps the
    /// projector's default marker (query `MtmdContext::marker` after load).
    pub media_marker: Option<String>,
    /// Print mtmd timing information to stderr.
    pub print_timings: bool,
    marker_cstr: Option<CString>,
}

impl Default for MtmdContextParams {
    fn default() -> Self {
        Self {
            use_gpu: true,
            n_threads: 0,
            warmup: true,
            image_min_tokens: 0,
            image_max_tokens: 0,
            batch_max_tokens: 0,
            media_marker: None,
            print_timings: false,
            marker_cstr: None,
        }
    }
}

impl MtmdContextParams {
    /// Override the media marker used to denote image positions in the prompt.
    pub fn with_media_marker(mut self, marker: impl Into<String>) -> Self {
        self.media_marker = Some(marker.into());
        self
    }

    /// Set whether the vision projector runs on the GPU.
    pub fn with_use_gpu(mut self, use_gpu: bool) -> Self {
        self.use_gpu = use_gpu;
        self
    }

    /// Set the encoder thread count.
    pub fn with_n_threads(mut self, n_threads: i32) -> Self {
        self.n_threads = n_threads;
        self
    }

    /// Build the raw `mtmd_context_params`, seeding from the FFI default then
    /// applying overrides. When a media marker is set, the CString is stored in
    /// `self.marker_cstr` so the pointer stays valid for the synchronous
    /// `mtmd_init_from_file` call that follows (mtmd copies it internally).
    pub(crate) fn build_raw(&mut self, lib: &SharedMtmdLib) -> slab_mtmd_sys::mtmd_context_params {
        // SAFETY: `mtmd_context_params_default()` is a trivial by-value getter.
        let mut raw: slab_mtmd_sys::mtmd_context_params =
            unsafe { lib.mtmd_context_params_default() };
        raw.use_gpu = self.use_gpu;
        raw.print_timings = self.print_timings;
        raw.warmup = self.warmup;
        if self.n_threads > 0 {
            raw.n_threads = self.n_threads;
        }
        if self.image_min_tokens > 0 {
            raw.image_min_tokens = self.image_min_tokens;
        }
        if self.image_max_tokens > 0 {
            raw.image_max_tokens = self.image_max_tokens;
        }
        if self.batch_max_tokens > 0 {
            raw.batch_max_tokens = self.batch_max_tokens;
        }
        if let Some(marker) = self.media_marker.take() {
            self.marker_cstr = CString::new(marker).ok();
            if let Some(cstr) = &self.marker_cstr {
                raw.media_marker = cstr.as_ptr();
            }
        }
        raw
    }
}
