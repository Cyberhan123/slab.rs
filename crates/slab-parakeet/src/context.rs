use crate::context_params::InnerContextParams;
use crate::{ContextParams, Parakeet, ParakeetError, ParakeetState};
use std::ffi::{CString, c_int};
use std::sync::Arc;

/// Safe Rust wrapper around a parakeet context pointer.
///
/// `pub(crate)` — the public surface is [`ParakeetContext`].
#[derive(Debug)]
pub(crate) struct ParakeetInnerContext {
    pub(crate) ctx: *mut slab_parakeet_sys::parakeet_context,
    pub(crate) instance: Parakeet,
}

impl Parakeet {
    /// Create a new [`ParakeetContext`] from a file, with parameters.
    ///
    /// # C++ equivalent
    /// `struct parakeet_context * parakeet_init_from_file_with_params_no_state(const char * path_model, struct parakeet_context_params params);`
    pub fn new_context(&self, parameters: ContextParams) -> Result<ParakeetContext, ParakeetError> {
        let ctx = self.new_inner_context(parameters)?;
        Ok(ParakeetContext::wrap(ctx))
    }

    pub(crate) fn new_inner_context(
        &self,
        parameters: ContextParams,
    ) -> Result<ParakeetInnerContext, ParakeetError> {
        let model_path = parameters.model_path.as_ref().ok_or(ParakeetError::ModelPathNotSet)?;
        let path_cstr = CString::new(model_path.to_string_lossy().as_ref())?;
        let parameters = InnerContextParams::from_canonical(self.lib.as_ref(), &parameters)?;
        let ctx = unsafe {
            self.lib.parakeet_init_from_file_with_params_no_state(
                path_cstr.as_ptr(),
                parameters.into_inner(),
            )
        };
        if ctx.is_null() {
            Err(ParakeetError::InitError)
        } else {
            Ok(ParakeetInnerContext { ctx, instance: self.clone() })
        }
    }
}

impl ParakeetInnerContext {
    /// Get n_vocab.
    ///
    /// # C++ equivalent
    /// `int parakeet_n_vocab(struct parakeet_context * ctx)`
    pub fn n_vocab(&self) -> c_int {
        unsafe { self.instance.lib.parakeet_n_vocab(self.ctx) }
    }

    /// Get n_audio_ctx.
    ///
    /// # C++ equivalent
    /// `int parakeet_n_audio_ctx(struct parakeet_context * ctx)`
    pub fn n_audio_ctx(&self) -> c_int {
        unsafe { self.instance.lib.parakeet_n_audio_ctx(self.ctx) }
    }
}

impl Drop for ParakeetInnerContext {
    fn drop(&mut self) {
        if !self.ctx.is_null() {
            unsafe { self.instance.lib.parakeet_free(self.ctx) };
        }
    }
}

// SAFETY: The underlying `parakeet_context` pointer is only accessed through
// `&self`/`&mut self` methods, and the parakeet library supports multi-threaded
// use where each context is used from one thread at a time.
unsafe impl Send for ParakeetInnerContext {}
// SAFETY: Same as Send - all mutable access is exclusive through Rust's borrowing rules.
unsafe impl Sync for ParakeetInnerContext {}

/// Cheap, cloneable wrapper over an [`Arc<ParakeetInnerContext>`].
#[derive(Clone, Debug)]
pub struct ParakeetContext {
    ctx: Arc<ParakeetInnerContext>,
}

impl ParakeetContext {
    fn wrap(ctx: ParakeetInnerContext) -> Self {
        Self { ctx: Arc::new(ctx) }
    }

    /// Get n_vocab.
    pub fn n_vocab(&self) -> c_int {
        self.ctx.n_vocab()
    }

    /// Get n_audio_ctx.
    pub fn n_audio_ctx(&self) -> c_int {
        self.ctx.n_audio_ctx()
    }

    /// Create a new state object, ready for use.
    ///
    /// # C++ equivalent
    /// `struct parakeet_state * parakeet_init_state(struct parakeet_context * ctx);`
    pub fn create_state(&self) -> Result<ParakeetState, ParakeetError> {
        let state = unsafe { self.ctx.instance.lib.parakeet_init_state(self.ctx.ctx) };
        if state.is_null() {
            Err(ParakeetError::FailedToCreateState)
        } else {
            // SAFETY: this is known to be a valid pointer to a `parakeet_state` struct
            Ok(unsafe { ParakeetState::new(self.ctx.clone(), state) })
        }
    }
}
