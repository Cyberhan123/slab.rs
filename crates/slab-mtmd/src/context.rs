use std::ffi::CStr;
use std::path::Path;
use std::ptr::NonNull;
use std::sync::Arc;

use slab_llama::LlamaContext;
use slab_llama::LlamaModel;

use crate::Mtmd;
use crate::SharedMtmdLib;
use crate::bitmap::MtmdBitmap;
use crate::error::{MtmdError, Result};
use crate::input::{MtmdInputChunks, MtmdInputText};
use crate::params::MtmdContextParams;

/// Owned `mtmd_context` — a loaded vision/audio projector bound to a llama text
/// model. Created via [`MtmdContext::init_from_file`].
pub struct MtmdContext {
    ptr: NonNull<slab_mtmd_sys::mtmd_context>,
    lib: Arc<SharedMtmdLib>,
}

// SAFETY: the projector handle is used through `&self`/`&mut self` methods and
// the underlying library does not rely on thread-local state.
unsafe impl Send for MtmdContext {}
unsafe impl Sync for MtmdContext {}

impl MtmdContext {
    /// Load a multimodal projector (`mmproj` GGUF) bound to `text_model`.
    ///
    /// `text_model` must outlive the returned context (the projector only
    /// borrows the model pointer; the caller is responsible for drop order).
    pub fn init_from_file(
        mtmd: &Mtmd,
        mmproj_path: impl AsRef<Path>,
        text_model: &LlamaModel,
        mut params: MtmdContextParams,
    ) -> Result<Self> {
        let path = mmproj_path.as_ref().to_str().ok_or(MtmdError::InvalidPath)?;
        let c_path = std::ffi::CString::new(path)?;
        let raw_params = params.build_raw(&mtmd.lib);

        // SAFETY: `slab_mtmd_sys::llama_model` and `slab_llama_sys::llama_model`
        // are bindgen-emitted opaque structs for the *same* C type
        // (`vendor/llama/include/llama.h`). They are layout-identical opaque
        // pointers, so casting across the two `-sys` crates is sound. This is
        // the standard pattern for vendor-split bindings (cf. slab-ggml-sys's
        // two-DLL GGmlLib). The pointer stays valid for `text_model`'s
        // lifetime, which the caller guarantees outlives the returned context.
        let model_ptr = text_model.as_ptr() as *const slab_mtmd_sys::llama_model;
        let ptr = unsafe { mtmd.lib.mtmd_init_from_file(c_path.as_ptr(), model_ptr, raw_params) };
        let ptr = NonNull::new(ptr).ok_or(MtmdError::ContextCreateFailed)?;
        Ok(Self { ptr, lib: mtmd.lib_arc() })
    }

    pub(crate) fn as_ptr(&self) -> *mut slab_mtmd_sys::mtmd_context {
        self.ptr.as_ptr()
    }

    pub(crate) fn lib_arc(&self) -> Arc<SharedMtmdLib> {
        Arc::clone(&self.lib)
    }

    /// Whether this projector can encode images.
    pub fn supports_vision(&self) -> bool {
        // SAFETY: pointer valid until Drop.
        unsafe { self.lib.mtmd_support_vision(self.ptr.as_ptr()) }
    }

    /// Whether this projector can encode audio.
    pub fn supports_audio(&self) -> bool {
        unsafe { self.lib.mtmd_support_audio(self.ptr.as_ptr()) }
    }

    /// The media marker this projector expects in the prompt text (e.g.
    /// `<__media__>`). The returned slice borrows from the projector.
    pub fn marker(&self) -> &str {
        // SAFETY: the marker is a static string owned by the projector, valid
        // for its lifetime.
        let ptr = unsafe { self.lib.mtmd_get_marker(self.ptr.as_ptr()) };
        if ptr.is_null() {
            return "";
        }
        unsafe { CStr::from_ptr(ptr) }.to_str().unwrap_or("")
    }

