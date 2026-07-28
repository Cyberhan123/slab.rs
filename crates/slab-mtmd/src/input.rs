use std::ffi::CString;
use std::ptr::NonNull;
use std::sync::Arc;

use crate::Mtmd;
use crate::SharedMtmdLib;

/// Kind of a single [`mtmd_input_chunk`] produced by [`MtmdContext::tokenize`].
///
/// [`MtmdContext::tokenize`]: crate::MtmdContext::tokenize
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MtmdInputChunkType {
    Text,
    Image,
    Audio,
    Unknown(i32),
}

/// Owned text + tokenize options handed to [`MtmdContext::tokenize`].
///
/// [`MtmdContext::tokenize`]: crate::MtmdContext::tokenize
pub struct MtmdInputText {
    text: CString,
    add_special: bool,
    parse_special: bool,
}

impl MtmdInputText {
    /// Create a new input-text holder. `add_special` prepends/appends BOS/EOS
    /// special tokens; `parse_special` renders `<|…|>`-style special tokens.
    pub fn new(text: &str, add_special: bool, parse_special: bool) -> crate::Result<Self> {
        Ok(Self { text: CString::new(text)?, add_special, parse_special })
    }

    pub(crate) fn as_raw(&self) -> slab_mtmd_sys::mtmd_input_text {
        slab_mtmd_sys::mtmd_input_text {
            text: self.text.as_ptr(),
            text_len: self.text.as_bytes().len(),
            add_special: self.add_special,
            parse_special: self.parse_special,
        }
    }
}

/// Owned container of tokenized input chunks (text/image/audio interleaved).
///
/// Create with [`MtmdInputChunks::new`], pass to [`MtmdContext::tokenize`], then
/// feed to [`MtmdContext::eval_chunks`].
///
/// [`MtmdContext::tokenize`]: crate::MtmdContext::tokenize
/// [`MtmdContext::eval_chunks`]: crate::MtmdContext::eval_chunks
pub struct MtmdInputChunks {
    ptr: NonNull<slab_mtmd_sys::mtmd_input_chunks>,
    lib: Arc<SharedMtmdLib>,
}

// SAFETY: the chunks handle is an opaque allocation not tied to thread-local
// state; it must be movable across threads (e.g. through slab-llama's
// `run_with_context` worker channel).
unsafe impl Send for MtmdInputChunks {}

impl MtmdInputChunks {
    /// Allocate an empty chunks container.
    pub fn new(mtmd: &Mtmd) -> Self {
        let lib = mtmd.lib_arc();
        // SAFETY: `mtmd_input_chunks_init` returns a fresh heap allocation (or
        // null, handled below).
        let ptr = unsafe { lib.mtmd_input_chunks_init() };
        let ptr = NonNull::new(ptr).expect("mtmd_input_chunks_init returned null");
        Self { ptr, lib }
    }

    pub(crate) fn as_ptr(&self) -> *const slab_mtmd_sys::mtmd_input_chunks {
        self.ptr.as_ptr()
    }

    pub(crate) fn as_mut_ptr(&mut self) -> *mut slab_mtmd_sys::mtmd_input_chunks {
        self.ptr.as_ptr()
    }

    /// Number of chunks held by this container.
    pub fn len(&self) -> usize {
        // SAFETY: the pointer is valid until Drop.
        unsafe { self.lib.mtmd_input_chunks_size(self.as_ptr()) }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Drop for MtmdInputChunks {
    fn drop(&mut self) {
        // SAFETY: the container owns its allocation and is dropped once.
        unsafe { self.lib.mtmd_input_chunks_free(self.ptr.as_ptr()) };
    }
}