    /// Tokenize `text` interleaved with `bitmaps`, writing the resulting
    /// text/image/audio chunks into `output`.
    ///
    /// The prompt must contain one [`marker`](Self::marker) per bitmap, at the
    /// position each image should occupy.
    pub fn tokenize(
        &self,
        text: &MtmdInputText,
        bitmaps: &[&MtmdBitmap],
        output: &mut MtmdInputChunks,
    ) -> Result<()> {
        let mut bitmap_ptrs: Vec<*const slab_mtmd_sys::mtmd_bitmap> =
            bitmaps.iter().map(|b| b.as_const_ptr()).collect();
        let raw_text = text.as_raw();
        // SAFETY: `output` owns a valid chunks pointer; `raw_text` borrows the
        // `MtmdInputText`'s CString for the call; bitmap_ptrs outlives the call.
        let rc = unsafe {
            self.lib.mtmd_tokenize(
                self.ptr.as_ptr(),
                output.as_mut_ptr(),
                &raw_text as *const _,
                bitmap_ptrs.as_mut_ptr(),
                bitmap_ptrs.len(),
            )
        };
        if rc != 0 { Err(MtmdError::TokenizeError(rc)) } else { Ok(()) }
    }

    /// Drive `llama_decode` for text chunks and `mtmd_encode_chunk` +
    /// `llama_decode` for image/audio chunks, advancing the live context.
    ///
    /// On success `new_n_past` is updated to the new sequence position.
    #[allow(clippy::too_many_arguments)]
    pub fn eval_chunks(
        &self,
        lctx: &LlamaContext,
        chunks: &MtmdInputChunks,
        n_past: i32,
        seq_id: i32,
        n_batch: i32,
        logits_last: bool,
        new_n_past: &mut i32,
    ) -> Result<()> {
        // SAFETY: same opaque-pointer cast rationale as `init_from_file`.
        let lctx_ptr = lctx.as_ptr() as *mut slab_mtmd_sys::llama_context as *mut std::ffi::c_void;
        self.eval_chunks_raw(lctx_ptr, chunks, n_past, seq_id, n_batch, logits_last, new_n_past)
    }

    /// Like [`eval_chunks`](Self::eval_chunks) but takes a raw `llama_context*`
    /// as `*mut c_void`, so callers that hold the pointer across a crate
    /// boundary (e.g. slab-llama's `run_with_context` escape-hatch) do not need
    /// to depend on `slab-mtmd-sys` directly.
    #[allow(clippy::too_many_arguments)]
    pub fn eval_chunks_raw(
        &self,
        lctx_ptr: *mut std::ffi::c_void,
        chunks: &MtmdInputChunks,
        n_past: i32,
        seq_id: i32,
        n_batch: i32,
        logits_last: bool,
        new_n_past: &mut i32,
    ) -> Result<()> {
        let mut out: slab_mtmd_sys::llama_pos = n_past;
        // SAFETY: `lctx_ptr` aliases a live context owned by the caller (it is
        // the same opaque `llama_context*` produced by slab-llama-sys); `chunks`
        // owns its container for the call.
        let rc = unsafe {
            self.lib.mtmd_helper_eval_chunks(
                self.ptr.as_ptr(),
                lctx_ptr as *mut slab_mtmd_sys::llama_context,
                chunks.as_ptr(),
                n_past,
                seq_id,
                n_batch,
                logits_last,
                &mut out as *mut slab_mtmd_sys::llama_pos,
            )
        };
        *new_n_past = out;
        if rc != 0 { Err(MtmdError::EvalError(rc)) } else { Ok(()) }
    }

    /// Total number of tokens (text + image + audio) the chunks will consume.
    pub fn n_tokens_of(&self, chunks: &MtmdInputChunks) -> usize {
        // SAFETY: chunks pointer valid for the borrow.
        unsafe { self.lib.mtmd_helper_get_n_tokens(chunks.as_ptr()) }
    }
}

impl Drop for MtmdContext {
    fn drop(&mut self) {
        // SAFETY: the context owns its allocation and is dropped once.
        unsafe { self.lib.mtmd_free(self.ptr.as_ptr()) };
    }
}

impl std::fmt::Debug for MtmdContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MtmdContext").finish_non_exhaustive()
    }
}
